//! Phase 334: Race Weekend Session Progression
//!
//! Orchestrates multi-session "weekends" (Practice → Qualifying → Race) on top of
//! the existing AC dedicated server infrastructure. The actual acServer process is
//! managed by `ac_server.rs` — this module adds the orchestration layer that:
//!
//! 1. Creates a weekend with configurable session durations
//! 2. Tracks which session phase is currently active
//! 3. Polls the acServer HTTP API to detect session transitions
//! 4. Broadcasts phase changes to dashboards via WebSocket

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::ac_server;
use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{AcLanSessionConfig, AcSessionBlock, SessionType};

// ─── Types ───────────────────────────────────────────────────────────────────

/// Which session phase a weekend is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeekendPhase {
    Practice,
    Qualifying,
    Race,
    Finished,
}

impl std::fmt::Display for WeekendPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Practice => write!(f, "practice"),
            Self::Qualifying => write!(f, "qualifying"),
            Self::Race => write!(f, "race"),
            Self::Finished => write!(f, "finished"),
        }
    }
}

/// Full state of a race weekend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekendState {
    pub weekend_id: String,
    pub ac_session_id: String,
    pub phase: WeekendPhase,
    pub track: String,
    pub car_class: String,
    pub pod_ids: Vec<String>,
    pub connected_pods: Vec<String>,
    pub practice_minutes: u32,
    pub quali_minutes: u32,
    pub race_laps: u32,
    pub created_at: DateTime<Utc>,
    pub phase_changed_at: DateTime<Utc>,
}

/// In-memory store for active weekends. Keyed by weekend_id.
pub struct WeekendManager {
    pub weekends: RwLock<HashMap<String, WeekendState>>,
}

impl Default for WeekendManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WeekendManager {
    pub fn new() -> Self {
        Self {
            weekends: RwLock::new(HashMap::new()),
        }
    }
}

#[path = "weekend_types.rs"]
mod weekend_types;
pub use weekend_types::{CreateWeekendRequest, WeekendSummary, WeekendStatus};

// ─── Core Logic ──────────────────────────────────────────────────────────────

/// Create a race weekend: builds an AcLanSessionConfig with Practice+Quali+Race
/// session blocks and starts the AC server.
pub async fn create_weekend(
    state: &Arc<AppState>,
    req: CreateWeekendRequest,
) -> Result<WeekendSummary, String> {
    if req.pod_ids.is_empty() {
        return Err("pod_ids must not be empty".to_string());
    }
    if req.track.is_empty() {
        return Err("track is required".to_string());
    }
    if req.car_class.is_empty() {
        return Err("car_class is required".to_string());
    }

    let weekend_id = uuid::Uuid::new_v4().to_string();

    // Build session blocks: Practice → Qualifying → Race
    let sessions = vec![
        AcSessionBlock {
            name: "Practice".to_string(),
            session_type: SessionType::Practice,
            duration_minutes: req.practice_minutes,
            laps: 0,
            wait_time_secs: 30,
        },
        AcSessionBlock {
            name: "Qualifying".to_string(),
            session_type: SessionType::Qualifying,
            duration_minutes: req.quali_minutes,
            laps: 0,
            wait_time_secs: 60,
        },
        AcSessionBlock {
            name: "Race".to_string(),
            session_type: SessionType::Race,
            duration_minutes: 0,
            laps: req.race_laps,
            wait_time_secs: 60,
        },
    ];

    // Build the full LAN session config
    let config = AcLanSessionConfig {
        name: format!("Weekend: {} - {}", req.track, req.car_class),
        track: req.track.clone(),
        track_config: String::new(),
        cars: vec![req.car_class.clone()],
        max_clients: req.pod_ids.len() as u32,
        password: String::new(),
        sessions,
        pickup_mode: false, // Weekend mode: no pickup, structured sessions
        ..AcLanSessionConfig::default()
    };

    // Start the AC server via existing infrastructure
    let ac_session_id = ac_server::start_ac_server(state, config, req.pod_ids.clone(), None)
        .await
        .map_err(|e| format!("Failed to start AC server: {}", e))?;

    let now = Utc::now();
    let weekend_state = WeekendState {
        weekend_id: weekend_id.clone(),
        ac_session_id: ac_session_id.clone(),
        phase: WeekendPhase::Practice,
        track: req.track.clone(),
        car_class: req.car_class.clone(),
        pod_ids: req.pod_ids.clone(),
        connected_pods: Vec::new(),
        practice_minutes: req.practice_minutes,
        quali_minutes: req.quali_minutes,
        race_laps: req.race_laps,
        created_at: now,
        phase_changed_at: now,
    };

    // Store in manager
    {
        let mut weekends = state.weekend.weekends.write().await;
        weekends.insert(weekend_id.clone(), weekend_state);
    }

    // Broadcast initial weekend event to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::WeekendUpdate {
        weekend_id: weekend_id.clone(),
        phase: "practice".to_string(),
        track: req.track.clone(),
        car_class: req.car_class.clone(),
        connected_pods: 0,
        total_pods: req.pod_ids.len(),
    });

    // Spawn background poller for session transitions
    let state_clone = Arc::clone(state);
    let wid = weekend_id.clone();
    let sid = ac_session_id.clone();
    tokio::spawn(async move {
        poll_session_transitions(state_clone, wid, sid).await;
    });

    Ok(WeekendSummary {
        weekend_id,
        ac_session_id,
        phase: WeekendPhase::Practice,
        track: req.track,
        car_class: req.car_class,
        pod_ids: req.pod_ids,
        practice_minutes: req.practice_minutes,
        quali_minutes: req.quali_minutes,
        race_laps: req.race_laps,
    })
}

