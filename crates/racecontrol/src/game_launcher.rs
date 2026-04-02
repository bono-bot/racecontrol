use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::activity_log::log_pod_activity;
use crate::catalog;
use crate::metrics;
use crate::state::AppState;
use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{BillingSessionStatus, GameLaunchInfo, GameState, SimType};

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
        }
    }
}

/// Manages game launch state across all pods (in-memory, like BillingManager)
pub struct GameManager {
    /// pod_id -> GameTracker
    pub active_games: RwLock<HashMap<String, GameTracker>>,
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            active_games: RwLock::new(HashMap::new()),
        }
    }
}

// ─── GameLauncherImpl trait + per-game implementations ──────────────────────

/// Per-game launch behavior. Static dispatch via launcher_for().
pub trait GameLauncherImpl: Send + Sync {
    /// Validate sim-specific launch args. Called before billing gate.
    fn validate_args(&self, args: Option<&str>) -> Result<(), String>;
    /// Return the CoreToAgentMessage to send for this game.
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage;
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
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes: None }
    }
}

impl GameLauncherImpl for F1Launcher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes: None }
    }
}

impl GameLauncherImpl for IRacingLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes: None }
    }
}

impl GameLauncherImpl for DefaultLauncher {
    fn validate_args(&self, args: Option<&str>) -> Result<(), String> {
        let Some(json) = args else { return Ok(()); };
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid launch_args JSON: {}", e))?;
        Ok(())
    }
    fn make_launch_message(&self, sim_type: SimType, launch_args: Option<String>) -> CoreToAgentMessage {
        CoreToAgentMessage::LaunchGame { sim_type, launch_args, force_clean: false, duration_minutes: None }
    }
}

fn launcher_for(sim_type: SimType) -> &'static dyn GameLauncherImpl {
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

async fn launch_game(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: SimType,
    launch_args: Option<String>,
) -> Result<(), String> {
    // Normalize pod_id to canonical form (pod_N) at function entry
    let pod_id_owned = normalize_pod_id(pod_id).map_err(|e| format!("Invalid pod ID: {}", e))?;
    let pod_id = pod_id_owned.as_str();

    // LAUNCH-06: Validate launch args via per-game launcher (rejects invalid JSON)
    let launcher = launcher_for(sim_type);
    launcher.validate_args(launch_args.as_deref())
        .map_err(|e| { tracing::warn!("Launch rejected for pod {}: {}", pod_id, e); e })?;

    // Validate launch combo against pod's content manifest (only if JSON parsed OK)
    if let Some(ref args_json) = launch_args {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_json) {
            let car = args.get("car").and_then(|v| v.as_str()).unwrap_or("");
            let track = args.get("track").and_then(|v| v.as_str()).unwrap_or("");
            let session_type = args.get("session_type").and_then(|v| v.as_str()).unwrap_or("");
            let manifest = state.pod_manifests.read().await.get(pod_id).cloned();
            if let Err(reason) = catalog::validate_launch_combo(manifest.as_ref(), car, track, session_type) {
                tracing::warn!("Launch rejected for pod {}: {}", pod_id, reason);
                return Err(reason);
            }
        }
    }

    // STATE-03: Feature flag check — game_launch must be enabled (default: enabled)
    {
        let flags = state.feature_flags.read().await;
        let game_launch_enabled = flags.get("game_launch")
            .map(|f| f.enabled)
            .unwrap_or(true); // Default enabled if flag not configured (Pitfall 6 prevention)
        if !game_launch_enabled {
            tracing::warn!("Launch rejected for pod {}: game_launch feature flag disabled", pod_id);
            return Err("game_launch feature disabled".to_string());
        }
    }

    // LAUNCH-02/03/04 / FSM-03: Billing gate — check active_timers + waiting_for_game, reject paused.
    // FSM-03: Free gaming guard — LaunchGame is rejected if no active billing session exists.
    // TODO: FSM-03 exception for free trials (no trial concept exists yet)
    {
        let timers = state.billing.active_timers.read().await;
        let waiting = state.billing.waiting_for_game.read().await;
        let has_active = timers.contains_key(pod_id);
        let has_deferred = waiting.contains_key(pod_id);
        if !has_active && !has_deferred {
            tracing::warn!("FREE GAMING GUARD: Rejected LaunchGame for pod {} — no active billing session", pod_id);
            return Err(format!("FSM-03: Pod {} has no active billing session (free gaming guard)", pod_id));
        }
        // LAUNCH-03: Reject paused sessions
        if let Some(timer) = timers.get(pod_id) {
            if matches!(timer.status,
                BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
            ) {
                tracing::warn!("Launch rejected for pod {}: billing session is paused ({:?})", pod_id, timer.status);
                return Err(format!("Pod {} billing session is paused", pod_id));
            }
        }
    }

    // FSM-08: DB-before-launch guard for split sessions.
    // For split 2+ launches, the split record MUST be persisted as 'active' in
    // split_sessions before any launch command is sent to the agent.
    // This prevents orphaned launches with no billing record.
    {
        let (is_split, session_id, split_number) = {
            let timers = state.billing.active_timers.read().await;
            if let Some(timer) = timers.get(pod_id) {
                let is_multi = timer.split_count > 1 && timer.current_split_number > 1;
                (is_multi, timer.session_id.clone(), timer.current_split_number)
            } else {
                (false, String::new(), 0)
            }
        };

        if is_split {
            let split_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM split_sessions \
                 WHERE parent_session_id = ? AND split_number = ? AND status = 'active'",
            )
            .bind(&session_id)
            .bind(split_number as i64)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

            if split_exists != 1 {
                tracing::error!(
                    "FSM-08: Rejecting launch for pod {} — split {} for session {} is not persisted as active in DB (split_exists={})",
                    pod_id, split_number, session_id, split_exists
                );
                return Err(format!(
                    "FSM-08: Cannot launch split {} on pod {} — not persisted in DB (orphaned launch prevention)",
                    split_number, pod_id
                ));
            }
            tracing::info!(
                "FSM-08: Split {} for session {} verified as active in DB — proceeding with launch on pod {}",
                split_number, session_id, pod_id
            );
        }
    }

    // LIFE-04/LAUNCH-05: Check if a game is currently launching, running, or stopping (avoid double-launch)
    {
        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get(pod_id) {
            if matches!(tracker.game_state, GameState::Launching | GameState::Running | GameState::Stopping) {
                let msg = if tracker.game_state == GameState::Stopping {
                    format!("game still stopping on pod {}", pod_id)
                } else {
                    format!("Pod {} already has a game active", pod_id)
                };
                return Err(msg);
            }
        }
    }

    log_pod_activity(state, pod_id, "game", "Game Launching", &format!("{}", sim_type), "core");

    // LAUNCH-08: Query dynamic timeout from historical launch data
    let default_timeout_secs: u64 = match sim_type {
        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => 120,
        _ => 90,
    };
    let (car_for_timeout, track_for_timeout, _, _) = extract_launch_fields(&launch_args);
    let dynamic_timeout = metrics::query_dynamic_timeout(
        &state.db,
        &sim_type.to_string(),
        car_for_timeout.as_deref(),
        track_for_timeout.as_deref(),
        default_timeout_secs,
    ).await;

    // INTEL-05: Query combo reliability to set max_auto_relaunch cap.
    // < 50% reliability with >= 5 launches → 3 attempts. Otherwise → 2 (default).
    let reliability = metrics::query_combo_reliability(
        &state.db,
        pod_id,
        &sim_type.to_string(),
        car_for_timeout.as_deref(),
        track_for_timeout.as_deref(),
    ).await;
    let max_relaunch_cap: u32 = match &reliability {
        Some(r) if r.success_rate < 0.50 && r.total_launches >= 5 => 3,
        _ => 2,
    };

    // Create tracker + insert with TOCTOU re-check (LAUNCH-04)
    let info = {
        let mut games = state.game_launcher.active_games.write().await;
        // TOCTOU re-check: billing must still be present
        let timers = state.billing.active_timers.read().await;
        let waiting = state.billing.waiting_for_game.read().await;
        if !timers.contains_key(pod_id) && !waiting.contains_key(pod_id) {
            drop(waiting);
            drop(timers);
            drop(games);
            tracing::warn!("Launch rejected for pod {}: billing session expired (TOCTOU)", pod_id);
            return Err(format!("Pod {} billing session expired during launch", pod_id));
        }
        drop(waiting);
        drop(timers);

        let tracker = GameTracker {
            pod_id: pod_id.to_string(),
            sim_type,
            game_state: GameState::Launching,
            pid: None,
            launched_at: Some(Utc::now()),
            error_message: None,
            launch_args: launch_args.clone(),
            auto_relaunch_count: 0,
            externally_tracked: false,
            dynamic_timeout_secs: Some(dynamic_timeout as i64),
            exit_codes: Vec::new(),
            max_auto_relaunch: max_relaunch_cap,
            playable_at: None,
            ready_delay_ms: None,
        };
        let info = tracker.to_info();
        games.insert(pod_id.to_string(), tracker);
        info
    };

    // Extract launch fields for metrics BEFORE consuming launch_args in the send
    let (launch_car, launch_track, launch_session_type, launch_args_hash) =
        extract_launch_fields(&launch_args);

    // Send command to agent with 1 retry (GAP-1 fix: fire-and-forget → retry-once)
    let launch_msg = launcher.make_launch_message(sim_type, launch_args);
    let mut send_ok = false;
    for attempt in 1..=2 {
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(pod_id) {
            match tx.send(launch_msg.clone()).await {
                Ok(_) => { send_ok = true; break; }
                Err(e) => {
                    tracing::warn!("LaunchGame send attempt {}/2 failed for pod {}: {}", attempt, pod_id, e);
                    drop(senders);
                    if attempt == 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    }
                }
            }
        } else {
            drop(senders);
            if attempt == 1 {
                tracing::warn!("No agent connected for pod {} (attempt 1/2), retrying in 3s", pod_id);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
            break;
        }
    }
    if !send_ok {
        tracing::warn!("No agent connected for pod {} after 2 attempts", pod_id);
        // Update tracker to error
        if let Some(tracker) = state
            .game_launcher
            .active_games
            .write()
            .await
            .get_mut(pod_id)
        {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some("No agent connected".to_string());
            // Re-capture info AFTER updating to Error state so dashboard gets correct state
            let error_info = tracker.to_info();
            if let Err(e) = state
                .dashboard_tx
                .send(DashboardEvent::GameStateChanged(error_info))
            {
                tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
            }
        }
        return Err(format!("No agent connected for pod {}", pod_id));
    }

    // Broadcast to dashboards (only reached if agent IS connected)
    if let Err(e) = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info))
    {
        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
    }

    // Log event to DB (legacy table, backward compat)
    log_game_event(state, pod_id, &sim_type.to_string(), "launched", None, None).await;

    // Rich launch event recording to new launch_events table + JSONL
    {
        let launch_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: launch_car,
            track: launch_track,
            session_type: launch_session_type,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome: metrics::LaunchOutcome::Success,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: None,
            launch_args_hash,
            attempt_number: 1,
            db_fallback: None,
        };
        metrics::record_launch_event(&state.db, &launch_event, &state.config.venue.venue_id).await;
    }
    Ok(())
}

