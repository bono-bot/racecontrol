use super::*;
    use sqlx::{sqlite::SqlitePool, Row};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Create an in-memory SQLite pool with the minimal schema session_audit reads/writes.
    /// Mirrors the `create_test_db()` pattern at billing.rs:7562.
    async fn audit_test_pool() -> SqlitePool {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let tid = std::thread::current().id();
        let path = std::env::temp_dir()
            .join(format!("rc_session_audit_test_{:?}_{}.db", tid, nonce))
            .to_string_lossy()
            .to_string();

        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
        )
        .await
        .expect("test pool connect failed");

        // Minimal schema: only the tables session_audit reads/writes
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL DEFAULT 'test-driver',
                pod_id TEXT NOT NULL DEFAULT 'pod1',
                pricing_tier_id TEXT NOT NULL DEFAULT 'tier1',
                allocated_seconds INTEGER NOT NULL DEFAULT 1800,
                driving_seconds INTEGER DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'completed',
                lap_count_expected INTEGER,
                lap_count_actual INTEGER,
                lap_count_flag TEXT DEFAULT 'UNVERIFIED',
                telemetry_coverage_pct REAL,
                suspect BOOLEAN NOT NULL DEFAULT 0,
                suspect_reasons TEXT,
                csv_fallback_received_at TEXT,
                lap_reject_grace_until TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create billing_sessions failed");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS laps (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                driver_id TEXT,
                lap_number INTEGER,
                lap_time_ms INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create laps failed");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS feature_flags (
                name TEXT PRIMARY KEY,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                default_value BOOLEAN NOT NULL DEFAULT 1,
                overrides TEXT NOT NULL DEFAULT '{}'
            )",
        )
        .execute(&pool)
        .await
        .expect("create feature_flags failed");

        // Seed the phase363 flag enabled
        sqlx::query(
            "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
             VALUES ('phase363_session_audit', 1, 1, '{}')",
        )
        .execute(&pool)
        .await
        .expect("seed feature_flag failed");

        pool
    }

    /// Build a HashMap<String, FeatureFlagRow> with the given flag value.
    fn make_flags(enabled: bool) -> HashMap<String, crate::flags::FeatureFlagRow> {
        use crate::flags::FeatureFlagRow;
        let mut map = HashMap::new();
        map.insert(
            "phase363_session_audit".to_string(),
            FeatureFlagRow {
                name: "phase363_session_audit".to_string(),
                enabled,
                default_value: true,
                overrides: "{}".to_string(),
                version: 1,
                updated_at: None,
            },
        );
        map
    }

    // ─── Pure function tests ──────────────────────────────────────────────────

    /// Test all expected_laps cases from the behavior block.
    #[test]
    fn test_lap_heuristic() {
        // Trackday/practice: floor(minutes / 3)
        assert_eq!(expected_laps("trackday", 30), 10);
        assert_eq!(expected_laps("practice", 30), 10);
        // Hotlap: floor(minutes / 2)
        assert_eq!(expected_laps("hotlap", 30), 15);
        // Edge: 0 minutes → max(1, 0) = 1
        assert_eq!(expected_laps("trackday", 0), 1);
        assert_eq!(expected_laps("hotlap", 0), 1);
        // Edge: 1 minute
        assert_eq!(expected_laps("trackday", 1), 1);
    }

    #[test]
    fn test_lap_audit_under_recorded() {
        // 8 actual < 10 * 0.9 = 9 → UNDER_RECORDED
        assert_eq!(compute_lap_flag(10, 8), LapCountFlag::UnderRecorded);
    }

    #[test]
    fn test_lap_audit_ok_over_expected() {
        // 12 actual > 10 expected — directional, over is fine (D-02)
        assert_eq!(compute_lap_flag(10, 12), LapCountFlag::Ok);
    }

    #[test]
    fn test_lap_audit_ok_boundary() {
        // 9 actual == 10 * 0.9 = 9.0 → OK (boundary, not strictly less)
        assert_eq!(compute_lap_flag(10, 9), LapCountFlag::Ok);
    }

    #[test]
    fn test_telemetry_coverage_suspect() {
        let pct = coverage_pct(1200, 1800);
        assert!((pct - 66.666).abs() < 0.01, "expected ~66.67, got {}", pct);
        let (suspect, reasons) = compute_suspect(LapCountFlag::Ok, Some(pct));
        assert!(suspect, "should be suspect at 66.7%");
        assert!(reasons.contains(&"telemetry_low"), "missing telemetry_low reason");
    }

    #[test]
    fn test_telemetry_coverage_ok() {
        let pct = coverage_pct(1500, 1800);
        assert!((pct - 83.333).abs() < 0.01, "expected ~83.33, got {}", pct);
        let (suspect, reasons) = compute_suspect(LapCountFlag::Ok, Some(pct));
        assert!(!suspect, "should NOT be suspect at 83.3%");
        assert!(reasons.is_empty(), "reasons should be empty, got: {:?}", reasons);
    }

    #[test]
    fn test_suspect_reasons_multi() {
        // UNDER_RECORDED + low coverage → two reasons
        let (suspect, reasons) = compute_suspect(LapCountFlag::UnderRecorded, Some(66.0));
        assert!(suspect);
        assert!(reasons.contains(&"under_recorded"), "missing under_recorded");
        assert!(reasons.contains(&"telemetry_low"), "missing telemetry_low");
    }

    #[test]
    fn test_crash_unverified() {
        // Unverified lap flag + None coverage → suspect with "unverified" reason
        let (suspect, reasons) = compute_suspect(LapCountFlag::Unverified, None);
        assert!(suspect, "crash path should be suspect");
        assert!(reasons.contains(&"unverified"), "missing unverified reason");
    }

    // ─── Integration tests ────────────────────────────────────────────────────

    /// Feature flag kill switch: phase363_session_audit=false → columns stay NULL.
    #[tokio::test]
    async fn test_feature_flag_kill_switch() {
        let pool = audit_test_pool().await;
        let session_id = "test-session-killswitch";
        sqlx::query(
            "INSERT INTO billing_sessions (id, allocated_seconds) VALUES (?, 1800)",
        )
        .bind(session_id)
        .execute(&pool)
        .await
        .expect("insert session failed");

        // Flag disabled
        let flags = RwLock::new(make_flags(false));
        run_session_audit(&pool, &flags, session_id, 1000)
            .await
            .expect("run_session_audit should return Ok even when disabled");

        // Columns should remain NULL (not updated)
        let row = sqlx::query(
            "SELECT lap_count_flag, telemetry_coverage_pct FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("fetch session failed");

        let flag: Option<String> = row.try_get("lap_count_flag").unwrap_or(None);
        let cov: Option<f64> = row.try_get("telemetry_coverage_pct").unwrap_or(None);
        assert_eq!(
            flag.as_deref(),
            Some("UNVERIFIED"),
            "flag should stay at default UNVERIFIED when audit disabled"
        );
        assert!(cov.is_none(), "telemetry_coverage_pct should be NULL when audit disabled");
    }

    /// Full integration: billing_session (trackday, 30 min) + 5 laps → UNDER_RECORDED.
    #[tokio::test]
    async fn test_run_audit_integration() {
        let pool = audit_test_pool().await;
        let session_id = "test-session-integration";

        sqlx::query(
            "INSERT INTO billing_sessions (id, allocated_seconds, status) VALUES (?, 1800, 'completed')",
        )
        .bind(session_id)
        .execute(&pool)
        .await
        .expect("insert session failed");

        // Insert 5 laps with session_id = billing_session_id (per research Open Question 1)
        for i in 0..5u32 {
            sqlx::query(
                "INSERT INTO laps (id, session_id, lap_number) VALUES (?, ?, ?)",
            )
            .bind(format!("lap-{}-{}", session_id, i))
            .bind(session_id)
            .bind(i as i64)
            .execute(&pool)
            .await
            .expect("insert lap failed");
        }

        // 1000 seconds covered out of 1800 → ~55.6% coverage → suspect=true
        let flags = RwLock::new(make_flags(true));
        run_session_audit(&pool, &flags, session_id, 1000)
            .await
            .expect("run_session_audit failed");

        let row = sqlx::query(
            "SELECT lap_count_actual, lap_count_expected, lap_count_flag, suspect
             FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .expect("fetch session failed");

        let actual: i64 = row.try_get("lap_count_actual").unwrap();
        let expected_col: i64 = row.try_get("lap_count_expected").unwrap();
        let flag: String = row.try_get("lap_count_flag").unwrap();
        let suspect: i64 = row.try_get("suspect").unwrap();

        assert_eq!(actual, 5, "lap_count_actual should be 5");
        assert_eq!(expected_col, 10, "lap_count_expected should be 10 for 30min trackday");
        assert_eq!(flag, "UNDER_RECORDED", "5 laps < 90% of 10 expected → UNDER_RECORDED");
        assert_eq!(suspect, 1, "should be suspect (UNDER_RECORDED + low coverage)");
    }
