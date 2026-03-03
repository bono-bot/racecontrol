use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::state::AppState;
use rc_common::protocol::{CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{GameLaunchInfo, GameState, SimType};

/// In-memory tracker for a game running on a pod (mirrors BillingTimer pattern)
pub struct GameTracker {
    pub pod_id: String,
    pub sim_type: SimType,
    pub game_state: GameState,
    pub pid: Option<u32>,
    pub launched_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
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
pub async fn handle_dashboard_command(state: &Arc<AppState>, cmd: DashboardCommand) {
    match cmd {
        DashboardCommand::LaunchGame {
            pod_id,
            sim_type,
            launch_args,
        } => {
            launch_game(state, &pod_id, sim_type, launch_args).await;
        }
        DashboardCommand::StopGame { pod_id } => {
            stop_game(state, &pod_id).await;
        }
        _ => {}
    }
}

async fn launch_game(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: SimType,
    launch_args: Option<String>,
) {
    // Check if a game is currently launching (avoid double-launch race)
    {
        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get(pod_id) {
            if matches!(tracker.game_state, GameState::Launching) {
                tracing::warn!(
                    "Pod {} already launching a game, ignoring",
                    pod_id
                );
                return;
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
    };

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
        }
    }

    // Broadcast to dashboards
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info));

    // Log event to DB
    log_game_event(state, pod_id, &sim_type.to_string(), "launched", None, None).await;
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
        .send(DashboardEvent::GameStateChanged(info));
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

        let info = GameLaunchInfo {
            pod_id: pod_id.clone(),
            sim_type,
            game_state: GameState::Error,
            pid: None,
            launched_at: None,
            error_message: Some("Launch timed out (60s)".to_string()),
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
