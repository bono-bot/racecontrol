//! Intel & observability metrics — game alternatives, launch matrix, combo list, observability
//!
//! Extracted from api/metrics.rs (ARCH-03 split).

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::state::AppState;

use super::metrics::FailureMode;

// ─── Game Alternatives (INTEL-03) ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AlternativesParams {
    pub game: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub pod: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlternativeCombo {
    pub car: Option<String>,
    pub track: Option<String>,
    pub success_rate: f64,
    pub avg_time_ms: Option<f64>,
    pub total_launches: i64,
}

/// Query combo_reliability for high-reliability alternatives.
/// If pod-specific results < 3, falls back to fleet-wide data.
/// Excludes the exact (car, track) combo from the request.
/// Orders by similarity (same car OR same track) first, then success_rate DESC.
pub async fn query_alternatives(
    db: &sqlx::SqlitePool,
    params: &AlternativesParams,
) -> Vec<AlternativeCombo> {
    let car = params.car.as_deref().unwrap_or("");
    let track = params.track.as_deref().unwrap_or("");
    let sim_type = &params.game;

    // Attempt pod-specific query first if pod param provided.
    if let Some(ref pod_id) = params.pod {
        let pod_results = sqlx::query_as::<_, (Option<String>, Option<String>, f64, Option<f64>, i64)>(
            "SELECT car, track, success_rate, avg_time_to_track_ms, total_launches
             FROM combo_reliability
             WHERE sim_type = ?
               AND pod_id = ?
               AND success_rate > 0.90
               AND total_launches >= 5
               AND NOT (COALESCE(car, '') = COALESCE(?, '') AND COALESCE(track, '') = COALESCE(?, ''))
             ORDER BY
               (CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) DESC,
               success_rate DESC
             LIMIT 3",
        )
        .bind(sim_type)
        .bind(pod_id)
        .bind(car)
        .bind(track)
        .bind(if car.is_empty() { None } else { Some(car) })
        .bind(if track.is_empty() { None } else { Some(track) })
        .fetch_all(db)
        .await
        .unwrap_or_default();

        if pod_results.len() >= 3 {
            return pod_results
                .into_iter()
                .map(|(c, t, sr, avg, total)| AlternativeCombo {
                    car: c,
                    track: t,
                    success_rate: sr,
                    avg_time_ms: avg,
                    total_launches: total,
                })
                .collect();
        }

        // < 3 pod-specific results — fall back to fleet-wide, excluding the failing combo.
        // Use a UNION approach: pod-specific first, then fill from fleet (different pods only).
        let pod_count = pod_results.len() as i64;
        let remaining = 3 - pod_count;

        // Collect pod-specific combos as a base set (already valid).
        let mut combined: Vec<AlternativeCombo> = pod_results
            .into_iter()
            .map(|(c, t, sr, avg, total)| AlternativeCombo {
                car: c,
                track: t,
                success_rate: sr,
                avg_time_ms: avg,
                total_launches: total,
            })
            .collect();

        // Fetch fleet-wide from other pods to fill up to 3.
        let fleet_results = sqlx::query_as::<_, (Option<String>, Option<String>, f64, Option<f64>, i64)>(
            "SELECT car, track, success_rate, avg_time_to_track_ms, total_launches
             FROM combo_reliability
             WHERE sim_type = ?
               AND pod_id != ?
               AND success_rate > 0.90
               AND total_launches >= 5
               AND NOT (COALESCE(car, '') = COALESCE(?, '') AND COALESCE(track, '') = COALESCE(?, ''))
             ORDER BY
               (CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) DESC,
               success_rate DESC
             LIMIT ?",
        )
        .bind(sim_type)
        .bind(pod_id)
        .bind(car)
        .bind(track)
        .bind(if car.is_empty() { None } else { Some(car) })
        .bind(if track.is_empty() { None } else { Some(track) })
        .bind(remaining)
        .fetch_all(db)
        .await
        .unwrap_or_default();

        for (c, t, sr, avg, total) in fleet_results {
            combined.push(AlternativeCombo {
                car: c,
                track: t,
                success_rate: sr,
                avg_time_ms: avg,
                total_launches: total,
            });
        }

        return combined;
    }

    // No pod param — fleet-wide query directly.
    sqlx::query_as::<_, (Option<String>, Option<String>, f64, Option<f64>, i64)>(
        "SELECT car, track, success_rate, avg_time_to_track_ms, total_launches
         FROM combo_reliability
         WHERE sim_type = ?
           AND success_rate > 0.90
           AND total_launches >= 5
           AND NOT (COALESCE(car, '') = COALESCE(?, '') AND COALESCE(track, '') = COALESCE(?, ''))
         ORDER BY
           (CASE WHEN car = ? OR track = ? THEN 1 ELSE 0 END) DESC,
           success_rate DESC
         LIMIT 3",
    )
    .bind(sim_type)
    .bind(car)
    .bind(track)
    .bind(if car.is_empty() { None } else { Some(car) })
    .bind(if track.is_empty() { None } else { Some(track) })
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(c, t, sr, avg, total)| AlternativeCombo {
        car: c,
        track: t,
        success_rate: sr,
        avg_time_ms: avg,
        total_launches: total,
    })
    .collect()
}

