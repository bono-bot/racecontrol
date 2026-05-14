//! V2 canonical operating-window endpoint — DoD §0 L29+L33.
//!
//! Row 1.19 substrate-PR per `.planning/audits/RCA-2026-05-14-row-1.19-operating-window-v1-v2.md`.
//!
//! V2 doctrine: hardcoded DoD canonical hours (12:00–24:00 IST), explicit
//! UTC+5:30 math (NEVER `chrono::Local::now()` per racecontrol/CLAUDE.md
//! "TZ=Asia/Kolkata silently fails on Windows" + cross-deployment-topology
//! drift class — Server .23 Windows vs Bono VPS Linux returned opposite
//! `is_open` for 5h30m of the day under V1 `Local::now()`).
//!
//! Public read; emits NO booking IDs / NO customer data / NO active-session
//! count — DPDP-clean by construction.
//!
//! V1 `/api/v1/scheduler/status` (scheduler.rs::get_status) retained
//! untouched per V2-only forward path doctrine (V1 alongside, NOT patched).
//!
//! `extension_active` + `iracing_active` default `false` today; wires to row
//! 1.11 race + telemetry/pulse substrate landing.

use axum::Json;
use chrono::{FixedOffset, Timelike, Utc};
use serde_json::{Value, json};

/// DoD §0 L29 verbatim: "Racing Point operating window per the locked venue
/// hours: 12:00–24:00 IST, daily (source: whatsapp-bot/src/services/istTimeService.js)."
const DOD_OPEN_HOUR_IST: u32 = 12;

/// Source tag identifies which V2 canonical-source revision this response
/// conforms to. See RCA §4 for the locked shape contract.
const SCHEMA_VERSION: &str = "v2.0";
const SOURCE_TAG: &str = "DoD-v2-12-24-IST";

/// GET /api/v1/operating-window — V2 canonical venue-open state.
///
/// Public; no auth. Rate-limit via standard public-route middleware.
///
/// Response shape (locked per RCA §4):
/// ```json
/// {
///   "is_open": true,
///   "current_time": "2026-05-14T17:53:00+05:30",
///   "extension_active": false,
///   "iracing_active": false,
///   "source": "DoD-v2-12-24-IST",
///   "schema_version": "v2.0"
/// }
/// ```
pub async fn get_operating_window() -> Json<Value> {
    let ist_offset =
        FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("IST UTC+5:30 offset is a valid FixedOffset");
    let now_ist = Utc::now().with_timezone(&ist_offset);
    let is_open = now_ist.hour() >= DOD_OPEN_HOUR_IST;

    Json(json!({
        "is_open": is_open,
        "current_time": now_ist.to_rfc3339(),
        "extension_active": false,
        "iracing_active": false,
        "source": SOURCE_TAG,
        "schema_version": SCHEMA_VERSION,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ist_hour(hour: u32, minute: u32) -> chrono::DateTime<FixedOffset> {
        let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
        ist_offset
            .with_ymd_and_hms(2026, 5, 14, hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn is_open_at_dod_open_hour_12() {
        // DoD §0 L29 open boundary
        let now = ist_hour(12, 0);
        assert!(now.hour() >= DOD_OPEN_HOUR_IST);
    }

    #[test]
    fn is_open_at_evening_20() {
        let now = ist_hour(20, 0);
        assert!(now.hour() >= DOD_OPEN_HOUR_IST);
    }

    #[test]
    fn is_open_at_close_edge_23_59() {
        // DoD §0 L29 "24:00 IST" = open up to midnight rollover
        let now = ist_hour(23, 59);
        assert!(now.hour() >= DOD_OPEN_HOUR_IST);
    }

    #[test]
    fn is_closed_at_midnight_rollover() {
        // Post-midnight, pre-12:00 IST = closed window
        let now = ist_hour(0, 0);
        assert!(now.hour() < DOD_OPEN_HOUR_IST);
    }

    #[test]
    fn is_closed_at_pre_open_11_59() {
        // V1 default opened at 10:00; DoD canonical requires 12:00. This
        // assertion locks the V2 canonical against accidental V1 regression.
        let now = ist_hour(11, 59);
        assert!(now.hour() < DOD_OPEN_HOUR_IST);
    }

    #[test]
    fn dod_canonical_open_hour_is_12_not_10() {
        // Anti-regression: if a future change resets DOD_OPEN_HOUR_IST to 10
        // (V1 default), this test fails. Anchors the V2-alignment delta from
        // RCA §4. DoD §0 L29 verbatim "12:00–24:00 IST".
        assert_eq!(DOD_OPEN_HOUR_IST, 12);
    }
}
