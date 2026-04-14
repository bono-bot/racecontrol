//! AI WebSocket handler — Bono ↔ James AI-to-AI messaging + pod exec proxy.
//!
//! Extracted from ws/mod.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::state::AppState;
use rc_common::protocol::{AiChannelMessage, CoreMessage, DashboardEvent};
use super::{WsAuthParams, verify_ws_token};

pub async fn ai_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_ws_token(&state, &params.token) {
        tracing::warn!("WS AI channel connection rejected — invalid or missing token");
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(ws.on_upgrade(|socket| handle_ai(socket, state)))
}

pub async fn handle_ai(socket: WebSocket, state: Arc<AppState>) {
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
                            if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                                tracing::warn!("[ws] AI channel: failed to send AuthFailed response: {}", e);
                            }
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

/// Send a shell command to a pod agent via WebSocket and wait for the result.
///
/// Uses pod-prefixed request_id (e.g. "pod_3:uuid") so disconnect cleanup
/// can identify and remove stale entries.
///
/// Returns (success, stdout, stderr) or an error string.
pub async fn ws_exec_on_pod(
    state: &std::sync::Arc<crate::state::AppState>,
    pod_id: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    let request_id = format!("{}:{}", pod_id, uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Register pending response
    state.pending_ws_execs.write().await.insert(request_id.clone(), tx);

    // Clone the sender, drop the lock immediately (avoid holding lock across await)
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(pod_id).cloned()
            .ok_or_else(|| format!("Pod {} not connected via WebSocket", pod_id))?
    };

    // Send the command
    if sender.send(CoreMessage::wrap(rc_common::protocol::CoreToAgentMessage::Exec {
        request_id: request_id.clone(),
        cmd: cmd.to_string(),
        timeout_ms,
    })).await.is_err() {
        state.pending_ws_execs.write().await.remove(&request_id);
        return Err(format!("Failed to send command to pod {}", pod_id));
    }

    // Wait for response with buffer timeout (command timeout + 5s for WS round trip)
    match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms + 5000),
        rx,
    ).await {
        Ok(Ok(result)) => Ok((result.success, result.stdout, result.stderr)),
        Ok(Err(_)) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err("WS response channel closed unexpectedly".to_string())
        }
        Err(_) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err(format!("WS command timed out after {}ms", timeout_ms + 5000))
        }
    }
}