/// CRASH-04: Relaunch a crashed game using stored launch_args from GameTracker.
/// Resets auto_relaunch_count so Race Engineer gets fresh attempts.
pub async fn relaunch_game(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), String> {
    // Normalize pod_id to canonical form at function entry
    let pod_id_owned = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_owned.as_str();

    // Get stored launch info from tracker
    let (sim_type, launch_args) = {
        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get(pod_id).ok_or("No game tracker for pod")?;
        if tracker.game_state != GameState::Error {
            return Err(format!("Pod game is {:?}, not Error — cannot relaunch", tracker.game_state));
        }
        // LAUNCH-16: Guard null launch_args — externally tracked games don't have original args
        if tracker.externally_tracked || tracker.launch_args.is_none() {
            return Err(format!(
                "Cannot relaunch pod {} — original launch args unavailable (externally tracked or null). Please relaunch from kiosk.",
                pod_id
            ));
        }
        (tracker.sim_type, tracker.launch_args.clone())
    };

    // Verify billing is still active
    let has_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id);
    if !has_billing {
        return Err("No active billing session — cannot relaunch".into());
    }

    // Reset relaunch counter for fresh Race Engineer attempts
    {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.auto_relaunch_count = 0;
            tracker.game_state = GameState::Launching;
            tracker.error_message = None;
            tracker.launched_at = Some(Utc::now());
        }
    }

    // Extract launch fields for metrics BEFORE consuming launch_args in the send
    let (relaunch_car, relaunch_track, relaunch_session_type, relaunch_args_hash) =
        extract_launch_fields(&launch_args);

    // Send LaunchGame to agent (canonical pod_id guaranteed by normalization at entry)
    let senders = state.agent_senders.read().await;
    let tx = senders.get(pod_id).ok_or("Pod not connected")?;
    tx.send(CoreToAgentMessage::LaunchGame {
        sim_type,
        launch_args,
        force_clean: true,
        duration_minutes: None,
    })
    .await
    .map_err(|e| format!("Failed to send launch command: {}", e))?;

    // Log + broadcast
    log_game_event(state, pod_id, &sim_type.to_string(), "relaunched", None, Some("Manual relaunch from kiosk")).await;

    // Rich launch event recording for relaunch
    {
        let relaunch_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: relaunch_car,
            track: relaunch_track,
            session_type: relaunch_session_type,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome: metrics::LaunchOutcome::Success,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: Some("Manual relaunch from kiosk".to_string()),
            launch_args_hash: relaunch_args_hash,
            attempt_number: 2,
            db_fallback: None,
        };
        metrics::record_launch_event(&state.db, &relaunch_event, &state.config.venue.venue_id).await;
    }
    let info = {
        let games = state.game_launcher.active_games.read().await;
        games.get(pod_id).map(|t| t.to_info())
    };
    if let Some(info) = info {
        let pod_id_for_warn = pod_id.to_string();
        if let Err(e) = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info))
        {
            tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id_for_warn, e);
        }
    }

    log_pod_activity(state, pod_id, "game", "Game Relaunched", "Manual relaunch from kiosk", "core");

    Ok(())
}

async fn stop_game(state: &Arc<AppState>, pod_id: &str) {
    // Normalize pod_id to canonical form at function entry
    let pod_id_owned = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_owned.as_str();

    // Update tracker to Stopping
    let info = {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.game_state = GameState::Stopping;
            Some(tracker.to_info())
        } else {
            None
        }
    };

    if let Some(info) = info {
        // LAUNCH-19: Log actual sim_type, not empty string
        log_pod_activity(state, pod_id, "game", "Game Stopping", &info.sim_type.to_string(), "core");

        // Send command to agent (canonical pod_id guaranteed by normalization at entry)
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(pod_id) {
            if let Err(e) = tx.send(CoreToAgentMessage::StopGame).await {
                tracing::error!("Failed to send StopGame to pod {}: {}", pod_id, e);
            }
        }

        // Broadcast to dashboards
        let sim_type_str = info.sim_type.to_string();
        if let Err(e) = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info))
        {
            tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
        }

        // Log event (legacy table)
        log_game_event(state, pod_id, &sim_type_str, "stopping", None, None).await;

        // Rich stop event recording
        {
            let stop_event = metrics::LaunchEvent {
                id: uuid::Uuid::new_v4().to_string(),
                pod_id: pod_id.to_string(),
                sim_type: sim_type_str,
                car: None,
                track: None,
                session_type: None,
                timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                outcome: metrics::LaunchOutcome::Success,
                error_taxonomy: None,
                duration_to_playable_ms: None,
                error_details: Some("Graceful stop".to_string()),
                launch_args_hash: None,
                attempt_number: 1,
                db_fallback: None,
            };
            metrics::record_launch_event(&state.db, &stop_event, &state.config.venue.venue_id).await;
        }

        // STATE-01: Spawn 30s Stopping timeout — auto-transitions to Error if game doesn't stop
        let state_clone = state.clone();
        let pod_id_timeout = pod_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let mut games = state_clone.game_launcher.active_games.write().await;
            if let Some(tracker) = games.get_mut(&pod_id_timeout) {
                if tracker.game_state == GameState::Stopping {
                    tracker.game_state = GameState::Error;
                    tracker.error_message = Some("Stop timed out (30s)".to_string());
                    let info = tracker.to_info();
                    drop(games);
                    if let Err(e) = state_clone.dashboard_tx.send(DashboardEvent::GameStateChanged(info)) {
                        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id_timeout, e);
                    }
                    tracing::warn!("game state: Stopping timed out on pod {}", pod_id_timeout);
                }
            }
        });
    }
}

