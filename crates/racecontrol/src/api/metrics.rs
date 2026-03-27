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
    // Bug #21: Changed from i64 to f64 to preserve sub-millisecond precision from AVG()
    pub avg_time_to_track_ms: Option<f64>,
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
        avg_time_to_track_ms: avg_ms, // Bug #21: pass f64 directly, no truncation
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

// ─── Tests (TDD RED phase — 200-02) ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// Build an in-memory DB with combo_reliability and launch_events tables.
    async fn make_test_db() -> SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        // launch_events table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS launch_events (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                car TEXT,
                track TEXT,
                session_type TEXT,
                timestamp TEXT NOT NULL,
                outcome TEXT NOT NULL,
                error_taxonomy TEXT,
                duration_to_playable_ms INTEGER,
                error_details TEXT,
                launch_args_hash TEXT,
                attempt_number INTEGER DEFAULT 1,
                db_fallback INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&db)
        .await
        .expect("create launch_events");

        // combo_reliability table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS combo_reliability (
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                car TEXT,
                track TEXT,
                success_rate REAL NOT NULL DEFAULT 0.0,
                avg_time_to_track_ms REAL,
                p95_time_to_track_ms REAL,
                total_launches INTEGER NOT NULL DEFAULT 0,
                common_failure_modes TEXT,
                last_updated TEXT NOT NULL
            )",
        )
        .execute(&db)
        .await
        .expect("create combo_reliability");

        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_combo_rel_pk ON combo_reliability(pod_id, sim_type, COALESCE(car, ''), COALESCE(track, ''))",
        )
        .execute(&db)
        .await
        .expect("create unique index");

        db
    }

    /// Insert a row into combo_reliability directly (for alternatives tests).
    async fn seed_combo(
        db: &SqlitePool,
        pod_id: &str,
        sim_type: &str,
        car: Option<&str>,
        track: Option<&str>,
        success_rate: f64,
        total_launches: i64,
    ) {
        let now = "2026-03-26T00:00:00Z";
        sqlx::query(
            "INSERT INTO combo_reliability (pod_id, sim_type, car, track, success_rate, total_launches, last_updated)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(pod_id)
        .bind(sim_type)
        .bind(car)
        .bind(track)
        .bind(success_rate)
        .bind(total_launches)
        .bind(now)
        .execute(db)
        .await
        .expect("seed combo_reliability");
    }

    /// Insert a launch_events row for matrix tests.
    async fn seed_launch_event(
        db: &SqlitePool,
        pod_id: &str,
        sim_type: &str,
        outcome: &str,
        duration_ms: Option<i64>,
        error_taxonomy: Option<&str>,
        created_at: &str,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO launch_events (id, pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, attempt_number, created_at)
             VALUES (?, ?, ?, NULL, NULL, NULL, ?, ?, ?, ?, 1, ?)",
        )
        .bind(&id)
        .bind(pod_id)
        .bind(sim_type)
        .bind(created_at)
        .bind(outcome)
        .bind(error_taxonomy)
        .bind(duration_ms)
        .bind(created_at)
        .execute(db)
        .await
        .expect("seed launch_event");
    }

    // ─── Alternatives Tests ─────────────────────────────────────────────────

    /// INTEL-03: alternatives returns max 3 high-reliability combos, sorted DESC by success_rate.
    #[tokio::test]
    async fn test_alternatives_top3() {
        let db = make_test_db().await;

        // Seed 5 combos for assetto_corsa/pod-5 with varying rates (all with >= 5 launches)
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.50, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("nurburgring"), 0.95, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.98, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ford"), Some("nurburgring"), 0.92, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_porsche"), Some("spa"), 0.91, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_lamborghini"), Some("mugello"), 0.78, 10).await;

        let params = AlternativesParams {
            game: "assetto_corsa".to_string(),
            car: Some("ks_ferrari".to_string()),
            track: Some("spa".to_string()),
            pod: Some("pod-5".to_string()),
        };

        let result = query_alternatives(&db, &params).await;

        // Must return max 3 results
        assert!(result.len() <= 3, "Must return at most 3 alternatives, got {}", result.len());
        // All results must have success_rate > 0.90
        for combo in &result {
            assert!(combo.success_rate > 0.90, "All alternatives must have success_rate > 0.90, got {}", combo.success_rate);
        }
        // Must return at least 1 result
        assert!(!result.is_empty(), "Must return at least 1 alternative");
    }

    /// INTEL-03: alternatives prefers combos that share car or track with the request.
    #[tokio::test]
    async fn test_alternatives_similarity() {
        let db = make_test_db().await;

        // Seed combos: one shares car (ks_ferrari), one shares track (spa), one is unrelated
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("nurburgring"), 0.93, 10).await; // shares car
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("spa"), 0.94, 10).await;             // shares track
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ford"), Some("monza"), 0.92, 10).await;          // unrelated

        let params = AlternativesParams {
            game: "assetto_corsa".to_string(),
            car: Some("ks_ferrari".to_string()),
            track: Some("spa".to_string()),
            pod: Some("pod-5".to_string()),
        };

        let result = query_alternatives(&db, &params).await;

        assert!(!result.is_empty(), "Must return alternatives");
        // At least 1 result must share car or track with request
        let has_similar = result.iter().any(|c| {
            c.car.as_deref() == Some("ks_ferrari") || c.track.as_deref() == Some("spa")
        });
        assert!(has_similar, "At least 1 alternative must share car or track with the request");
    }

    /// INTEL-03: the failing combo itself is excluded from alternatives.
    #[tokio::test]
    async fn test_alternatives_excludes_self() {
        let db = make_test_db().await;

        // Seed the "failing" combo itself with high success_rate (should still be excluded)
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.95, 10).await;
        // Seed a different combo that should appear
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.96, 10).await;

        let params = AlternativesParams {
            game: "assetto_corsa".to_string(),
            car: Some("ks_ferrari".to_string()),
            track: Some("spa".to_string()),
            pod: Some("pod-5".to_string()),
        };

        let result = query_alternatives(&db, &params).await;

        // The failing combo (ks_ferrari/spa) must NOT appear in alternatives
        let has_self = result.iter().any(|c| {
            c.car.as_deref() == Some("ks_ferrari") && c.track.as_deref() == Some("spa")
        });
        assert!(!has_self, "The failing combo (ks_ferrari/spa) must not appear in alternatives");
    }

    /// INTEL-03: pod-specific < 3 results falls back to fleet-wide data.
    #[tokio::test]
    async fn test_alternatives_pod_fallback() {
        let db = make_test_db().await;

        // Pod-5 has only 1 high-reliability combo (not the failing one)
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_bmw"), Some("monza"), 0.97, 10).await;
        seed_combo(&db, "pod-5", "assetto_corsa", Some("ks_ferrari"), Some("spa"), 0.50, 10).await; // the failing one

        // Other pods have fleet-wide high-reliability combos
        seed_combo(&db, "pod-3", "assetto_corsa", Some("ks_ford"), Some("nurburgring"), 0.95, 10).await;
        seed_combo(&db, "pod-1", "assetto_corsa", Some("ks_porsche"), Some("spa"), 0.94, 10).await;
        seed_combo(&db, "pod-2", "assetto_corsa", Some("ks_lamborghini"), Some("mugello"), 0.93, 10).await;

        let params = AlternativesParams {
            game: "assetto_corsa".to_string(),
            car: Some("ks_ferrari".to_string()),
            track: Some("spa".to_string()),
            pod: Some("pod-5".to_string()),
        };

        let result = query_alternatives(&db, &params).await;

        // With fallback, should return up to 3 total
        assert!(result.len() >= 2, "Fallback should return results from fleet, got {}", result.len());
        assert!(result.len() <= 3, "Must cap at 3 alternatives, got {}", result.len());
    }

    // ─── Launch Matrix Tests ────────────────────────────────────────────────

    /// INTEL-04: launch matrix flags pods with < 70% success rate.
    #[tokio::test]
    async fn test_launch_matrix_flagged() {
        let db = make_test_db().await;
        let now = "2026-03-26T00:00:00Z";

        // pod-1: 9 success / 10 total = 90% → not flagged
        for _ in 0..9 {
            seed_launch_event(&db, "pod-1", "assetto_corsa", "\"Success\"", Some(20000), None, now).await;
        }
        seed_launch_event(&db, "pod-1", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;

        // pod-5: 3 success / 5 total = 60% → flagged
        for _ in 0..3 {
            seed_launch_event(&db, "pod-5", "assetto_corsa", "\"Success\"", Some(25000), None, now).await;
        }
        for _ in 0..2 {
            seed_launch_event(&db, "pod-5", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
        }

        // pod-8: 8 success / 10 total = 80% → not flagged
        for _ in 0..8 {
            seed_launch_event(&db, "pod-8", "assetto_corsa", "\"Success\"", Some(22000), None, now).await;
        }
        for _ in 0..2 {
            seed_launch_event(&db, "pod-8", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
        }

        let result = query_launch_matrix(&db, "assetto_corsa").await;

        assert_eq!(result.len(), 3, "Matrix must have 3 rows");

        let pod5 = result.iter().find(|r| r.pod_id == "pod-5").expect("pod-5 must be in matrix");
        assert!(pod5.flagged, "pod-5 (60% success) must be flagged=true");

        let pod1 = result.iter().find(|r| r.pod_id == "pod-1").expect("pod-1 must be in matrix");
        assert!(!pod1.flagged, "pod-1 (90% success) must be flagged=false");

        let pod8 = result.iter().find(|r| r.pod_id == "pod-8").expect("pod-8 must be in matrix");
        assert!(!pod8.flagged, "pod-8 (80% success) must be flagged=false");
    }

    /// INTEL-04: launch matrix populates top_3_failure_modes per pod.
    #[tokio::test]
    async fn test_launch_matrix_failure_modes() {
        let db = make_test_db().await;
        let now = "2026-03-26T00:00:00Z";

        // pod-3: failures with different taxonomies
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Crash\"", None, Some("ProcessCrash"), now).await;
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Timeout\"", None, Some("LaunchTimeout"), now).await;
        seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Error\"", None, Some("OutOfMemory"), now).await;
        // Add some successes
        for _ in 0..4 {
            seed_launch_event(&db, "pod-3", "assetto_corsa", "\"Success\"", Some(20000), None, now).await;
        }

        let result = query_launch_matrix(&db, "assetto_corsa").await;

        let pod3 = result.iter().find(|r| r.pod_id == "pod-3").expect("pod-3 must be in matrix");
        // top_3_failure_modes must be populated
        assert!(!pod3.top_3_failure_modes.is_empty(), "top_3_failure_modes must be populated for pod-3");
        assert!(pod3.top_3_failure_modes.len() <= 3, "At most 3 failure modes");
        // LaunchTimeout (count=3) must be first (highest count)
        assert_eq!(pod3.top_3_failure_modes[0].mode, "LaunchTimeout",
            "LaunchTimeout (count=3) must be first failure mode, got: {:?}",
            pod3.top_3_failure_modes.iter().map(|m| &m.mode).collect::<Vec<_>>());
    }
}
