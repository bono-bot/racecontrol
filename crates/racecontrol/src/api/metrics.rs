//! Metrics API handlers — launch statistics and billing accuracy (METRICS-05, METRICS-06)
//!
//! Endpoints:
//!   GET /api/v1/metrics/launch-stats   — filterable by pod/game/car/track
//!   GET /api/v1/metrics/billing-accuracy — 30-day rolling billing delta stats

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::state::AppState;

// ─── Launch Stats (METRICS-05) ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LaunchStatsParams {
    pub pod: Option<String>,
    pub game: Option<String>,
    pub car: Option<String>,
    pub track: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FailureMode {
    pub mode: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct LaunchStatsResponse {
    pub success_rate: f64,
    pub avg_time_to_track_ms: Option<i64>,
    pub p95_time_to_track_ms: Option<i64>,
    pub total_launches: i64,
    pub common_failure_modes: Vec<FailureMode>,
    pub last_30d_trend: String,
}

pub async fn launch_stats_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LaunchStatsParams>,
) -> impl IntoResponse {
    // Build dynamic WHERE clause. All values are bound via sqlx — never interpolated.
    // NOTE: outcome is stored as JSON-serialized enum, e.g. '"Success"' (with quotes).
    let mut conditions = vec!["created_at >= datetime('now', '-30 days')".to_string()];
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref pod) = params.pod {
        conditions.push(format!("pod_id = ?{}", binds.len() + 1));
        binds.push(pod.clone());
    }
    if let Some(ref game) = params.game {
        conditions.push(format!("sim_type = ?{}", binds.len() + 1));
        binds.push(game.clone());
    }
    if let Some(ref car) = params.car {
        conditions.push(format!("car = ?{}", binds.len() + 1));
        binds.push(car.clone());
    }
    if let Some(ref track) = params.track {
        conditions.push(format!("track = ?{}", binds.len() + 1));
        binds.push(track.clone());
    }

    let where_clause = conditions.join(" AND ");

    // ── Total, success count, average duration ─────────────────────────────
    let total_query = format!(
        "SELECT COUNT(*) as total,
                SUM(CASE WHEN outcome = '\"Success\"' THEN 1 ELSE 0 END) as successes,
                AVG(CASE WHEN duration_to_playable_ms IS NOT NULL THEN CAST(duration_to_playable_ms AS REAL) END) as avg_ms
         FROM launch_events WHERE {where_clause}"
    );

    let mut q = sqlx::query_as::<_, (i64, i64, Option<f64>)>(&total_query);
    for b in &binds {
        q = q.bind(b);
    }

    let (total, successes, avg_ms) = match q.fetch_one(&state.db).await {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("launch_stats total query failed: {e}");
            return Json(serde_json::json!({"error": "query failed"}));
        }
    };

    let success_rate = if total > 0 {
        successes as f64 / total as f64
    } else {
        0.0
    };

    // ── P95 duration: fetch sorted, pick 95th percentile index ────────────
    let p95_query = format!(
        "SELECT duration_to_playable_ms FROM launch_events
         WHERE {where_clause} AND duration_to_playable_ms IS NOT NULL
         ORDER BY duration_to_playable_ms ASC"
    );
    let mut p95_q = sqlx::query_as::<_, (i64,)>(&p95_query);
    for b in &binds {
        p95_q = p95_q.bind(b);
    }
    let durations: Vec<(i64,)> = p95_q.fetch_all(&state.db).await.unwrap_or_default();
    let p95 = if !durations.is_empty() {
        let idx = ((durations.len() as f64 * 0.95).ceil() as usize)
            .min(durations.len())
            .saturating_sub(1);
        Some(durations[idx].0)
    } else {
        None
    };

    // ── Common failure modes (top 5 by count) ─────────────────────────────
    let fail_query = format!(
        "SELECT error_taxonomy, COUNT(*) as cnt FROM launch_events
         WHERE {where_clause} AND outcome != '\"Success\"' AND error_taxonomy IS NOT NULL
         GROUP BY error_taxonomy ORDER BY cnt DESC LIMIT 5"
    );
    let mut fail_q = sqlx::query_as::<_, (String, i64)>(&fail_query);
    for b in &binds {
        fail_q = fail_q.bind(b);
    }
    let failure_modes: Vec<FailureMode> = fail_q
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(mode, count)| FailureMode { mode, count })
        .collect();

    // ── Trend: last-15-days vs previous-15-days success rate ──────────────
    let trend_query = format!(
        "SELECT
            SUM(CASE WHEN created_at >= datetime('now', '-15 days') AND outcome = '\"Success\"' THEN 1 ELSE 0 END) as recent_success,
            SUM(CASE WHEN created_at >= datetime('now', '-15 days') THEN 1 ELSE 0 END) as recent_total,
            SUM(CASE WHEN created_at < datetime('now', '-15 days') AND outcome = '\"Success\"' THEN 1 ELSE 0 END) as prev_success,
            SUM(CASE WHEN created_at < datetime('now', '-15 days') THEN 1 ELSE 0 END) as prev_total
         FROM launch_events WHERE {where_clause}"
    );
    let mut trend_q = sqlx::query_as::<_, (i64, i64, i64, i64)>(&trend_query);
    for b in &binds {
        trend_q = trend_q.bind(b);
    }
    let trend = match trend_q.fetch_one(&state.db).await {
        Ok((rs, rt, ps, pt)) => {
            let recent_rate = if rt > 0 { rs as f64 / rt as f64 } else { 0.0 };
            let prev_rate = if pt > 0 { ps as f64 / pt as f64 } else { 0.0 };
            if recent_rate > prev_rate + 0.05 {
                "improving"
            } else if recent_rate < prev_rate - 0.05 {
                "degrading"
            } else {
                "stable"
            }
        }
        Err(_) => "stable",
    };

    let response = LaunchStatsResponse {
        success_rate,
        avg_time_to_track_ms: avg_ms.map(|v| v as i64),
        p95_time_to_track_ms: p95,
        total_launches: total,
        common_failure_modes: failure_modes,
        last_30d_trend: trend.to_string(),
    };

    Json(serde_json::to_value(&response).unwrap_or_default())
}

