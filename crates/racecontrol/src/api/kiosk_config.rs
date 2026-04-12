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

// ─── Kiosk Allowlist (Phase 48 — ALLOW-01/02/05) ────────────────────────────
//
// Well-known system processes that staff might accidentally try to add.
// This is a UX guard only — the authoritative ~70-entry baseline lives in
// rc-agent's ALLOWED_PROCESSES constant and is never modified here.
pub(crate) const BASELINE_PROCESSES: &[&str] = &[
    "svchost.exe",
    "csrss.exe",
    "explorer.exe",
    "lsass.exe",
    "winlogon.exe",
    "services.exe",
    "smss.exe",
    "taskmgr.exe",
    "spoolsv.exe",
    "dwm.exe",
    "wininit.exe",
    "conhost.exe",
    "ntoskrnl.exe",
    "system",
];

pub(crate) async fn list_kiosk_allowlist(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        "SELECT id, process_name, added_by, notes, created_at
         FROM kiosk_allowlist ORDER BY process_name ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(entries) => {
            let list: Vec<Value> = entries
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0,
                        "process_name": r.1,
                        "added_by": r.2,
                        "notes": r.3,
                        "created_at": r.4,
                    })
                })
                .collect();
            Json(json!({
                "allowlist": list,
                "hardcoded_count": 70,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn add_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    let process_name = match body.get("process_name").and_then(|v| v.as_str()) {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "process_name is required" })),
            );
        }
    };
    let notes = body.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());
    let added_by = body.get("added_by").and_then(|v| v.as_str()).unwrap_or("staff").to_string();

    // UX guard: check if it matches the well-known baseline
    let lower = process_name.to_lowercase();
    for baseline in BASELINE_PROCESSES {
        if lower == *baseline {
            return (
                axum::http::StatusCode::OK,
                Json(json!({
                    "status": "already_in_baseline",
                    "message": format!(
                        "'{}' is already in the hardcoded baseline allowlist — no action needed",
                        process_name
                    ),
                })),
            );
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO kiosk_allowlist (id, process_name, added_by, notes)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&process_name)
    .bind(&added_by)
    .bind(&notes)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => {
            // UNIQUE constraint — already exists
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "status": "already_exists",
                    "message": format!("'{}' is already in the staff allowlist", process_name),
                })),
            )
        }
        Ok(_) => (
            axum::http::StatusCode::CREATED,
            Json(json!({ "id": id, "process_name": process_name })),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub(crate) async fn delete_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::http::StatusCode {
    match sqlx::query(
        "DELETE FROM kiosk_allowlist WHERE LOWER(process_name) = LOWER(?)",
    )
    .bind(&name)
    .execute(&state.db)
    .await
    {
        Ok(_) => axum::http::StatusCode::NO_CONTENT,
        Err(e) => {
            tracing::error!("delete_kiosk_allowlist_entry error for '{}': {}", name, e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ─── Phase 335: Spectator Circuit Viewer Endpoints ───────────────────────────

/// GET /api/v1/spectator/tracks — list all available track outlines
pub(crate) async fn spectator_list_tracks(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let outlines = state.track_outlines.list();
    let summary: Vec<Value> = outlines
        .iter()
        .map(|o| {
            json!({
                "track_id": o.track_id,
                "config": o.config,
                "point_count": o.points.len(),
                "original_point_count": o.original_point_count,
            })
        })
        .collect();
    Json(json!({ "tracks": summary, "count": summary.len() }))
}

/// GET /api/v1/spectator/track/{track_id} — get track outline (normalized points)
pub(crate) async fn spectator_get_track(
    State(state): State<Arc<AppState>>,
    Path(track_id): Path<String>,
    Query(params): Query<SpectatorTrackQuery>,
) -> impl IntoResponse {
    let config = params.config.as_deref().unwrap_or("");
    match state.track_outlines.get(&track_id, config) {
        Some(outline) => (StatusCode::OK, Json(json!(outline))),
        None => (StatusCode::NOT_FOUND, Json(json!({
            "error": "track_not_found",
            "track_id": track_id,
        }))),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct SpectatorTrackQuery {
    config: Option<String>,
}

/// GET /api/v1/spectator/positions — current car positions from live telemetry
pub(crate) async fn spectator_get_positions(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Snapshot pods + telemetry from in-memory state
    let pods = {
        let guard = state.pods.read().await;
        guard.clone()
    };

    let mut positions: Vec<Value> = Vec::new();

    // Build positions from pods that have active games with telemetry
    for (pod_id, pod) in &pods {
        // Only include pods that are in a session
        if pod.status != PodStatus::InSession {
            continue;
        }

        // Look up latest telemetry from the dashboard broadcast state
        // We'll collect what we can from pod info
        let driver_name = pod.current_driver.clone().unwrap_or_default();

        positions.push(json!({
            "pod_id": pod_id,
            "pod_number": pod.number,
            "driver_name": driver_name,
            "track": pod.current_game.as_ref().map(|g| format!("{:?}", g)).unwrap_or_default(),
            "sim_type": format!("{:?}", pod.sim_type),
            "status": format!("{:?}", pod.status),
        }));
    }

    Json(json!({
        "positions": positions,
        "count": positions.len(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
