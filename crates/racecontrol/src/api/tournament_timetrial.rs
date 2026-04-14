#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Time Trial Admin ────────────────────────────────────────────────────────

pub(crate) async fn list_time_trials(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, track, car, week_start, week_end, is_active
         FROM time_trials ORDER BY week_start DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(trials) => {
            let list: Vec<Value> = trials.iter().map(|t| json!({
                "id": t.0, "track": t.1, "car": t.2,
                "week_start": t.3, "week_end": t.4, "is_active": t.5,
            })).collect();
            Json(json!({ "time_trials": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_time_trial(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "track required" })),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "car required" })),
    };
    let week_start = match body.get("week_start").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return Json(json!({ "error": "week_start required (YYYY-MM-DD)" })),
    };
    let week_end = match body.get("week_end").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return Json(json!({ "error": "week_end required (YYYY-MM-DD)" })),
    };

    let result = sqlx::query(
        "INSERT INTO time_trials (id, track, car, week_start, week_end) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(track)
    .bind(car)
    .bind(week_start)
    .bind(week_end)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn update_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE time_trials SET
            track = COALESCE(?, track), car = COALESCE(?, car),
            week_start = COALESCE(?, week_start), week_end = COALESCE(?, week_end),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("week_start").and_then(|v| v.as_str()))
    .bind(body.get("week_end").and_then(|v| v.as_str()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn delete_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM time_trials WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}
