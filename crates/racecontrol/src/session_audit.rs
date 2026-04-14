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

#[cfg(test)]
#[path = "session_audit_tests.rs"]
mod tests;