pub async fn alternatives_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlternativesParams>,
) -> impl IntoResponse {
    let results = query_alternatives(&state.db, &params).await;
    Json(serde_json::to_value(&results).unwrap_or_default())
}

// ─── Admin Launch Matrix (INTEL-04) ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LaunchMatrixParams {
    pub game: String,
}

#[derive(Debug, Serialize)]
pub struct LaunchMatrixRow {
    pub pod_id: String,
    pub total_launches: i64,
    pub success_rate: f64,
    pub avg_time_ms: Option<f64>,
    pub top_3_failure_modes: Vec<FailureMode>,
    pub flagged: bool,
}

/// Query launch_events for a per-pod reliability grid across all combos.
/// Uses a 30-day rolling window. Flags pods with success_rate < 0.70.
pub async fn query_launch_matrix(
    db: &sqlx::SqlitePool,
    sim_type: &str,
) -> Vec<LaunchMatrixRow> {
    // Fetch per-pod aggregate stats.
    let pod_rows = sqlx::query_as::<_, (String, i64, i64, Option<f64>)>(
        "SELECT pod_id,
                COUNT(*) as total,
                SUM(CASE WHEN outcome = '\"Success\"' THEN 1 ELSE 0 END) as successes,
                AVG(CASE WHEN duration_to_playable_ms IS NOT NULL THEN CAST(duration_to_playable_ms AS REAL) END) as avg_ms
         FROM launch_events
         WHERE sim_type = ?
           AND created_at >= datetime('now', '-30 days')
         GROUP BY pod_id
         ORDER BY pod_id",
    )
    .bind(sim_type)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut rows: Vec<LaunchMatrixRow> = Vec::with_capacity(pod_rows.len());

    for (pod_id, total, successes, avg_ms) in pod_rows {
        let success_rate = if total > 0 {
            successes as f64 / total as f64
        } else {
            0.0
        };

        // Per-pod top 3 failure modes.
        let failure_modes: Vec<FailureMode> = sqlx::query_as::<_, (String, i64)>(
            "SELECT error_taxonomy, COUNT(*) as cnt
             FROM launch_events
             WHERE pod_id = ?
               AND sim_type = ?
               AND outcome != '\"Success\"'
               AND error_taxonomy IS NOT NULL
               AND created_at >= datetime('now', '-30 days')
             GROUP BY error_taxonomy
             ORDER BY cnt DESC
             LIMIT 3",
        )
        .bind(&pod_id)
        .bind(sim_type)
        .fetch_all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(mode, count)| FailureMode { mode, count })
        .collect();

        rows.push(LaunchMatrixRow {
            pod_id,
            total_launches: total,
            success_rate,
            avg_time_ms: avg_ms,
            top_3_failure_modes: failure_modes,
            flagged: success_rate < 0.70,
        });
    }

    rows
}

