//! Billing session summary handler — extracted from billing_views.rs for <500 line compliance.

use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

pub(crate) async fn billing_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get billing session info (including discount fields)
    let session = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get discount info
    let discount_info = sqlx::query_as::<_, (Option<i64>, Option<String>, Option<i64>, Option<String>)>(
        "SELECT discount_paise, coupon_id, original_price_paise, discount_reason FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get laps for this session
    let laps = sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? ORDER BY lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as u32;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Check personal best
    let track = laps.first().map(|l| l.7.as_str()).unwrap_or("");
    let car = laps.first().map(|l| l.8.as_str()).unwrap_or("");

    let pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
    )
    .bind(&session.1)
    .bind(track)
    .bind(car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let personal_best_broken = best_lap_ms.map(|b| pb.map(|p| b <= p.0).unwrap_or(true)).unwrap_or(false);

    // Check leaderboard position
    let leaderboard_position = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) + 1 FROM personal_bests WHERE track = ? AND car = ? AND best_lap_ms < ?",
        )
        .bind(track)
        .bind(car)
        .bind(best_lap_ms.unwrap_or(i64::MAX))
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
    } else {
        None
    };

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
            })
        })
        .collect();

    Json(json!({
        "summary": {
            "billing_session_id": session.0,
            "driver_id": session.1,
            "driver_name": session.2,
            "pod_id": session.3,
            "track": track,
            "car": car,
            "allocated_seconds": session.4,
            "driving_seconds": session.5,
            "status": session.6,
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "personal_best_broken": personal_best_broken,
            "leaderboard_position": leaderboard_position,
            "laps": laps_json,
            "discount_paise": discount_info.as_ref().and_then(|d| d.0),
            "coupon_id": discount_info.as_ref().and_then(|d| d.1.clone()),
            "original_price_paise": discount_info.as_ref().and_then(|d| d.2),
            "discount_reason": discount_info.as_ref().and_then(|d| d.3.clone()),
        }
    }))
}
