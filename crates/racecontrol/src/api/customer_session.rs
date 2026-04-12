#![allow(unused_imports)]
use super::customer_auth::{extract_driver_id, compute_percentile};
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

pub(crate) async fn customer_session_detail(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch the billing session, ensuring it belongs to this customer
    let row = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<i64>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name,
                bs.car, bs.track, bs.sim_type,
                bs.wallet_debit_paise
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Fetch discount info separately (avoids sqlx 16-field tuple limit)
    let discount_info = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<String>)>(
        "SELECT discount_paise, original_price_paise, discount_reason FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Look up any refund for this session
    let refund_paise: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM wallet_transactions
         WHERE reference_id = ? AND txn_type IN ('refund_session', 'refund_manual')",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get all laps for this session
    let laps = sqlx::query_as::<_, (
        String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String, String,
    )>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms,
                valid, track, car, created_at
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as i64;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Determine track and car from laps or session fields
    let track = laps.first().map(|l| l.7.clone()).unwrap_or_else(|| session.12.clone().unwrap_or_default());
    let car = laps.first().map(|l| l.8.clone()).unwrap_or_else(|| session.11.clone().unwrap_or_default());

    // Percentile ranking (shared function, >= 5 driver threshold)
    let percentile = if let Some(best) = best_lap_ms {
        compute_percentile(&state.db, best, &track, &car).await
    } else {
        None
    };

    // Personal best for this track+car
    let personal_best: Option<(i64,)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
        )
        .bind(&driver_id)
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // is_new_pb: true if this session's best lap IS the current personal best
    let is_new_pb = personal_best.map(|pb| best_lap_ms == Some(pb.0)).unwrap_or(false);

    // improvement_ms: how much faster this session's best was vs the previous PB
    // Only meaningful if is_new_pb; look for a second-best time (prior PB) excluding this session
    let improvement_ms: Option<i64> = if is_new_pb {
        if let Some(best) = best_lap_ms {
            let prev: Option<(i64,)> = sqlx::query_as(
                "SELECT MIN(lap_time_ms) FROM laps
                 WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
                 AND lap_time_ms > ? AND session_id != ?",
            )
            .bind(&driver_id)
            .bind(&track)
            .bind(&car)
            .bind(best)
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
            prev.map(|p| p.0 - best)
        } else {
            None
        }
    } else {
        None
    };

    // Peak moment: lap number of the best lap in this session
    let peak_lap_number = valid_laps.iter().min_by_key(|l| l.2).map(|l| l.1);

    // group_session_id for this billing session
    let group_session_id_val: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "id": l.0,
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
                "track": l.7,
                "car": l.8,
                "created_at": l.9,
            })
        })
        .collect();

    // Fetch billing events timeline for this session
    let events = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let events_json: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "id": e.0,
                "event_type": e.1,
                "driving_seconds_at_event": e.2,
                "metadata": e.3,
                "created_at": e.4,
            })
        })
        .collect();

    Json(json!({
        "session": {
            "id": session.0,
            "pod_id": session.1,
            "pricing_tier_name": session.2,
            "allocated_seconds": session.3,
            "driving_seconds": session.4,
            "status": session.5,
            "price_paise": session.6,
            "started_at": session.7,
            "ended_at": session.8,
            "experience_id": session.9,
            "experience_name": session.10,
            "car": session.11,
            "track": session.12,
            "sim_type": session.13,
            "wallet_debit_paise": session.14,
            "discount_paise": discount_info.as_ref().and_then(|d| d.0),
            "original_price_paise": discount_info.as_ref().and_then(|d| d.1),
            "discount_reason": discount_info.as_ref().and_then(|d| d.2.clone()),
            "refund_paise": refund_paise.map(|r| r.0).filter(|&r| r > 0),
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "percentile_rank": percentile,
            "percentile_text": percentile.map(|p| format!("Faster than {}% of drivers", p)),
            "is_new_pb": is_new_pb,
            "personal_best_ms": personal_best.map(|pb| pb.0),
            "improvement_ms": improvement_ms,
            "peak_lap_number": peak_lap_number,
            "group_session_id": group_session_id_val,
        },
        "laps": laps_json,
        "events": events_json,
    }))
}

/// Polling endpoint for active session PB events.
/// Returns PB events since a given timestamp for the customer's active billing session.
/// PWA calls this every 5 seconds during active sessions.
pub(crate) async fn customer_active_session_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Find active billing session for this driver
    let active_session: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM billing_sessions WHERE driver_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let session_id = match active_session {
        Some((id,)) => id,
        None => return Json(json!({ "events": [] })),
    };

    let since = params.get("since").cloned().unwrap_or_default();

    // Query laps that are PBs since the given timestamp
    let pb_laps = sqlx::query_as::<_, (String, i64, String, String, String)>(
        "SELECT l.id, l.lap_time_ms, l.track, l.car, l.created_at
         FROM laps l
         JOIN personal_bests pb ON l.id = pb.lap_id
         WHERE l.session_id = ? AND l.driver_id = ? AND l.created_at > ?
         ORDER BY l.created_at ASC",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&since)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "events": pb_laps.iter().map(|l| json!({
            "type": "pb",
            "lap_id": l.0,
            "lap_time_ms": l.1,
            "track": l.2,
            "car": l.3,
            "at": l.4,
        })).collect::<Vec<_>>()
    }))
}

// ─── Remote Booking Reservation Handlers ─────────────────────────────────────

pub(crate) async fn customer_create_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<reservation::CreateReservationRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::create_reservation(&state, &driver_id, &req).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_get_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::get_active_reservation(&state, &driver_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_cancel_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::cancel_reservation(&state, &driver_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_modify_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<reservation::CreateReservationRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::modify_reservation(&state, &driver_id, &req).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool, String)>(
        "SELECT id, track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at
         FROM laps
         WHERE driver_id = ?
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "track": l.1,
                        "car": l.2,
                        "sim_type": l.3,
                        "lap_time_ms": l.4,
                        "sector1_ms": l.5,
                        "sector2_ms": l.6,
                        "sector3_ms": l.7,
                        "valid": l.8,
                        "created_at": l.9,
                    })
                })
                .collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn customer_stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Total laps and time
    let totals = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(total_laps, 0), COALESCE(total_time_ms, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, 0));

    // Total sessions
    let session_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Total driving time (seconds)
    let total_driving_secs = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(driving_seconds), 0) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Favourite car (most laps)
    let fav_car = sqlx::query_as::<_, (String, i64)>(
        "SELECT car, COUNT(*) as cnt FROM laps WHERE driver_id = ? GROUP BY car ORDER BY cnt DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Personal bests count
    let pb_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "stats": {
            "total_laps": totals.0,
            "total_time_ms": totals.1,
            "total_sessions": session_count,
            "total_driving_seconds": total_driving_secs,
            "favourite_car": fav_car.as_ref().map(|c| &c.0),
            "personal_bests": pb_count,
        }
    }))
}