pub async fn launch_matrix_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LaunchMatrixParams>,
) -> impl IntoResponse {
    let rows = query_launch_matrix(&state.db, &params.game).await;
    Json(serde_json::to_value(&rows).unwrap_or_default())
}

// ─── Admin Combo List (DASH-02) ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ComboListParams {
    pub game: Option<String>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ComboListRow {
    pub pod_id: String,
    pub sim_type: String,
    pub car: Option<String>,
    pub track: Option<String>,
    pub success_rate: f64,
    pub avg_time_ms: Option<f64>,
    pub total_launches: i64,
    pub flagged: bool,
}

pub async fn combo_list_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ComboListParams>,
) -> impl IntoResponse {
    // Whitelist sort_by values to prevent SQL injection via column name interpolation.
    let sort_col = match params.sort_by.as_deref() {
        Some("total_launches") => "total_launches",
        Some("avg_time_ms") => "avg_time_to_track_ms",
        _ => "success_rate",
    };
    let sort_dir = match params.order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    let sql = format!(
        "SELECT pod_id, sim_type, car, track, success_rate, avg_time_to_track_ms, total_launches
         FROM combo_reliability
         WHERE (?1 IS NULL OR sim_type = ?1)
         ORDER BY {sort_col} {sort_dir}"
    );

    let rows: Vec<ComboListRow> = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, f64, Option<f64>, i64)>(&sql)
        .bind(params.game.as_deref())
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(pod_id, sim_type, car, track, success_rate, avg_time_ms, total_launches)| ComboListRow {
            pod_id,
            sim_type,
            car,
            track,
            success_rate,
            avg_time_ms,
            total_launches,
            flagged: success_rate < 0.70,
        })
        .collect();

    Json(serde_json::to_value(&rows).unwrap_or_default())
}

// ─── Launch Observability (Phase 284) ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SlowLaunch {
    pub pod_id: String,
    pub sim_type: String,
    pub duration_to_playable_ms: i64,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ReadyDelayBySim {
    pub sim_type: String,
    pub avg_ready_delay_ms: f64,
    pub total_launches: i64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct LaunchObservabilityResponse {
    pub top_slow_launches: Vec<SlowLaunch>,
    pub by_sim_type: Vec<ReadyDelayBySim>,
}

pub async fn launch_observability_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Top 10 slowest launches (last 30 days)
    let slow_launches: Vec<SlowLaunch> = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT pod_id, sim_type, duration_to_playable_ms, created_at
         FROM launch_events
         WHERE duration_to_playable_ms IS NOT NULL
           AND created_at >= datetime('now', '-30 days')
         ORDER BY duration_to_playable_ms DESC
         LIMIT 10",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(pod_id, sim_type, ms, ts)| SlowLaunch {
        pod_id,
        sim_type,
        duration_to_playable_ms: ms,
        timestamp: ts,
    })
    .collect();

    // Average ready_delay, total launches, success rate by sim_type (last 7 days)
    let by_sim: Vec<ReadyDelayBySim> = sqlx::query_as::<_, (String, f64, i64, i64)>(
        "SELECT sim_type,
                AVG(CAST(duration_to_playable_ms AS REAL)) as avg_ms,
                COUNT(*) as total,
                SUM(CASE WHEN outcome = '\"Success\"' THEN 1 ELSE 0 END) as successes
         FROM launch_events
         WHERE created_at >= datetime('now', '-7 days')
         GROUP BY sim_type
         ORDER BY total DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(sim_type, avg_ms, total, successes)| ReadyDelayBySim {
        sim_type,
        avg_ready_delay_ms: avg_ms,
        total_launches: total,
        success_rate: if total > 0 { successes as f64 / total as f64 } else { 0.0 },
    })
    .collect();

    let response = LaunchObservabilityResponse {
        top_slow_launches: slow_launches,
        by_sim_type: by_sim,
    };

    Json(serde_json::to_value(&response).unwrap_or_default())
}
