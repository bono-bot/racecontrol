#![allow(unused_imports)]
use super::terminal_handlers::check_terminal_auth;
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Cloud Action Queue Endpoints ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CreateActionRequest {
    action_type: String,
    payload: Value,
}

/// POST /actions — create a new action for the venue to pick up.
/// Auth: x-terminal-secret header (same as sync endpoints).
/// When comms_link_url is configured, also pushes the action via relay for sub-second delivery.
pub(crate) async fn create_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateActionRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let payload_str = serde_json::to_string(&body.payload).unwrap_or_else(|_| "{}".to_string());

    let result = sqlx::query(
        "INSERT INTO action_queue (id, action_type, payload, status, created_at)
         VALUES (?, ?, ?, 'pending', datetime('now'))",
    )
    .bind(&id)
    .bind(&body.action_type)
    .bind(&payload_str)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Action queue: created {} ({})", id, body.action_type);

            // Also push via relay for sub-second delivery (fire-and-forget).
            // If relay fails, venue will still pick up via polling fallback.
            if let Some(relay_url) = &state.config.cloud.comms_link_url {
                let relay_action_url = format!("{}/relay/action", relay_url);
                let relay_payload = json!({
                    "action_id": &id,
                    "action_type": &body.action_type,
                    "payload": &body.payload,
                });
                let client = state.http_client.clone();
                let id_clone = id.clone();
                tokio::spawn(async move {
                    match client
                        .post(&relay_action_url)
                        .json(&relay_payload)
                        .timeout(std::time::Duration::from_secs(2))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status().is_success() => {
                            tracing::debug!("Action {} pushed via relay", id_clone);
                        }
                        Ok(resp) => {
                            tracing::debug!("Action relay push returned {}", resp.status());
                        }
                        Err(e) => {
                            tracing::debug!("Action relay push failed (venue will poll): {}", e);
                        }
                    }
                });
            }

            Json(json!({ "ok": true, "id": id, "action_type": body.action_type }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to create action: {}", e) })),
    }
}

/// POST /actions/process — receive a pushed action from comms-link relay.
/// Called by comms-link when it receives a sync_action WS message from the cloud.
/// Auth: x-terminal-secret header.
pub(crate) async fn process_action_endpoint(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    // Parse the action from the request body
    let action: CloudAction = match serde_json::from_value(body.get("action").cloned().unwrap_or(body.clone())) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("Failed to parse pushed action: {}", e);
            return Json(json!({ "status": "failed", "error": format!("Invalid action: {}", e) }));
        }
    };

    let action_id = body
        .get("action_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    tracing::info!("Processing pushed action: {}", action_id);

    match crate::action_queue::process_action(&state, &action).await {
        Ok(()) => {
            tracing::info!("Pushed action {} completed", action_id);
            Json(json!({ "status": "completed" }))
        }
        Err(e) => {
            tracing::warn!("Pushed action {} failed: {}", action_id, e);
            Json(json!({ "status": "failed", "error": e.to_string() }))
        }
    }
}

/// GET /actions/pending — returns all pending actions for the venue to process.
/// Auth: x-terminal-secret header.
pub(crate) async fn pending_actions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, action_type, payload, created_at
         FROM action_queue
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, created_at)| {
                    let payload_val: Value =
                        serde_json::from_str(payload).unwrap_or(json!({}));
                    // Build the PendingCloudAction format expected by venue action_queue.rs
                    json!({
                        "id": id,
                        "action": {
                            "action_type": action_type,
                            "payload": payload_val,
                        },
                        "created_at": created_at,
                    })
                })
                .collect();

            // Mark returned actions as processing to avoid re-delivery
            for (id, _, _, _) in &rows {
                let _ = sqlx::query(
                    "UPDATE action_queue SET status = 'processing', processed_at = datetime('now') WHERE id = ?",
                )
                .bind(id)
                .execute(&state.db)
                .await;
            }

            Json(json!({ "actions": actions }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch actions: {}", e) })),
    }
}

/// POST /actions/{id}/ack — venue acknowledges a processed action.
/// Auth: x-terminal-secret header.
pub(crate) async fn ack_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");
    let error = body.get("error").and_then(|v| v.as_str());

    let result = sqlx::query(
        "UPDATE action_queue SET status = ?, error = ?, acked_at = datetime('now') WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Action queue: acked {} → {}", id, status);
            Json(json!({ "ok": true, "id": id, "status": status }))
        }
        Ok(_) => Json(json!({ "error": "Action not found" })),
        Err(e) => Json(json!({ "error": format!("Failed to ack: {}", e) })),
    }
}

/// GET /actions/history — recent action history for debugging.
/// Auth: x-terminal-secret header.
pub(crate) async fn action_history(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Option<String>, Option<String>)>(
        "SELECT id, action_type, payload, status, error, created_at, processed_at, acked_at
         FROM action_queue
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, status, error, created_at, processed_at, acked_at)| {
                    json!({
                        "id": id,
                        "action_type": action_type,
                        "payload": serde_json::from_str::<Value>(payload).unwrap_or(json!({})),
                        "status": status,
                        "error": error,
                        "created_at": created_at,
                        "processed_at": processed_at,
                        "acked_at": acked_at,
                    })
                })
                .collect();
            Json(json!({ "actions": actions, "total": actions.len() }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch history: {}", e) })),
    }
}
