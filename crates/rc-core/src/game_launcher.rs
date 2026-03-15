use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::activity_log::log_pod_activity;
use crate::catalog;
use crate::state::AppState;
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

async fn launch_game(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: SimType,
    launch_args: Option<String>,
) -> Result<(), String> {
    // Validate launch combo against pod's content manifest
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

    // LIFE-02: Reject launch if no active billing session
    {
        let timers = state.billing.active_timers.read().await;
        if !timers.contains_key(pod_id) {
            tracing::warn!("Launch rejected for pod {}: no active billing session", pod_id);
            return Err(format!("Pod {} has no active billing session", pod_id));
        }
    }

    // LIFE-04: Check if a game is currently launching or running (avoid double-launch)
    {
        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get(pod_id) {
            if matches!(tracker.game_state, GameState::Launching | GameState::Running) {
                return Err(format!("Pod {} already has a game active", pod_id));
            }
        }
    }

    // Create tracker in Launching state
    let tracker = GameTracker {
        pod_id: pod_id.to_string(),
        sim_type,
        game_state: GameState::Launching,
        pid: None,
        launched_at: Some(Utc::now()),
        error_message: None,
        launch_args: launch_args.clone(),
        auto_relaunch_count: 0,
    };

    log_pod_activity(state, pod_id, "game", "Game Launching", &format!("{}", sim_type), "core");

    let info = tracker.to_info();

    state
        .game_launcher
        .active_games
        .write()
        .await
        .insert(pod_id.to_string(), tracker);

    // Send command to agent
    let senders = state.agent_senders.read().await;
    if let Some(tx) = senders.get(pod_id) {
        let cmd = CoreToAgentMessage::LaunchGame {
            sim_type,
            launch_args,
        };
        if let Err(e) = tx.send(cmd).await {
            tracing::error!("Failed to send LaunchGame to pod {}: {}", pod_id, e);
        }
    } else {
        tracing::warn!("No agent connected for pod {}", pod_id);
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
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::GameStateChanged(error_info));
        }
        return Err(format!("No agent connected for pod {}", pod_id));
    }

    // Broadcast to dashboards (only reached if agent IS connected)
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info));

    // Log event to DB
    log_game_event(state, pod_id, &sim_type.to_string(), "launched", None, None).await;
    Ok(())
}

/// CRASH-04: Relaunch a crashed game using stored launch_args from GameTracker.
/// Resets auto_relaunch_count so Race Engineer gets fresh attempts.
pub async fn relaunch_game(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), String> {
    // Get stored launch info from tracker
    let (sim_type, launch_args) = {
        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get(pod_id).ok_or("No game tracker for pod")?;
        if tracker.game_state != GameState::Error {
            return Err(format!("Pod game is {:?}, not Error — cannot relaunch", tracker.game_state));
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

    // Send LaunchGame to agent
    let senders = state.agent_senders.read().await;
    let tx = senders.get(pod_id).ok_or("Pod not connected")?;
    tx.send(CoreToAgentMessage::LaunchGame {
        sim_type,
        launch_args,
    })
    .await
    .map_err(|e| format!("Failed to send launch command: {}", e))?;

    // Log + broadcast
    log_game_event(state, pod_id, &sim_type.to_string(), "relaunched", None, Some("Manual relaunch from kiosk")).await;
    let info = {
        let games = state.game_launcher.active_games.read().await;
        games.get(pod_id).map(|t| t.to_info())
    };
    if let Some(info) = info {
        let _ = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info));
    }

    log_pod_activity(state, pod_id, "game", "Game Relaunched", "Manual relaunch from kiosk", "core");

    Ok(())
}

async fn stop_game(state: &Arc<AppState>, pod_id: &str) {
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
        log_pod_activity(state, pod_id, "game", "Game Stopping", "", "core");

        // Send command to agent
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(pod_id) {
            if let Err(e) = tx.send(CoreToAgentMessage::StopGame).await {
                tracing::error!("Failed to send StopGame to pod {}: {}", pod_id, e);
            }
        }

        // Broadcast to dashboards
        let _ = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info));

        // Log event
        log_game_event(state, pod_id, "", "stopping", None, None).await;
    }
}

