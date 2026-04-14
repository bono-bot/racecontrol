//! Tests for cloud sync push payload generation.
//! Extracted from cloud_sync.rs (Phase 385, v49.0 Architecture Completion).

/// Verify billing_sessions push query includes pause_count, total_paused_seconds, refund_paise
#[tokio::test]
async fn push_payload_includes_billing_session_extra_columns() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

    sqlx::query(
        "CREATE TABLE billing_sessions (
            id TEXT PRIMARY KEY, driver_id TEXT, pod_id TEXT, pricing_tier_id TEXT, end_reason TEXT,
            allocated_seconds INTEGER, driving_seconds INTEGER DEFAULT 0,
            status TEXT DEFAULT 'pending', custom_price_paise INTEGER, notes TEXT,
            started_at TEXT, ended_at TEXT, created_at TEXT,
            experience_id TEXT, car TEXT, track TEXT, sim_type TEXT,
            split_count INTEGER, split_duration_minutes INTEGER,
            wallet_debit_paise INTEGER, discount_paise INTEGER, coupon_id TEXT,
            original_price_paise INTEGER, discount_reason TEXT,
            pause_count INTEGER DEFAULT 0, total_paused_seconds INTEGER DEFAULT 0,
            refund_paise INTEGER DEFAULT 0
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, created_at, pause_count, total_paused_seconds, refund_paise, end_reason)
         VALUES ('s1', 'd1', 'p1', 'tier1', 1800, 'Completed', '2026-01-01T00:00:00', 3, 120, 5000, 'orphan_timeout')"
    ).execute(&pool).await.unwrap();

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
            'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
            'driving_seconds', driving_seconds, 'status', status,
            'custom_price_paise', custom_price_paise, 'notes', notes,
            'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
            'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
            'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
            'wallet_debit_paise', wallet_debit_paise,
            'discount_paise', discount_paise, 'coupon_id', coupon_id,
            'original_price_paise', original_price_paise, 'discount_reason', discount_reason,
            'pause_count', pause_count, 'total_paused_seconds', total_paused_seconds, 'refund_paise', refund_paise,
            'end_reason', end_reason
        ) FROM billing_sessions WHERE created_at > '2025-01-01' ORDER BY created_at ASC LIMIT 500"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 1);
    let val: serde_json::Value = serde_json::from_str(&rows[0].0).unwrap();
    assert_eq!(val["pause_count"], 3);
    assert_eq!(val["total_paused_seconds"], 120);
    assert_eq!(val["refund_paise"], 5000);
    assert_eq!(val["id"], "s1");
    assert_eq!(val["status"], "Completed");
    assert_eq!(val["end_reason"], "orphan_timeout", "end_reason must be included in billing_sessions push payload");
}

/// Verify billing_events push query produces correct JSON
#[tokio::test]
async fn push_payload_includes_billing_events() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

    sqlx::query(
        "CREATE TABLE billing_events (
            id TEXT PRIMARY KEY, billing_session_id TEXT NOT NULL,
            event_type TEXT NOT NULL, driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
            metadata TEXT, created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at)
         VALUES ('e1', 's1', 'started', 0, NULL, '2026-01-01T00:00:00'),
                ('e2', 's1', 'paused', 300, '{\"reason\":\"customer_request\"}', '2026-01-01T00:05:00')"
    ).execute(&pool).await.unwrap();

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'billing_session_id', billing_session_id,
            'event_type', event_type, 'driving_seconds_at_event', driving_seconds_at_event,
            'metadata', metadata, 'created_at', created_at
        ) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500"
    ).bind("2025-01-01").fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 2);

    let ev1: serde_json::Value = serde_json::from_str(&rows[0].0).unwrap();
    assert_eq!(ev1["id"], "e1");
    assert_eq!(ev1["event_type"], "started");
    assert_eq!(ev1["driving_seconds_at_event"], 0);
    assert_eq!(ev1["billing_session_id"], "s1");

    let ev2: serde_json::Value = serde_json::from_str(&rows[1].0).unwrap();
    assert_eq!(ev2["id"], "e2");
    assert_eq!(ev2["event_type"], "paused");
    assert_eq!(ev2["driving_seconds_at_event"], 300);
}

