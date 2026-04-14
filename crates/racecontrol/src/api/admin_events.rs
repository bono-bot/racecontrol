#![allow(unused_imports)]
use super::terminal_handlers::check_terminal_auth;
use super::auth_staff::venue_authority_guard;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Staff: Hotlap Events ─────────────────────────────────────────────────────

/// POST /staff/events — create a new hotlap event
pub(crate) async fn create_hotlap_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "hotlap_events") {
        return rejection.into_response();
    }
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e })).into_response();
    }

    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })).into_response(),
    };
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return Json(json!({ "error": "track is required" })).into_response(),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car is required" })).into_response(),
    };
    let car_class = match body.get("car_class").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car_class is required" })).into_response(),
    };
    let sim_type = body
        .get("sim_type")
        .and_then(|v| v.as_str())
        .unwrap_or("assetto_corsa")
        .to_string();
    let description: Option<String> = body
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let starts_at: Option<String> = body
        .get("starts_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let ends_at: Option<String> = body
        .get("ends_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let reference_time_ms: Option<i64> = body
        .get("reference_time_ms")
        .and_then(|v| v.as_i64());
    let rule_107_percent: i64 = body
        .get("rule_107_percent")
        .and_then(|v| v.as_bool())
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(1);

    let result = sqlx::query(
        "INSERT INTO hotlap_events
            (id, name, description, track, car, car_class, sim_type, status,
             starts_at, ends_at, reference_time_ms, rule_107_percent, created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'upcoming', ?, ?, ?, ?, datetime('now'), datetime('now'), ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&track)
    .bind(&car)
    .bind(&car_class)
    .bind(&sim_type)
    .bind(&starts_at)
    .bind(&ends_at)
    .bind(reference_time_ms)
    .bind(rule_107_percent)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Hotlap event created: {} ({})", id, name);
            Json(json!({ "id": id, "status": "created" })).into_response()
        }
        Err(e) => Json(json!({ "error": format!("Failed to create event: {}", e) })).into_response(),
    }
}

/// GET /staff/events — list all hotlap events
pub(crate) async fn list_staff_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, rule_107_percent,
                championship_id, created_at, updated_at
         FROM hotlap_events ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let events: Vec<Value> = rows
                .iter()
                .map(|r| {
                    use sqlx::Row;
                    json!({
                        "id": r.try_get::<String, _>("id").unwrap_or_default(),
                        "name": r.try_get::<String, _>("name").unwrap_or_default(),
                        "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                        "track": r.try_get::<String, _>("track").unwrap_or_default(),
                        "car": r.try_get::<String, _>("car").unwrap_or_default(),
                        "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                        "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                        "status": r.try_get::<String, _>("status").unwrap_or_default(),
                        "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                        "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                        "reference_time_ms": r.try_get::<Option<i64>, _>("reference_time_ms").unwrap_or(None),
                        "rule_107_percent": r.try_get::<i64, _>("rule_107_percent").unwrap_or(1),
                        "championship_id": r.try_get::<Option<String>, _>("championship_id").unwrap_or(None),
                        "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                        "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
                    })
                })
                .collect();
            Json(json!({ "events": events }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to list events: {}", e) })),
    }
}

/// GET /staff/events/{id} — get a single hotlap event
pub(crate) async fn get_staff_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let row = sqlx::query(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, rule_107_percent,
                championship_id, created_at, updated_at
         FROM hotlap_events WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(r)) => {
            use sqlx::Row;
            Json(json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "name": r.try_get::<String, _>("name").unwrap_or_default(),
                "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                "track": r.try_get::<String, _>("track").unwrap_or_default(),
                "car": r.try_get::<String, _>("car").unwrap_or_default(),
                "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                "status": r.try_get::<String, _>("status").unwrap_or_default(),
                "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                "reference_time_ms": r.try_get::<Option<i64>, _>("reference_time_ms").unwrap_or(None),
                "rule_107_percent": r.try_get::<i64, _>("rule_107_percent").unwrap_or(1),
                "championship_id": r.try_get::<Option<String>, _>("championship_id").unwrap_or(None),
                "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
            }))
        }
        Ok(None) => Json(json!({ "error": "Event not found" })),
        Err(e) => Json(json!({ "error": format!("Database error: {}", e) })),
    }
}

/// PUT /staff/events/{id} — update a hotlap event
/// Uses COALESCE so only provided fields are changed; omitted fields keep existing values.
pub(crate) async fn update_hotlap_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "hotlap_events") {
        return rejection.into_response();
    }
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e })).into_response();
    }

    let status: Option<String> = body.get("status").and_then(|v| v.as_str()).map(|s| s.to_string());
    let name: Option<String> = body.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let description: Option<String> = body.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
    let starts_at: Option<String> = body.get("starts_at").and_then(|v| v.as_str()).map(|s| s.to_string());
    let ends_at: Option<String> = body.get("ends_at").and_then(|v| v.as_str()).map(|s| s.to_string());
    let reference_time_ms: Option<i64> = body.get("reference_time_ms").and_then(|v| v.as_i64());

    if status.is_none() && name.is_none() && description.is_none()
        && starts_at.is_none() && ends_at.is_none() && reference_time_ms.is_none()
    {
        return Json(json!({ "error": "No updatable fields provided" })).into_response();
    }

    let result = sqlx::query(
        "UPDATE hotlap_events SET
            status = COALESCE(?, status),
            name = COALESCE(?, name),
            description = COALESCE(?, description),
            starts_at = COALESCE(?, starts_at),
            ends_at = COALESCE(?, ends_at),
            reference_time_ms = COALESCE(?, reference_time_ms),
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(name)
    .bind(description)
    .bind(starts_at)
    .bind(ends_at)
    .bind(reference_time_ms)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Event not found" })).into_response(),
        Ok(_) => Json(json!({ "status": "updated" })).into_response(),
        Err(e) => Json(json!({ "error": format!("Failed to update event: {}", e) })).into_response(),
    }
}
