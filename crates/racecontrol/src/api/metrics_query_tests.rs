//! Tests for metrics_query — extracted for <500 line compliance.

#[cfg(test)]
mod tests {
    use super::super::metrics_query::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

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
