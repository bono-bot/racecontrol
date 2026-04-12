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

pub(crate) async fn customer_friends(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friends(&state, &driver_id).await {
        Ok(list) => Json(json!({ "friends": list })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_friend_requests(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friend_requests(&state, &driver_id).await {
        Ok((incoming, outgoing)) => Json(json!({
            "incoming": incoming,
            "outgoing": outgoing,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_send_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let identifier = match req.get("identifier").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'identifier' (phone or customer_id)" })),
    };

    match friends::send_friend_request(&state, &driver_id, &identifier).await {
        Ok(request_id) => Json(json!({ "status": "ok", "request_id": request_id })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_accept_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::accept_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_reject_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::reject_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_remove_friend(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(friend_driver_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::remove_friend(&state, &driver_id, &friend_driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_set_presence(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let presence = match req.get("presence").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(json!({ "error": "Missing 'presence' (online/hidden)" })),
    };

    match friends::set_presence(&state, &driver_id, &presence).await {
        Ok(()) => Json(json!({ "status": "ok", "presence": presence })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Multiplayer ──────────────────────────────────────────────────────────

pub(crate) async fn customer_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str()).map(String::from);

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let friend_ids: Vec<String> = match req.get("friend_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'friend_ids' array" })),
    };

    if friend_ids.is_empty() {
        return Json(json!({ "error": "Need at least one friend for multiplayer" }));
    }

    // Accept optional custom booking payload (same as single-player custom booking)
    let custom = req.get("custom").and_then(|v| {
        let game = v.get("game")?.as_str()?.to_string();
        let track = v.get("track")?.as_str()?.to_string();
        let car = v.get("car")?.as_str()?.to_string();
        Some((game, track, car))
    });

    if experience_id.is_none() && custom.is_none() {
        return Json(json!({ "error": "Must provide 'experience_id' or 'custom' booking payload" }));
    }

    match multiplayer::book_multiplayer(&state, &driver_id, experience_id.as_deref(), &pricing_tier_id, friend_ids, custom).await {
        Ok(info) => Json(json!({ "status": "ok", "group_session": info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_group_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::get_active_group_session(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "group_session": info })),
        Ok(None) => Json(json!({ "group_session": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_accept_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::accept_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(member) => Json(json!({ "status": "ok", "member": member })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_decline_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::decline_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Telemetry ────────────────────────────────────────────────────────────

pub(crate) async fn customer_telemetry(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Find the pod this driver is actively billing on
    let active_pod: Option<(String,)> = sqlx::query_as(
        "SELECT pod_id FROM billing_sessions WHERE driver_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let pod_id = match active_pod {
        Some((pid,)) => pid,
        None => return Json(json!({ "error": "No active session" })),
    };

    // Get latest telemetry sample for this pod
    let sample: Option<(String, String)> = sqlx::query_as(
        "SELECT data, sampled_at FROM telemetry_samples WHERE pod_id = ? ORDER BY sampled_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match sample {
        Some((data, sampled_at)) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                Json(json!({
                    "pod_id": pod_id,
                    "telemetry": parsed,
                    "sampled_at": sampled_at,
                }))
            } else {
                Json(json!({ "pod_id": pod_id, "telemetry": data, "sampled_at": sampled_at }))
            }
        }
        None => Json(json!({ "pod_id": pod_id, "telemetry": null })),
    }
}
