#![allow(unused_imports)]
use super::auth_staff::venue_authority_guard;
use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

pub(crate) async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT id, type, sim_type, track, status, started_at FROM sessions ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions.iter().map(|s| json!({
                "id": s.0, "type": s.1, "sim_type": s.2,
                "track": s.3, "status": s.4, "started_at": s.5,
            })).collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_sessions") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let session_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("practice");
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("monza");
    let car_class = body.get("car_class").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track, car_class, status, venue_id) VALUES (?, ?, ?, ?, ?, 'pending', ?)"
    )
    .bind(&id)
    .bind(session_type)
    .bind(sim_type)
    .bind(track)
    .bind(car_class)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "type": session_type, "track": track })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT bs.id, bs.driver_id, d.name as driver_name, bs.pod_id,
                bs.pricing_tier_id, pt.name as tier_name,
                bs.allocated_seconds, bs.driving_seconds, bs.status,
                COALESCE(bs.custom_price_paise, pt.price_paise) as price_paise,
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name as experience_name,
                bs.car, bs.track, bs.sim_type,
                bs.reservation_id, bs.wallet_txn_id,
                bs.wallet_debit_paise, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get laps count and best lap for this session
    let lap_stats = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT COUNT(*), MIN(CASE WHEN valid = 1 THEN lap_time_ms END)
         FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, None));

    Json(json!({
        "session": {
            "id": row.get::<String, _>("id"),
            "driver_id": row.get::<String, _>("driver_id"),
            "driver_name": row.get::<String, _>("driver_name"),
            "pod_id": row.get::<String, _>("pod_id"),
            "pricing_tier_id": row.get::<String, _>("pricing_tier_id"),
            "pricing_tier_name": row.get::<String, _>("tier_name"),
            "allocated_seconds": row.get::<i64, _>("allocated_seconds"),
            "driving_seconds": row.get::<i64, _>("driving_seconds"),
            "status": row.get::<String, _>("status"),
            "price_paise": row.get::<i64, _>("price_paise"),
            "started_at": row.get::<Option<String>, _>("started_at"),
            "ended_at": row.get::<Option<String>, _>("ended_at"),
            "experience_id": row.get::<Option<String>, _>("experience_id"),
            "experience_name": row.get::<Option<String>, _>("experience_name"),
            "car": row.get::<Option<String>, _>("car"),
            "track": row.get::<Option<String>, _>("track"),
            "sim_type": row.get::<Option<String>, _>("sim_type"),
            "reservation_id": row.get::<Option<String>, _>("reservation_id"),
            "wallet_txn_id": row.get::<Option<String>, _>("wallet_txn_id"),
            "wallet_debit_paise": row.get::<Option<i64>, _>("wallet_debit_paise"),
            "created_at": row.get::<String, _>("created_at"),
            "total_laps": lap_stats.0,
            "best_lap_ms": lap_stats.1,
        }
    }))
}

pub(crate) async fn list_laps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT id, driver_id, track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid FROM laps ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps.iter().map(|l| json!({
                "id": l.0, "driver_id": l.1, "track": l.2, "car": l.3,
                "lap_time_ms": l.4, "sector1_ms": l.5, "sector2_ms": l.6,
                "sector3_ms": l.7, "valid": l.8,
            })).collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn session_laps(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, i64, i64,
        Option<i64>, Option<i64>, Option<i64>, bool, String,
    )>(
        "SELECT l.id, l.driver_id, l.pod_id, l.track, l.car, l.lap_number, l.lap_time_ms,
                l.sector1_ms, l.sector2_ms, l.sector3_ms, l.valid, l.created_at
         FROM laps l
         WHERE l.session_id = ?
         ORDER BY l.lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "driver_id": l.1,
                        "pod_id": l.2,
                        "track": l.3,
                        "car": l.4,
                        "lap_number": l.5,
                        "lap_time_ms": l.6,
                        "sector1_ms": l.7,
                        "sector2_ms": l.8,
                        "sector3_ms": l.9,
                        "valid": l.10,
                        "created_at": l.11,
                    })
                })
                .collect();
            let count = list.len();
            Json(json!({ "session_id": id, "laps": list, "count": count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct StaffTrackLeaderboardQuery {
    sim_type: Option<String>,
    /// Filter by car class — UX-05 segmentation (staff view)
    car_class: Option<String>,
    /// Filter by assist tier — UX-05 segmentation (staff view)
    assist_tier: Option<String>,
}

pub(crate) async fn track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<StaffTrackLeaderboardQuery>,
) -> Json<Value> {
    // UX-04: JOIN laps to apply billing_session_id IS NOT NULL filter
    // UX-05: car_class + assist_tier segmentation for staff consistency with public view
    // UX-07: validity = 'valid' enforced — staff never see unverifiable laps
    let sim_clause = if params.sim_type.is_some() { " AND tr.sim_type = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { " AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { " AND l.assist_tier = ?" } else { "" };

    let query_str = format!(
        "SELECT tr.track, tr.car,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                tr.best_lap_ms, tr.achieved_at, tr.lap_id, tr.sim_type
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         LEFT JOIN laps l ON l.id = tr.lap_id
         WHERE tr.track = ?
           AND (l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)
           AND (l.validity IS NULL OR l.validity = 'valid')
           {}{}{}
         ORDER BY tr.best_lap_ms ASC",
        sim_clause, car_class_clause, assist_tier_clause
    );

    let mut q = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, String)>(&query_str);
    q = q.bind(&track);
    if let Some(ref st) = params.sim_type { q = q.bind(st); }
    if let Some(ref cc) = params.car_class { q = q.bind(cc); }
    if let Some(ref at) = params.assist_tier { q = q.bind(at); }
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(records) => {
            let list: Vec<Value> = records.iter().enumerate().map(|(i, r)| json!({
                "position": i + 1,
                "track": r.0, "car": r.1, "driver": r.2,
                "best_lap_ms": r.3, "achieved_at": r.4, "lap_id": r.5,
                "sim_type": r.6,
            })).collect();
            Json(json!({
                "track": track,
                "sim_type": params.sim_type,
                "car_class": params.car_class,
                "assist_tier": params.assist_tier,
                "records": list,
                "last_updated": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn list_events(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "events": [] }))
}

pub(crate) async fn create_event(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_event" }))
}

pub(crate) async fn list_bookings(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "bookings": [] }))
}

pub(crate) async fn create_booking(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_booking" }))
}
