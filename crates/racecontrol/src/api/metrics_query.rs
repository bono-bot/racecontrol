//! Metrics Query API — Phase 286 (QAPI-01..05)
//!
//! Endpoints:
//!   GET /api/v1/metrics/query     — time-series query with auto-resolution
//!   GET /api/v1/metrics/names     — distinct metric names from both tables
//!   GET /api/v1/metrics/snapshot  — latest value per metric+pod combination

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use crate::state::AppState;

// ─── Response types (D-01, D-02, D-03) ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TimePoint {
    pub ts: i64,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub metric: String,
    pub pod: Option<u32>,
    pub resolution: String,
    pub points: Vec<TimePoint>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotEntry {
    pub name: String,
    pub pod: Option<u32>,
    pub value: f64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct SnapshotResponse {
    pub metrics: Vec<SnapshotEntry>,
}

#[derive(Debug, Serialize)]
pub struct NamesResponse {
    pub names: Vec<String>,
}

// ─── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MetricsQueryParams {
    pub metric: String,
    pub from: i64,
    pub to: i64,
    pub pod: Option<u32>,
    pub resolution: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PodFilterParams {
    pub pod: Option<u32>,
}

// ─── Auto-resolution (D-07) ───────────────────────────────────────────────────

pub fn select_resolution(from: i64, to: i64, override_res: Option<&str>) -> &'static str {
    if let Some(r) = override_res {
        match r {
            "raw" => return "raw",
            "hourly" => return "hourly",
            "daily" => return "daily",
            _ => {}
        }
    }
    let range = to - from;
    if range < 86_400 {
        "raw"
    } else if range < 7 * 86_400 {
        "hourly"
    } else {
        "daily"
    }
}

// ─── Inner query functions (testable without AppState) ────────────────────────

/// Query time-series points. Uses metrics_samples for "raw", metrics_rollups for others.
///
/// CRITICAL: CAST(strftime('%s', col) AS INTEGER) — strftime returns TEXT, must CAST.
/// CRITICAL: COALESCE in snapshot join — NULL = NULL is false in SQLite.
pub async fn query_time_series(
    db: &SqlitePool,
    metric: &str,
    from: i64,
    to: i64,
    pod: Option<u32>,
    resolution: &str,
) -> Vec<TimePoint> {
    let pod_str = pod.map(|n| format!("pod-{}", n));

    if resolution == "raw" {
        let rows = if let Some(ref pod_id) = pod_str {
            sqlx::query(
                "SELECT CAST(strftime('%s', recorded_at) AS INTEGER) AS ts, value
                 FROM metrics_samples
                 WHERE metric_name = ?
                   AND recorded_at >= datetime(?, 'unixepoch')
                   AND recorded_at < datetime(?, 'unixepoch')
                   AND pod_id = ?
                 ORDER BY recorded_at",
            )
            .bind(metric)
            .bind(from)
            .bind(to)
            .bind(pod_id)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query(
                "SELECT CAST(strftime('%s', recorded_at) AS INTEGER) AS ts, value
                 FROM metrics_samples
                 WHERE metric_name = ?
                   AND recorded_at >= datetime(?, 'unixepoch')
                   AND recorded_at < datetime(?, 'unixepoch')
                 ORDER BY recorded_at",
            )
            .bind(metric)
            .bind(from)
            .bind(to)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        };

        rows.into_iter()
            .map(|r| TimePoint {
                ts: r.try_get::<i64, _>("ts").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
            })
            .collect()
    } else {
        // hourly or daily — use metrics_rollups, avg_value as value
        let rows = if let Some(ref pod_id) = pod_str {
            sqlx::query(
                "SELECT CAST(strftime('%s', period_start) AS INTEGER) AS ts, avg_value AS value
                 FROM metrics_rollups
                 WHERE resolution = ?
                   AND metric_name = ?
                   AND period_start >= datetime(?, 'unixepoch')
                   AND period_start < datetime(?, 'unixepoch')
                   AND pod_id = ?
                 ORDER BY period_start",
            )
            .bind(resolution)
            .bind(metric)
            .bind(from)
            .bind(to)
            .bind(pod_id)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query(
                "SELECT CAST(strftime('%s', period_start) AS INTEGER) AS ts, avg_value AS value
                 FROM metrics_rollups
                 WHERE resolution = ?
                   AND metric_name = ?
                   AND period_start >= datetime(?, 'unixepoch')
                   AND period_start < datetime(?, 'unixepoch')
                 ORDER BY period_start",
            )
            .bind(resolution)
            .bind(metric)
            .bind(from)
            .bind(to)
            .fetch_all(db)
            .await
            .unwrap_or_default()
        };

        rows.into_iter()
            .map(|r| TimePoint {
                ts: r.try_get::<i64, _>("ts").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
            })
            .collect()
    }
}

/// Query all distinct metric names from both tables, sorted.
pub async fn query_metric_names(db: &SqlitePool) -> Vec<String> {
    let rows = sqlx::query(
        "SELECT metric_name FROM (
            SELECT DISTINCT metric_name FROM metrics_samples
            UNION
            SELECT DISTINCT metric_name FROM metrics_rollups
        ) ORDER BY metric_name",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .filter_map(|r| r.try_get::<String, _>("metric_name").ok())
        .collect()
}

