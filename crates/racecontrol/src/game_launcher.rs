use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::state::AppState;
use rc_common::protocol::{CoreToAgentMessage, DashboardCommand};
use rc_common::types::{GameLaunchInfo, GameState, SimType};

// Used by #[cfg(test)] via `use super::*`
#[cfg(test)]
use crate::metrics;
#[cfg(test)]
use rc_common::types::BillingSessionStatus;

// Re-export extracted modules for backward compatibility
pub use crate::game_launcher_ops::*;
pub use crate::game_launcher_state::*;
pub use crate::game_launcher_support::*;

/// In-memory tracker for a game running on a pod (mirrors BillingTimer pattern)
pub struct GameTracker {
    pub pod_id: String,
    pub sim_type: SimType,
    pub game_state: GameState,
    pub pid: Option<u32>,
    pub launched_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    /// Stored launch_args for auto-relaunch on crash
    pub launch_args: Option<String>,
    /// How many times Race Engineer has auto-relaunched after crash (max 2)
    pub auto_relaunch_count: u32,
    /// True when the server learned about this game from an agent report
    /// rather than initiating the launch itself. Auto-relaunch is prohibited.
    pub externally_tracked: bool,
    /// Dynamic timeout in seconds computed from historical launch data (LAUNCH-08).
    /// None = use game-specific default (AC=120s, others=90s).
    pub dynamic_timeout_secs: Option<i64>,
    /// Exit codes accumulated across all failed relaunch attempts (RECOVER-05).
    /// Included in staff WhatsApp alert for diagnostics.
    pub exit_codes: Vec<Option<i32>>,
    /// Maximum auto-relaunch attempts allowed for this combo (INTEL-05).
    /// Default: 2. Set to 3 for combos with < 50% reliability (>= 5 launches).
    pub max_auto_relaunch: u32,
    /// Phase 282: When the game became playable (PlayableSignal received).
    pub playable_at: Option<DateTime<Utc>>,
    /// Phase 282: Milliseconds from launch command to PlayableSignal.
    pub ready_delay_ms: Option<i64>,
    /// Phase 310: Billing session ID for end-to-end customer journey tracing.
    pub billing_session_id: Option<String>,
    /// Phase 318 (LAUNCH-05): UUID v4 generated when tracking starts.
    /// Used to correlate timeline spans with this specific launch attempt.
    pub launch_id: String,
}

impl GameTracker {
    pub fn to_info(&self) -> GameLaunchInfo {
        GameLaunchInfo {
            pod_id: self.pod_id.clone(),
            sim_type: self.sim_type,
            game_state: self.game_state,
            pid: self.pid,
            launched_at: self.launched_at,
            error_message: self.error_message.clone(),
            diagnostics: None,
            exit_code: None,
            playable_at: self.playable_at,
            ready_delay_ms: self.ready_delay_ms,
            session_id: self.billing_session_id.clone(),
            launch_stage: None, clean_exit_heuristic: None,
        }
    }
}

/// Manages game launch state across all pods (in-memory, like BillingManager)
pub struct GameManager {
    /// pod_id -> GameTracker
    pub active_games: RwLock<HashMap<String, GameTracker>>,
    /// CLOSED-LOOP: Result of the last launch verification (used by API response).
    pub last_launch_verified: std::sync::atomic::AtomicBool,
    /// RESIL-06: Concurrency gate — max 4 simultaneous game launches.
    /// Prevents port starvation (16 ports / 4 = 4 ports per launch for retry headroom)
    /// and reduces server load during peak launch storms (8 pods all launching at once).
    pub launch_semaphore: tokio::sync::Semaphore,
    /// STOP-GUARD (2026-04-12): Timestamp of the most recent StopGame dispatch per pod.
    /// Used by handle_game_state_update to reject zombie non-Idle updates that arrive
    /// from rc-agent's 100ms sim polling loop after a stop has been issued but before
    /// the agent has processed it. Without this guard, a late Running update spawns a
    /// phantom externally_tracked tracker, which causes /fleet/health to report
    /// game_state: "running" for minutes after the game has actually stopped (root
    /// cause of the Issue 4 cache desync observed in the 2026-04-11 E2E test).
    /// Entries are opportunistically pruned (>30s) on each handle_game_state_update.
    pub recent_stops: RwLock<HashMap<String, Instant>>,
}

/// Result of a game launch with closed-loop verification.
pub struct LaunchResult {
    /// Whether the game process was confirmed running (not just command sent).
    pub verified: bool,
    /// How long verification took (seconds).
    pub verify_time_secs: f64,
}

/// LAUNCH-TIMELINE-STOPPED (2026-04-12): Snapshot captured from the GameTracker at
/// stop_game entry, used to persist a launch_timeline_spans row for launches that
/// the agent will not otherwise report. The agent emits LaunchTimelineReport only
/// on BillingStarted (success) or LaunchTimedOut (timeout) — launches that never
/// reached `AcStatus::Live` (e.g. staff aborted a pre-playable launch, or an AC
/// launch where nobody drove) are currently invisible in /launch-timeline/recent.
/// This is the Issue 5 gap from the 2026-04-11 E2E test. INSERT OR IGNORE is used
/// so the agent's authoritative success row always wins if it arrives.
pub struct LaunchSpanSnapshot {
    pub launch_id: String,
    pub pod_id: String,
    pub sim_type: String,
    pub billing_session_id: Option<String>,
    pub launched_at: Option<DateTime<Utc>>,
    pub playable_at: Option<DateTime<Utc>>,
    pub outcome: String,
}

impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            active_games: RwLock::new(HashMap::new()),
            last_launch_verified: std::sync::atomic::AtomicBool::new(false),
            launch_semaphore: tokio::sync::Semaphore::new(4),
            recent_stops: RwLock::new(HashMap::new()),
        }
    }
}

/// STOP-GUARD helper: returns true if a StopGame was dispatched for this pod
/// within the last `window_secs` seconds. Used to reject zombie GameStateUpdate
/// messages from rc-agent's sim polling loop that arrive after a stop.
/// Also opportunistically prunes entries older than 30s.
pub async fn is_stop_guarded(state: &Arc<AppState>, pod_id: &str, window_secs: u64) -> bool {
    let now = Instant::now();
    let mut guard = state.game_launcher.recent_stops.write().await;
    // Opportunistic cleanup of stale entries (>30s)
    guard.retain(|_, t| now.duration_since(*t) < std::time::Duration::from_secs(30));
    guard
        .get(pod_id)
        .map(|t| now.duration_since(*t) < std::time::Duration::from_secs(window_secs))
        .unwrap_or(false)
}

// ─── GameLauncherImpl trait + per-game implementations ──────────────────────

/// Per-game launch behavior. Static dispatch via launcher_for().
pub trait GameLauncherImpl: Send + Sync {
    /// Validate sim-specific launch args. Called before billing gate.
    fn validate_args(&self, args: Option<&str>) -> Result<(), String>;
    /// Return the CoreToAgentMessage to send for this game.
    /// Phase 368 D-01: launch_id is threaded from the server-minted UUID so rc-agent receives
    /// the same launch_id that is registered in LaunchStateMachine.
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>, duration_minutes: Option<u32>, launch_id: String) -> CoreToAgentMessage;
    /// Optional cleanup on launch failure. Default: no-op.
    fn cleanup_on_failure(&self, _pod_id: &str) {}
}

pub struct AcLauncher;
pub struct F1Launcher;
pub struct IRacingLauncher;
pub struct DefaultLauncher;

impl GameLauncherImpl for AcLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>, duration_minutes: Option<u32>, launch_id: String) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes, launch_id: Some(launch_id) }
    }
}

impl GameLauncherImpl for F1Launcher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>, duration_minutes: Option<u32>, launch_id: String) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes, launch_id: Some(launch_id) }
    }
}

impl GameLauncherImpl for IRacingLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>, duration_minutes: Option<u32>, launch_id: String) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes, launch_id: Some(launch_id) }
    }
}

impl GameLauncherImpl for DefaultLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>, duration_minutes: Option<u32>, launch_id: String) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes, launch_id: Some(launch_id) }
    }
}

pub fn launcher_for(sim_type: SimType) -> &'static dyn GameLauncherImpl {
    match sim_type {
        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => &AcLauncher,
        SimType::F125 => &F1Launcher,
        SimType::IRacing => &IRacingLauncher,
        _ => &DefaultLauncher,
    }
}

/// Handle dashboard commands for game launching/stopping
pub async fn handle_dashboard_command(state: &Arc<AppState>, cmd: DashboardCommand) -> Result<(), String> {
    match cmd {
        DashboardCommand::LaunchGame {
            pod_id,
            sim_type,
            launch_args,
        } => {
            launch_game(state, &pod_id, sim_type, launch_args).await
        }
        DashboardCommand::StopGame { pod_id } => {
            stop_game(state, &pod_id).await;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// FSM-08: Transition to the next split for a pod's active billing session.
///
/// Persists the split transition to DB BEFORE any new launch command is issued.
/// This is the ordering guarantee that prevents orphaned launches (FSM-08).
///
/// Steps:
/// 1. Complete current split + activate next in DB (via transition_split CAS)
/// 2. Verify the new active split record exists in DB
/// 3. Update in-memory billing timer's current_split_number
///
/// Returns Ok(next_split_number) if transition succeeded and next split is ready for launch.
/// Returns Err("All splits completed") if there are no more splits (caller should end session).
/// Returns Err(...) if DB CAS fails (concurrent transition guard).
pub async fn transition_to_next_split(
    state: &Arc<AppState>,
    pod_id: &str,
    parent_session_id: &str,
    current_split: i64,
) -> Result<i64, String> {
    // Step 1: Complete current split and activate next in DB
    let next_split = crate::billing::transition_split(&state.db, parent_session_id, current_split).await?;

    let next_number = match next_split {
        Some(n) => n,
        None => {
            tracing::info!(
                "FSM-08: All splits completed for session {} on pod {}",
                parent_session_id, pod_id
            );
            return Err("All splits completed — session should end".to_string());
        }
    };

    // Step 2: Verify the new active split record exists in DB (DB-before-launch invariant)
    let verified = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM split_sessions \
         WHERE parent_session_id = ? AND split_number = ? AND status = 'active'",
    )
    .bind(parent_session_id)
    .bind(next_number)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("FSM-08: DB verification query failed: {}", e))?;

    if verified != 1 {
        return Err(format!(
            "FSM-08: Split {} for session {} not persisted as active after transition — aborting launch",
            next_number, parent_session_id
        ));
    }

    // Step 3: Update in-memory billing timer's current_split_number
    {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id) {
            timer.current_split_number = next_number as u32;
        }
    }

    tracing::info!(
        "FSM-08: Split {} persisted and verified as active — ready for launch on pod {}",
        next_number, pod_id
    );
    Ok(next_number)
}

#[cfg(test)]
#[path = "game_launcher_tests.rs"]
mod tests;
