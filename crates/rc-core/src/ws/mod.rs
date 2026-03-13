use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::ac_camera;
use crate::ac_server;
use crate::activity_log::log_pod_activity;
use crate::auth;
use crate::billing;
use crate::game_launcher;
use crate::state::AppState;
use rc_common::protocol::{
    AgentMessage, AiChannelMessage, CoreToAgentMessage, DashboardCommand, DashboardEvent,
};
use rc_common::types::GameState;

/// WebSocket endpoint for pod agents
pub async fn agent_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_agent(socket, state))
}

/// WebSocket endpoint for dashboard clients
pub async fn dashboard_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_dashboard(socket, state))
}

async fn handle_agent(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Unique ID for this connection — used to avoid stale disconnect cleanup
    static CONN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let conn_id = CONN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    tracing::info!("Pod agent connected (conn_id={})", conn_id);

    // Create mpsc channel for sending commands back to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CoreToAgentMessage>(64);
    let mut registered_pod_id: Option<String> = None;

    // Shared state for pending application-level ping measurement
    // send_task writes (id, Instant) when it sends a Ping; receive loop reads+clears it on Pong
    static PING_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pending_ping: Arc<tokio::sync::Mutex<Option<(u64, Instant)>>> = Arc::new(tokio::sync::Mutex::new(None));
    let pending_ping_send = pending_ping.clone();

    // Spawn task to forward commands from mpsc to WebSocket sender.
    // Also sends WS-level keepalive ping every 15s (CONN-01) and
    // an app-level measurement Ping every 30s (PERF-03).
    let send_task = tokio::spawn(async move {
        let mut ping_interval = interval(Duration::from_secs(15));
        ping_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut measure_interval = interval(Duration::from_secs(30));
        measure_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        // Consume the immediate first tick so the first real tick fires after the full interval
        ping_interval.tick().await;
        measure_interval.tick().await;

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => {
                            if let Ok(json) = serde_json::to_string(&cmd) {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        None => break, // Channel closed — handle_agent is exiting
                    }
                }
                _ = ping_interval.tick() => {
                    // WS-level keepalive ping to prevent TCP idle timeout during CPU spikes
                    tracing::trace!("WS ping sent (conn_id={})", conn_id);
                    if ws_sender.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                _ = measure_interval.tick() => {
                    // Application-level ping for round-trip latency measurement
                    let ping_id = PING_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let msg = CoreToAgentMessage::Ping { id: ping_id };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        // Record send time before sending
                        *pending_ping_send.lock().await = Some((ping_id, Instant::now()));
                        if ws_sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Listen for messages from the agent
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<AgentMessage>(&text) {
                Ok(agent_msg) => {
                    match &agent_msg {
                        AgentMessage::Register(pod_info) => {
                            tracing::info!("Pod {} registered (conn_id={}): {}", pod_info.number, conn_id, pod_info.name);
                            registered_pod_id = Some(pod_info.id.clone());
                            log_pod_activity(&state, &pod_info.id, "system", "Pod Online", &format!("Pod {} connected (conn_id={})", pod_info.number, conn_id), "agent");

                            // Store agent sender and connection ID for this pod
                            state
                                .agent_senders
                                .write()
                                .await
                                .insert(pod_info.id.clone(), cmd_tx.clone());
                            state
                                .agent_conn_ids
                                .write()
                                .await
                                .insert(pod_info.id.clone(), conn_id);

                            state
                                .pods
                                .write()
                                .await
                                .insert(pod_info.id.clone(), pod_info.clone());

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::PodUpdate(pod_info.clone()));

                            // Reconcile game_launcher.active_games with pod's reported state
                            {
                                let mut games = state.game_launcher.active_games.write().await;
                                let pod_game_state = pod_info.game_state.unwrap_or(GameState::Idle);
                                match pod_game_state {
                                    GameState::Running | GameState::Launching => {
                                        // Pod reports a game is active — ensure tracker exists
                                        if let Some(tracker) = games.get_mut(&pod_info.id) {
                                            tracker.game_state = pod_game_state;
                                        } else if let Some(sim) = pod_info.current_game {
                                            games.insert(
                                                pod_info.id.clone(),
                                                game_launcher::GameTracker {
                                                    pod_id: pod_info.id.clone(),
                                                    sim_type: sim,
                                                    game_state: pod_game_state,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: None,
                                                    launch_args: None,
                                                    auto_relaunch_count: 0,
                                                },
                                            );
                                            tracing::info!("Reconciled game tracker for pod {} on reconnect ({:?})", pod_info.number, pod_game_state);
                                        }
                                    }
                                    GameState::Idle | GameState::Stopping | GameState::Error => {
                                        // Pod reports idle — remove any stale tracker
                                        if games.remove(&pod_info.id).is_some() {
                                            tracing::info!("Removed stale game tracker for pod {} on reconnect", pod_info.number);
                                        }
                                    }
                                }
                            }

                            // Resync active billing session to reconnected agent
                            {
                                let resync = {
                                    let timers = state.billing.active_timers.read().await;
                                    timers.get(&pod_info.id).map(|timer| (
                                        timer.session_id.clone(),
                                        timer.driver_name.clone(),
                                        timer.allocated_seconds,
                                        timer.remaining_seconds(),
                                    ))
                                };
                                if let Some((session_id, driver_name, allocated_seconds, remaining)) = resync {
                                    let _ = cmd_tx.send(CoreToAgentMessage::BillingStarted {
                                        billing_session_id: session_id.clone(),
                                        driver_name: driver_name.clone(),
                                        allocated_seconds,
                                    }).await;
                                    let _ = cmd_tx.send(CoreToAgentMessage::BillingTick {
                                        remaining_seconds: remaining,
                                        allocated_seconds,
                                        driver_name: driver_name.clone(),
                                        elapsed_seconds: None,
                                        cost_paise: None,
                                        rate_per_min_paise: None,
                                        paused: None,
                                        minutes_to_value_tier: None,
                                    }).await;
                                    // Restore pod state (agent Register overwrites with Idle)
                                    {
                                        let mut pods = state.pods.write().await;
                                        if let Some(pod) = pods.get_mut(&pod_info.id) {
                                            pod.billing_session_id = Some(session_id.clone());
                                            pod.current_driver = Some(driver_name.clone());
                                            pod.status = rc_common::types::PodStatus::InSession;
                                            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                                        }
                                    }
                                    tracing::info!(
                                        "Resynced billing session {} to pod {} ({}s remaining)",
                                        session_id, pod_info.number, remaining
                                    );
                                }
                            }

                            // Send current kiosk settings to newly connected agent
                            if let Ok(rows) = sqlx::query_as::<_, (String, String)>(
                                "SELECT key, value FROM kiosk_settings",
                            )
                            .fetch_all(&state.db)
                            .await
                            {
                                if !rows.is_empty() {
                                    let settings: std::collections::HashMap<String, String> =
                                        rows.into_iter().collect();
                                    let pod_settings = state.settings_for_pod(&settings, pod_info.number).await;
                                    let _ = cmd_tx.send(CoreToAgentMessage::SettingsUpdated { settings: pod_settings }).await;
                                    tracing::info!("Sent initial kiosk settings to pod {}", pod_info.number);
                                }
                            }
                        }
                        AgentMessage::Heartbeat(pod_info) => {
                            // Merge agent-reported fields with core-managed fields
                            // (billing_session_id, current_driver, status are managed by rc-core billing)
                            let mut pods = state.pods.write().await;
                            let updated = if let Some(existing) = pods.get_mut(&pod_info.id) {
                                // Preserve core-managed billing state
                                existing.ip_address = pod_info.ip_address.clone();
                                existing.last_seen = Some(chrono::Utc::now());
                                existing.driving_state = pod_info.driving_state;
                                existing.game_state = pod_info.game_state;
                                existing.current_game = pod_info.current_game;
                                existing.clone()
                            } else {
                                pods.insert(pod_info.id.clone(), pod_info.clone());
                                pod_info.clone()
                            };
                            drop(pods);
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::PodUpdate(updated));
                        }
                        AgentMessage::Telemetry(frame) => {
                            // Feed telemetry to camera controller
                            crate::ac_camera::on_telemetry(&state, &frame).await;
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::Telemetry(frame.clone()));
                        }
                        AgentMessage::LapCompleted(lap) => {
                            let mut lap = lap.clone();

                            // Resolve driver from active billing session on this pod
                            if let Some((driver_id, session_id)) =
                                crate::lap_tracker::resolve_driver_for_pod(&state, &lap.pod_id).await
                            {
                                lap.driver_id = driver_id;
                                lap.session_id = session_id;
                            }

                            tracing::info!(
                                "Lap completed: {} - {}ms on {}",
                                lap.driver_id, lap.lap_time_ms, lap.track
                            );

                            // Persist to DB and update leaderboards
                            crate::lap_tracker::persist_lap(&state, &lap).await;

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::LapCompleted(lap));
                        }
                        AgentMessage::SessionUpdate(session) => {
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::SessionUpdate(session.clone()));
                        }
                        AgentMessage::DrivingStateUpdate { pod_id, state: driving_state } => {
                            tracing::debug!("Pod {} driving state: {:?}", pod_id, driving_state);

                            // Update pod info
                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                pod.driving_state = Some(*driving_state);
                            }

                            // Update billing timer
                            billing::update_driving_state(&state, pod_id, *driving_state).await;
                        }
                        AgentMessage::GameStateUpdate(info) => {
                            tracing::info!(
                                "Pod {} game state: {:?} ({:?})",
                                info.pod_id, info.game_state, info.sim_type
                            );
                            let gs_action = match info.game_state {
                                GameState::Running => "Game Running",
                                GameState::Error => "Game Crashed",
                                GameState::Idle => "Game Stopped",
                                GameState::Launching => "Game Launching",
                                GameState::Stopping => "Game Stopping",
                            };
                            let gs_details = match &info.error_message {
                                Some(err) => format!("{}: {}", info.sim_type, err),
                                None => format!("{}", info.sim_type),
                            };
                            log_pod_activity(&state, &info.pod_id, "game", gs_action, &gs_details, "agent");
                            game_launcher::handle_game_state_update(&state, info.clone()).await;
                        }
                        AgentMessage::AiDebugResult(suggestion) => {
                            tracing::info!(
                                "AI debug suggestion for pod {}: {}",
                                suggestion.pod_id, suggestion.model
                            );
                            // Persist to DB
                            let id = uuid::Uuid::new_v4().to_string();
                            let _ = sqlx::query(
                                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                                 VALUES (?, ?, ?, ?, ?, ?, 'crash')"
                            )
                            .bind(&id)
                            .bind(&suggestion.pod_id)
                            .bind(format!("{:?}", suggestion.sim_type))
                            .bind(&suggestion.error_context)
                            .bind(&suggestion.suggestion)
                            .bind(&suggestion.model)
                            .execute(&state.db)
                            .await;

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::AiDebugSuggestion(suggestion.clone()));
                        }
                        AgentMessage::PinEntered { pod_id, pin } => {
                            tracing::info!("PIN entered on pod {}", pod_id);
                            log_pod_activity(&state, pod_id, "auth", "PIN Entered", "", "agent");
                            auth::handle_pin_entered(&state, pod_id.clone(), pin.clone()).await;
                        }
                        AgentMessage::Pong { id } => {
                            // Application-level round-trip measurement response
                            let mut guard = pending_ping.lock().await;
                            if let Some((pending_id, sent_at)) = guard.take() {
                                if pending_id == *id {
                                    let elapsed_ms = sent_at.elapsed().as_millis();
                                    let fallback_label = format!("conn_{}", conn_id);
                                    let label = registered_pod_id.as_deref().unwrap_or(&fallback_label);
                                    if elapsed_ms > 200 {
                                        tracing::warn!(
                                            "WS round-trip slow: {} took {}ms (threshold 200ms)",
                                            label, elapsed_ms
                                        );
                                    } else {
                                        tracing::debug!(
                                            "WS round-trip: {}ms ({})",
                                            elapsed_ms, label
                                        );
                                    }
                                } else {
                                    // Stale pong (id mismatch) — discard
                                    tracing::debug!(
                                        "Stale pong id={} (expected {}), discarding",
                                        id, pending_id
                                    );
                                }
                            }
                        }
                        AgentMessage::GameStatusUpdate { pod_id, ac_status } => {
                            // Phase 03 Plan 03 will wire this to billing lifecycle.
                            // For now, log the status change.
                            tracing::info!(
                                "AC STATUS update from pod {}: {:?}",
                                pod_id, ac_status
                            );
                        }
                        AgentMessage::Disconnect { pod_id } => {
                            tracing::info!("Pod {} disconnected", pod_id);
                            log_pod_activity(&state, pod_id, "system", "Pod Offline", "Agent sent disconnect", "agent");
                            let has_active_billing = state
                                .billing
                                .active_timers
                                .read()
                                .await
                                .contains_key(pod_id.as_str());

                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                // Don't overwrite Disabled — admin intentionally shut it down
                                if pod.status == rc_common::types::PodStatus::Disabled {
                                    break;
                                }
                                pod.status = rc_common::types::PodStatus::Offline;
                                pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                                // Preserve game_state if billing is active — agent will resync on reconnect
                                if !has_active_billing {
                                    pod.game_state = Some(GameState::Idle);
                                    pod.current_game = None;
                                }
                                let _ = state
                                    .dashboard_tx
                                    .send(DashboardEvent::PodUpdate(pod.clone()));
                            }
                            // Update billing timer to no-device
                            billing::update_driving_state(
                                &state,
                                pod_id,
                                rc_common::types::DrivingState::NoDevice,
                            )
                            .await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Invalid agent message: {}", e);
                }
            }
        }
    }

    // Cleanup: only remove sender and mark offline if THIS connection is still the active one.
    // A newer connection may have already replaced us in agent_senders/agent_conn_ids,
    // in which case this is a stale zombie disconnect and we must NOT touch the pod state.
    if let Some(pod_id) = &registered_pod_id {
        let current_conn_id = state.agent_conn_ids.read().await.get(pod_id).copied();
        let is_stale = current_conn_id.is_some() && current_conn_id != Some(conn_id);

        if is_stale {
            tracing::info!(
                "Stale WebSocket cleanup for pod {} (conn_id={}, current={}). Skipping.",
                pod_id, conn_id, current_conn_id.unwrap()
            );
        } else {
            state.agent_senders.write().await.remove(pod_id);
            state.agent_conn_ids.write().await.remove(pod_id);

            let has_active_billing = state
                .billing
                .active_timers
                .read()
                .await
                .contains_key(pod_id.as_str());

            // Mark pod offline on ungraceful disconnect (WebSocket dropped without Disconnect message)
            if let Some(pod) = state.pods.write().await.get_mut(pod_id.as_str()) {
                if pod.status != rc_common::types::PodStatus::Offline
                    && pod.status != rc_common::types::PodStatus::Disabled
                {
                    tracing::warn!("Pod {} WebSocket dropped without Disconnect (conn_id={}) — marking Offline", pod_id, conn_id);
                    log_pod_activity(&state, pod_id, "system", "Pod Disconnected", &format!("WebSocket dropped unexpectedly (conn_id={})", conn_id), "core");
                    pod.status = rc_common::types::PodStatus::Offline;
                    pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                    // Preserve game_state if billing is active — agent will resync on reconnect
                    if !has_active_billing {
                        pod.game_state = Some(GameState::Idle);
                        pod.current_game = None;
                    }
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }

            billing::update_driving_state(&state, pod_id, rc_common::types::DrivingState::NoDevice)
                .await;
        }
    }

    send_task.abort();
    tracing::info!("Pod agent disconnected");
}

