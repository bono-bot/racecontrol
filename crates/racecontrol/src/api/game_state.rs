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

pub(crate) async fn active_games(State(state): State<Arc<AppState>>) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    let list: Vec<_> = games.values().map(|g| g.to_info()).collect();
    Json(json!({ "games": list }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RecentTimelineQuery {
    limit: Option<i64>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct GameHistoryQuery {
    pod_id: Option<String>,
    limit: Option<i64>,
    /// Pattern H: when true, excludes events where clean_exit_heuristic=1
    /// (customers closing the game cleanly rather than crashes).
    real_crashes_only: Option<bool>,
}

pub(crate) async fn game_launch_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GameHistoryQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100).min(1000).max(1);

    // Pattern H: when real_crashes_only=true, filter out clean-exit-heuristic rows.
    // Default (false) returns the raw event stream so dashboards can show BOTH
    // "all events" count and "real crashes only" count.
    let real_crashes_only = params.real_crashes_only.unwrap_or(false);
    let clean_filter = if real_crashes_only {
        " AND clean_exit_heuristic = 0"
    } else {
        ""
    };

    let rows = if let Some(pod_id) = &params.pod_id {
        let sql = format!(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at, \
             COALESCE(clean_exit_heuristic, 0) \
             FROM game_launch_events WHERE pod_id = ?{clean_filter} ORDER BY created_at DESC LIMIT ?",
        );
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String, i64)>(&sql)
            .bind(pod_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await
    } else {
        // Note: without pod_id, `AND clean_filter` won't parse — use `WHERE 1=1` prefix
        let sql = format!(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at, \
             COALESCE(clean_exit_heuristic, 0) \
             FROM game_launch_events WHERE 1=1{clean_filter} ORDER BY created_at DESC LIMIT ?",
        );
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String, i64)>(&sql)
            .bind(limit)
            .fetch_all(&state.db)
            .await
    };

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "pod_id": e.1, "sim_type": e.2,
                        "event_type": e.3, "pid": e.4,
                        "error_message": e.5, "created_at": e.6,
                        "clean_exit_heuristic": e.7 != 0,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Phase 319-02 (DASH-03): GET /api/v1/launch-timeline/recent?limit=50
/// Returns a list of recent launch timeline summaries (without events_json) for the dashboard list view.
/// Default limit=50, max=200, min=1. Returns [] for empty table.
/// Route must be registered BEFORE /launch-timeline/:launch_id to prevent Axum treating "recent" as a param.
pub(crate) async fn get_recent_launch_timelines(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RecentTimelineQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(200).max(1);
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, i64, String)>(
        "SELECT launch_id, pod_id, sim_type, preset_id, outcome, total_duration_ms, started_at
         FROM launch_timeline_spans
         ORDER BY started_at DESC
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let list: Vec<Value> = rows.iter().map(|r| json!({
        "launch_id": r.0,
        "pod_id": r.1,
        "sim_type": r.2,
        "preset_id": r.3,
        "outcome": r.4,
        "total_duration_ms": r.5,
        "started_at": r.6,
    })).collect();
    Json(json!(list))
}

/// Phase 318 (LAUNCH-05): GET /api/v1/launch-timeline/:launch_id
/// Returns the step-level launch timeline for a specific launch attempt.
/// Returns { launch_id, events: [] } for unknown launch_ids (never 404 — safe for dashboard polling).
pub(crate) async fn get_launch_timeline(
    State(state): State<Arc<AppState>>,
    Path(launch_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, String, i64, String, String, String)>(
        "SELECT launch_id, pod_id, sim_type, preset_id, billing_session_id,
                outcome, total_duration_ms, started_at, events_json, created_at
         FROM launch_timeline_spans WHERE launch_id = ?"
    )
    .bind(&launch_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(r)) => {
            let events: Value = serde_json::from_str(&r.8).unwrap_or(Value::Array(vec![]));
            Json(json!({
                "launch_id": r.0,
                "pod_id": r.1,
                "sim_type": r.2,
                "preset_id": r.3,
                "billing_session_id": r.4,
                "outcome": r.5,
                "total_duration_ms": r.6,
                "started_at": r.7,
                "events": events,
                "created_at": r.9,
            }))
        }
        Ok(None) => Json(json!({ "launch_id": launch_id, "events": [] })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn pod_game_state(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    match games.get(&pod_id) {
        Some(tracker) => Json(json!({ "game": tracker.to_info() })),
        None => Json(json!({ "game": null, "state": "idle" })),
    }
}