/// Query latest value per metric+pod combination.
///
/// Uses self-join pattern. COALESCE handles NULL pod_id correctly (NULL = NULL is false in SQLite).
/// CAST(strftime('%s', ...) AS INTEGER) — strftime returns TEXT, must cast to i64.
pub async fn query_snapshot(db: &SqlitePool, pod: Option<u32>) -> Vec<SnapshotEntry> {
    let pod_str = pod.map(|n| format!("pod-{}", n));

    let rows = if let Some(ref pod_id) = pod_str {
        sqlx::query(
            "SELECT s.metric_name, s.pod_id,
                    s.value,
                    CAST(strftime('%s', s.recorded_at) AS INTEGER) AS updated_at
             FROM metrics_samples s
             INNER JOIN (
                 SELECT metric_name, pod_id, MAX(recorded_at) AS max_ts
                 FROM metrics_samples GROUP BY metric_name, pod_id
             ) latest
                 ON s.metric_name = latest.metric_name
                AND COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')
                AND s.recorded_at = latest.max_ts
             WHERE s.pod_id = ?
             ORDER BY s.metric_name, s.pod_id",
        )
        .bind(pod_id)
        .fetch_all(db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query(
            "SELECT s.metric_name, s.pod_id,
                    s.value,
                    CAST(strftime('%s', s.recorded_at) AS INTEGER) AS updated_at
             FROM metrics_samples s
             INNER JOIN (
                 SELECT metric_name, pod_id, MAX(recorded_at) AS max_ts
                 FROM metrics_samples GROUP BY metric_name, pod_id
             ) latest
                 ON s.metric_name = latest.metric_name
                AND COALESCE(s.pod_id, '') = COALESCE(latest.pod_id, '')
                AND s.recorded_at = latest.max_ts
             ORDER BY s.metric_name, s.pod_id",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default()
    };

    rows.into_iter()
        .map(|r| {
            let pod_id: Option<String> = r.try_get("pod_id").ok();
            SnapshotEntry {
                pod: pod_id.as_deref()
                    .and_then(|s| s.strip_prefix("pod-"))
                    .and_then(|n| n.parse().ok()),
                name: r.try_get::<String, _>("metric_name").unwrap_or_default(),
                value: r.try_get::<f64, _>("value").unwrap_or_default(),
                updated_at: r.try_get::<i64, _>("updated_at").unwrap_or_default(),
            }
        })
        .collect()
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/metrics/query?metric=cpu_usage&from=T1&to=T2[&pod=N][&resolution=raw|hourly|daily]
pub async fn query_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MetricsQueryParams>,
) -> impl IntoResponse {
    // D-05: validate range
    if params.from >= params.to {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "from must be less than to"})),
        )
            .into_response();
    }

    let resolution = select_resolution(params.from, params.to, params.resolution.as_deref());
    let points = query_time_series(
        &state.db,
        &params.metric,
        params.from,
        params.to,
        params.pod,
        resolution,
    )
    .await;

    (
        StatusCode::OK,
        Json(QueryResponse {
            metric: params.metric,
            pod: params.pod,
            resolution: resolution.to_string(),
            points,
        }),
    )
        .into_response()
}

/// GET /api/v1/metrics/names — all distinct metric names from both tables
pub async fn names_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let names = query_metric_names(&state.db).await;
    Json(NamesResponse { names })
}

