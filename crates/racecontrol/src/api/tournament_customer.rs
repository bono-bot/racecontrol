#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Customer Tournament Endpoints ──────────────────────────────────────────

pub(crate) async fn customer_list_tournaments(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants,
                entry_fee_paise, prize_pool_paise, status, event_date
         FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC",
    )
    .fetch_all(&state.db)
    .await;

    let tournaments = match rows {
        Ok(t) => t,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Check which the driver is registered for
    let registered: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT tournament_id FROM tournament_registrations WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    let list: Vec<Value> = tournaments.iter().map(|t| {
        json!({
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6,
            "entry_fee_display": if t.7 > 0 { format!("Rs.{}", t.7 / 100) } else { "Free".to_string() },
            "prize_pool_display": if t.8 > 0 { format!("Rs.{}", t.8 / 100) } else { "TBD".to_string() },
            "status": t.9, "event_date": t.10,
            "is_registered": registered.contains(&t.0),
        })
    }).collect();

    Json(json!({ "tournaments": list }))
}

pub(crate) async fn customer_register_tournament(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check tournament exists and is open
    let status: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, max_participants FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match &status {
        Some((s, _)) if s != "registration" && s != "upcoming" => {
            return Json(json!({ "error": "Registration is not open" }));
        }
        None => return Json(json!({ "error": "Tournament not found" })),
        _ => {}
    }

    let max = status.unwrap().1;

    // Check capacity
    let count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    if count >= max {
        return Json(json!({ "error": "Tournament is full" }));
    }

    let reg_id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO tournament_registrations (id, tournament_id, driver_id, venue_id) VALUES (?, ?, ?, ?)",
    )
    .bind(&reg_id)
    .bind(&id)
    .bind(&driver_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "registration_id": reg_id })),
        Err(e) if e.to_string().contains("UNIQUE") => {
            Json(json!({ "error": "Already registered" }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
