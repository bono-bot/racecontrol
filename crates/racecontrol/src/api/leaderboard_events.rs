#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// Re-export from split submodules so existing `use super::leaderboard_events::*` keeps working
pub(crate) use super::events_public::*;
pub(crate) use super::championships_public::*;

// ─── Public Lap Telemetry (No Auth Required) ────────────────────────────────

pub(crate) async fn public_lap_telemetry(
    State(state): State<Arc<AppState>>,
    Path(lap_id): Path<String>,
) -> Json<Value> {
    // First verify lap exists and get metadata
    let lap = sqlx::query_as::<_, (String, String, String, i64, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms FROM laps WHERE id = ?",
    )
    .bind(&lap_id)
    .fetch_optional(&state.db)
    .await;

    let lap = match lap {
        Ok(Some(l)) => l,
        Ok(None) => return Json(json!({ "error": "Lap not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Phase 251: Fetch telemetry samples from telemetry.db if available, else fall back to main DB
    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
    let samples = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)>(
        "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm
         FROM telemetry_samples
         WHERE lap_id = ?
         ORDER BY offset_ms ASC",
    )
    .bind(&lap_id)
    .fetch_all(telem_pool)
    .await;

    match samples {
        Ok(rows) => {
            let data: Vec<Value> = rows.iter().map(|s| {
                json!({
                    "offset_ms": s.0,
                    "speed": s.1,
                    "throttle": s.2,
                    "brake": s.3,
                    "steering": s.4,
                    "gear": s.5,
                    "rpm": s.6,
                })
            }).collect();

            let sample_count = data.len();
            Json(json!({
                "lap_id": lap_id,
                "track": lap.0,
                "car": lap.1,
                "sim_type": lap.2,
                "lap_time_ms": lap.3,
                "sector1_ms": lap.4,
                "sector2_ms": lap.5,
                "sector3_ms": lap.6,
                "samples": data,
                "sample_count": sample_count,
            }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Public Session Summary ──────────────────────────────────────────────────

/// Public session summary — no auth required. Shows first name only (privacy).
/// Used for shareable session links.
pub(crate) async fn public_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Query session + driver name + pricing tier (no auth - public endpoint)
    let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, d.name, bs.status, bs.allocated_seconds, bs.driving_seconds,
                pt.name, bs.car, bs.track, bs.sim_type
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Extract first name only (privacy -- per user decision)
    let first_name = session.1.split_whitespace().next().unwrap_or("Racer");

    // Best lap from laps table (valid laps only)
    let best_lap: Option<(i64,)> = sqlx::query_as(
        "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let total_laps: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "driver_first_name": first_name,
        "status": session.2,
        "duration_seconds": session.4,
        "pricing_tier": session.5,
        "car": session.6,
        "track": session.7,
        "sim_type": session.8,
        "best_lap_ms": best_lap.map(|b| b.0),
        "total_laps": total_laps.map(|t| t.0).unwrap_or(0),
    }))
}