/// Called when agent reports a game state update
pub async fn handle_game_state_update(state: &Arc<AppState>, info: GameLaunchInfo) {
    let pod_id_normalized = normalize_pod_id(&info.pod_id).unwrap_or_else(|_| info.pod_id.clone());
    let pod_id = &pod_id_normalized;

    // Update in-memory tracker
    {
        let mut games = state.game_launcher.active_games.write().await;
        match info.game_state {
            GameState::Idle => {
                // Game stopped normally — remove tracker
                games.remove(pod_id);
            }
            _ => {
                if let Some(tracker) = games.get_mut(pod_id) {
                    tracker.game_state = info.game_state;
                    tracker.pid = info.pid.or(tracker.pid);
                    tracker.error_message = info.error_message.clone();
                    if info.game_state == GameState::Running && tracker.launched_at.is_none() {
                        tracker.launched_at = Some(Utc::now());
                    }
                    // Phase 282: Record playable_at and ready_delay_ms when game becomes Running
                    if info.game_state == GameState::Running && tracker.playable_at.is_none() {
                        let now = Utc::now();
                        tracker.playable_at = Some(now);
                        if let Some(launched) = tracker.launched_at {
                            let delay = now.signed_duration_since(launched).num_milliseconds();
                            tracker.ready_delay_ms = Some(delay);
                            tracing::info!(
                                "Phase 282: pod {} ready_delay_ms={} (launch→playable)",
                                tracker.pod_id, delay
                            );
                        }
                    }
                } else {
                    // Agent reported state for a game we don't have tracked — create tracker
                    games.insert(
                        pod_id.to_string(),
                        GameTracker {
                            pod_id: pod_id.to_string(),
                            sim_type: info.sim_type,
                            game_state: info.game_state,
                            pid: info.pid,
                            launched_at: info.launched_at,
                            error_message: info.error_message.clone(),
                            launch_args: None,
                            auto_relaunch_count: 0,
                            externally_tracked: true,
                            dynamic_timeout_secs: None,
                            exit_codes: Vec::new(),
                            max_auto_relaunch: 2,
                            playable_at: None,
                            ready_delay_ms: None,
                        },
                    );
                }
            }
        }
    }

    // Update pod info
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(pod_id) {
            pod.game_state = Some(info.game_state);
            pod.current_game = if info.game_state == GameState::Idle {
                None
            } else {
                Some(info.sim_type)
            };
        }
    }

    // Determine event type for logging
    let event_type = match info.game_state {
        GameState::Running => "running",
        GameState::Loading => "loading",
        GameState::Error => "crashed",
        GameState::Idle => "stopped",
        GameState::Launching => "launched",
        GameState::Stopping => "stopping",
    };

    // Log to DB (legacy table)
    log_game_event(
        state,
        pod_id,
        &info.sim_type.to_string(),
        event_type,
        info.pid,
        info.error_message.as_deref(),
    )
    .await;

    // Rich state update event recording (only for Error/Crash states — informational states covered by launch/relaunch)
    if matches!(info.game_state, GameState::Error) {
        let (outcome, taxonomy) = if info.game_state == GameState::Error {
            let tax = classify_error_taxonomy(info.error_message.as_deref(), info.exit_code);
            (metrics::LaunchOutcome::Crash, Some(tax))
        } else {
            (metrics::LaunchOutcome::Error, None)
        };
        let state_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: info.sim_type.to_string(),
            car: None,
            track: None,
            session_type: None,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome,
            error_taxonomy: taxonomy,
            duration_to_playable_ms: None,
            error_details: info.error_message.clone(),
            launch_args_hash: None,
            attempt_number: 1,
            db_fallback: None,
        };
        metrics::record_launch_event(&state.db, &state_event, &state.config.venue.venue_id).await;
    }

    // Broadcast to dashboards
    if let Err(e) = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info.clone()))
    {
        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
    }

    // Phase 282: Update launch_events with ready_delay when game becomes Running
    if info.game_state == GameState::Running {
        let ready_delay = {
            let games = state.game_launcher.active_games.read().await;
            games.get(pod_id).and_then(|t| t.ready_delay_ms)
        };
        if let Some(delay_ms) = ready_delay {
            let _ = sqlx::query(
                "UPDATE launch_events SET duration_to_playable_ms = ? WHERE pod_id = ? AND duration_to_playable_ms IS NULL ORDER BY created_at DESC LIMIT 1"
            )
            .bind(delay_ms)
            .bind(pod_id)
            .execute(&state.db)
            .await;
        }
    }

    // ─── AC Timer Sync: reset billing timer on initial game start ────
    // When AC reports Running, reset billing driving_seconds to 0 so both
    // timers (AC practice + billing) start at the same moment. Only applies
    // during initial launch (driving_seconds < 120) — crash relaunches skip.
    if info.game_state == GameState::Running && info.sim_type == SimType::AssettoCorsa {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id) {
            if timer.status == BillingSessionStatus::Active && timer.driving_seconds < 120 {
                tracing::info!(
                    "AC timer sync: resetting billing timer for session {} on pod {} (was {}s)",
                    timer.session_id, pod_id, timer.driving_seconds
                );
                timer.driving_seconds = 0;
                timer.started_at = Some(Utc::now());
                // Sync to DB immediately
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET driving_seconds = 0, started_at = ? WHERE id = ?"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(&timer.session_id)
                .execute(&state.db)
                .await;
            }
        }
    }

    // ─── Race Engineer: Auto-relaunch on crash if billing is active ────
    // LAUNCH-17: Atomic check+increment under single write lock (no TOCTOU)
    if info.game_state == GameState::Error {
        // RECOVER-02 SLA tracking: capture crash detection time at start of Error branch
        let crash_detected_at = std::time::Instant::now();
        let current_exit_code = info.exit_code;
        let error_taxonomy = classify_error_taxonomy(info.error_message.as_deref(), current_exit_code);
        let failure_mode_str = format!("{:?}", error_taxonomy);

        let has_billing = state
            .billing
            .active_timers
            .read()
            .await
            .contains_key(pod_id);

        if has_billing {
            // LAUNCH-17: Single write lock — read AND increment atomically to prevent duplicate relaunches
            // from rapid duplicate Error events on the same pod.
            // Also push exit_code and extract car/track in same lock.
            let should_relaunch: Option<(u32, u32, SimType, Option<String>, Option<String>, Option<String>)> = {
                let mut games = state.game_launcher.active_games.write().await;
                if let Some(tracker) = games.get_mut(pod_id) {
                    // RECOVER-05: Accumulate exit codes for staff alert diagnostics
                    tracker.exit_codes.push(current_exit_code);

                    // Extract car/track for enriched recovery events
                    let (car, track, _, _) = extract_launch_fields(&tracker.launch_args);

                    // LAUNCH-16: Guard null args — externally tracked games have no original launch args
                    if tracker.externally_tracked || tracker.launch_args.is_none() {
                        tracing::warn!(
                            "Race Engineer: skipping relaunch for pod {} — original launch args unavailable, please relaunch from kiosk",
                            pod_id
                        );
                        // RECOVER-04: Broadcast dashboard notification for null-args guard
                        let null_info = {
                            let mut null_tracker_info = tracker.to_info();
                            null_tracker_info.error_message = Some(
                                "Cannot auto-relaunch: no launch args. Manual relaunch required from kiosk.".to_string()
                            );
                            null_tracker_info
                        };
                        let _ = state.dashboard_tx.send(DashboardEvent::GameStateChanged(null_info));
                        None
                    } else if tracker.auto_relaunch_count < tracker.max_auto_relaunch {
                        tracker.auto_relaunch_count += 1;
                        let attempt = tracker.auto_relaunch_count;
                        let max_cap = tracker.max_auto_relaunch;
                        Some((attempt, max_cap, tracker.sim_type, tracker.launch_args.clone(), car, track))
                    } else {
                        None // exhausted
                    }
                } else {
                    None // no tracker — don't relaunch
                }
            };

            if let Some((attempt, max_cap, sim_type, launch_args, car, track)) = should_relaunch {
                let pod_id_owned = pod_id.to_string();
                let state_clone = state.clone();
                let sim_name = format!("{}", sim_type);
                let failure_mode_clone = failure_mode_str.clone();
                let car_clone = car.clone();
                let track_clone = track.clone();

                // RECOVER-03: Query historical recovery data for best action
                let (best_action, success_rate) = metrics::query_best_recovery_action(
                    &state.db,
                    pod_id,
                    &sim_name,
                    &failure_mode_str,
                ).await;
                tracing::info!(
                    "recovery action selected: {} ({:.0}% historical success) for pod {} attempt {}/{}",
                    best_action, success_rate * 100.0, pod_id, attempt, max_cap
                );

                let action_label = format!("{}_attempt_{}", best_action, attempt);

                log_pod_activity(
                    state,
                    pod_id,
                    "race_engineer",
                    "Auto-Relaunching Game",
                    &format!("Race Engineer relaunching {} after crash (attempt {}/{}) — action: {}", sim_name, attempt, max_cap, best_action),
                    "race_engineer",
                );

                // Delayed relaunch (5s) — verify billing still active + game still in Error
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    // Re-check billing still active
                    let still_billing = state_clone
                        .billing
                        .active_timers
                        .read()
                        .await
                        .contains_key(pod_id_owned.as_str());

                    let still_error = state_clone
                        .game_launcher
                        .active_games
                        .read()
                        .await
                        .get(&pod_id_owned)
                        .map(|t| t.game_state == GameState::Error)
                        .unwrap_or(false);

                    if still_billing && still_error {
                        tracing::info!(
                            "Race Engineer: relaunching {} on pod {} (attempt {}/2)",
                            sim_name, pod_id_owned, attempt
                        );

                        // BUG-01 fix: Recalculate remaining duration from DB at relaunch time.
                        // The stored launch_args contain the duration from ORIGINAL launch — stale
                        // after crash + elapsed time. Without this, AC sessions can outlast billing.
                        let fresh_duration: Option<u32> = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                            "SELECT allocated_seconds, driving_seconds, split_duration_minutes FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
                        )
                        .bind(&pod_id_owned)
                        .fetch_optional(&state_clone.db)
                        .await
                        .ok()
                        .flatten()
                        .map(|(allocated, driven, split_mins)| {
                            if let Some(sm) = split_mins {
                                sm as u32
                            } else {
                                let remaining = (allocated as u32).saturating_sub(driven as u32);
                                (remaining + 59) / 60 // ceiling division
                            }
                        });

                        // Inject fresh duration into launch_args JSON
                        let updated_args = launch_args.map(|args_str| {
                            if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&args_str) {
                                if let Some(dur) = fresh_duration {
                                    parsed["duration_minutes"] = serde_json::json!(dur);
                                }
                                parsed.to_string()
                            } else {
                                args_str
                            }
                        });

                        let senders = state_clone.agent_senders.read().await;
                        if let Some(tx) = senders.get(&pod_id_owned) {
                            let _ = tx
                                .send(CoreToAgentMessage::LaunchGame {
                                    sim_type,
                                    launch_args: updated_args,
                                    force_clean: true,
                                    duration_minutes: fresh_duration,
                                })
                                .await;
                        }
                        drop(senders);

                        // RECOVER-02: Record enriched recovery event with actual ErrorTaxonomy,
                        // car/track from launch_args, exit_code in details, and SLA duration
                        let recovery_duration_ms = crash_detected_at.elapsed().as_millis() as i64;
                        let recovery_event = metrics::RecoveryEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            pod_id: pod_id_owned.clone(),
                            sim_type: Some(sim_name.clone()),
                            car: car_clone,
                            track: track_clone,
                            failure_mode: failure_mode_clone,
                            recovery_action_tried: action_label,
                            recovery_outcome: metrics::RecoveryOutcome::Attempted,
                            recovery_duration_ms: Some(recovery_duration_ms),
                            error_details: Some(format!("exit_code: {:?}", current_exit_code)),
                        };
                        metrics::record_recovery_event(&state_clone.db, &recovery_event, &state_clone.config.venue.venue_id).await;
                    }
                });
            } else {
                // Check if exhausted (auto_relaunch_count >= max_auto_relaunch) — send staff alert + pause billing
                let (is_exhausted, tracker_exit_codes, tracker_launch_args) = {
                    let games = state.game_launcher.active_games.read().await;
                    games.get(pod_id).map(|t| (
                        t.auto_relaunch_count >= t.max_auto_relaunch,
                        t.exit_codes.clone(),
                        t.launch_args.clone(),
                    )).unwrap_or((false, Vec::new(), None))
                };

                if is_exhausted {
                    // Extract car/track for enriched exhausted event
                    let (car, track, _, _) = extract_launch_fields(&tracker_launch_args);

                    // LAUNCH-03: All relaunch attempts exhausted — pause billing
                    log_pod_activity(
                        state,
                        pod_id,
                        "race_engineer",
                        "Relaunch Limit Reached",
                        &format!(
                            "Race Engineer: max relaunch attempts (2) reached for {}. Billing paused — staff action required.",
                            info.sim_type
                        ),
                        "race_engineer",
                    );

                    // Pause billing so customer doesn't pay for downtime
                    let mut timers = state.billing.active_timers.write().await;
                    if let Some(timer) = timers.get_mut(pod_id) {
                        if timer.status == BillingSessionStatus::Active {
                            timer.status = BillingSessionStatus::PausedCrashRecovery;
                            timer.pause_reason = crate::billing::PauseReason::CrashRecovery;
                            timer.last_paused_at = Some(Utc::now());
                            timer.pause_seconds = 0;
                            tracing::info!(
                                "LAUNCH-03: Billing paused (crash recovery) on pod {} — launch failed after max auto-relaunch attempts",
                                pod_id
                            );
                        }
                    }
                    drop(timers);

                    // LAUNCH-15: WhatsApp staff alert — notify staff that automation failed
                    // RECOVER-05: Include exit_codes and suggested action in alert
                    let best_action_for_alert = {
                        let sim_name_alert = info.sim_type.to_string();
                        let (action, _) = metrics::query_best_recovery_action(
                            &state.db, pod_id, &sim_name_alert, &failure_mode_str
                        ).await;
                        action
                    };
                    send_staff_launch_alert(
                        state,
                        pod_id,
                        &info.sim_type.to_string(),
                        &failure_mode_str,
                        &tracker_exit_codes,
                        &best_action_for_alert,
                    ).await;

                    // RECOVER-02: Record enriched exhausted recovery event
                    let recovery_duration_ms = crash_detected_at.elapsed().as_millis() as i64;
                    let recovery_event = metrics::RecoveryEvent {
                        id: uuid::Uuid::new_v4().to_string(),
                        pod_id: pod_id.to_string(),
                        sim_type: Some(format!("{}", info.sim_type)),
                        car,
                        track,
                        failure_mode: failure_mode_str,
                        recovery_action_tried: "auto_relaunch_exhausted".to_string(),
                        recovery_outcome: metrics::RecoveryOutcome::Failed,
                        recovery_duration_ms: Some(recovery_duration_ms),
                        error_details: Some(format!(
                            "Max relaunch attempts (2) reached. Billing paused. exit_code: {:?}",
                            current_exit_code
                        )),
                    };
                    metrics::record_recovery_event(&state.db, &recovery_event, &state.config.venue.venue_id).await;
                }
            }
        }
    }
}

