#![allow(unused_imports)]
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

// ─── STAFF-05: Shift Handoff ──────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct ShiftHandoffRequest {
    incoming_staff_id: Option<String>,
    notes: Option<String>,
}

/// POST /api/v1/staff/shift-handoff
///
/// STAFF-05: Outgoing staff member logs shift handoff.
/// - Reads all active billing sessions from active_timers.
/// - If active sessions exist and incoming_staff_id is not provided, returns 400.
/// - Inserts audit_log entry: action='shift_handoff', actor=outgoing staff sub.
/// - Returns list of active sessions for handoff acknowledgment.
pub(crate) async fn shift_handoff_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<ShiftHandoffRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // Snapshot active timers — drop lock before any async work
    let active_sessions: Vec<Value> = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .map(|t| {
                let elapsed_minutes = t.elapsed_seconds / 60;
                let game_type = t
                    .sim_type
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".to_string());
                json!({
                    "pod_id": t.pod_id,
                    "billing_session_id": t.session_id,
                    "driver_name": t.driver_name,
                    "elapsed_minutes": elapsed_minutes,
                    "game_type": game_type,
                })
            })
            .collect()
    };

    // If active sessions exist, incoming_staff_id is mandatory
    if !active_sessions.is_empty() && req.incoming_staff_id.is_none() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Active sessions require incoming staff acknowledgment — provide incoming_staff_id",
                "active_session_count": active_sessions.len(),
            })),
        );
    }

    let handoff_at = chrono::Utc::now().to_rfc3339();
    let session_ids: Vec<String> = active_sessions
        .iter()
        .filter_map(|s| s["billing_session_id"].as_str().map(str::to_string))
        .collect();

    let details = json!({
        "incoming_staff_id": req.incoming_staff_id,
        "active_session_count": active_sessions.len(),
        "session_ids": session_ids,
        "notes": req.notes,
        "handoff_at": handoff_at,
    });

    // Insert audit log entry
    crate::accounting::log_admin_action(
        &state,
        "shift_handoff",
        &details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        incoming_staff_id = ?req.incoming_staff_id,
        active_session_count = active_sessions.len(),
        "STAFF-05: Shift handoff logged"
    );

    (
        axum::http::StatusCode::OK,
        Json(json!({
            "outgoing_staff_id": claims.sub,
            "incoming_staff_id": req.incoming_staff_id,
            "active_sessions": active_sessions,
            "handoff_at": handoff_at,
        })),
    )
}

/// GET /api/v1/staff/shift-briefing
///
/// STAFF-05: Returns current active session summary for incoming staff.
/// Also includes the last shift_handoff event from audit_log for context.
pub(crate) async fn shift_briefing_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(_claims): axum::Extension<crate::auth::middleware::StaffClaims>,
) -> Json<Value> {
    // Snapshot active timers — drop lock before DB query
    let active_sessions: Vec<Value> = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .map(|t| {
                let elapsed_minutes = t.elapsed_seconds / 60;
                let game_type = t
                    .sim_type
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".to_string());
                json!({
                    "pod_id": t.pod_id,
                    "billing_session_id": t.session_id,
                    "driver_name": t.driver_name,
                    "elapsed_minutes": elapsed_minutes,
                    "game_type": game_type,
                })
            })
            .collect()
    };

    // Query most recent shift_handoff from audit_log
    let last_handoff: Option<Value> = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT staff_id, new_values, id
         FROM audit_log
         WHERE action_type = 'shift_handoff'
         ORDER BY rowid DESC
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(staff_id, details_json, _id)| {
        let details: Value = serde_json::from_str(&details_json).unwrap_or(Value::Null);
        json!({
            "outgoing_staff": staff_id,
            "notes": details.get("notes"),
            "incoming_staff_id": details.get("incoming_staff_id"),
            "handoff_at": details.get("handoff_at"),
        })
    });

    Json(json!({
        "active_sessions": active_sessions,
        "last_handoff": last_handoff,
    }))
}
