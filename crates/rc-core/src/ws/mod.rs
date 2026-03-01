use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::billing;
use crate::state::AppState;
use rc_common::protocol::{AgentMessage, CoreToAgentMessage, DashboardCommand, DashboardEvent};

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
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::Telemetry(frame.clone()));
                        }
                        AgentMessage::LapCompleted(lap) => {
                            tracing::info!(
                                "Lap completed: {} - {}ms on {}",
                                lap.driver_id, lap.lap_time_ms, lap.track
                            );
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::LapCompleted(lap.clone()));
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
                        AgentMessage::Disconnect { pod_id } => {
                            tracing::info!("Pod {} disconnected", pod_id);
                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                pod.status = rc_common::types::PodStatus::Offline;
                                pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
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

    // Cleanup: remove agent sender
    if let Some(pod_id) = &registered_pod_id {
        state.agent_senders.write().await.remove(pod_id);
        // Set driving state to NoDevice for any active billing
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
                    Ok(cmd) => {
                        billing::handle_dashboard_command(&cmd_state, cmd).await;
                    }
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
