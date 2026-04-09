//! # session_audit.rs — Phase 363 Data Recording Verification
//!
//! Provides GLD-C-01 (per-session lap audit) and GLD-C-02 (telemetry completeness check).
//! Called from `post_session_hooks()` in billing.rs at session end.
//!
//! ## Design decisions
//! - D-01: Conservative floor heuristic for expected laps (no AI-tier data yet).
//! - D-02: Directional flag — only "too few" laps triggers UNDER_RECORDED.
//! - D-04: Telemetry completeness = seconds_with_any_packet / total_session_seconds * 100.
//! - D-05: Coverage histogram maintained in BillingTimer; lost on crash → NULL.
//! - D-06: suspect = lap_flag != OK OR coverage < 80%; suspect_reasons is a JSON array.
//! - Feature flag `phase363_session_audit` (default true) provides a kill switch.

use sqlx::SqlitePool;
use tokio::sync::RwLock;
use std::collections::HashMap;

// ─── Public types ─────────────────────────────────────────────────────────────

/// GLD-C-01: Lap count audit result stored in billing_sessions.lap_count_flag.
/// Values must match the TEXT stored in the DB column exactly (see as_str()).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LapCountFlag {
    /// Actual laps >= 90% of expected laps.
    Ok,
    /// Actual laps < 90% of expected (too few recorded).
    UnderRecorded,
    /// Session ended before the audit ran (crash, shutdown, etc.).
    Unverified,
}

impl LapCountFlag {
    /// DB-canonical string representation. Matches DEFAULT value in migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            LapCountFlag::Ok => "OK",
            LapCountFlag::UnderRecorded => "UNDER_RECORDED",
            LapCountFlag::Unverified => "UNVERIFIED",
        }
    }
}

// ─── Pure functions ───────────────────────────────────────────────────────────

/// GLD-C-01 D-01: Conservative floor heuristic for expected lap count.
///
/// - "hotlap" sessions: `max(1, session_minutes / 2)` (hotlap sessions have shorter laps)
/// - All other types (trackday, practice, race, unknown): `max(1, session_minutes / 3)`
///
/// Phase 365 will refine this to per-car/track AI-tier-aware targets once the data exists.
/// For now, the conservative floor catches catastrophic under-recording (0 laps in 30 min)
/// without false-positive flagging slow drivers.
pub fn expected_laps(session_type: &str, minutes: u32) -> u32 {
    match session_type.to_lowercase().as_str() {
        "hotlap" => (minutes / 2).max(1),
        _ => (minutes / 3).max(1),
    }
}

/// GLD-C-01 D-02: Compute the directional lap count flag.
///
/// Only under-recording is flagged. Recording more laps than expected is fine
/// (the driver was fast) — per D-02 "directional" design.
pub fn compute_lap_flag(expected: u32, actual: u32) -> LapCountFlag {
    if expected == 0 {
        return LapCountFlag::Unverified;
    }
    if (actual as f64) < (expected as f64) * 0.9 {
        LapCountFlag::UnderRecorded
    } else {
        LapCountFlag::Ok
    }
}

/// GLD-C-02 D-04: Compute telemetry coverage percentage.
///
/// Returns the percentage of session-seconds that had at least one telemetry packet.
/// Returns 0.0 if total_seconds is 0 (avoids division by zero).
pub fn coverage_pct(seconds_covered: u32, total_seconds: u32) -> f64 {
    if total_seconds == 0 {
        return 0.0;
    }
    (seconds_covered as f64 / total_seconds as f64) * 100.0
}

/// GLD-C-02 D-06: Compute the suspect flag and reason list.
///
/// Returns `(suspect: bool, reasons: Vec<&'static str>)` where reasons is the JSON array
/// that gets stored in billing_sessions.suspect_reasons.
///
/// Trigger conditions:
/// - "under_recorded" if lap_flag == UnderRecorded
/// - "telemetry_low" if coverage < 80% (threshold per D-04)
/// - "unverified" if coverage is None OR lap_flag == Unverified
pub fn compute_suspect(
    lap_flag: LapCountFlag,
    coverage: Option<f64>,
) -> (bool, Vec<&'static str>) {
    let mut reasons: Vec<&'static str> = Vec::new();

    if lap_flag == LapCountFlag::UnderRecorded {
        reasons.push("under_recorded");
    }

    if lap_flag == LapCountFlag::Unverified {
        reasons.push("unverified");
    }

    match coverage {
        Some(pct) if pct < 80.0 => reasons.push("telemetry_low"),
        None => {
            // Only add "unverified" once (might already be pushed for lap_flag)
            if !reasons.contains(&"unverified") {
                reasons.push("unverified");
            }
        }
        _ => {}
    }

    let suspect = !reasons.is_empty();
    (suspect, reasons)
}

