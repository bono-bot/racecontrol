use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::sync::broadcast;

use crate::alerts::types::AlertEvent;

/// Shared state for the alert WebSocket endpoint.
pub struct AlertWsState {
    pub alert_tx: broadcast::Sender<AlertEvent>,
}

/// Build the alerts WebSocket router.
pub fn alerts_router(state: Arc<AlertWsState>) -> axum::Router {
    axum::Router::new()
        .route("/ws/alerts", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AlertWsState>>,
) -> impl IntoResponse {
    let alert_rx = state.alert_tx.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, alert_rx))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut alert_rx: broadcast::Receiver<AlertEvent>,
) {
    tracing::info!("alert WS client connected");

    loop {
        match alert_rx.recv().await {
            Ok(event) => {
                let json = match serde_json::to_string(&event) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to serialize AlertEvent");
                        continue;
                    }
                };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    tracing::info!("alert WS client disconnected (send error)");
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "alert WS receiver lagged");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("alert broadcast closed, closing WS");
                break;
            }
        }
    }
}
