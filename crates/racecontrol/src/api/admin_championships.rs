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

// ─── Staff: Championships ─────────────────────────────────────────────────────

/// POST /staff/championships — create a new championship
pub(crate) async fn create_championship(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "championships") {
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
    let car_class = match body.get("car_class").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car_class is required" })).into_response(),
    };
    let description: Option<String> = body
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let sim_type = body
        .get("sim_type")
        .and_then(|v| v.as_str())
        .unwrap_or("assetto_corsa")
        .to_string();
    let season: Option<String> = body
        .get("season")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO championships
            (id, name, description, car_class, sim_type, season,
             status, scoring_system, total_rounds, completed_rounds,
             created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, 'upcoming', 'f1_2010', 0, 0, datetime('now'), datetime('now'), ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&car_class)
    .bind(&sim_type)
    .bind(&season)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Championship created: {} ({})", id, name);
            Json(json!({ "id": id, "status": "created" })).into_response()
        }
        Err(e) => Json(json!({ "error": format!("Failed to create championship: {}", e) })).into_response(),
    }
}

/// GET /staff/championships — list all championships
pub(crate) async fn list_staff_championships(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query(
        "SELECT id, name, description, car_class, sim_type, season,
                status, scoring_system, total_rounds, completed_rounds,
                created_at, updated_at
         FROM championships ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let championships: Vec<Value> = rows
                .iter()
                .map(|r| {
                    use sqlx::Row;
                    json!({
                        "id": r.try_get::<String, _>("id").unwrap_or_default(),
                        "name": r.try_get::<String, _>("name").unwrap_or_default(),
                        "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                        "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                        "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                        "season": r.try_get::<Option<String>, _>("season").unwrap_or(None),
                        "status": r.try_get::<String, _>("status").unwrap_or_default(),
                        "scoring_system": r.try_get::<String, _>("scoring_system").unwrap_or_default(),
                        "total_rounds": r.try_get::<i64, _>("total_rounds").unwrap_or(0),
                        "completed_rounds": r.try_get::<i64, _>("completed_rounds").unwrap_or(0),
                        "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                        "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
                    })
                })
                .collect();
            Json(json!({ "championships": championships }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to list championships: {}", e) })),
    }
}

/// GET /staff/championships/{id} — get a championship with its rounds
pub(crate) async fn get_staff_championship(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let champ_row = sqlx::query(
        "SELECT id, name, description, car_class, sim_type, season,
                status, scoring_system, total_rounds, completed_rounds,
                created_at, updated_at
         FROM championships WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let championship = match champ_row {
        Ok(Some(r)) => {
            use sqlx::Row;
            json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "name": r.try_get::<String, _>("name").unwrap_or_default(),
                "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                "season": r.try_get::<Option<String>, _>("season").unwrap_or(None),
                "status": r.try_get::<String, _>("status").unwrap_or_default(),
                "scoring_system": r.try_get::<String, _>("scoring_system").unwrap_or_default(),
                "total_rounds": r.try_get::<i64, _>("total_rounds").unwrap_or(0),
                "completed_rounds": r.try_get::<i64, _>("completed_rounds").unwrap_or(0),
                "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
            })
        }
        Ok(None) => return Json(json!({ "error": "Championship not found" })),
        Err(e) => return Json(json!({ "error": format!("Database error: {}", e) })),
    };

    let rounds_rows = sqlx::query(
        "SELECT cr.round_number, cr.event_id,
                he.name AS event_name, he.track, he.car_class, he.status AS event_status,
                he.starts_at, he.ends_at
         FROM championship_rounds cr
         JOIN hotlap_events he ON he.id = cr.event_id
         WHERE cr.championship_id = ?
         ORDER BY cr.round_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    let rounds: Vec<Value> = match rounds_rows {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                use sqlx::Row;
                json!({
                    "round_number": r.try_get::<i64, _>("round_number").unwrap_or(0),
                    "event_id": r.try_get::<String, _>("event_id").unwrap_or_default(),
                    "event_name": r.try_get::<String, _>("event_name").unwrap_or_default(),
                    "track": r.try_get::<String, _>("track").unwrap_or_default(),
                    "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                    "event_status": r.try_get::<String, _>("event_status").unwrap_or_default(),
                    "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                    "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                })
            })
            .collect(),
        Err(_) => vec![],
    };

    Json(json!({ "championship": championship, "rounds": rounds }))
}

/// POST /staff/championships/{id}/rounds — add a round to a championship
pub(crate) async fn add_championship_round(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(championship_id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "championships") {
        return rejection.into_response();
    }
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e })).into_response();
    }

    let event_id = match body.get("event_id").and_then(|v| v.as_str()) {
        Some(e) => e.to_string(),
        None => return Json(json!({ "error": "event_id is required" })).into_response(),
    };
    let round_number = match body.get("round_number").and_then(|v| v.as_i64()) {
        Some(n) => n,
        None => return Json(json!({ "error": "round_number is required" })).into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO championship_rounds (championship_id, event_id, round_number, venue_id)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&championship_id)
    .bind(&event_id)
    .bind(round_number)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Json(json!({ "error": format!("Failed to add round: {}", e) })).into_response();
    }

    // Link event back to championship
    let _ = sqlx::query(
        "UPDATE hotlap_events SET championship_id = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&championship_id)
    .bind(&event_id)
    .execute(&state.db)
    .await;

    // Increment total_rounds on championship
    let _ = sqlx::query(
        "UPDATE championships SET total_rounds = total_rounds + 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&championship_id)
    .execute(&state.db)
    .await;

    tracing::info!(
        "Championship round added: {} round {} = event {}",
        championship_id, round_number, event_id
    );
    Json(json!({ "status": "round_added" })).into_response()
}

/// POST /staff/group-sessions/{id}/complete — mark a group session completed and score the linked event
pub(crate) async fn complete_group_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    // Fetch group session and its hotlap_event_id
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, hotlap_event_id FROM group_sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let hotlap_event_id = match row {
        None => return Json(json!({ "error": "Group session not found" })),
        Some((_, None)) => {
            return Json(json!({
                "error": "Group session not linked to an event. Use POST /staff/events/{id}/link-session first."
            }));
        }
        Some((_, Some(event_id))) => event_id,
    };

    // Mark session as completed
    let result = sqlx::query(
        "UPDATE group_sessions SET status = 'completed', completed_at = datetime('now') WHERE id = ?",
    )
    .bind(&session_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Json(json!({ "error": format!("Failed to complete session: {e}") }));
    }

    // Score the event from multiplayer_results
    if let Err(e) = crate::lap_tracker::score_group_event(&state.db, &session_id, &hotlap_event_id, &state.config.venue.venue_id).await {
        return Json(json!({ "error": format!("Session marked complete but scoring failed: {e}") }));
    }

    Json(json!({
        "status": "completed",
        "scored_event": hotlap_event_id
    }))
}

/// POST /staff/events/{id}/link-session — link a group session to a hotlap event
pub(crate) async fn link_group_session_to_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(event_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let group_session_id = match body.get("group_session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "group_session_id is required" })),
    };

    let result = sqlx::query(
        "UPDATE group_sessions SET hotlap_event_id = ? WHERE id = ?",
    )
    .bind(&event_id)
    .bind(&group_session_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Group session not found" })),
        Ok(_) => Json(json!({ "status": "linked" })),
        Err(e) => Json(json!({ "error": format!("Failed to link session: {}", e) })),
    }
}
