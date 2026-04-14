//! Driver rating and rating history handlers — split from leaderboard_public.rs
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Driver Rating (Public, No Auth — Phase 253) ─────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DriverRatingQuery {
    sim_type: Option<String>,
}

pub(crate) async fn public_driver_rating(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<DriverRatingQuery>,
) -> Json<Value> {
    if let Some(ref sim_type) = params.sim_type {
        // Single sim_type
        let row = sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? AND sim_type = ?",
        )
        .bind(&driver_id)
        .bind(sim_type)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        match row {
            Some(r) => Json(json!({
                "driver_id": r.0,
                "sim_type": r.1,
                "composite_rating": r.2,
                "rating_class": r.3,
                "pace_score": r.4,
                "consistency_score": r.5,
                "experience_score": r.6,
                "total_laps": r.7,
                "updated_at": r.8,
            })),
            None => Json(json!({
                "driver_id": driver_id,
                "sim_type": sim_type,
                "composite_rating": null,
                "rating_class": "Unrated",
                "message": "No rating data available",
            })),
        }
    } else {
        // All sim_types for this driver
        let rows = sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? ORDER BY composite_rating DESC",
        )
        .bind(&driver_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        Json(json!({
            "driver_id": driver_id,
            "ratings": rows.iter().map(|r| json!({
                "sim_type": r.1,
                "composite_rating": r.2,
                "rating_class": r.3,
                "pace_score": r.4,
                "consistency_score": r.5,
                "experience_score": r.6,
                "total_laps": r.7,
                "updated_at": r.8,
            })).collect::<Vec<_>>(),
        }))
    }
}

// ─── Driver Rating History (Staff-Only — Phase 253) ──────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RatingHistoryQuery {
    sim_type: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn staff_driver_rating_history(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<RatingHistoryQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(200);

    // Current ratings serve as the "history" snapshot.
    // A full temporal history table would require a separate audit_log table.
    // For now, return all current ratings for this driver as their progression.
    let rows = if let Some(ref sim_type) = params.sim_type {
        sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? AND sim_type = ? ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(&driver_id)
        .bind(sim_type)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(&driver_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    Json(json!({
        "driver_id": driver_id,
        "history": rows.iter().map(|r| json!({
            "sim_type": r.1,
            "composite_rating": r.2,
            "rating_class": r.3,
            "pace_score": r.4,
            "consistency_score": r.5,
            "experience_score": r.6,
            "total_laps": r.7,
            "updated_at": r.8,
        })).collect::<Vec<_>>(),
    }))
}
