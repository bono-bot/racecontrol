---
phase: 365-ai-behavior-validation-via-mma
plan: "03"
subsystem: api
tags: [rust, websocket, dashboard-event, anomaly-detection, protocol]

requires:
  - phase: 365-01
    provides: ai_behavior_batch.rs module, tier_for_level, AnomalyDirection, check_anomaly, check_and_broadcast_anomaly, ac_server.rs session end hook
provides:
  - AiBehaviorAnomaly variant in DashboardEvent enum (rc-common/src/protocol.rs)
  - Fields: pod_id, session_id, car, track, difficulty_tier, expected_p10_ms, expected_p90_ms, observed_median_ms, observed_lap_count, direction, timestamp
  - Roundtrip serialization test for AiBehaviorAnomaly
  - read_kb_entry() KB TOML reader
  - check_anomaly() pure function with AnomalyDirection (TooSlow/TooFast/None)
  - check_and_broadcast_anomaly() session-end broadcaster
  - Anomaly check wired into ac_server.rs alongside collector hook
affects: []

tech-stack:
  added: []
  patterns:
    - "WS event for anomaly: DashboardEvent::AiBehaviorAnomaly broadcast via state.dashboard_tx.send()"
    - "Anomaly direction: too_slow if median > p90, too_fast if median < p10, None if within band"
    - "KB read: parse TOML sections dynamically, each top-level key = tier name"
    - "No anomaly if KB file absent: silently skip (debug log only)"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/ai_behavior_batch.rs
    - crates/racecontrol/src/ac_server.rs

key-decisions:
  - "AiBehaviorAnomaly is an additive DashboardEvent variant -- backward-compatible (old clients ignore unknown types)"
  - "No anomaly fired if KB file does not exist for (car, track) pair"
  - "direction field: 'too_slow' | 'too_fast' (string, not enum) for WS JSON compatibility"
  - "feature flag phase365_anomaly_detection kill-switch checked at broadcast entry"
  - "Pod binary rebuild NOT strictly required for Phase 365 correctness -- AiBehaviorAnomaly is server-to-dashboard only"

requirements-completed: [GLD-E-04]

duration: estimated
completed: 2026-04-11
---

# Phase 365 Plan 03: Live anomaly detector + WS event Summary

**AiBehaviorAnomaly DashboardEvent variant added to protocol.rs, with KB TOML reader and p10-p90 band check that broadcasts WS anomaly alert when session-end AI median falls outside expected tier band**

## Performance

- **Duration:** committed as part of prior agent session
- **Completed:** 2026-04-11
- **Tasks:** 3 (DashboardEvent variant, KB reader + anomaly check, ac_server.rs wiring)
- **Files modified:** 3

## Accomplishments

- Added `AiBehaviorAnomaly` variant to `DashboardEvent` enum in `rc-common/src/protocol.rs` with 11 fields (pod_id, session_id, car, track, difficulty_tier, expected_p10_ms, expected_p90_ms, observed_median_ms, observed_lap_count, direction, timestamp)
- Added `test_ai_behavior_anomaly_roundtrip` unit test verifying serde serialize/deserialize correctness with all fields
- Implemented `read_kb_entry()` that reads `.planning/kb/ai-behavior/{car_slug}-{track_slug}.toml` and parses all tier sections into a KbEntry
- Implemented `AnomalyDirection` enum (TooSlow/TooFast/None) and `check_anomaly()` pure function
- Implemented `check_and_broadcast_anomaly()` that reads KB, checks band, logs WARN and broadcasts `DashboardEvent::AiBehaviorAnomaly` on deviation
- All anomaly logic (read_kb_entry, check_anomaly, check_and_broadcast_anomaly) already present in 365-01 commit (`773fff93`) since all Phase 365 code was bundled into ai_behavior_batch.rs at that point
- 4 anomaly tests pass: test_anomaly_too_slow, test_anomaly_too_fast, test_no_anomaly_within_band, test_no_kb_no_anomaly
- Protocol change is additive and backward-compatible (new serde tag ignored by old clients)

## Task Commits

1. **Task 365-03-01+02+03: Protocol variant + KB reader + anomaly broadcaster** - `39674046` (feat)

## Files Created/Modified

- `crates/rc-common/src/protocol.rs` - Added AiBehaviorAnomaly variant to DashboardEvent enum + roundtrip test
- `crates/racecontrol/src/ai_behavior_batch.rs` - Added read_kb_entry(), AnomalyDirection, check_anomaly(), check_and_broadcast_anomaly() (in 365-01 commit per bundled delivery)
- `crates/racecontrol/src/ac_server.rs` - Anomaly check wired alongside collector in session end hook (in 365-01 commit)

## Decisions Made

- `AiBehaviorAnomaly` uses `direction: String` ("too_slow"/"too_fast") rather than a serialized enum to keep WS JSON readable and stable across versions
- No event fired when KB file is absent — operational decision to avoid false positives during initial data collection period before first MMA batch run
- Anomaly check is additive to existing collect hook — same session-end code path, no architectural changes

## Deviations from Plan

None - all acceptance criteria met. Note: per commit structure, the anomaly detection code in ai_behavior_batch.rs was delivered in commit `773fff93` (365-01) rather than `39674046` (365-03) since the prior agent built the entire ai_behavior_batch.rs module in one pass. The protocol.rs change (AiBehaviorAnomaly variant + test) is correctly in `39674046`.

## Issues Encountered

None.

## Next Phase Readiness

- Phase 365 is complete: collector (GLD-E-01) + MMA batch (GLD-E-02) + KB files (GLD-E-03) + anomaly detector (GLD-E-04) all shipped
- Deploy required: racecontrol server binary must be rebuilt and deployed to server .23 and cloud (Bono VPS) for Phase 365 to go live
- rc-agent pod rebuild NOT strictly required for Phase 365 (AiBehaviorAnomaly is server-to-dashboard only), but recommended for next fleet deploy to ensure rc-common is in sync

---
*Phase: 365-ai-behavior-validation-via-mma*
*Completed: 2026-04-11*
