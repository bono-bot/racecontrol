#![allow(unused_imports)]
use super::auth_staff::venue_authority_guard;
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

pub(crate) async fn list_ac_presets(State(state): State<Arc<AppState>>) -> Json<Value> {
    match ac_server::list_presets(&state).await {
        Ok(presets) => Json(json!({ "presets": presets })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn save_ac_preset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })),
    };

    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    match ac_server::save_preset(&state, &name, &config).await {
        Ok(id) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn get_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match ac_server::load_preset(&state, &id).await {
        Ok((name, config)) => Json(json!({ "id": id, "name": name, "config": config })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn update_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "ac_presets") {
        return rejection.into_response();
    }
    let name = body.get("name").and_then(|v| v.as_str());
    let config = body.get("config").and_then(|c| serde_json::from_value::<AcLanSessionConfig>(c.clone()).ok());

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(cfg) = &config {
        updates.push("config_json = ?");
        binds.push(serde_json::to_string(cfg).unwrap_or_default());
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE ac_presets SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Preset not found" })).into_response(),
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "ac_presets") {
        return rejection.into_response();
    }
    match ac_server::delete_preset(&state, &id).await {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn start_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    let pod_ids: Vec<String> = body
        .get("pod_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let ai_level = body.get("ai_level").and_then(|v| v.as_u64()).map(|v| v as u32);

    match ac_server::start_ac_server(&state, config, pod_ids, ai_level).await {
        Ok(session_id) => Json(json!({ "session_id": session_id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn stop_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let session_id = match body.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "session_id is required" })),
    };

    match ac_server::stop_ac_server(&state, session_id).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn active_ac_session(State(state): State<Arc<AppState>>) -> Json<Value> {
    let instances = state.ac_server.instances.read().await;
    let active: Vec<_> = instances
        .values()
        .filter(|i| matches!(i.status, AcServerStatus::Running | AcServerStatus::Starting))
        .map(|i| i.to_info())
        .collect();
    Json(json!({ "sessions": active }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct AcSessionsQuery {
    status: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn list_ac_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AcSessionsQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(1000).max(1);

    let rows = if let Some(status) = &params.status {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions WHERE status = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "preset_id": s.1, "status": s.2,
                        "pod_ids": s.3, "pid": s.4, "join_url": s.5,
                        "error_message": s.6, "started_at": s.7,
                        "ended_at": s.8, "created_at": s.9,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// AC session leaderboard, list_ac_tracks, and list_ac_cars are in game_ac_data.rs

// ─── Phase 334: Race Weekend Session Progression ──────────────────────────

/// POST /games/weekend — Create a multi-session weekend (practice + qualifying + race).
pub(crate) async fn create_weekend(
    State(state): State<Arc<AppState>>,
    Json(body): Json<weekend::CreateWeekendRequest>,
) -> Json<Value> {
    match weekend::create_weekend(&state, body).await {
        Ok(summary) => Json(json!({
            "status": "ok",
            "weekend_id": summary.weekend_id,
            "ac_session_id": summary.ac_session_id,
            "phase": summary.phase,
            "track": summary.track,
            "car_class": summary.car_class,
            "pod_ids": summary.pod_ids,
            "practice_minutes": summary.practice_minutes,
            "quali_minutes": summary.quali_minutes,
            "race_laps": summary.race_laps,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// GET /games/weekend — List all active (non-finished) weekends.
pub(crate) async fn list_active_weekends(State(state): State<Arc<AppState>>) -> Json<Value> {
    let weekends = weekend::list_active_weekends(&state).await;
    Json(json!({ "weekends": weekends }))
}

/// GET /games/weekend/{id}/status — Get status of a specific weekend.
pub(crate) async fn get_weekend_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match weekend::get_weekend_status(&state, &id).await {
        Some(status) => Json(json!({ "status": status })),
        None => Json(json!({ "error": "Weekend not found" })),
    }
}

/// POST /games/weekend/{id}/stop — Stop a weekend early.
pub(crate) async fn stop_weekend(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match weekend::stop_weekend(&state, &id).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// GROUP-02: Enable/disable continuous mode on an AC server session.
pub(crate) async fn ac_server_set_continuous(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let enabled = req.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);

    // Look up the group_session_id for this AC session
    let group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM group_sessions WHERE ac_session_id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match ac_server::set_continuous_mode(&state, &session_id, enabled, group_session_id).await {
        Ok(()) => {
            if enabled {
                // Spawn the continuous monitor
                let state_clone = state.clone();
                let sid = session_id.clone();
                tokio::spawn(async move {
                    ac_server::monitor_continuous_session(state_clone, sid).await;
                });
            }
            Json(json!({ "status": "ok", "continuous_mode": enabled }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GROUP-03: Retry a failed pod join — re-sends LaunchGame to the pod.
pub(crate) async fn ac_session_retry_pod(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let session_id = match req.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'session_id'" })),
    };
    let pod_id = match req.get("pod_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pod_id'" })),
    };

    match ac_server::retry_pod_join(&state, &session_id, &pod_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GROUP-04: Update track/car config on a continuous-mode session.
/// Takes effect on the next race restart.
pub(crate) async fn ac_session_update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let session_id = match req.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'session_id'" })),
    };
    let track = req.get("track").and_then(|v| v.as_str()).map(String::from);
    let track_config = req.get("track_config").and_then(|v| v.as_str()).map(String::from);
    let cars: Option<Vec<String>> = req.get("cars").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter().filter_map(|c| c.as_str().map(String::from)).collect()
        })
    });

    if track.is_none() && cars.is_none() {
        return Json(json!({ "error": "Must provide 'track' or 'cars' to update" }));
    }

    match ac_server::update_session_config(&state, &session_id, track, track_config, cars).await {
        Ok(()) => Json(json!({ "status": "ok", "message": "Config updated — takes effect on next race restart" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