// ─── Billing Accuracy (METRICS-06) ───────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BillingAccuracyResponse {
    pub avg_delta_ms: Option<f64>,
    pub max_delta_ms: Option<i64>,
    pub sessions_with_zero_delta: i64,
    pub sessions_where_billing_never_started: i64,
    pub false_playable_signals: i64,
}

pub async fn billing_accuracy_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // ── Aggregate delta stats for 'start' events in last 30 days ──────────
    let result = sqlx::query_as::<_, (Option<f64>, Option<i64>, i64)>(
        "SELECT
            AVG(CAST(delta_ms AS REAL)) as avg_delta,
            MAX(delta_ms) as max_delta,
            SUM(CASE WHEN delta_ms = 0 THEN 1 ELSE 0 END) as zero_delta
         FROM billing_accuracy_events
         WHERE event_type = 'start' AND created_at >= datetime('now', '-30 days')",
    )
    .fetch_one(&state.db)
    .await;

    let (avg_delta, max_delta, zero_delta) = match result {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("billing_accuracy aggregate query failed: {e}");
            return Json(serde_json::json!({"error": "query failed"}));
        }
    };

    // ── Sessions where billing_start_at was never recorded ────────────────
    let never_started: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_accuracy_events
         WHERE event_type = 'start'
           AND launch_command_at IS NOT NULL
           AND billing_start_at IS NULL
           AND created_at >= datetime('now', '-30 days')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // ── False playable signals: discrepancy events tagged false_playable ───
    let false_signals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_accuracy_events
         WHERE event_type = 'discrepancy'
           AND details LIKE '%false_playable%'
           AND created_at >= datetime('now', '-30 days')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let response = BillingAccuracyResponse {
        avg_delta_ms: avg_delta,
        max_delta_ms: max_delta,
        sessions_with_zero_delta: zero_delta,
        sessions_where_billing_never_started: never_started,
        false_playable_signals: false_signals,
    };

    Json(serde_json::to_value(&response).unwrap_or_default())
}
