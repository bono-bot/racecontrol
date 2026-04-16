//! Dashboard WebSocket handler — staff dashboard real-time updates.
//!
//! Extracted from ws/mod.rs (Phase 385, v49.0 Architecture Completion).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};

use crate::{ac_camera, ac_server, auth, billing, game_launcher};
use crate::state::AppState;
use rc_common::protocol::{DashboardCommand, DashboardEvent};
use super::{DASHBOARD_CLIENT_COUNT, DASHBOARD_WS_CONNECTS, DASHBOARD_WS_DISCONNECTS};

pub async fn handle_dashboard(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // MI Phase 1: Track connected dashboard clients for DASHBOARD_ORPHAN detection
    let prev = DASHBOARD_CLIENT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    DASHBOARD_WS_CONNECTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Dashboard client connected (total: {})", prev + 1);

    // Send current pod list on connect (only physical pods 1-8, exclude POS/utility agents)
    let pods = state.pods.read().await;
    let pod_list: Vec<_> = pods.values().filter(|p| p.number >= 1 && p.number <= 8).cloned().collect();
    drop(pods);

    let init_msg = DashboardEvent::PodList(pod_list);
    if let Ok(json) = serde_json::to_string(&init_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active billing sessions on connect
    let rate_tiers = state.billing.rate_tiers.read().await;
    let timers = state.billing.active_timers.read().await;
    let billing_list: Vec<_> = timers.values().map(|t| t.to_info(&rate_tiers)).collect();
    drop(timers);
    drop(rate_tiers);

    let billing_msg = DashboardEvent::BillingSessionList(billing_list);
    if let Ok(json) = serde_json::to_string(&billing_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active game sessions on connect
    let games = state.game_launcher.active_games.read().await;
    let game_list: Vec<_> = games.values().map(|g| g.to_info()).collect();
    drop(games);

    let game_msg = DashboardEvent::GameSessionList(game_list);
    if let Ok(json) = serde_json::to_string(&game_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active AC server sessions on connect
    {
        let instances = state.ac_server.instances.read().await;
        for inst in instances.values() {
            if matches!(inst.status, rc_common::types::AcServerStatus::Running | rc_common::types::AcServerStatus::Starting) {
                let msg = DashboardEvent::AcServerUpdate(inst.to_info());
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        }
    }

    // Send AC preset list on connect
    if let Ok(presets) = ac_server::list_presets(&state).await {
        let msg = DashboardEvent::AcPresetList(presets);
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    // Send recent activity log on connect (last 100 entries)
    {
        let rows: Vec<(String, String, i64, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, pod_id, pod_number, timestamp, category, action, details, source
             FROM pod_activity_log ORDER BY timestamp DESC LIMIT 100"
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let entries: Vec<rc_common::types::PodActivityEntry> = rows.into_iter().map(|r| {
            rc_common::types::PodActivityEntry {
                id: r.0, pod_id: r.1, pod_number: r.2 as u32, timestamp: r.3,
                category: r.4, action: r.5, details: r.6, source: r.7,
            }
        }).collect();

        let msg = DashboardEvent::PodActivityList(entries);
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    // Subscribe to broadcast events
    let mut rx = state.dashboard_tx.subscribe();

    // Forward broadcast events to this dashboard client (filter non-physical pods)
    // WS-HARDEN: ping every 20s, timeout after 45s no pong, slow client drop after 5s send
    let send_task = tokio::spawn(async move {
        // Phase 254: Debounce RecordBroken broadcasts — max 1 per second per (track, sim_type)
        let mut record_debounce: HashMap<(String, String), Instant> = HashMap::new();
        let last_pong = Instant::now();
        let mut ping_interval = tokio::time::interval(Duration::from_secs(20));
        ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ping_interval.tick().await; // consume first immediate tick

        loop {
        tokio::select! {
        _ = ping_interval.tick() => {
            // WS-HARDEN: Check pong timeout (45s)
            if last_pong.elapsed() > Duration::from_secs(45) {
                tracing::warn!("Dashboard WS client pong timeout (45s) — dropping");
                break;
            }
            if sender.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
        event = rx.recv() => {
        let event = match event {
            Ok(e) => e,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                // RESIL-05: Escalate lagged consumers from debug to warn.
                // At >500 messages lag (~5 seconds at 100 msg/sec), disconnect to prevent cascade.
                // Threshold must be high enough to survive brief WiFi hiccups (1-2s = 100-200 msgs)
                // without triggering reconnect storms, but low enough to drop truly dead clients.
                if n > 500 {
                    tracing::warn!("Dashboard WS broadcast lagged by {n} messages — disconnecting slow consumer");
                    break;
                }
                tracing::warn!("Dashboard WS broadcast lagged by {n} messages — catching up");
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        };
            // Phase 254: Debounce RecordBroken events per (track, sim_type) — max 1/sec
            if let DashboardEvent::RecordBroken { ref track, ref sim_type, .. } = event {
                let key = (track.clone(), sim_type.clone());
                let now = Instant::now();
                if let Some(last) = record_debounce.get(&key)
                    && now.duration_since(*last).as_secs() < 1 {
                        continue;
                    }
                record_debounce.insert(key, now);
            }

            // Skip PodUpdate for non-physical pods (e.g. POS PC registering as pod 9)
            let filtered = match &event {
                DashboardEvent::PodUpdate(pod) if pod.number < 1 || pod.number > 8 => continue,
                DashboardEvent::PodList(pods) => {
                    let physical: Vec<_> = pods.iter().filter(|p| p.number >= 1 && p.number <= 8).cloned().collect();
                    DashboardEvent::PodList(physical)
                }
                _ => event,
            };
            // WS-HARDEN: timeout on slow client (5s send deadline)
            if let Ok(json) = serde_json::to_string(&filtered) {
                match tokio::time::timeout(
                    Duration::from_secs(5),
                    sender.send(Message::Text(json.into()))
                ).await {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) => break, // send error — socket closed
                    Err(_) => {
                        tracing::warn!("Dashboard WS slow client — send timed out after 5s, dropping");
                        break;
                    }
                }
            }
        } // end select event branch
        } // end select!
        } // end loop
    });

    // Handle incoming commands from dashboard
    let cmd_state = state.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            // WS-HARDEN: Pong received — reset pong timeout in send_task
            // Note: browser auto-responds to server Ping with Pong, so this fires naturally
            Message::Pong(_) => continue,
            Message::Text(text) => {
                // WS-HARDEN: message size limit (1MB) to prevent DoS
                if text.len() > 1_048_576 {
                    tracing::warn!("Dashboard WS message too large ({} bytes) — dropping", text.len());
                    continue;
                }
                match serde_json::from_str::<DashboardCommand>(&text) {
                    Ok(cmd) => match &cmd {
                        DashboardCommand::LaunchGame { .. }
                        | DashboardCommand::StopGame { .. } => {
                            let _ = game_launcher::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::StartAcSession { .. }
                        | DashboardCommand::StopAcSession { .. }
                        | DashboardCommand::SaveAcPreset { .. }
                        | DashboardCommand::DeleteAcPreset { .. }
                        | DashboardCommand::LoadAcPreset { .. } => {
                            ac_server::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::AssignCustomer { .. }
                        | DashboardCommand::CancelAssignment { .. } => {
                            auth::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::SetCameraMode { mode, enabled } => {
                            if let Some(en) = enabled {
                                ac_camera::set_enabled(&cmd_state, *en).await;
                            }
                            if !mode.is_empty() {
                                let cam_mode = match mode.as_str() {
                                    "closest_cycle" => ac_camera::CameraMode::ClosestCycle,
                                    "leader" => ac_camera::CameraMode::Leader,
                                    "closest" => ac_camera::CameraMode::Closest,
                                    "cycle" => ac_camera::CameraMode::Cycle,
                                    "off" => ac_camera::CameraMode::Off,
                                    _ => ac_camera::CameraMode::ClosestCycle,
                                };
                                ac_camera::set_mode(&cmd_state, cam_mode).await;
                            }
                        }
                        DashboardCommand::DeployPod { pod_id, binary_url } => {
                            // Look up pod IP
                            let pod_ip = {
                                let pods = cmd_state.pods.read().await;
                                pods.get(pod_id).map(|p| p.ip_address.clone())
                            };
                            if let Some(pod_ip) = pod_ip {
                                // Check no active deploy in progress
                                let is_active = {
                                    let ds = cmd_state.pod_deploy_states.read().await;
                                    ds.get(pod_id).map(|s| s.is_active()).unwrap_or(false)
                                };
                                if !is_active {
                                    let deploy_state = Arc::clone(&cmd_state);
                                    let deploy_pod_id = pod_id.clone();
                                    let deploy_pod_ip = pod_ip;
                                    let deploy_url = binary_url.clone();
                                    tokio::spawn(async move {
                                        crate::deploy::deploy_pod(
                                            deploy_state,
                                            deploy_pod_id,
                                            deploy_pod_ip,
                                            deploy_url,
                                        )
                                        .await;
                                    });
                                } else {
                                    tracing::warn!(
                                        "DeployPod [{}]: deploy already in progress — ignoring",
                                        pod_id
                                    );
                                }
                            } else {
                                tracing::warn!("DeployPod: unknown pod_id {}", pod_id);
                            }
                        }
                        DashboardCommand::DeployRolling { binary_url } => {
                            // Rolling deploy via kiosk WebSocket command.
                            // Delegates to deploy_rolling() which handles:
                            //   - Canary-first (pod_8), halt on canary failure
                            //   - WaitingSession for pods with active billing
                            //   - Session-end hook triggers deferred deploys
                            let deploy_state = Arc::clone(&cmd_state);
                            let deploy_url = binary_url.clone();
                            tokio::spawn(async move {
                                // Dashboard-initiated deploy: no force override (DEPLOY-03), actor="dashboard"
                                if let Err(e) = crate::deploy::deploy_rolling(deploy_state, deploy_url, false, "dashboard").await {
                                    tracing::error!("Rolling deploy via dashboard failed: {}", e);
                                }
                            });
                        }
                        DashboardCommand::CancelDeploy { pod_id } => {
                            // Mark the deploy state as Failed to signal cancellation.
                            // The running deploy_pod() task checks is_cancelled() at each step
                            // and exits early if it finds a Failed state.
                            let mut deploy_states = cmd_state.pod_deploy_states.write().await;
                            if let Some(ds) = deploy_states.get(pod_id)
                                && ds.is_active() {
                                    let cancel_state = rc_common::types::DeployState::Failed {
                                        reason: "Cancelled by staff".to_string(),
                                    };
                                    deploy_states
                                        .insert(pod_id.clone(), cancel_state.clone());
                                    let _ = cmd_state.dashboard_tx.send(
                                        rc_common::protocol::DashboardEvent::DeployProgress {
                                            pod_id: pod_id.clone(),
                                            state: cancel_state,
                                            message: "Deploy cancelled by staff".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    tracing::info!(
                                        "Deploy [{}]: cancelled by staff via dashboard",
                                        pod_id
                                    );
                                }
                        }
                        _ => {
                            billing::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                    },
                    Err(e) => {
                        tracing::debug!("Non-command dashboard message: {}", e);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
    // MI Phase 1: Decrement on disconnect
    let prev = DASHBOARD_CLIENT_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    DASHBOARD_WS_DISCONNECTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Dashboard client disconnected (remaining: {})", prev.saturating_sub(1));
}
