//! Fleet Intelligence — Phase 366 GLD-F-01 + GLD-F-02
//!
//! Provides:
//! - `compute_pod_health_score`: per-pod composite 0-100 health score from billing_sessions
//! - `compute_time_patterns`: time-of-day failure pattern analysis (30-day window)
//! - `fleet_intelligence_handler`: GET /api/v1/fleet/intelligence (staff JWT)

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::fleet_health::FleetHealthStore;
use crate::state::AppState;

// ─── Response types ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PodHealthComponents {
    pub session_success_rate: f64,
    pub telemetry_completeness_avg: f64,
    pub config_mismatch_rate: f64,
    pub crashes_last_hour: i32,
}

#[derive(Debug, Serialize)]
pub struct PodIntelligence {
    pub pod_id: String,
    pub score: Option<f64>,
    pub insufficient_data: bool,
    pub components: PodHealthComponents,
    pub window_days: u32,
    pub sessions_in_window: i64,
}

#[derive(Debug, Serialize)]
pub struct TimePatternHour {
    pub hour: u32,
    pub failure_rate: f64,
    pub sample_count: i64,
}

#[derive(Debug, Serialize)]
pub struct PodTimePattern {
    pub pod_id: String,
    pub flagged_hours: Vec<TimePatternHour>,
    pub threshold_pct: u32,
}

#[derive(Debug, Serialize)]
pub struct FleetIntelligenceResponse {
    pub generated_at: String,
    pub pods: Vec<PodIntelligence>,
    pub time_patterns: Vec<PodTimePattern>,
}

// ─── Health score computation (GLD-F-01, D-01) ─────────────────────────────

/// Compute composite 0-100 health score for a single pod.
///
/// Weights (per D-01): session_success_rate 40 / telemetry_completeness 30 /
/// config_mismatch_rate 20 / crash_penalty 10.
///
/// Returns `score: None` when fewer than 3 completed sessions exist in the
/// 7-day window (insufficient data — D-02).
pub async fn compute_pod_health_score(
    pool: &SqlitePool,
    pod_id: &str,
    fleet_store: Option<&FleetHealthStore>,
) -> PodIntelligence {
    // Query completed sessions in 7-day window. Uses COALESCE for Phase 363
    // columns that may be NULL on older DBs (pitfall #3 from research).
    let result = sqlx::query_as::<_, (i64, f64, f64)>(
        "SELECT COUNT(*) as total,
                COALESCE(AVG(CASE WHEN COALESCE(suspect, 0) = 0 THEN 1.0 ELSE 0.0 END), 1.0) as success_rate,
                COALESCE(AVG(COALESCE(telemetry_coverage_pct, 100.0)), 100.0) as avg_coverage
         FROM billing_sessions
         WHERE pod_id = ?
           AND status = 'completed'
           AND started_at >= datetime('now', '-7 days')",
    )
    .bind(pod_id)
    .fetch_one(pool)
    .await;

    let (total, success_rate, avg_coverage) = match result {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(pod_id = pod_id, "Fleet intelligence query failed: {}", e);
            (0, 1.0, 100.0)
        }
    };

    let crashes = fleet_store.map(|s| s.crashes_last_hour).unwrap_or(0);

    if total < 3 {
        return PodIntelligence {
            pod_id: pod_id.to_string(),
            score: None,
            insufficient_data: true,
            components: PodHealthComponents {
                session_success_rate: success_rate,
                telemetry_completeness_avg: avg_coverage,
                config_mismatch_rate: 0.0,
                crashes_last_hour: crashes,
            },
            window_days: 7,
            sessions_in_window: total,
        };
    }

    // crash_penalty = min(crashes / 5, 1.0) per D-01
    let crash_penalty = (crashes as f64 / 5.0).min(1.0);

    // Composite: 40/30/20/10 weights
    let score = (success_rate * 40.0)
        + (avg_coverage / 100.0 * 30.0)
        + (1.0 * 20.0) // config_mismatch_rate defaults to 0.0 (clean) — Phase 362 data not wired yet
        + ((1.0 - crash_penalty) * 10.0);
    let score = score.round().max(0.0).min(100.0);

    PodIntelligence {
        pod_id: pod_id.to_string(),
        score: Some(score),
        insufficient_data: false,
        components: PodHealthComponents {
            session_success_rate: success_rate,
            telemetry_completeness_avg: avg_coverage,
            config_mismatch_rate: 0.0,
            crashes_last_hour: crashes,
        },
        window_days: 7,
        sessions_in_window: total,
    }
}

// ─── Time-of-day pattern analysis (GLD-F-02, D-05) ─────────────────────────

