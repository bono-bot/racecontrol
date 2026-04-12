#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
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

// ─── PWA: Customer game launch request ─────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct GameRequestBody {
    pod_id: String,
    sim_type: SimType,
}

pub(crate) async fn pwa_game_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<GameRequestBody>,
) -> (axum::http::StatusCode, Json<Value>) {
    // Authenticate: extract driver_id from customer JWT
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            )
        }
    };

    // Look up driver name for the broadcast payload
    let driver_name = match sqlx::query_as::<_, (String,)>(
        "SELECT name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some((name,))) => name,
        Ok(None) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Driver not found" })),
            )
        }
        Err(e) => {
            tracing::error!("pwa_game_request: DB error looking up driver {}: {}", driver_id, e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            );
        }
    };

    // Validate pod exists
    let pods = state.pods.read().await;
    let pod = match pods.get(&body.pod_id) {
        Some(p) => p.clone(),
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Pod '{}' not found", body.pod_id) })),
            )
        }
    };
    drop(pods);

    // Validate game is installed on that pod
    if !pod.installed_games.contains(&body.sim_type) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("{:?} is not installed on pod '{}'", body.sim_type, body.pod_id)
            })),
        );
    }

    // Generate unique request ID
    let request_id = uuid::Uuid::new_v4().to_string();

    // BILL-03: Insert into game_launch_requests with 10-minute server-side TTL
    let sim_type_str = format!("{:?}", body.sim_type);
    if let Err(e) = sqlx::query(
        "INSERT INTO game_launch_requests (id, driver_id, pod_id, sim_type, status, expires_at, venue_id)
         VALUES (?, ?, ?, ?, 'pending', datetime('now', '+10 minutes'), ?)",
    )
    .bind(&request_id)
    .bind(&driver_id)
    .bind(&body.pod_id)
    .bind(&sim_type_str)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("pwa_game_request: Failed to insert game_launch_request {}: {}", request_id, e);
        // Non-fatal: still broadcast to staff
    }

    // Broadcast to staff dashboard -- staff confirms via existing launch endpoint
    let _ = state.dashboard_tx.send(DashboardEvent::GameLaunchRequested {
        pod_id: body.pod_id.clone(),
        sim_type: body.sim_type,
        driver_name: driver_name.clone(),
        request_id: request_id.clone(),
    });

    tracing::info!(
        "pwa_game_request: driver '{}' ({}) requested {:?} on pod '{}' (request_id={}, pwa_request_timeout=10min)",
        driver_name, driver_id, body.sim_type, body.pod_id, request_id
    );

    (
        axum::http::StatusCode::OK,
        Json(json!({ "ok": true, "request_id": request_id })),
    )
}

/// GET /customer/game-request/{id} — Check status of a PWA game request.
/// BILL-03: Returns status including "expired" if the 10-minute TTL has passed.
pub(crate) async fn get_game_request_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> (axum::http::StatusCode, Json<Value>) {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            )
        }
    };

    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT status, expires_at, resolved_at FROM game_launch_requests WHERE id = ? AND driver_id = ?",
    )
    .bind(&request_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Request not found" })),
        ),
        Some((status, expires_at, resolved_at)) => {
            // BILL-03: If pending but TTL passed, return expired status in real-time
            let effective_status = if status == "pending" {
                let is_expired: Option<(i64,)> = sqlx::query_as(
                    "SELECT CASE WHEN ? < datetime('now') THEN 1 ELSE 0 END",
                )
                .bind(&expires_at)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
                if is_expired.map(|(v,)| v).unwrap_or(0) == 1 {
                    "expired".to_string()
                } else {
                    status
                }
            } else {
                status
            };
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "request_id": request_id,
                    "status": effective_status,
                    "expires_at": expires_at,
                    "resolved_at": resolved_at,
                })),
            )
        }
    }
}