/// Get the status of a specific weekend.
pub async fn get_weekend_status(
    state: &Arc<AppState>,
    weekend_id: &str,
) -> Option<WeekendStatus> {
    let weekends = state.weekend.weekends.read().await;
    weekends.get(weekend_id).map(|w| {
        WeekendStatus {
            weekend_id: w.weekend_id.clone(),
            current_session: w.phase,
            track: w.track.clone(),
            car_class: w.car_class.clone(),
            connected_pods: w.connected_pods.clone(),
            total_pods: w.pod_ids.len(),
            practice_minutes: w.practice_minutes,
            quali_minutes: w.quali_minutes,
            race_laps: w.race_laps,
            phase_changed_at: w.phase_changed_at,
            created_at: w.created_at,
        }
    })
}

/// List all active (non-finished) weekends.
pub async fn list_active_weekends(state: &Arc<AppState>) -> Vec<WeekendStatus> {
    let weekends = state.weekend.weekends.read().await;
    weekends
        .values()
        .filter(|w| w.phase != WeekendPhase::Finished)
        .map(|w| WeekendStatus {
            weekend_id: w.weekend_id.clone(),
            current_session: w.phase,
            track: w.track.clone(),
            car_class: w.car_class.clone(),
            connected_pods: w.connected_pods.clone(),
            total_pods: w.pod_ids.len(),
            practice_minutes: w.practice_minutes,
            quali_minutes: w.quali_minutes,
            race_laps: w.race_laps,
            phase_changed_at: w.phase_changed_at,
            created_at: w.created_at,
        })
        .collect()
}

/// Stop a weekend early — kills the AC server and marks it finished.
pub async fn stop_weekend(
    state: &Arc<AppState>,
    weekend_id: &str,
) -> Result<(), String> {
    let ac_session_id = {
        let weekends = state.weekend.weekends.read().await;
        match weekends.get(weekend_id) {
            Some(w) => w.ac_session_id.clone(),
            None => return Err("Weekend not found".to_string()),
        }
    };

    // Stop the underlying AC server
    ac_server::stop_ac_server(state, &ac_session_id)
        .await
        .map_err(|e| format!("Failed to stop AC server: {}", e))?;

    // Mark weekend finished
    {
        let mut weekends = state.weekend.weekends.write().await;
        if let Some(w) = weekends.get_mut(weekend_id) {
            w.phase = WeekendPhase::Finished;
            w.phase_changed_at = Utc::now();
        }
    }

    // Broadcast finished state
    let _ = state.dashboard_tx.send(DashboardEvent::WeekendUpdate {
        weekend_id: weekend_id.to_string(),
        phase: "finished".to_string(),
        track: String::new(),
        car_class: String::new(),
        connected_pods: 0,
        total_pods: 0,
    });

    Ok(())
}

// ─── Session Transition Poller ───────────────────────────────────────────────