/// Called when agent reports a game state update
pub async fn handle_game_state_update(state: &Arc<AppState>, info: GameLaunchInfo) {
    let pod_id = &info.pod_id;

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
        GameState::Error => "crashed",
        GameState::Idle => "stopped",
        GameState::Launching => "launched",
        GameState::Stopping => "stopping",
    };

    // Log to DB
    log_game_event(
        state,
        pod_id,
        &info.sim_type.to_string(),
        event_type,
        info.pid,
        info.error_message.as_deref(),
    )
    .await;

    // Broadcast to dashboards
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info.clone()));

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
    if info.game_state == GameState::Error {
        let has_billing = state
            .billing
            .active_timers
            .read()
            .await
            .contains_key(pod_id);

        if has_billing {
            let (relaunch_count, sim_type, launch_args) = {
                let games = state.game_launcher.active_games.read().await;
                if let Some(tracker) = games.get(pod_id) {
                    (tracker.auto_relaunch_count, tracker.sim_type, tracker.launch_args.clone())
                } else {
                    (999, info.sim_type, None) // no tracker = don't relaunch
                }
            };

            if relaunch_count < 2 {
                // Increment counter
                {
                    let mut games = state.game_launcher.active_games.write().await;
                    if let Some(tracker) = games.get_mut(pod_id) {
                        tracker.auto_relaunch_count += 1;
                    }
                }

                let attempt = relaunch_count + 1;
                let pod_id_owned = pod_id.to_string();
                let state_clone = state.clone();
                let sim_name = format!("{}", sim_type);

                log_pod_activity(
                    state,
                    pod_id,
                    "race_engineer",
                    "Auto-Relaunching Game",
                    &format!("Race Engineer relaunching {} after crash (attempt {}/2)", sim_name, attempt),
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
                        let senders = state_clone.agent_senders.read().await;
                        if let Some(tx) = senders.get(&pod_id_owned) {
                            let _ = tx
                                .send(CoreToAgentMessage::LaunchGame {
                                    sim_type,
                                    launch_args,
                                })
                                .await;
                        }
                    }
                });
            } else {
                log_pod_activity(
                    state,
                    pod_id,
                    "race_engineer",
                    "Relaunch Limit Reached",
                    &format!("Race Engineer: max relaunch attempts (2) reached for {}", info.sim_type),
                    "race_engineer",
                );
            }
        }
    }
}

/// Periodic health check: detect stale Launching states (timeout after 60s)
pub async fn check_game_health(state: &Arc<AppState>) {
    let now = Utc::now();
    let mut timed_out = Vec::new();

    {
        let games = state.game_launcher.active_games.read().await;
        for (pod_id, tracker) in games.iter() {
            if tracker.game_state == GameState::Launching {
                if let Some(launched_at) = tracker.launched_at {
                    let elapsed = now.signed_duration_since(launched_at);
                    if elapsed.num_seconds() > 60 {
                        timed_out.push((pod_id.clone(), tracker.sim_type));
                    }
                }
            }
        }
    }

    for (pod_id, sim_type) in timed_out {
        tracing::warn!("Game launch timed out on pod {}", pod_id);
        log_pod_activity(state, &pod_id, "game", "Launch Timeout", &format!("{} failed to start within 60s", sim_type), "core");

        let info = GameLaunchInfo {
            pod_id: pod_id.clone(),
            sim_type,
            game_state: GameState::Error,
            pid: None,
            launched_at: None,
            error_message: Some("Launch timed out (60s)".to_string()),
            diagnostics: None,
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
            tracker.error_message = Some("Launch timed out (60s)".to_string());
        }

        // Log and broadcast
        log_game_event(&state, &pod_id, &sim_type.to_string(), "timeout", None, None).await;
        let _ = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info));
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
    let _ = sqlx::query(
        "INSERT INTO game_launch_events (id, pod_id, sim_type, event_type, pid, error_message)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(pod_id)
    .bind(sim_type)
    .bind(event_type)
    .bind(pid.map(|p| p as i64))
    .bind(error_message)
    .execute(&state.db)
    .await;
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

        let config = Config::default_test();
        Arc::new(AppState::new(config, db))
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
}
