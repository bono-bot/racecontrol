//! Driver search, profile, and vehicle records — split from leaderboard_public.rs
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Vehicle Records (Public, No Auth) ────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct VehicleRecordsQuery {
    sim_type: Option<String>,
}

pub(crate) async fn public_vehicle_records(
    State(state): State<Arc<AppState>>,
    Path(car): Path<String>,
    Query(params): Query<VehicleRecordsQuery>,
) -> Json<Value> {
    let sim_type_filter = params.sim_type.as_deref().unwrap_or("");
    let sim_clause = if sim_type_filter.is_empty() { "" } else { "AND l.sim_type = ?" };

    let query_str = format!(
        "SELECT l.track, l.sim_type, MIN(l.lap_time_ms),
                (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                 FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                 WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                   AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                 ORDER BY l2.lap_time_ms ASC LIMIT 1)
         FROM laps l
         WHERE l.car = ? AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         {sim_clause}
         GROUP BY l.track, l.sim_type
         ORDER BY l.track"
    );

    let mut q = sqlx::query_as::<_, (String, String, i64, String)>(&query_str)
        .bind(&car);
    if !sim_type_filter.is_empty() {
        q = q.bind(sim_type_filter);
    }
    let records = q.fetch_all(&state.db).await;

    Json(json!({
        "car": car,
        "records": records.unwrap_or_default().iter().map(|r| json!({
            "track": r.0,
            "sim_type": r.1,
            "best_lap_ms": r.2,
            "best_lap_display": format!("{}:{:02}.{:03}", r.2 / 60000, (r.2 % 60000) / 1000, r.2 % 1000),
            "driver": r.3,
        })).collect::<Vec<_>>(),
    }))
}

// ─── Public Driver Search & Profile (No Auth Required) ────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DriverSearchQuery {
    name: String,
}

pub(crate) async fn public_drivers_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DriverSearchQuery>,
) -> Json<Value> {
    let results = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT id, CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END,
                total_laps, avatar_url
         FROM drivers
         WHERE name LIKE '%' || ? || '%' COLLATE NOCASE
            OR nickname LIKE '%' || ? || '%' COLLATE NOCASE
         LIMIT 20"
    )
    .bind(&params.name)
    .bind(&params.name)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "drivers": results.iter().map(|r| json!({
            "id": r.0,
            "display_name": r.1,
            "total_laps": r.2,
            "avatar_url": r.3,
        })).collect::<Vec<_>>(),
        "count": results.len(),
    }))
}

pub(crate) async fn public_driver_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    // Query 1: Driver stats (NO PII — no email, phone, wallet, billing)
    let driver = sqlx::query_as::<_, (String, i64, i64, Option<String>, String)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END,
                total_laps, total_time_ms, avatar_url, created_at
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let driver = match driver {
        Some(d) => d,
        None => return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Driver not found" })),
        )),
    };

    // Query 2: Personal bests
    let personal_bests = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT track, car, best_lap_ms, achieved_at
         FROM personal_bests WHERE driver_id = ?
         ORDER BY achieved_at DESC"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Query 3: Lap history (exclude suspect laps, sector 0 → null)
    let laps = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool, String)>(
        "SELECT track, car, lap_time_ms,
                CASE WHEN sector1_ms > 0 THEN sector1_ms ELSE NULL END,
                CASE WHEN sector2_ms > 0 THEN sector2_ms ELSE NULL END,
                CASE WHEN sector3_ms > 0 THEN sector3_ms ELSE NULL END,
                valid, created_at
         FROM laps
         WHERE driver_id = ? AND (suspect IS NULL OR suspect = 0)
         ORDER BY created_at DESC
         LIMIT 100"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "driver": {
            "display_name": driver.0,
            "total_laps": driver.1,
            "total_time_ms": driver.2,
            "avatar_url": driver.3,
            "member_since": driver.4,
            "class_badge": null,
        },
        "personal_bests": personal_bests.iter().map(|pb| json!({
            "track": pb.0,
            "car": pb.1,
            "best_lap_ms": pb.2,
            "best_lap_display": format!("{}:{:02}.{:03}", pb.2 / 60000, (pb.2 % 60000) / 1000, pb.2 % 1000),
            "achieved_at": pb.3,
        })).collect::<Vec<_>>(),
        "lap_history": laps.iter().map(|l| json!({
            "track": l.0,
            "car": l.1,
            "lap_time_ms": l.2,
            "lap_time_display": format!("{}:{:02}.{:03}", l.2 / 60000, (l.2 % 60000) / 1000, l.2 % 1000),
            "sector1_ms": l.3,
            "sector2_ms": l.4,
            "sector3_ms": l.5,
            "valid": l.6,
            "created_at": l.7,
        })).collect::<Vec<_>>(),
    })))
}