async fn handle_dashboard(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("Dashboard client connected");

    // Send current pod list on connect
    let pods = state.pods.read().await;
    let pod_list: Vec<_> = pods.values().cloned().collect();
    drop(pods);

    let init_msg = DashboardEvent::PodList(pod_list);
    if let Ok(json) = serde_json::to_string(&init_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active billing sessions on connect
    let timers = state.billing.active_timers.read().await;
    let billing_list: Vec<_> = timers.values().map(|t| t.to_info()).collect();
    drop(timers);

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

    // Forward broadcast events to this dashboard client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming commands from dashboard
    let cmd_state = state.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
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
                                if let Err(e) = crate::deploy::deploy_rolling(deploy_state, deploy_url).await {
                                    tracing::error!("Rolling deploy via dashboard failed: {}", e);
                                }
                            });
                        }
                        DashboardCommand::CancelDeploy { pod_id } => {
                            // Mark the deploy state as Failed to signal cancellation.
                            // The running deploy_pod() task checks is_cancelled() at each step
                            // and exits early if it finds a Failed state.
                            let mut deploy_states = cmd_state.pod_deploy_states.write().await;
                            if let Some(ds) = deploy_states.get(pod_id) {
                                if ds.is_active() {
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
    tracing::info!("Dashboard client disconnected");
}

/// WebSocket endpoint for AI-to-AI messaging (Bono ↔ James)
pub async fn ai_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ai(socket, state))
}