// ─── Async orchestrator ────────────────────────────────────────────────────────

/// GLD-C-01/C-02 orchestrator. Called from `billing::post_session_hooks()` at session end.
///
/// 1. Checks phase363_session_audit feature flag (kill switch — returns early if disabled).
/// 2. Reads billing_session allocated_seconds (session type defaults to "trackday").
/// 3. Counts laps for this billing session via `SELECT COUNT(*) FROM laps WHERE session_id = ?`.
/// 4. Computes expected laps, lap flag, coverage, suspect flag.
/// 5. UPDATEs billing_sessions with all audit columns.
///
/// `seconds_covered` is the len() of `BillingTimer.telemetry_seconds_covered` at session end.
/// If the server crashed mid-session, the HashSet is lost and seconds_covered == 0, which
/// maps to coverage=None (UNVERIFIED path per D-05).
pub async fn run_session_audit(
    pool: &SqlitePool,
    flags: &RwLock<HashMap<String, crate::flags::FeatureFlagRow>>,
    billing_session_id: &str,
    seconds_covered: u32,
) -> anyhow::Result<()> {
    // a. Kill switch: check feature flag. Snapshot + drop guard before any DB await.
    let audit_enabled = {
        let guard = flags.read().await;
        guard
            .get("phase363_session_audit")
            .map(|r| r.enabled)
            .unwrap_or(true) // Intentional default: true. Flag missing = treat as enabled.
    }; // guard dropped here — CLAUDE.md never-hold-lock-across-await

    if !audit_enabled {
        tracing::debug!(
            billing_session_id = %billing_session_id,
            "phase363_session_audit flag disabled — skipping session audit"
        );
        return Ok(());
    }

    // b. Read allocated_seconds from billing_sessions.
    // session_type is not stored on billing_sessions — default to "trackday" (conservative).
    let session_row: Option<(i64,)> = sqlx::query_as(
        "SELECT allocated_seconds FROM billing_sessions WHERE id = ?",
    )
    .bind(billing_session_id)
    .fetch_optional(pool)
    .await?;

    let (session_type, allocated_seconds) = match session_row {
        Some((secs,)) => ("trackday", secs),
        None => {
            tracing::warn!(
                billing_session_id = %billing_session_id,
                "run_session_audit: billing_session not found — skipping"
            );
            return Ok(());
        }
    };

    // c. Compute expected laps from session minutes.
    let minutes = (allocated_seconds / 60) as u32;
    let expected = expected_laps(session_type, minutes);

    // d/e. Count actual laps (laps.session_id holds billing_session_id per research).
    let actual_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM laps WHERE session_id = ?",
    )
    .bind(billing_session_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let actual = actual_count as u32;

    // f. Compute lap count flag.
    let lap_flag = compute_lap_flag(expected, actual);

    // g. Compute coverage (D-05: if seconds_covered == 0 AND minutes > 0, treat as crash path → None).
    let total_seconds = (allocated_seconds as u32).min(minutes * 60);
    let coverage: Option<f64> = if seconds_covered == 0 && minutes > 0 {
        None // Crash path or no telemetry at all — UNVERIFIED
    } else if total_seconds > 0 {
        Some(coverage_pct(seconds_covered, total_seconds))
    } else {
        None
    };

    // h. Compute suspect flag.
    let (suspect, reasons) = compute_suspect(lap_flag.clone(), coverage);
    let reasons_json = serde_json::to_string(&reasons).unwrap_or_else(|_| "[]".to_string());

    // i. Write audit results back to billing_sessions.
    sqlx::query(
        "UPDATE billing_sessions
         SET lap_count_expected = ?,
             lap_count_actual = ?,
             lap_count_flag = ?,
             telemetry_coverage_pct = ?,
             suspect = ?,
             suspect_reasons = ?
         WHERE id = ?",
    )
    .bind(expected as i64)
    .bind(actual as i64)
    .bind(lap_flag.as_str())
    .bind(coverage)
    .bind(suspect as i64)
    .bind(&reasons_json)
    .bind(billing_session_id)
    .execute(pool)
    .await?;

    tracing::info!(
        billing_session_id = %billing_session_id,
        expected_laps = expected,
        actual_laps = actual,
        lap_flag = lap_flag.as_str(),
        telemetry_coverage_pct = coverage,
        suspect = suspect,
        suspect_reasons = %reasons_json,
        "session_audit complete"
    );

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
}