/// Compute time-of-day failure patterns across all pods.
///
/// 30-day window, flags hours with failure_rate >= 30% and sample_count >= 3.
pub async fn compute_time_patterns(pool: &SqlitePool) -> Vec<PodTimePattern> {
    let rows = sqlx::query_as::<_, (String, i64, i64, f64)>(
        "SELECT pod_id,
                CAST(strftime('%H', started_at) AS INTEGER) as hour_of_day,
                COUNT(*) as sample_count,
                CAST(SUM(CASE WHEN COALESCE(suspect, 0) = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as failure_rate
         FROM billing_sessions
         WHERE started_at >= datetime('now', '-30 days')
           AND status = 'completed'
         GROUP BY pod_id, strftime('%H', started_at)
         HAVING COUNT(*) >= 3 AND failure_rate >= 0.30
         ORDER BY pod_id, hour_of_day",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group by pod_id
    let mut map: HashMap<String, Vec<TimePatternHour>> = HashMap::new();
    for (pod_id, hour, count, rate) in rows {
        map.entry(pod_id).or_default().push(TimePatternHour {
            hour: hour as u32,
            failure_rate: rate,
            sample_count: count,
        });
    }
    map.into_iter()
        .map(|(pod_id, flagged_hours)| PodTimePattern {
            pod_id,
            flagged_hours,
            threshold_pct: 30,
        })
        .collect()
}

// ─── Handler (D-03) ─────────────────────────────────────────────────────────

/// GET /api/v1/fleet/intelligence — staff JWT required.
///
/// Returns composite health scores per pod + time-of-day failure patterns.
pub async fn fleet_intelligence_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Snapshot pod IDs from fleet health store (avoid holding lock across awaits)
    let pod_ids: Vec<(String, Option<FleetHealthStore>)> = {
        let guard = state.pod_fleet_health.read().await;
        guard
            .iter()
            .map(|(pod_id, store)| (pod_id.clone(), Some(store.clone())))
            .collect()
    };

    let mut pod_intelligence = Vec::new();
    for (pod_id, fleet_store) in &pod_ids {
        let intel =
            compute_pod_health_score(&state.db, pod_id, fleet_store.as_ref()).await;
        pod_intelligence.push(intel);
    }

    let time_patterns = compute_time_patterns(&state.db).await;

    let now = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
        .to_rfc3339();

    Json(
        serde_json::to_value(FleetIntelligenceResponse {
            generated_at: now,
            pods: pod_intelligence,
            time_patterns,
        })
        .unwrap_or_default(),
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE billing_sessions (
                id TEXT PRIMARY KEY, driver_id TEXT, pod_id TEXT,
                pricing_tier_id TEXT, allocated_seconds INTEGER,
                status TEXT DEFAULT 'completed',
                suspect BOOLEAN DEFAULT 0,
                telemetry_coverage_pct REAL,
                started_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create billing_sessions");
        pool
    }

    #[tokio::test]
    async fn test_insufficient_data_when_less_than_3_sessions() {
        let pool = test_pool().await;
        for i in 0..2 {
            sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status)
                 VALUES (?, 'd1', 'pod-1', 't1', 3600, 'completed')",
            )
            .bind(format!("s{}", i))
            .execute(&pool)
            .await
            .expect("insert");
        }
        let result = compute_pod_health_score(&pool, "pod-1", None).await;
        assert!(result.insufficient_data);
        assert!(result.score.is_none());
        assert_eq!(result.sessions_in_window, 2);
    }

    #[tokio::test]
    async fn test_score_100_for_clean_pod() {
        let pool = test_pool().await;
        for i in 0..5 {
            sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, suspect, telemetry_coverage_pct)
                 VALUES (?, 'd1', 'pod-1', 't1', 3600, 'completed', 0, 100.0)",
            )
            .bind(format!("s{}", i))
            .execute(&pool)
            .await
            .expect("insert");
        }
        let result = compute_pod_health_score(&pool, "pod-1", None).await;
        assert!(!result.insufficient_data);
        let score = result.score.expect("score should be Some");
        // 1.0*40 + 100/100*30 + 1.0*20 + 1.0*10 = 100
        assert!(
            (score - 100.0).abs() < 0.1,
            "Expected ~100, got {}",
            score
        );
    }

    #[tokio::test]
    async fn test_score_reduced_for_suspect_sessions() {
        let pool = test_pool().await;
        for i in 0..10 {
            let suspect = if i < 5 { 1 } else { 0 };
            sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, suspect, telemetry_coverage_pct)
                 VALUES (?, 'd1', 'pod-1', 't1', 3600, 'completed', ?, 100.0)",
            )
            .bind(format!("s{}", i))
            .bind(suspect)
            .execute(&pool)
            .await
            .expect("insert");
        }
        let result = compute_pod_health_score(&pool, "pod-1", None).await;
        assert!(!result.insufficient_data);
        let score = result.score.expect("score should be Some");
        // success_rate = 0.5: 0.5*40=20 + 30 + 20 + 10 = 80
        assert!(
            (score - 80.0).abs() < 0.1,
            "Expected ~80, got {}",
            score
        );
    }

    #[tokio::test]
    async fn test_time_patterns_flagged_above_threshold() {
        let pool = test_pool().await;
        // Insert 4 sessions at a fixed hour (14:00 UTC) for pod-6, 3 suspect (75% failure rate).
        // Use a fixed date within the 30-day window to avoid timezone issues with datetime('now').
        let base_date = chrono::Utc::now() - chrono::Duration::days(2);
        let fixed_ts = base_date
            .format("%Y-%m-%d")
            .to_string()
            + " 14:30:00";
        for i in 0..4 {
            let suspect = if i < 3 { 1 } else { 0 };
            sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, suspect, started_at)
                 VALUES (?, 'd1', 'pod-6', 't1', 3600, 'completed', ?, ?)",
            )
            .bind(format!("tp{}", i))
            .bind(suspect)
            .bind(&fixed_ts)
            .execute(&pool)
            .await
            .expect("insert");
        }
        let patterns = compute_time_patterns(&pool).await;
        let pod6 = patterns.iter().find(|p| p.pod_id == "pod-6");
        assert!(pod6.is_some(), "pod-6 should have flagged hours");
        let hours = &pod6.expect("pod-6").flagged_hours;
        assert!(!hours.is_empty(), "hour 14 should be flagged");
        assert!(hours.iter().any(|h| h.hour == 14));
    }
}