async fn handle_ai(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    tracing::info!("AI channel: connection attempt");

    // First message must be Auth
    let identity = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<AiChannelMessage>(&text) {
                Ok(AiChannelMessage::Auth { secret, identity }) => {
                    let expected = state.config.cloud.terminal_secret.as_deref();
                    if expected.is_some() && expected != Some(&secret) {
                        let fail = AiChannelMessage::AuthFailed {
                            reason: "Invalid secret".to_string(),
                        };
                        if let Ok(json) = serde_json::to_string(&fail) {
                            let _ = ws_sender.send(Message::Text(json.into())).await;
                        }
                        tracing::warn!("AI channel: auth failed for {}", identity);
                        return;
                    }
                    identity
                }
                _ => {
                    tracing::warn!("AI channel: first message was not Auth");
                    return;
                }
            }
        }
        _ => {
            tracing::warn!("AI channel: connection closed before auth");
            return;
        }
    };

    // Send AuthOk
    let auth_ok = AiChannelMessage::AuthOk {
        identity: identity.clone(),
    };
    if let Ok(json) = serde_json::to_string(&auth_ok) {
        if ws_sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    tracing::info!("AI channel: {} authenticated", identity);

    // Create mpsc channel for sending messages to this peer
    let (msg_tx, mut msg_rx) = mpsc::channel::<AiChannelMessage>(256);

    // Store sender so HTTP endpoints can push via WS
    *state.ai_peer_tx.write().await = Some(msg_tx.clone());

    // Deliver any pending messages from DB
    let pending: Vec<(String, String, String, String, Option<String>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT id, sender, content, message_type, metadata, in_reply_to, created_at
             FROM ai_messages WHERE recipient = ? AND status = 'pending'
             ORDER BY created_at ASC",
        )
        .bind(&identity)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    for (id, sender, content, msg_type, metadata, in_reply_to, created_at) in &pending {
        let msg = AiChannelMessage::Message {
            id: id.clone(),
            sender: sender.clone(),
            content: content.clone(),
            message_type: msg_type.clone(),
            metadata: metadata.as_ref().and_then(|m| serde_json::from_str(m).ok()),
            in_reply_to: in_reply_to.clone(),
            created_at: created_at.clone(),
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
            // Mark as delivered
            let _ = sqlx::query(
                "UPDATE ai_messages SET status = 'delivered', channel = 'ws',
                 delivered_at = datetime('now') WHERE id = ?",
            )
            .bind(id)
            .execute(&state.db)
            .await;
        }
    }

    if !pending.is_empty() {
        tracing::info!("AI channel: delivered {} pending messages to {}", pending.len(), identity);
    }

    // Spawn task to forward mpsc messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Listen for incoming messages from peer
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<AiChannelMessage>(&text) {
                    Ok(ai_msg) => match &ai_msg {
                        AiChannelMessage::Message {
                            id,
                            sender,
                            content,
                            message_type,
                            metadata,
                            in_reply_to,
                            created_at,
                        } => {
                            let recipient = if sender == "james" { "bono" } else { "james" };
                            let meta_str = metadata.as_ref().map(|v| v.to_string());
                            let _ = sqlx::query(
                                "INSERT OR IGNORE INTO ai_messages
                                 (id, sender, recipient, content, message_type, metadata, channel, status, in_reply_to, created_at)
                                 VALUES (?, ?, ?, ?, ?, ?, 'ws', 'delivered', ?, ?)",
                            )
                            .bind(id)
                            .bind(sender)
                            .bind(recipient)
                            .bind(content)
                            .bind(message_type)
                            .bind(&meta_str)
                            .bind(in_reply_to)
                            .bind(created_at)
                            .execute(&state.db)
                            .await;

                            let _ = state.dashboard_tx.send(DashboardEvent::AiMessage {
                                id: id.clone(),
                                sender: sender.clone(),
                                recipient: recipient.to_string(),
                                content: content.clone(),
                                message_type: message_type.clone(),
                                created_at: created_at.clone(),
                            });

                            // Send Ack
                            let _ = msg_tx
                                .send(AiChannelMessage::Ack {
                                    message_id: id.clone(),
                                })
                                .await;
                        }
                        AiChannelMessage::Ack { message_id } => {
                            let _ = sqlx::query(
                                "UPDATE ai_messages SET status = 'delivered', delivered_at = datetime('now')
                                 WHERE id = ? AND status = 'pending'",
                            )
                            .bind(message_id)
                            .execute(&state.db)
                            .await;
                        }
                        AiChannelMessage::MarkRead { message_id } => {
                            let _ = sqlx::query(
                                "UPDATE ai_messages SET status = 'read', read_at = datetime('now') WHERE id = ?",
                            )
                            .bind(message_id)
                            .execute(&state.db)
                            .await;
                        }
                        AiChannelMessage::Ping => {
                            let _ = msg_tx.send(AiChannelMessage::Pong).await;
                        }
                        _ => {}
                    },
                    Err(e) => {
                        tracing::warn!("AI channel: invalid message: {}", e);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup
    *state.ai_peer_tx.write().await = None;
    send_task.abort();
    tracing::info!("AI channel: {} disconnected", identity);
}
