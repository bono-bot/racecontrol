//! Spectator WebSocket handler — circuit viewer for TV displays.
//!
//! Extracted from ws/mod.rs (Phase 385, v49.0 Architecture Completion).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::state::AppState;

// ─── Phase 335: Spectator Circuit Viewer WebSocket ───────────────────────────

/// WebSocket endpoint for spectator displays (TV).
/// No auth required — spectator displays have no JWT.
/// Subscribes to dashboard_tx and forwards only telemetry position data at ~10Hz.
pub async fn spectator_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_spectator(socket, state))
}

/// Spectator car position message sent at ~10Hz.
#[derive(serde::Serialize, Clone, Debug)]
struct SpectatorCarPosition {
    pod_id: String,
    pod_number: u32,
    driver_name: String,
    normalized_position: f32,
    lap: u32,
    speed_kmh: f32,
    track: String,
    car: String,
    is_in_pit: bool,
}

/// Message type sent to spectator clients.
#[derive(serde::Serialize)]
#[serde(tag = "type")]
enum SpectatorMessage {
    #[serde(rename = "car_positions")]
    CarPositions {
        positions: Vec<SpectatorCarPosition>,
        track: String,
        timestamp: String,
    },
    #[serde(rename = "track_changed")]
    TrackChanged {
        track_id: String,
        has_outline: bool,
    },
}

pub async fn handle_spectator(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    tracing::info!(target: "spectator-ws", "Spectator client connected");

    // Subscribe to dashboard broadcast channel
    let mut dashboard_rx = state.dashboard_tx.subscribe();

    // Track latest telemetry per pod for aggregated position broadcasts
    let positions: Arc<tokio::sync::RwLock<HashMap<String, SpectatorCarPosition>>> =
        Arc::new(tokio::sync::RwLock::new(HashMap::new()));
    let positions_clone = positions.clone();

    // Spawn a task that listens to dashboard events and updates positions
    let update_task = tokio::spawn(async move {
        loop {
            match dashboard_rx.recv().await {
                Ok(event) => {
                    if let rc_common::protocol::DashboardEvent::Telemetry(frame) = event {
                        let normalized = frame.normalized_car_position.unwrap_or(0.0);
                        // Only track pods with valid position data
                        if normalized >= 0.0 && normalized <= 1.0 {
                            let pos = SpectatorCarPosition {
                                pod_id: frame.pod_id.clone(),
                                pod_number: frame.pod_id
                                    .strip_prefix("pod_")
                                    .and_then(|n| n.parse().ok())
                                    .unwrap_or(0),
                                driver_name: frame.driver_name.clone(),
                                normalized_position: normalized,
                                lap: frame.lap_number,
                                speed_kmh: frame.speed_kmh,
                                track: frame.track.clone(),
                                car: frame.car.clone(),
                                is_in_pit: frame.is_in_pit.unwrap_or(false),
                            };
                            positions_clone.write().await.insert(frame.pod_id.clone(), pos);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!(target: "spectator-ws", "Spectator lagged {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Spawn a task that sends position updates at 10Hz
    let positions_for_send = positions.clone();
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

    let send_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(100));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut last_track = String::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let snapshot = {
                        let guard = positions_for_send.read().await;
                        guard.values().cloned().collect::<Vec<_>>()
                    };

                    if snapshot.is_empty() {
                        continue;
                    }

                    // Detect dominant track (most pods are on)
                    let track = snapshot.first().map(|p| p.track.clone()).unwrap_or_default();

                    // Notify on track change
                    if track != last_track && !track.is_empty() {
                        last_track = track.clone();
                        // We don't have access to state here for outline check,
                        // but the client can fetch the outline via REST
                        let change_msg = SpectatorMessage::TrackChanged {
                            track_id: track.clone(),
                            has_outline: true,
                        };
                        if let Ok(json) = serde_json::to_string(&change_msg) {
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }

                    let msg = SpectatorMessage::CarPositions {
                        positions: snapshot,
                        track: track.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };

                    if let Ok(json) = serde_json::to_string(&msg) {
                        if ws_sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
                _ = &mut stop_rx => break,
            }
        }
    });

    // Wait for client disconnect
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(Message::Ping(data)) => {
                // Pong handled by axum automatically
                let _ = data;
            }
            _ => {} // Spectator is read-only, ignore other messages
        }
    }

    let _ = stop_tx.send(());
    update_task.abort();
    send_task.abort();

    tracing::info!(target: "spectator-ws", "Spectator client disconnected");
}