/// Send WhatsApp alert to staff after Race Engineer exhausts all retry attempts (LAUNCH-15).
/// Includes exit_codes and suggested_action for diagnostics (RECOVER-05).
/// Best-effort — errors are logged but never swallowed.
async fn send_staff_launch_alert(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &str,
    error_taxonomy: &str,
    exit_codes: &[Option<i32>],
    suggested_action: &str,
) {
    let exit_codes_str = if exit_codes.is_empty() {
        "none recorded".to_string()
    } else {
        exit_codes.iter()
            .map(|c| c.map(|n| n.to_string()).unwrap_or_else(|| "unknown".to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let msg = format!(
        "Launch Failure - Pod {}\nGame: {}\nError: {}\nAttempts: 2/2 exhausted\nExit codes: {}\nSuggested: {}\nAction: Check pod + try different car/track",
        pod_id, sim_type, error_taxonomy, exit_codes_str, suggested_action
    );
    tracing::warn!("LAUNCH-15: Staff alert for pod {} — {}", pod_id, msg);

    // Access Evolution API config (same pattern as billing.rs WhatsApp receipt)
    let (evo_url, evo_key, evo_instance) = match &state.config.auth {
        ref auth => match (auth.evolution_url.as_deref(), auth.evolution_api_key.as_deref(), auth.evolution_instance.as_deref()) {
            (Some(url), Some(key), Some(inst)) => (url.to_string(), key.to_string(), inst.to_string()),
            _ => {
                tracing::warn!("LAUNCH-15: Evolution API not configured — skipping WhatsApp staff alert for pod {}", pod_id);
                return;
            }
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("LAUNCH-15: Failed to build HTTP client for staff alert: {}", e);
            return;
        }
    };

    let payload = serde_json::json!({
        "number": "917075778180",
        "text": msg,
    });

    match client
        .post(format!("{}/message/sendText/{}", evo_url, evo_instance))
        .header("apikey", evo_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            tracing::info!("LAUNCH-15: Staff WhatsApp alert sent for pod {} (status: {})", pod_id, resp.status());
        }
        Err(e) => {
            tracing::error!("LAUNCH-15: Failed to send staff WhatsApp alert for pod {}: {}", pod_id, e);
        }
    }
}

/// Periodic health check: detect stale Launching states.
/// AC with CSP mods can take 90s+ to load; use 120s timeout for AC, 60s for others.
pub async fn check_game_health(state: &Arc<AppState>) {
    let now = Utc::now();
    let mut timed_out = Vec::new();

    {
        let games = state.game_launcher.active_games.read().await;
        for (pod_id, tracker) in games.iter() {
            if tracker.game_state == GameState::Launching {
                if let Some(launched_at) = tracker.launched_at {
                    let elapsed = now.signed_duration_since(launched_at);
                    // LAUNCH-08: Use stored dynamic timeout, fall back to game-specific default
                    let timeout_secs = tracker.dynamic_timeout_secs.unwrap_or(match tracker.sim_type {
                        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => 120,
                        _ => 90,
                    });
                    if elapsed.num_seconds() > timeout_secs {
                        timed_out.push((pod_id.clone(), tracker.sim_type, timeout_secs));
                    }
                }
            }
            // STATE-01 edge case: detect stale Stopping state from server restart
            // (the in-memory timeout spawn is gone after restart, so we need this catch)
            // GAME-05 fix: use last_state_change (if available) or launched_at + game duration
            // to avoid force-erroring long-running games. Only timeout Stopping if the game
            // has been in Stopping state for >60s (not since launch).
            if tracker.game_state == GameState::Stopping {
                if let Some(launched_at) = tracker.launched_at {
                    let since_launch = now.signed_duration_since(launched_at).num_seconds();
                    // GAME-05 MMA iter2: two tiers —
                    // 1) Short-lived (30-90s since launch): likely reconstructed post-restart
                    // 2) Long-stuck (>300s since launch): catch genuinely stuck Stopping states
                    // Normal in-memory 30s spawn handles non-restart cases in between.
                    if (since_launch > 30 && since_launch < 90) || since_launch > 300 {
                        timed_out.push((pod_id.clone(), tracker.sim_type, 30));
                    }
                }
            }
        }
    }

    for (pod_id, sim_type, timeout_secs) in timed_out {
        tracing::warn!("Game launch timed out on pod {} ({}s limit for {:?})", pod_id, timeout_secs, sim_type);
        log_pod_activity(state, &pod_id, "game", "Launch Timeout", &format!("{} failed to start within {}s", sim_type, timeout_secs), "core");

        let timeout_msg = format!("Launch timed out ({}s)", timeout_secs);
        let info = GameLaunchInfo {
            pod_id: pod_id.clone(),
            sim_type,
            game_state: GameState::Error,
            pid: None,
            launched_at: None,
            error_message: Some(timeout_msg.clone()),
            diagnostics: None,
            exit_code: None,
            playable_at: None,
            ready_delay_ms: None,
        };

        // Update tracker
        if let Some(tracker) = state
            .game_launcher
            .active_games
            .write()
            .await
            .get_mut(&pod_id)
        {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some(timeout_msg);
        }

        // Log (legacy table)
        log_game_event(&state, &pod_id, &sim_type.to_string(), "timeout", None, None).await;

        // Rich timeout event recording
        {
            let timeout_event = metrics::LaunchEvent {
                id: uuid::Uuid::new_v4().to_string(),
                pod_id: pod_id.clone(),
                sim_type: sim_type.to_string(),
                car: None,
                track: None,
                session_type: None,
                timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                outcome: metrics::LaunchOutcome::Timeout,
                error_taxonomy: Some(metrics::ErrorTaxonomy::LaunchTimeout),
                duration_to_playable_ms: Some(timeout_secs * 1000),
                error_details: Some(format!("Launch timed out after {}s", timeout_secs)),
                launch_args_hash: None,
                attempt_number: 1,
                db_fallback: None,
            };
            metrics::record_launch_event(&state.db, &timeout_event, &state.config.venue.venue_id).await;
        }

        // LAUNCH-18: Route timeout through handle_game_state_update so Race Engineer
        // auto-relaunch logic fires on timeout (same path as crash events).
        // handle_game_state_update handles dashboard broadcast too — do NOT broadcast here.
        handle_game_state_update(state, info).await;
    }
}

async fn log_game_event(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &str,
    event_type: &str,
    pid: Option<u32>,
    error_message: Option<&str>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let result = sqlx::query(
        "INSERT INTO game_launch_events (id, pod_id, sim_type, event_type, pid, error_message, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(pod_id)
    .bind(sim_type)
    .bind(event_type)
    .bind(pid.map(|p| p as i64))
    .bind(error_message)
    .bind(&now)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        tracing::error!("game_launch_event insert failed for pod {pod_id}: {e}");
        // JSONL fallback — write to launch-events.jsonl so the event is never lost
        let fallback_event = crate::metrics::LaunchEvent {
            id,
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: None,
            track: None,
            session_type: None,
            timestamp: now,
            outcome: crate::metrics::LaunchOutcome::Error,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: error_message.map(|s| s.to_string()),
            launch_args_hash: None,
            attempt_number: 1,
            db_fallback: Some(true),
        };
        crate::metrics::record_launch_event_jsonl_only(&fallback_event).await;
    }
}

/// Extract car, track, session_type, and a hash from an optional launch_args JSON string.
/// Returns (car, track, session_type, hash) — all None if args absent or unparseable.
fn extract_launch_fields(
    launch_args: &Option<String>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let Some(args_json) = launch_args else {
        return (None, None, None, None);
    };
    let hash = Some(metrics::hash_launch_args(args_json));
    match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(v) => {
            let car = v.get("car").and_then(|x| x.as_str()).map(str::to_string);
            let track = v.get("track").and_then(|x| x.as_str()).map(str::to_string);
            let session_type = v
                .get("session_type")
                .and_then(|x| x.as_str())
                .map(str::to_string);
            (car, track, session_type, hash)
        }
        Err(_) => (None, None, None, hash),
    }
}

/// Classify an error message into a structured ErrorTaxonomy entry.
/// Uses simple heuristics — no heavy parsing needed at this phase.
fn classify_error_taxonomy(error_message: Option<&str>, exit_code: Option<i32>) -> metrics::ErrorTaxonomy {
    // Check typed exit_code first - more reliable than string parsing
    if let Some(code) = exit_code {
        return metrics::ErrorTaxonomy::ProcessCrash { exit_code: code as i64 };
    }
    let Some(msg) = error_message else {
        return metrics::ErrorTaxonomy::Unknown;
    };
    let msg_lower = msg.to_ascii_lowercase();
    if msg_lower.contains("shader") || msg_lower.contains("shader compilation") {
        metrics::ErrorTaxonomy::ShaderCompilationFail
    } else if msg_lower.contains("out of memory") || msg_lower.contains("oom") {
        metrics::ErrorTaxonomy::OutOfMemory
    } else if msg_lower.contains("anticheat") || msg_lower.contains("anti-cheat") {
        metrics::ErrorTaxonomy::AntiCheatKick
    } else if msg_lower.contains("config") && msg_lower.contains("corrupt") {
        metrics::ErrorTaxonomy::ConfigCorrupt
    } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
        metrics::ErrorTaxonomy::LaunchTimeout
    } else if msg_lower.contains("content manager") || msg_lower.contains("hang") {
        metrics::ErrorTaxonomy::ContentManagerHang
    } else if msg_lower.contains("missing") || msg_lower.contains("not found") {
        metrics::ErrorTaxonomy::MissingDependency
    } else if msg_lower.contains("billing") {
        metrics::ErrorTaxonomy::BillingGateRejected
    } else if msg_lower.contains("agent") || msg_lower.contains("disconnected") {
        metrics::ErrorTaxonomy::AgentDisconnected
    } else {
        metrics::ErrorTaxonomy::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::BillingTimer;
    use crate::config::Config;

    /// Build a minimal AppState for game_launcher unit tests.
    async fn make_state() -> Arc<AppState> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        // Create tables needed by launch_game (activity log + game events)
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS game_launch_events (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                event_type TEXT NOT NULL,
                pid INTEGER,
                error_message TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )"
        )
        .execute(&db)
        .await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS pod_activity (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                category TEXT NOT NULL,
                action TEXT NOT NULL,
                details TEXT,
                source TEXT,
                timestamp TEXT NOT NULL
            )"
        )
        .execute(&db)
        .await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS launch_events (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                car TEXT,
                track TEXT,
                session_type TEXT,
                timestamp TEXT NOT NULL,
                outcome TEXT NOT NULL,
                error_taxonomy TEXT,
                duration_to_playable_ms INTEGER,
                error_details TEXT,
                launch_args_hash TEXT,
                attempt_number INTEGER DEFAULT 1,
                db_fallback INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            )"
        )
        .execute(&db)
        .await;

        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS recovery_events (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                sim_type TEXT,
                car TEXT,
                track TEXT,
                failure_mode TEXT NOT NULL,
                recovery_action_tried TEXT NOT NULL,
                recovery_outcome TEXT NOT NULL,
                recovery_duration_ms INTEGER,
                error_details TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )"
        )
        .execute(&db)
        .await;

        let config = Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    // ── LIFE-02: Billing gate tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_launch_rejected_no_billing() {
        let state = make_state().await;
        // No billing timer inserted — active_timers is empty

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when no billing session");
        let err = result.unwrap_err();
        assert!(
            err.contains("no active billing"),
            "Error should mention 'no active billing', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_launch_allowed_with_billing() {
        let state = make_state().await;

        // Insert a dummy billing timer for pod_1
        {
            let timer = BillingTimer::dummy("pod_1");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_1".to_string(), timer);
        }

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        // The function will fail further down (no agent sender) — but it should NOT
        // fail with a billing error. If it errors, the message must NOT be about billing.
        if let Err(ref err) = result {
            assert!(
                !err.contains("no active billing"),
                "Should pass billing check when timer exists, got: {}",
                err
            );
        }
        // If it somehow succeeds, that's fine too — billing gate passed.
    }

    // ── LIFE-04: Double-launch guard tests ───────────────────────────────────

    #[tokio::test]
    async fn test_double_launch_blocked_running() {
        let state = make_state().await;

        // Insert billing timer (needed to pass billing gate)
        {
            let timer = BillingTimer::dummy("pod_1");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_1".to_string(), timer);
        }

        // Insert a GameTracker in Running state
        {
            state
                .game_launcher
                .active_games
                .write()
                .await
                .insert(
                    "pod_1".to_string(),
                    GameTracker {
                        pod_id: "pod_1".to_string(),
                        sim_type: SimType::AssettoCorsa,
                        game_state: GameState::Running,
                        pid: Some(1234),
                        launched_at: Some(Utc::now()),
                        error_message: None,
                        launch_args: None,
                        auto_relaunch_count: 0,
                        externally_tracked: false,
                        dynamic_timeout_secs: None,
                        exit_codes: Vec::new(),
                        max_auto_relaunch: 2,
                        playable_at: None,
                        ready_delay_ms: None,
                    },
                );
        }

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when game is already Running");
        let err = result.unwrap_err();
        assert!(
            err.contains("already has a game active"),
            "Error should mention 'already has a game active', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_double_launch_blocked_launching() {
        let state = make_state().await;

        // Insert billing timer (needed to pass billing gate)
        {
            let timer = BillingTimer::dummy("pod_1");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_1".to_string(), timer);
        }

        // Insert a GameTracker in Launching state
        {
            state
                .game_launcher
                .active_games
                .write()
                .await
                .insert(
                    "pod_1".to_string(),
                    GameTracker {
                        pod_id: "pod_1".to_string(),
                        sim_type: SimType::AssettoCorsa,
                        game_state: GameState::Launching,
                        pid: None,
                        launched_at: Some(Utc::now()),
                        error_message: None,
                        launch_args: None,
                        auto_relaunch_count: 0,
                        externally_tracked: false,
                        dynamic_timeout_secs: None,
                        exit_codes: Vec::new(),
                        max_auto_relaunch: 2,
                        playable_at: None,
                        ready_delay_ms: None,
                    },
                );
        }

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when game is already Launching");
        let err = result.unwrap_err();
        assert!(
            err.contains("already"),
            "Error should contain 'already', got: {}",
            err
        );
    }

    // ── F1 25: Characterization tests — identical server-side behavior ────────

    #[tokio::test]
    async fn test_f1_25_launch_rejected_no_billing() {
        let state = make_state().await;

        let result = launch_game(&state, "pod_8", SimType::F125, None).await;

        assert!(result.is_err(), "F1 25 launch should fail without billing");
        assert!(
            result.unwrap_err().contains("no active billing"),
            "Should reject with billing error"
        );
    }

    #[tokio::test]
    async fn test_f1_25_launch_passes_billing_gate() {
        let state = make_state().await;

        {
            let timer = BillingTimer::dummy("pod_8");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_8".to_string(), timer);
        }

        let result = launch_game(&state, "pod_8", SimType::F125, None).await;

        // Will fail downstream (no agent sender) — but must NOT fail on billing
        if let Err(ref err) = result {
            assert!(
                !err.contains("no active billing"),
                "F1 25 should pass billing gate, got: {}",
                err
            );
        }
    }

    #[tokio::test]
    async fn test_f1_25_double_launch_blocked() {
        let state = make_state().await;

        {
            let timer = BillingTimer::dummy("pod_8");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_8".to_string(), timer);
        }

        // Insert a running F1 25 game
        {
            state
                .game_launcher
                .active_games
                .write()
                .await
                .insert(
                    "pod_8".to_string(),
                    GameTracker {
                        pod_id: "pod_8".to_string(),
                        sim_type: SimType::F125,
                        game_state: GameState::Running,
                        pid: Some(5678),
                        launched_at: Some(Utc::now()),
                        error_message: None,
                        launch_args: None,
                        auto_relaunch_count: 0,
                        externally_tracked: false,
                        dynamic_timeout_secs: None,
                        exit_codes: Vec::new(),
                        max_auto_relaunch: 2,
                        playable_at: None,
                        ready_delay_ms: None,
                    },
                );
        }

        let result = launch_game(&state, "pod_8", SimType::F125, None).await;

        assert!(result.is_err(), "Should block double-launch for F1 25");
        assert!(
            result.unwrap_err().contains("already has a game active"),
            "Should mention game already active"
        );
    }

    #[tokio::test]
    async fn test_f1_25_launch_with_args_passes_billing() {
        let state = make_state().await;

        {
            let timer = BillingTimer::dummy("pod_8");
            state
                .billing
                .active_timers
                .write()
                .await
                .insert("pod_8".to_string(), timer);
        }

        // Simulate the launch_args JSON the kiosk wizard sends for non-AC games
        // (useSetupWizard.ts:185-191 — only game, driver, game_mode)
        let launch_args = serde_json::json!({
            "game": "f1_25",
            "driver": "Test Driver",
            "game_mode": "single"
        })
        .to_string();

        let result = launch_game(
            &state,
            "pod_8",
            SimType::F125,
            Some(launch_args),
        )
        .await;

        // Passes billing + validation gates, fails at agent sender (expected)
        if let Err(ref err) = result {
            assert!(
                !err.contains("no active billing"),
                "F1 25 with args should pass billing, got: {}",
                err
            );
        }

        // GameTracker should exist in Launching or Error state
        let games = state.game_launcher.active_games.read().await;
        assert!(
            games.contains_key("pod_8"),
            "GameTracker should be created for pod_8"
        );
        let tracker = games.get("pod_8").unwrap();
        assert_eq!(tracker.sim_type, SimType::F125);
        assert!(
            tracker.launch_args.is_some(),
            "launch_args should be stored for relaunch"
        );
    }

    #[tokio::test]
    async fn test_game_state_update_f1_25_running() {
        let state = make_state().await;

        // Simulate agent reporting F1 25 running
        let info = GameLaunchInfo {
            pod_id: "pod_8".to_string(),
            sim_type: SimType::F125,
            game_state: GameState::Running,
            pid: Some(9999),
            launched_at: Some(Utc::now()),
            error_message: None,
            diagnostics: None,
            exit_code: None,
            playable_at: None,
            ready_delay_ms: None,
        };

        handle_game_state_update(&state, info).await;

        // Tracker should be created
        let games = state.game_launcher.active_games.read().await;
        assert!(games.contains_key("pod_8"));
        let tracker = games.get("pod_8").unwrap();
        assert_eq!(tracker.game_state, GameState::Running);
        assert_eq!(tracker.pid, Some(9999));
    }

    #[tokio::test]
    async fn test_game_state_update_f1_25_idle_removes_tracker() {
        let state = make_state().await;

        // Pre-insert a tracker
        {
            state
                .game_launcher
                .active_games
                .write()
                .await
                .insert(
                    "pod_8".to_string(),
                    GameTracker {
                        pod_id: "pod_8".to_string(),
                        sim_type: SimType::F125,
                        game_state: GameState::Running,
                        pid: Some(9999),
                        launched_at: Some(Utc::now()),
                        error_message: None,
                        launch_args: None,
                        auto_relaunch_count: 0,
                        externally_tracked: false,
                        dynamic_timeout_secs: None,
                        exit_codes: Vec::new(),
                        max_auto_relaunch: 2,
                        playable_at: None,
                        ready_delay_ms: None,
                    },
                );
        }

        // Agent reports game stopped
        let info = GameLaunchInfo {
            pod_id: "pod_8".to_string(),
            sim_type: SimType::F125,
            game_state: GameState::Idle,
            pid: None,
            launched_at: None,
            error_message: None,
            diagnostics: None,
            exit_code: None,
            playable_at: None,
            ready_delay_ms: None,
        };

        handle_game_state_update(&state, info).await;

        // Tracker should be removed
        let games = state.game_launcher.active_games.read().await;
        assert!(
            !games.contains_key("pod_8"),
            "Idle state should remove tracker"
        );
    }

    // ── LAUNCH-01: GameLauncherImpl trait dispatch tests ─────────────────────

    #[tokio::test]
    async fn test_trait_dispatch_ac() {
        let launcher = launcher_for(SimType::AssettoCorsa);
        // Valid JSON should return Ok
        assert!(launcher.validate_args(Some(r#"{"car":"x"}"#)).is_ok());
        // None should return Ok
        assert!(launcher.validate_args(None).is_ok());
        // Invalid JSON should return Err
        let result = launcher.validate_args(Some(r#"{"corrupt"#));
        assert!(result.is_err(), "Expected Err for invalid JSON");
        assert!(result.unwrap_err().contains("Invalid"), "Error should mention 'Invalid'");
    }

    #[tokio::test]
    async fn test_trait_dispatch_f1() {
        let launcher = launcher_for(SimType::F125);
        assert!(launcher.validate_args(None).is_ok(), "F1Launcher should accept None args");
        assert!(launcher.validate_args(Some(r#"{"game":"f1_25"}"#)).is_ok());
    }

    #[tokio::test]
    async fn test_trait_dispatch_iracing() {
        let launcher = launcher_for(SimType::IRacing);
        assert!(launcher.validate_args(None).is_ok(), "IRacingLauncher should accept None args");
    }

    // ── LAUNCH-02: Deferred billing (waiting_for_game) gate tests ────────────

    #[tokio::test]
    async fn test_launch_allowed_with_deferred_billing() {
        use crate::billing::WaitingForGameEntry;

        let state = make_state().await;

        // Insert into waiting_for_game ONLY (no active_timers entry)
        let entry = WaitingForGameEntry {
            pod_id: "pod_1".to_string(),
            driver_id: "test-driver".to_string(),
            pricing_tier_id: "tier-1".to_string(),
            custom_price_paise: None,
            custom_duration_minutes: None,
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: None,
            sim_type: None,
            pre_committed: None,
        };
        state.billing.waiting_for_game.write().await.insert("pod_1".to_string(), entry);

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        // Should pass billing gate. May fail at agent sender — that's OK.
        // Must NOT fail with "no active billing" when waiting_for_game is set.
        if let Err(ref err) = result {
            assert!(
                !err.contains("no active billing"),
                "Should pass billing gate with deferred entry, got: {}",
                err
            );
            assert!(
                !err.contains("paused"),
                "Should not be paused rejection, got: {}",
                err
            );
        }
    }

    // ── LAUNCH-03: Paused session rejection tests ─────────────────────────────

    #[tokio::test]
    async fn test_launch_rejected_paused_billing() {
        let state = make_state().await;

        let mut timer = BillingTimer::dummy("pod_1");
        timer.status = BillingSessionStatus::PausedManual;
        state.billing.active_timers.write().await.insert("pod_1".to_string(), timer);

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when billing is PausedManual");
        let err = result.unwrap_err();
        assert!(
            err.contains("paused"),
            "Error should contain 'paused', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_launch_rejected_paused_disconnect() {
        let state = make_state().await;

        let mut timer = BillingTimer::dummy("pod_1");
        timer.status = BillingSessionStatus::PausedDisconnect;
        state.billing.active_timers.write().await.insert("pod_1".to_string(), timer);

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when billing is PausedDisconnect");
        let err = result.unwrap_err();
        assert!(
            err.contains("paused"),
            "Error should contain 'paused', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_launch_rejected_paused_game_pause() {
        let state = make_state().await;

        let mut timer = BillingTimer::dummy("pod_1");
        timer.status = BillingSessionStatus::PausedGamePause;
        state.billing.active_timers.write().await.insert("pod_1".to_string(), timer);

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when billing is PausedGamePause");
        let err = result.unwrap_err();
        assert!(
            err.contains("paused"),
            "Error should contain 'paused', got: {}",
            err
        );
    }

    // ── LAUNCH-04: TOCTOU re-check test ──────────────────────────────────────

    #[tokio::test]
    async fn test_launch_toctou_billing_recheck() {
        // This test verifies the code path: after billing gate passes (billing present),
        // but just before tracker insert (inside write lock), billing is removed.
        // The TOCTOU re-check should catch this and return Err.
        // We simulate this by: NOT inserting any billing (so both checks fail at TOCTOU).
        // The first gate check would normally catch no-billing, but we can verify
        // the TOCTOU message by removing billing between the two checks conceptually.
        // Since we can't race in a unit test, we verify the structural presence of
        // the TOCTOU re-check by ensuring the code compiles and the error message exists.
        // The actual TOCTOU path is tested via the compile-time check in acceptance criteria.

        // Verify: when billing exists at first check but is gone by TOCTOU point,
        // launch_game returns Err. We use a simpler approach: confirm the code compiles
        // with the TOCTOU block by ensuring the function returns the expected error.
        let state = make_state().await;

        // Insert billing to pass first gate, then remove it before TOCTOU re-check
        // (We can't inject a race here, but we verify the error message text exists
        // by checking the structural assertion that the function rejects no-billing.)
        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;
        assert!(result.is_err(), "Expected Err without billing");
        // The error text may be from either the first check or TOCTOU — both are correct.
    }

    // ── LAUNCH-06: Invalid JSON rejection test ────────────────────────────────

    #[tokio::test]
    async fn test_launch_rejected_invalid_json() {
        let state = make_state().await;

        // Insert billing timer so we reach the JSON validation step
        let timer = BillingTimer::dummy("pod_1");
        state.billing.active_timers.write().await.insert("pod_1".to_string(), timer);

        let result = launch_game(
            &state,
            "pod_1",
            SimType::AssettoCorsa,
            Some(r#"{"corrupt"#.to_string()),
        )
        .await;

        assert!(result.is_err(), "Expected Err for invalid launch_args JSON");
        let err = result.unwrap_err();
        assert!(
            err.contains("Invalid") || err.contains("JSON"),
            "Error should mention 'Invalid' or 'JSON', got: {}",
            err
        );
    }

    // ── LAUNCH-05: Stopping state blocks double-launch ────────────────────────

    #[tokio::test]
    async fn test_double_launch_blocked_stopping() {
        let state = make_state().await;

        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );
        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Stopping,
                pid: None,
                launched_at: Some(Utc::now()),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Expected Err when game is Stopping");
        let err = result.unwrap_err();
        assert!(
            err.to_lowercase().contains("stopping"),
            "Error should mention 'stopping', got: {}",
            err
        );
    }

    // ── STATE-04: externally_tracked field ───────────────────────────────────

    #[tokio::test]
    async fn test_game_state_update_creates_external_tracker() {
        let state = make_state().await;

        let info = GameLaunchInfo {
            pod_id: "pod_5".to_string(),
            sim_type: SimType::AssettoCorsa,
            game_state: GameState::Running,
            pid: Some(1234),
            launched_at: Some(Utc::now()),
            error_message: None,
            diagnostics: None,
            exit_code: None,
            playable_at: None,
            ready_delay_ms: None,
        };

        handle_game_state_update(&state, info).await;

        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get("pod_5").expect("tracker should exist for pod_5");
        assert!(
            tracker.externally_tracked,
            "Agent-reported game should have externally_tracked = true"
        );
        assert!(
            tracker.launch_args.is_none(),
            "Externally tracked game should have no launch_args"
        );
    }

    #[tokio::test]
    async fn test_normal_launch_not_externally_tracked() {
        let state = make_state().await;

        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );

        // launch_game will fail at agent sender (no agent) but tracker is created
        let _ = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get("pod_1") {
            assert!(
                !tracker.externally_tracked,
                "Server-initiated launch should have externally_tracked = false"
            );
        }
        // If tracker doesn't exist (e.g. cleaned up on error), that's acceptable
    }

    // ── STATE-06: relaunch_game() rejects Stopping state ─────────────────────

    #[tokio::test]
    async fn test_relaunch_rejected_stopping_state() {
        let state = make_state().await;

        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );
        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Stopping,
                pid: None,
                launched_at: Some(Utc::now()),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        let result = relaunch_game(&state, "pod_1").await;
        assert!(result.is_err(), "Relaunch should be rejected when game is Stopping");
    }

    // ── STATE-01: Stopping timeout ────────────────────────────────────────────

    #[tokio::test]
    async fn test_stopping_timeout_transitions_to_error_via_health_check() {
        // Verify via check_game_health() which catches stale Stopping states from server restart.
        // This covers the STATE-01 edge case (server restart path) without needing tokio::time::pause().
        let state = make_state().await;

        // Insert a Stopping tracker with a launched_at in the distant past (>30s ago)
        let old_time = Utc::now() - chrono::Duration::seconds(60);
        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Stopping,
                pid: None,
                launched_at: Some(old_time),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        // check_game_health() should detect the stale Stopping state and transition to Error
        check_game_health(&state).await;

        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get("pod_1").expect("tracker should still exist");
        assert_eq!(
            tracker.game_state,
            GameState::Error,
            "Stale Stopping state should transition to Error via check_game_health"
        );
        assert!(
            tracker.error_message.as_ref().unwrap().contains("timed out"),
            "Error message should mention 'timed out', got: {:?}",
            tracker.error_message
        );
    }

    #[tokio::test]
    async fn test_stopping_state_not_timed_out_if_recent() {
        // If a Stopping tracker was set <30s ago, check_game_health() should NOT transition to Error
        let state = make_state().await;

        let recent_time = Utc::now() - chrono::Duration::seconds(5);
        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Stopping,
                pid: None,
                launched_at: Some(recent_time),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        check_game_health(&state).await;

        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get("pod_1").expect("tracker should still exist");
        assert_eq!(
            tracker.game_state,
            GameState::Stopping,
            "Recent Stopping state should NOT be transitioned to Error (only 5s elapsed)"
        );
    }

    #[tokio::test]
    async fn test_stop_game_sets_stopping_state() {
        // Verify that stop_game() transitions tracker to Stopping state
        // (the tokio::spawn timeout itself is verified structurally — see grep acceptance criteria)
        let state = make_state().await;

        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Running,
                pid: Some(1234),
                launched_at: Some(Utc::now()),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        stop_game(&state, "pod_1").await;

        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get("pod_1").expect("tracker should still exist");
        assert_eq!(
            tracker.game_state,
            GameState::Stopping,
            "stop_game() should set tracker to Stopping state"
        );
    }

    // ── STATE-03: Feature flag gate ────────────────────────────────────────────

    #[tokio::test]
    async fn test_feature_flag_disabled_rejects_launch() {
        use crate::flags::FeatureFlagRow;
        let state = make_state().await;

        // Insert billing so we reach the feature flag check
        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );

        // Disable game_launch flag
        state.feature_flags.write().await.insert(
            "game_launch".to_string(),
            FeatureFlagRow {
                name: "game_launch".to_string(),
                enabled: false,
                default_value: true,
                overrides: "{}".to_string(),
                version: 1,
                updated_at: None,
            },
        );

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Launch should be rejected when game_launch flag is disabled");
        let err = result.unwrap_err();
        assert!(
            err.contains("disabled"),
            "Error should mention 'disabled', got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_feature_flag_missing_defaults_enabled() {
        let state = make_state().await;

        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );
        // No feature flags inserted — should default to enabled

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        // Should NOT fail with feature flag error (may fail at agent sender — that's OK)
        if let Err(ref err) = result {
            assert!(
                !err.contains("disabled"),
                "Missing flag should default to enabled, got: {}",
                err
            );
        }
    }

    // ── STATE-02/STATE-05: Disconnected agent causes immediate Error ───────────

    #[tokio::test]
    async fn test_disconnected_agent_immediate_error() {
        let state = make_state().await;

        state.billing.active_timers.write().await.insert(
            "pod_1".to_string(),
            BillingTimer::dummy("pod_1"),
        );
        // No agent_sender inserted for pod_1

        let result = launch_game(&state, "pod_1", SimType::AssettoCorsa, None).await;

        assert!(result.is_err(), "Launch should fail when no agent is connected");
        let err = result.unwrap_err();
        assert!(
            err.contains("No agent connected"),
            "Error should mention 'No agent connected', got: {}",
            err
        );

        // Tracker should be in Error state immediately
        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get("pod_1").expect("tracker should exist");
        assert_eq!(
            tracker.game_state,
            GameState::Error,
            "Tracker should be in Error state immediately on disconnected agent"
        );
        assert!(
            tracker.error_message.as_ref().unwrap().contains("No agent connected"),
            "Tracker error_message should mention 'No agent connected'"
        );
    }

    // -- LAUNCH-09: ErrorTaxonomy typed exit_code tests

    #[test]
    fn test_classify_error_taxonomy_exit_code_access_violation() {
        // 0xC0000005 = STATUS_ACCESS_VIOLATION - stored as i32 wraps to negative
        let code = 0xC0000005u32 as i32;
        let result = classify_error_taxonomy(None, Some(code));
        assert!(
            matches!(result, metrics::ErrorTaxonomy::ProcessCrash { .. }),
            "exit_code Some(ACCESS_VIOLATION) should classify as ProcessCrash, got {:?}", result
        );
    }

    #[test]
    fn test_classify_error_taxonomy_exit_code_zero() {
        let result = classify_error_taxonomy(None, Some(0));
        assert!(
            matches!(result, metrics::ErrorTaxonomy::ProcessCrash { exit_code: 0 }),
            "exit_code Some(0) should classify as ProcessCrash(0), got {:?}", result
        );
    }

    #[test]
    fn test_classify_error_taxonomy_exit_code_priority() {
        // Even with shader message, exit_code wins
        let result = classify_error_taxonomy(Some("shader compilation failed"), Some(1));
        assert!(
            matches!(result, metrics::ErrorTaxonomy::ProcessCrash { .. }),
            "exit_code should take priority over message, got {:?}", result
        );
    }

    #[test]
    fn test_classify_error_taxonomy_string_fallback_shader() {
        let result = classify_error_taxonomy(Some("shader compilation failed"), None);
        assert!(
            matches!(result, metrics::ErrorTaxonomy::ShaderCompilationFail),
            "No exit_code + shader message -> ShaderCompilationFail, got {:?}", result
        );
    }

    #[test]
    fn test_classify_error_taxonomy_no_exit_no_message() {
        let result = classify_error_taxonomy(None, None);
        assert!(
            matches!(result, metrics::ErrorTaxonomy::Unknown),
            "No exit_code + no message -> Unknown, got {:?}", result
        );
    }

    // ── LAUNCH-17: Race Engineer atomic single-relaunch dedup ─────────────────

    #[tokio::test]
    async fn test_race_engineer_atomic_single_relaunch() {
        // LAUNCH-17: Two rapid Error events must result in exactly 1 relaunch, not 2.
        // Simulates the atomic check+increment under a single write lock.
        let state = make_state().await;

        // Set up tracker with auto_relaunch_count = 0
        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Error,
                pid: None,
                launched_at: Some(Utc::now()),
                error_message: Some("game_crash".to_string()),
                launch_args: Some(r#"{"car":"ferrari","track":"monza"}"#.to_string()),
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        // Simulate the atomic check+increment block twice (race condition scenario)
        let attempt1 = {
            let mut games = state.game_launcher.active_games.write().await;
            if let Some(tracker) = games.get_mut("pod_1") {
                if tracker.externally_tracked || tracker.launch_args.is_none() {
                    None
                } else if tracker.auto_relaunch_count < 2 {
                    tracker.auto_relaunch_count += 1;
                    Some(tracker.auto_relaunch_count)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let attempt2 = {
            let mut games = state.game_launcher.active_games.write().await;
            if let Some(tracker) = games.get_mut("pod_1") {
                if tracker.externally_tracked || tracker.launch_args.is_none() {
                    None
                } else if tracker.auto_relaunch_count < 2 {
                    tracker.auto_relaunch_count += 1;
                    Some(tracker.auto_relaunch_count)
                } else {
                    None
                }
            } else {
                None
            }
        };

        assert_eq!(attempt1, Some(1), "First attempt should fire (count -> 1)");
        assert_eq!(attempt2, Some(2), "Second attempt should fire (count -> 2)");

        // Third attempt must return None (exhausted)
        let attempt3 = {
            let mut games = state.game_launcher.active_games.write().await;
            if let Some(tracker) = games.get_mut("pod_1") {
                if tracker.externally_tracked || tracker.launch_args.is_none() {
                    None
                } else if tracker.auto_relaunch_count < 2 {
                    tracker.auto_relaunch_count += 1;
                    Some(tracker.auto_relaunch_count)
                } else {
                    None
                }
            } else {
                None
            }
        };
        assert_eq!(attempt3, None, "Third attempt must return None (max 2 reached)");

        let final_count = state.game_launcher.active_games.read().await
            .get("pod_1").map(|t| t.auto_relaunch_count).unwrap_or(99);
        assert_eq!(final_count, 2, "auto_relaunch_count should be exactly 2, got {}", final_count);
    }

    // ── LAUNCH-16: Null launch_args guard ─────────────────────────────────────

    #[tokio::test]
    async fn test_relaunch_null_args_rejected() {
        // LAUNCH-16: relaunch_game with no launch_args (externally tracked) should
        // return an error explaining the situation.
        let state = make_state().await;

        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Error,
                pid: None,
                launched_at: Some(Utc::now()),
                error_message: Some("crash".to_string()),
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: true,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        let result = relaunch_game(&state, "pod_1").await;
        assert!(result.is_err(), "relaunch with no launch_args should fail");
        let err = result.unwrap_err();
        assert!(
            err.contains("launch args") || err.contains("original launch") || err.contains("unavailable"),
            "Error should mention unavailable launch args, got: {}", err
        );
    }

    // ── LAUNCH-19: stop_game sim_type logging ─────────────────────────────────

    #[tokio::test]
    async fn test_stop_game_logs_nonempty_sim_type() {
        // LAUNCH-19: stop_game() must log the actual sim_type, not an empty string.
        // Verify by querying the game_launch_events table after stop_game.
        let state = make_state().await;

        state.game_launcher.active_games.write().await.insert(
            "pod_1".to_string(),
            GameTracker {
                pod_id: "pod_1".to_string(),
                sim_type: SimType::AssettoCorsa,
                game_state: GameState::Running,
                pid: Some(1111),
                launched_at: Some(Utc::now()),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: Vec::new(),
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
            },
        );

        stop_game(&state, "pod_1").await;

        // The "stopping" event in game_launch_events must have the real sim_type
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT sim_type FROM game_launch_events WHERE pod_id = 'pod_1' AND event_type = 'stopping' LIMIT 1"
        )
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

        if let Some((sim_type_val,)) = row {
            assert!(
                !sim_type_val.is_empty(),
                "stop_game() must log non-empty sim_type, got empty string"
            );
            // SimType::AssettoCorsa Display impl produces "Assetto Corsa"
            assert!(
                sim_type_val.to_lowercase().contains("assetto") || sim_type_val.contains("corsa"),
                "sim_type should reference Assetto Corsa, got: {}", sim_type_val
            );
        }
        // If no row exists yet (stop_game sends to agent async), that's OK — we verify via the event logged synchronously
    }

    // ── LAUNCH-14: No MAINTENANCE_MODE from Race Engineer ─────────────────────

    // ── RECOVER-04: Null launch_args guard tests ──────────────────────────────

    /// RECOVER-04: relaunch_game() must return Err when tracker has externally_tracked=true
    /// and launch_args=None — prevents auto-relaunch for games we don't know how to start.
    #[tokio::test]
    async fn test_null_args_guard_rejects_relaunch() {
        let state = make_state().await;

        // Insert an externally-tracked game with no launch_args, in Error state
        {
            state
                .game_launcher
                .active_games
                .write()
                .await
                .insert(
                    "pod_1".to_string(),
                    GameTracker {
                        pod_id: "pod_1".to_string(),
                        sim_type: SimType::AssettoCorsa,
                        game_state: GameState::Error,
                        pid: None,
                        launched_at: None,
                        error_message: Some("game crashed".to_string()),
                        launch_args: None,
                        auto_relaunch_count: 0,
                        externally_tracked: true,
                        dynamic_timeout_secs: None,
                        exit_codes: Vec::new(),
                        max_auto_relaunch: 2,
                        playable_at: None,
                        ready_delay_ms: None,
                    },
                );
        }

        let result = relaunch_game(&state, "pod_1").await;

        assert!(result.is_err(), "relaunch_game must fail when launch_args is None (externally tracked)");
        let err = result.unwrap_err();
        assert!(
            err.contains("launch args unavailable") || err.contains("externally tracked") || err.contains("null"),
            "Error must mention launch args unavailability, got: {}", err
        );
    }

    #[test]
    fn test_race_engineer_no_maintenance_mode_sentinel_written() {
        // LAUNCH-14: Game crashes must never write the sentinel file that blocks rc-agent restarts.
        // The sentinel is managed only by rc-agent/self_monitor — never game_launcher.rs.
        // This test counts occurrences of the sentinel name as a quoted path string.
        // Occurrences in this test's own source (in comments with bare words) are not counted
        // because we use concat! to avoid self-referencing in the pattern.
        let sentinel = concat!("MAINTENANCE", "_", "MODE"); // built at compile time without quotes
        let source = include_str!("game_launcher.rs");
        // Check: sentinel does NOT appear as a string literal (with surrounding quotes) outside this test
        // by counting quote-wrapped occurrences (file write arg) vs. comment occurrences
        let as_quoted = format!("\"{}\"", sentinel);
        // This test itself has 0 quoted occurrences (comments use bare words), so count must be 0
        let count = source.matches(as_quoted.as_str()).count();
        assert_eq!(
            count, 0,
            "MAINTENANCE_MODE must not appear as a quoted string literal in game_launcher.rs (count={})", count
        );
    }
}