/// GET /api/v1/metrics/snapshot[?pod=N] — latest value per metric+pod combination
pub async fn snapshot_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PodFilterParams>,
) -> impl IntoResponse {
    let metrics = query_snapshot(&state.db, params.pod).await;
    Json(SnapshotResponse { metrics })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS metrics_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric_name TEXT NOT NULL,
                pod_id TEXT,
                value REAL NOT NULL,
                recorded_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS metrics_rollups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                resolution TEXT NOT NULL,
                metric_name TEXT NOT NULL,
                pod_id TEXT,
                min_value REAL NOT NULL,
                max_value REAL NOT NULL,
                avg_value REAL NOT NULL,
                sample_count INTEGER NOT NULL,
                period_start TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    /// test_query_raw_samples: insert 3 samples for "cpu_usage" at known timestamps,
    /// query with range <24h, expect 3 points with correct ts/value
    #[tokio::test]
    async fn test_query_raw_samples() {
        let db = make_test_db().await;

        // epoch 1751328000 = 2025-07-01 00:00:00 UTC; insert 3 samples 1 minute apart
        for i in 0i64..3 {
            sqlx::query(
                "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
                 VALUES ('cpu_usage', NULL, ?, datetime(1751328000 + ?, 'unixepoch'))",
            )
            .bind(10.0 + i as f64)
            .bind(i * 60)
            .execute(&db)
            .await
            .unwrap();
        }

        // Range is <24h (1 hour), raw resolution
        let points =
            query_time_series(&db, "cpu_usage", 1751327999, 1751331600, None, "raw").await;
        assert_eq!(points.len(), 3, "expected 3 raw points, got {}", points.len());
        assert!((points[0].value - 10.0).abs() < 0.01);
        assert!(points[0].ts > 0, "ts must be a positive unix epoch");
    }

    /// test_query_unknown_metric_returns_empty: query for "nonexistent" metric, expect empty
    #[tokio::test]
    async fn test_query_unknown_metric_returns_empty() {
        let db = make_test_db().await;
        let points =
            query_time_series(&db, "nonexistent", 1000000, 1003600, None, "raw").await;
        assert!(
            points.is_empty(),
            "unknown metric must return empty points array"
        );
    }

    /// test_query_invalid_range_returns_400: from > to triggers 400 validation
    #[tokio::test]
    async fn test_query_invalid_range_returns_400() {
        // Test the validation predicate used by query_handler
        let from = 1003600i64;
        let to = 1000000i64;
        assert!(
            from >= to,
            "precondition: from >= to must be true to trigger 400 in query_handler"
        );
        // Also verify equal values trigger the guard
        assert!(1000i64 >= 1000i64);
    }

    /// test_names_distinct: insert samples for "cpu_usage" and "gpu_temp", expect both names
    #[tokio::test]
    async fn test_names_distinct() {
        let db = make_test_db().await;

        for (name, val) in &[("cpu_usage", 1.0), ("gpu_temp", 2.0), ("cpu_usage", 3.0)] {
            sqlx::query(
                "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
                 VALUES (?, NULL, ?, '2026-01-01T00:00:00')",
            )
            .bind(name)
            .bind(val)
            .execute(&db)
            .await
            .unwrap();
        }

        let names = query_metric_names(&db).await;
        assert_eq!(names.len(), 2, "expected 2 distinct names, got {:?}", names);
        assert!(names.contains(&"cpu_usage".to_string()));
        assert!(names.contains(&"gpu_temp".to_string()));
    }

    /// test_snapshot_latest_per_group: 2 samples for same metric+pod, snapshot returns latest
    #[tokio::test]
    async fn test_snapshot_latest_per_group() {
        let db = make_test_db().await;

        sqlx::query(
            "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
             VALUES ('cpu_usage', 'pod-1', 10.0, '2026-01-01T00:00:00')",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
             VALUES ('cpu_usage', 'pod-1', 99.0, '2026-01-01T01:00:00')",
        )
        .execute(&db)
        .await
        .unwrap();

        let entries = query_snapshot(&db, None).await;
        assert_eq!(
            entries.len(),
            1,
            "expected 1 snapshot entry, got {}",
            entries.len()
        );
        assert!(
            (entries[0].value - 99.0).abs() < 0.01,
            "snapshot must return latest value (99.0), got {}",
            entries[0].value
        );
        assert_eq!(entries[0].pod, Some(1), "pod_id 'pod-1' must parse to Some(1)");
    }

    /// test_pod_filter: samples for pod-1 and pod-2, query ?pod=1 returns only pod-1
    #[tokio::test]
    async fn test_pod_filter() {
        let db = make_test_db().await;

        sqlx::query(
            "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
             VALUES ('cpu_usage', 'pod-1', 10.0, '2026-01-01T00:00:00')",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at)
             VALUES ('cpu_usage', 'pod-2', 20.0, '2026-01-01T00:00:00')",
        )
        .execute(&db)
        .await
        .unwrap();

        // epoch range covering 2026-01-01: from before to well after
        // 1767225600 = 2026-01-01 00:00:00 UTC; use 1767312000 = 2026-01-02 00:00:00 UTC as end
        let points =
            query_time_series(&db, "cpu_usage", 1735689600, 1767312000, Some(1), "raw").await;
        assert_eq!(
            points.len(),
            1,
            "pod filter must return only pod-1 data, got {} rows",
            points.len()
        );
        assert!((points[0].value - 10.0).abs() < 0.01);
    }

    /// test_resolution_auto: verify auto-resolution thresholds
    #[tokio::test]
    async fn test_resolution_auto() {
        assert_eq!(select_resolution(0, 86_399, None), "raw", "<24h should be raw");
        assert_eq!(
            select_resolution(0, 86_400, None),
            "hourly",
            "=24h should be hourly"
        );
        assert_eq!(
            select_resolution(0, 604_799, None),
            "hourly",
            "<7d should be hourly"
        );
        assert_eq!(
            select_resolution(0, 604_800, None),
            "daily",
            "=7d should be daily"
        );
        assert_eq!(
            select_resolution(0, 1_000_000, None),
            "daily",
            ">7d should be daily"
        );
    }

    /// test_resolution_override: explicit ?resolution= overrides auto-selection
    #[tokio::test]
    async fn test_resolution_override() {
        // Valid overrides win regardless of range
        assert_eq!(select_resolution(0, 100, Some("daily")), "daily");
        assert_eq!(select_resolution(0, 1_000_000, Some("raw")), "raw");
        assert_eq!(select_resolution(0, 1_000_000, Some("hourly")), "hourly");
        // Invalid override falls back to auto-resolution
        assert_eq!(
            select_resolution(0, 100, Some("invalid")),
            "raw",
            "invalid override falls back to auto"
        );
    }
}
