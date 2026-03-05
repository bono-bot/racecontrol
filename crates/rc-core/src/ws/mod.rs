use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::ac_camera;
use crate::ac_server;
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

    tracing::info!("Pod agent connected");

    // Create mpsc channel for sending commands back to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CoreToAgentMessage>(64);
    let mut registered_pod_id: Option<String> = None;

    // Spawn task to forward commands from mpsc to WebSocket sender
    let send_task = tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&cmd) {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
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
                            tracing::info!("Pod {} registered: {}", pod_info.number, pod_info.name);
                            registered_pod_id = Some(pod_info.id.clone());

                            // Store agent sender for this pod
                            state
                                .agent_senders
                                .write()
                                .await
                                .insert(pod_info.id.clone(), cmd_tx.clone());

                            state
                                .pods
                                .write()
                                .await
                                .insert(pod_info.id.clone(), pod_info.clone());

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::PodUpdate(pod_info.clone()));
                        }
                        AgentMessage::Heartbeat(pod_info) => {
                            state
                                .pods
                                .write()
                                .await
                                .insert(pod_info.id.clone(), pod_info.clone());
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::PodUpdate(pod_info.clone()));
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
                            auth::handle_pin_entered(&state, pod_id.clone(), pin.clone()).await;
                        }
                        AgentMessage::Disconnect { pod_id } => {
                            tracing::info!("Pod {} disconnected", pod_id);
                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                // Don't overwrite Disabled — admin intentionally shut it down
                                if pod.status == rc_common::types::PodStatus::Disabled {
                                    break;
                                }
                                pod.status = rc_common::types::PodStatus::Offline;
                                pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                                pod.game_state = Some(GameState::Idle);
                                pod.current_game = None;
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

    // Cleanup: remove agent sender and mark pod offline
    if let Some(pod_id) = &registered_pod_id {
        state.agent_senders.write().await.remove(pod_id);

        // Mark pod offline on ungraceful disconnect (WebSocket dropped without Disconnect message)
        if let Some(pod) = state.pods.write().await.get_mut(pod_id.as_str()) {
            if pod.status != rc_common::types::PodStatus::Offline
                && pod.status != rc_common::types::PodStatus::Disabled
            {
                tracing::warn!("Pod {} WebSocket dropped without Disconnect — marking Offline", pod_id);
                pod.status = rc_common::types::PodStatus::Offline;
                pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                pod.game_state = Some(GameState::Idle);
                pod.current_game = None;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }

        billing::update_driving_state(&state, pod_id, rc_common::types::DrivingState::NoDevice)
            .await;
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
                            game_launcher::handle_dashboard_command(&cmd_state, cmd).await;
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