/// Background task that polls the acServer HTTP info endpoint to detect session
/// transitions. acServer exposes /INFO which returns JSON with the current session
/// index. When the index advances, we update the WeekendPhase accordingly.
///
/// The polling runs every 5 seconds and exits when the weekend is finished or
/// the AC server stops.
async fn poll_session_transitions(
    state: Arc<AppState>,
    weekend_id: String,
    ac_session_id: String,
) {
    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Look up the HTTP port for this AC server instance
    let http_port = {
        let instances = state.ac_server.instances.read().await;
        match instances.get(&ac_session_id) {
            Some(inst) => inst.config.http_port,
            None => {
                tracing::warn!(
                    weekend_id = %weekend_id,
                    "Weekend poller: AC session not found, exiting"
                );
                return;
            }
        }
    };

    let mut last_session_index: Option<u32> = None;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // Check if weekend still exists and is not finished
        {
            let weekends = state.weekend.weekends.read().await;
            match weekends.get(&weekend_id) {
                Some(w) if w.phase == WeekendPhase::Finished => {
                    tracing::info!(weekend_id = %weekend_id, "Weekend finished, poller exiting");
                    return;
                }
                None => {
                    tracing::info!(weekend_id = %weekend_id, "Weekend removed, poller exiting");
                    return;
                }
                _ => {}
            }
        }

        // Check if AC server is still running
        {
            let instances = state.ac_server.instances.read().await;
            match instances.get(&ac_session_id) {
                Some(inst) if matches!(
                    inst.status,
                    rc_common::types::AcServerStatus::Stopped | rc_common::types::AcServerStatus::Error
                ) => {
                    tracing::info!(
                        weekend_id = %weekend_id,
                        "AC server stopped/errored, marking weekend finished"
                    );
                    mark_weekend_finished(&state, &weekend_id).await;
                    return;
                }
                None => {
                    tracing::info!(
                        weekend_id = %weekend_id,
                        "AC session removed, marking weekend finished"
                    );
                    mark_weekend_finished(&state, &weekend_id).await;
                    return;
                }
                _ => {}
            }
        }

        // Poll acServer HTTP API for current session info
        // acServer exposes GET http://localhost:{http_port}/INFO
        let url = format!("http://127.0.0.1:{}/INFO", http_port);
        match state.http_client.get(&url).timeout(std::time::Duration::from_secs(3)).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    // acServer /INFO returns { "session": N, ... } where N is 0-indexed session
                    // 0 = first session (Practice), 1 = Qualifying, 2 = Race
                    if let Some(session_idx) = body.get("session").and_then(|v| v.as_u64()).map(|v| v as u32)
                        && last_session_index != Some(session_idx) {
                            last_session_index = Some(session_idx);
                            let new_phase = match session_idx {
                                0 => WeekendPhase::Practice,
                                1 => WeekendPhase::Qualifying,
                                2 => WeekendPhase::Race,
                                _ => WeekendPhase::Finished,
                            };
                            update_weekend_phase(&state, &weekend_id, new_phase).await;
                        }

                    // Update connected pods from acServer /INFO
                    // The "clients" field tells us how many are connected
                    if let Some(clients) = body.get("clients").and_then(|v| v.as_u64()) {
                        let mut weekends = state.weekend.weekends.write().await;
                        if let Some(w) = weekends.get_mut(&weekend_id) {
                            // We don't have per-pod names from /INFO, but we know the count.
                            // The connected_pods list is updated from the AC server instance.
                            let instances = state.ac_server.instances.read().await;
                            if let Some(inst) = instances.get(&ac_session_id) {
                                w.connected_pods = inst.connected_pods.clone();
                            }
                            drop(instances);

                            let _ = state.dashboard_tx.send(DashboardEvent::WeekendUpdate {
                                weekend_id: weekend_id.clone(),
                                phase: w.phase.to_string(),
                                track: w.track.clone(),
                                car_class: w.car_class.clone(),
                                connected_pods: clients as usize,
                                total_pods: w.pod_ids.len(),
                            });
                        }
                    }
                }
            }
            Ok(resp) => {
                tracing::debug!(
                    weekend_id = %weekend_id,
                    status = %resp.status(),
                    "acServer /INFO returned non-200"
                );
            }
            Err(e) => {
                tracing::debug!(
                    weekend_id = %weekend_id,
                    error = %e,
                    "Failed to poll acServer /INFO"
                );
            }
        }
    }
}

/// Update the phase of a weekend and broadcast to dashboards.
async fn update_weekend_phase(state: &Arc<AppState>, weekend_id: &str, new_phase: WeekendPhase) {
    let broadcast_data = {
        let mut weekends = state.weekend.weekends.write().await;
        if let Some(w) = weekends.get_mut(weekend_id) {
            if w.phase == new_phase {
                return; // No change
            }
            tracing::info!(
                weekend_id = %weekend_id,
                old_phase = %w.phase,
                new_phase = %new_phase,
                "Weekend session transition"
            );
            w.phase = new_phase;
            w.phase_changed_at = Utc::now();
            Some((
                w.track.clone(),
                w.car_class.clone(),
                w.connected_pods.len(),
                w.pod_ids.len(),
            ))
        } else {
            None
        }
    };

    if let Some((track, car_class, connected, total)) = broadcast_data {
        let _ = state.dashboard_tx.send(DashboardEvent::WeekendUpdate {
            weekend_id: weekend_id.to_string(),
            phase: new_phase.to_string(),
            track,
            car_class,
            connected_pods: connected,
            total_pods: total,
        });
    }
}

/// Mark a weekend as finished.
async fn mark_weekend_finished(state: &Arc<AppState>, weekend_id: &str) {
    update_weekend_phase(state, weekend_id, WeekendPhase::Finished).await;
}

#[cfg(test)]
#[path = "weekend_tests.rs"]
mod tests;
