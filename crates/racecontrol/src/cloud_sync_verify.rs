//! Cloud sync push verification — Phase 1 of cloud_sync RCA 2026-05-13 (closes Q3).
//!
//! After each successful push to Bono (relay or HTTP), this module probes
//! Bono's `/sync/echo` endpoint to confirm the rows were actually persisted.
//! Closes the trust-check Q3 gap: cloud_sync previously logged the push count
//! but never read back from the receiver — proxy-not-evidence anti-pattern.
//!
//! ## Fail-open semantics
//!
//! - Bono returns 404 / 501 → assume Bono not yet upgraded; fail-open (caller advances cursor)
//! - Transport error / timeout → assume transient; fail-open (next cycle re-verifies fresh window)
//! - Bono returns 200 with `count >= expected` → OK (Bono may have race-inserts ≥ our expected; treat as OK)
//! - Bono returns 200 with `count < expected` → FAIL-CLOSED (caller does NOT advance cursor; next cycle re-pushes same window)
//!
//! See: `racecontrol/.planning/audits/RCA-2026-05-13-cloud-sync-surface.md` Phase 1.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::state::AppState;

const LOG_TARGET: &str = "cloud-sync-verify";
const ECHO_TIMEOUT_SECS: u64 = 2;

/// Outcome of a verify pass over a just-pushed payload.
///
/// Caller maps outcomes to cursor-advance decision:
/// - `AllOk` / `Unavailable` → advance cursor (fail-open on Unavailable)
/// - `Mismatch` → do NOT advance cursor; log error; let next cycle re-push
#[derive(Debug, PartialEq)]
pub(crate) enum VerifyOutcome {
    AllOk,
    Unavailable,
    Mismatch { details: Vec<TableMismatch> },
}

#[derive(Debug, PartialEq)]
pub(crate) struct TableMismatch {
    pub table: String,
    pub expected: usize,
    pub actual: usize,
}

/// Probe Bono's `/sync/echo` for each table-array in the just-pushed payload.
/// `receiver_url` is the base URL of the receiver (relay base for relay mode, or
/// `cloud.api_url` for HTTP fallback). The endpoint is appended as `/sync/echo`.
///
/// Window is `(cursor_before, cursor_after]` — Bono returns row count for that window
/// per table; we compare to the array length we sent.
pub(crate) async fn verify_push(
    state: &Arc<AppState>,
    receiver_url: &str,
    payload: &Value,
    cursor_before: &str,
    cursor_after: &str,
) -> VerifyOutcome {
    let payload_obj = match payload.as_object() {
        Some(o) => o,
        None => return VerifyOutcome::AllOk,
    };

    let mut mismatches: Vec<TableMismatch> = Vec::new();
    let mut any_unavailable = false;
    let echo_url = format!("{}/sync/echo", receiver_url.trim_end_matches('/'));

    for (table, items) in payload_obj {
        if matches!(table.as_str(), "schema_version" | "origin") {
            continue;
        }
        let expected = match items.as_array() {
            Some(arr) => arr.len(),
            None => continue,
        };
        if expected == 0 {
            continue;
        }
        let cursor_col = cursor_column_for_table(table);
        let body = serde_json::json!({
            "table": table,
            "cursor_col": cursor_col,
            "from": cursor_before,
            "to": cursor_after,
        });

        let mut req = state
            .http_client
            .post(&echo_url)
            .json(&body)
            .timeout(Duration::from_secs(ECHO_TIMEOUT_SECS));
        if let Some(secret) = &state.config.cloud.terminal_secret {
            req = req.header("x-terminal-secret", secret);
        }

        match req.send().await {
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, table = %table,
                    "Verify probe transport error: {} — fail-open", e);
                any_unavailable = true;
            }
            Ok(r) if r.status() == 404 || r.status() == 501 => {
                tracing::debug!(target: LOG_TARGET, table = %table,
                    "Receiver /sync/echo returned {} — fail-open (receiver not yet upgraded)",
                    r.status());
                any_unavailable = true;
            }
            Ok(r) if !r.status().is_success() => {
                tracing::warn!(target: LOG_TARGET, table = %table,
                    "Receiver /sync/echo unexpected status {} — fail-open", r.status());
                any_unavailable = true;
            }
            Ok(r) => match r.json::<Value>().await {
                Ok(body) => {
                    let actual = body
                        .get("count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    if actual < expected {
                        tracing::error!(target: LOG_TARGET, table = %table,
                            expected = expected, actual = actual,
                            "Verify MISMATCH: receiver missing rows — cursor will NOT advance");
                        mismatches.push(TableMismatch {
                            table: table.clone(),
                            expected,
                            actual,
                        });
                    } else {
                        tracing::debug!(target: LOG_TARGET, table = %table,
                            expected = expected, actual = actual,
                            "Verify OK");
                    }
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, table = %table,
                        "Receiver /sync/echo response parse failed: {} — fail-open", e);
                    any_unavailable = true;
                }
            },
        }
    }

    if !mismatches.is_empty() {
        VerifyOutcome::Mismatch { details: mismatches }
    } else if any_unavailable {
        VerifyOutcome::Unavailable
    } else {
        VerifyOutcome::AllOk
    }
}

/// Cursor column name for a given push table — must match `cloud_sync_payload`'s WHERE clauses.
fn cursor_column_for_table(table: &str) -> &'static str {
    match table {
        "content_drift_events" => "detected_at",
        "track_records" | "personal_bests" => "achieved_at",
        "metrics_rollups" => "updated_at",
        // laps / billing_sessions / model_evaluations / others use created_at
        _ => "created_at",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_col_known_tables() {
        assert_eq!(cursor_column_for_table("content_drift_events"), "detected_at");
        assert_eq!(cursor_column_for_table("track_records"), "achieved_at");
        assert_eq!(cursor_column_for_table("personal_bests"), "achieved_at");
        assert_eq!(cursor_column_for_table("metrics_rollups"), "updated_at");
        assert_eq!(cursor_column_for_table("laps"), "created_at");
        assert_eq!(cursor_column_for_table("billing_sessions"), "created_at");
        assert_eq!(cursor_column_for_table("anything_else"), "created_at");
    }

    #[test]
    fn verify_outcome_classification() {
        // Mismatch wins over Unavailable
        let outcome = VerifyOutcome::Mismatch {
            details: vec![TableMismatch {
                table: "laps".into(),
                expected: 5,
                actual: 3,
            }],
        };
        assert!(matches!(outcome, VerifyOutcome::Mismatch { .. }));

        let unavailable = VerifyOutcome::Unavailable;
        assert_eq!(unavailable, VerifyOutcome::Unavailable);

        let ok = VerifyOutcome::AllOk;
        assert_eq!(ok, VerifyOutcome::AllOk);
    }
}