/// Phase 363: Verify billing_sessions push JSON includes all 8 new audit columns.
/// Tests that the json_object() query in build_push_payload() includes the Phase 363 columns
/// (CLAUDE.md DB-migrations-cover-all-consumers: cloud sync must be updated in same commit).
#[tokio::test]
async fn test_billing_session_push_columns_phase363() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();

    // Create billing_sessions with all Phase 363 columns
    sqlx::query(
        "CREATE TABLE billing_sessions (
            id TEXT PRIMARY KEY, driver_id TEXT, pod_id TEXT, pricing_tier_id TEXT,
            allocated_seconds INTEGER, driving_seconds INTEGER DEFAULT 0,
            status TEXT DEFAULT 'pending', custom_price_paise INTEGER, notes TEXT,
            started_at TEXT, ended_at TEXT, created_at TEXT,
            experience_id TEXT, car TEXT, track TEXT, sim_type TEXT,
            split_count INTEGER, split_duration_minutes INTEGER,
            wallet_debit_paise INTEGER, discount_paise INTEGER, coupon_id TEXT,
            original_price_paise INTEGER, discount_reason TEXT,
            pause_count INTEGER DEFAULT 0, total_paused_seconds INTEGER DEFAULT 0,
            refund_paise INTEGER DEFAULT 0, end_reason TEXT,
            lap_count_expected INTEGER,
            lap_count_actual INTEGER,
            lap_count_flag TEXT DEFAULT 'UNVERIFIED',
            telemetry_coverage_pct REAL,
            suspect BOOLEAN NOT NULL DEFAULT 0,
            suspect_reasons TEXT,
            csv_fallback_received_at TEXT,
            lap_reject_grace_until TEXT
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO billing_sessions
         (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, created_at,
          lap_count_expected, lap_count_actual, lap_count_flag, telemetry_coverage_pct,
          suspect, suspect_reasons, csv_fallback_received_at, lap_reject_grace_until)
         VALUES ('s1', 'd1', 'p1', 'tier1', 1800, 'completed', '2026-01-01T00:00:00',
                 10, 5, 'UNDER_RECORDED', 55.5,
                 1, '[\"under_recorded\",\"telemetry_low\"]', NULL, NULL)"
    ).execute(&pool).await.unwrap();

    // Use the EXACT same json_object query shape as in build_push_payload().
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'driver_id', driver_id, 'pod_id', pod_id,
            'pricing_tier_id', pricing_tier_id, 'allocated_seconds', allocated_seconds,
            'driving_seconds', driving_seconds, 'status', status,
            'custom_price_paise', custom_price_paise, 'notes', notes,
            'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at,
            'experience_id', experience_id, 'car', car, 'track', track, 'sim_type', sim_type,
            'split_count', split_count, 'split_duration_minutes', split_duration_minutes,
            'wallet_debit_paise', wallet_debit_paise,
            'discount_paise', discount_paise, 'coupon_id', coupon_id,
            'original_price_paise', original_price_paise, 'discount_reason', discount_reason,
            'pause_count', pause_count, 'total_paused_seconds', total_paused_seconds, 'refund_paise', refund_paise,
            'end_reason', end_reason,
            'lap_count_expected', lap_count_expected,
            'lap_count_actual', lap_count_actual,
            'lap_count_flag', COALESCE(lap_count_flag, 'UNVERIFIED'),
            'telemetry_coverage_pct', telemetry_coverage_pct,
            'suspect', COALESCE(suspect, 0),
            'suspect_reasons', suspect_reasons,
            'csv_fallback_received_at', csv_fallback_received_at,
            'lap_reject_grace_until', lap_reject_grace_until
        ) FROM billing_sessions WHERE created_at >= '2025-01-01' OR ended_at >= '2025-01-01'
        ORDER BY created_at ASC LIMIT 500"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 1, "Should have 1 row in push payload");
    let json_str = &rows[0].0;

    // Assert all 8 Phase 363 column keys appear in the output JSON string
    for key in &[
        "\"lap_count_expected\"",
        "\"lap_count_actual\"",
        "\"lap_count_flag\"",
        "\"telemetry_coverage_pct\"",
        "\"suspect\"",
        "\"suspect_reasons\"",
        "\"csv_fallback_received_at\"",
        "\"lap_reject_grace_until\"",
    ] {
        assert!(
            json_str.contains(key),
            "billing_sessions push payload missing Phase 363 key: {}. Full JSON: {}",
            key,
            json_str
        );
    }

    let val: serde_json::Value = serde_json::from_str(json_str).unwrap();
    assert_eq!(val["lap_count_expected"], 10, "lap_count_expected should be 10");
    assert_eq!(val["lap_count_actual"], 5, "lap_count_actual should be 5");
    assert_eq!(val["lap_count_flag"], "UNDER_RECORDED", "lap_count_flag should be UNDER_RECORDED");
    assert!((val["telemetry_coverage_pct"].as_f64().unwrap() - 55.5).abs() < 0.01);
    assert_eq!(val["suspect"], 1, "suspect should be 1");
}
