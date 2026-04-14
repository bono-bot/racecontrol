//! AC Server monitoring — lobby sync, continuous session, and health checks.
//!
//! Extracted from `ac_server.rs` for module size compliance (<500 lines).

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::*;

use crate::ac_server::{start_ac_server, stop_ac_server};
use crate::ac_server_lifecycle::{is_process_alive, set_continuous_mode};
use crate::state::AppState;

// ─── Lobby Sync Monitor (v44.0 Phase 333) ──────────────────────────────────

/// Poll the acServer HTTP API to track connected clients and update lobby status.
/// Runs as a background tokio task spawned after `start_ac_server()`.
///
/// Behavior:
/// - Polls `GET http://127.0.0.1:{http_port}/INFO` every 3 seconds
/// - Tracks connected client count from the `clients` field in the JSON response
/// - Updates `connected_pods` on the AcServerInstance
/// - Broadcasts `DashboardEvent::LobbyUpdate` with current lobby phase
/// - When all expected pods connect (or 120s timeout), transitions to Active phase
/// - If acServer HTTP is unreachable, keeps retrying (server may be starting)
pub async fn monitor_lobby_sync(
    state: Arc<AppState>,
    session_id: String,
    http_port: u16,
    expected_pod_count: usize,
) {
    use std::time::{Duration, Instant};

    let timeout = Duration::from_secs(120);
    let poll_interval = Duration::from_secs(3);
    let start = Instant::now();
    let mut last_client_count: i64 = -1;

    tracing::info!(
        session_id = %session_id,
        http_port = http_port,
        expected_pods = expected_pod_count,
        "Lobby sync monitor started — polling acServer HTTP API"
    );

    // Broadcast initial Forming status
    let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
        group_session_id: session_id.clone(),
        phase: LobbyPhase::Forming,
        total_pods: expected_pod_count as u32,
        ready_pods: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
    }));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    loop {
        // Check if session was stopped externally
        {
            let instances = state.ac_server.instances.read().await;
            match instances.get(&session_id) {
                Some(inst) if matches!(inst.status, AcServerStatus::Stopped | AcServerStatus::Error) => {
                    tracing::info!(session_id = %session_id, "Lobby monitor: session stopped/error, exiting");
                    let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
                        group_session_id: session_id.clone(),
                        phase: LobbyPhase::Cancelled,
                        total_pods: expected_pod_count as u32,
                        ready_pods: vec![],
                        created_at: chrono::Utc::now().to_rfc3339(),
                    }));
                    return;
                }
                None => {
                    tracing::info!(session_id = %session_id, "Lobby monitor: session removed, exiting");
                    return;
                }
                _ => {}
            }
        }

        // Check timeout — proceed with whatever pods are connected
        if start.elapsed() > timeout {
            tracing::warn!(
                session_id = %session_id,
                expected = expected_pod_count,
                "Lobby sync timeout (120s) — proceeding with available pods"
            );
            let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
                group_session_id: session_id.clone(),
                phase: LobbyPhase::Active,
                total_pods: expected_pod_count as u32,
                ready_pods: vec![],
                created_at: chrono::Utc::now().to_rfc3339(),
            }));
            return;
        }

        // Poll acServer HTTP API for connected client count
        let url = format!("http://127.0.0.1:{}/INFO", http_port);
        match client.get(&url).send().await {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(info) => {
                        let clients = info.get("clients")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;

                        if clients as i64 != last_client_count {
                            tracing::info!(
                                session_id = %session_id,
                                connected = clients,
                                expected = expected_pod_count,
                                elapsed_secs = start.elapsed().as_secs(),
                                "Lobby sync: client count changed"
                            );
                            last_client_count = clients as i64;
                        }

                        // Update connected_pods on the instance
                        {
                            let mut instances = state.ac_server.instances.write().await;
                            if let Some(inst) = instances.get_mut(&session_id) {
                                inst.connected_pods = (0..clients)
                                    .map(|i| format!("client_{}", i + 1))
                                    .collect();
                            }
                        }

                        // All pods connected — transition to Active
                        if clients >= expected_pod_count {
                            tracing::info!(
                                session_id = %session_id,
                                connected = clients,
                                expected = expected_pod_count,
                                elapsed_secs = start.elapsed().as_secs(),
                                "Lobby sync: all pods connected — race is ready"
                            );

                            let ready_pods: Vec<String> = (0..clients)
                                .map(|i| format!("client_{}", i + 1))
                                .collect();

                            let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
                                group_session_id: session_id.clone(),
                                phase: LobbyPhase::AllReady,
                                total_pods: expected_pod_count as u32,
                                ready_pods: ready_pods.clone(),
                                created_at: chrono::Utc::now().to_rfc3339(),
                            }));

                            tokio::time::sleep(Duration::from_secs(2)).await;

                            let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
                                group_session_id: session_id.clone(),
                                phase: LobbyPhase::Active,
                                total_pods: expected_pod_count as u32,
                                ready_pods,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            }));

                            // Broadcast server update with connected pods
                            let server_info = {
                                let instances = state.ac_server.instances.read().await;
                                instances.get(&session_id).map(|i| i.to_info())
                            };
                            if let Some(info) = server_info {
                                let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
                            }

                            return;
                        }

                        // Still forming — broadcast current count
                        let _ = state.dashboard_tx.send(DashboardEvent::LobbyUpdate(LobbyStatus {
                            group_session_id: session_id.clone(),
                            phase: LobbyPhase::Forming,
                            total_pods: expected_pod_count as u32,
                            ready_pods: (0..clients).map(|i| format!("client_{}", i + 1)).collect(),
                            created_at: chrono::Utc::now().to_rfc3339(),
                        }));
                    }
                    Err(e) => {
                        tracing::debug!(
                            session_id = %session_id,
                            error = %e,
                            "Lobby sync: failed to parse /INFO response (server may be starting)"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    session_id = %session_id,
                    error = %e,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Lobby sync: acServer HTTP not reachable yet (retrying)"
                );
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

/// GROUP-02: Monitor a continuous-mode AC server. When the process exits (race complete),
/// check if any pod still has active billing and restart if so.
/// Uses a mutable current_session_id to track restarts without recursive spawn.
pub async fn monitor_continuous_session(state: Arc<AppState>, initial_session_id: String) {
    let mut current_session_id = initial_session_id;

    'monitor: loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let (pid, pod_ids, group_session_id) = {
            let instances = state.ac_server.instances.read().await;
            let inst = match instances.get(&current_session_id) {
                Some(i) => i,
                None => {
                    tracing::debug!("Continuous monitor: session {} removed, exiting", current_session_id);
                    return;
                }
            };

            if !inst.continuous_mode {
                tracing::debug!("Continuous monitor: session {} no longer continuous, exiting", current_session_id);
                return;
            }

            if matches!(inst.status, AcServerStatus::Stopped | AcServerStatus::Error) {
                tracing::debug!("Continuous monitor: session {} stopped/error, exiting", current_session_id);
                return;
            }

            (
                inst.pid,
                inst.assigned_pods.clone(),
                inst.group_session_id.clone(),
            )
        };

        // Check if the process is still alive
        if let Some(pid_val) = pid {
            if is_process_alive(pid_val) {
                continue 'monitor; // Still running, check again later
            }
        } else {
            continue 'monitor; // No PID (dry-run mode), keep checking
        }

        // Process exited — check if any pod still has active billing
        let any_billing_active = {
            let timers = state.billing.active_timers.read().await;
            pod_ids.iter().any(|p| timers.contains_key(p))
        };

        if !any_billing_active {
            tracing::info!(
                "Continuous mode: AC session {} race ended, no active billing — stopping",
                current_session_id
            );
            let _ = stop_ac_server(&state, &current_session_id).await;
            return;
        }

        // Billing still active — restart the race
        tracing::info!(
            "GROUP-02: Continuous mode restart — AC session {} race ended, billing active, restarting within 15s",
            current_session_id
        );

        // Brief pause before restart (let results collect, clients disconnect)
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        // Re-read config in case track/car was changed mid-session (GROUP-04)
        let (current_config, current_pods) = {
            let instances = state.ac_server.instances.read().await;
            match instances.get(&current_session_id) {
                Some(inst) => {
                    if !inst.continuous_mode {
                        tracing::info!("Continuous mode disabled during restart delay for {}", current_session_id);
                        return;
                    }
                    (inst.config.clone(), inst.assigned_pods.clone())
                }
                None => return,
            }
        };

        // Stop the old session cleanly (removes from instances map)
        let _ = stop_ac_server(&state, &current_session_id).await;

        // Start a new session with the same (or updated) config
        match start_ac_server(&state, current_config, current_pods.clone(), None).await {
            Ok(new_session_id) => {
                // Enable continuous mode on the new session
                let _ = set_continuous_mode(&state, &new_session_id, true, group_session_id.clone()).await;

                // Update group_sessions.ac_session_id to point to new session
                if let Some(ref gsid) = group_session_id {
                    let _ = sqlx::query("UPDATE group_sessions SET ac_session_id = ? WHERE id = ?")
                        .bind(&new_session_id)
                        .bind(gsid)
                        .execute(&state.db)
                        .await;
                }

                tracing::info!(
                    "GROUP-02: Continuous restart complete — new session {} (was {})",
                    new_session_id, current_session_id
                );

                // Update the session_id for the next loop iteration
                current_session_id = new_session_id;
                // Continue the outer loop to monitor the new session
                continue 'monitor;
            }
            Err(e) => {
                tracing::error!(
                    "GROUP-02: Continuous restart failed for session {}: {}",
                    current_session_id, e
                );
                return;
            }
        }
    }
}

pub async fn check_ac_server_health(state: &Arc<AppState>) {
    let mut dead_sessions = Vec::new();

    {
        let mut instances = state.ac_server.instances.write().await;
        for (id, inst) in instances.iter_mut() {
            if !matches!(inst.status, AcServerStatus::Running | AcServerStatus::Starting) {
                continue;
            }

            // Check if acServer process is still alive
            if let Some(ref mut child) = inst.child {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // Process exited
                        inst.status = AcServerStatus::Error;
                        inst.error_message = Some("acServer process exited unexpectedly".to_string());
                        dead_sessions.push(id.clone());
                    }
                    Ok(None) => {} // Still running
                    Err(e) => {
                        inst.status = AcServerStatus::Error;
                        inst.error_message = Some(format!("Failed to check process: {}", e));
                        dead_sessions.push(id.clone());
                    }
                }
            }
        }
    }

    // Broadcast updates for dead sessions
    for id in dead_sessions {
        let info = {
            let instances = state.ac_server.instances.read().await;
            instances.get(&id).map(|i| i.to_info())
        };
        if let Some(info) = info {
            let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
        }
    }
}
