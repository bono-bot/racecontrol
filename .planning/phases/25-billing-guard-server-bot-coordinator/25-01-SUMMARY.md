---
phase: 25-billing-guard-server-bot-coordinator
plan: "01"
subsystem: billing + rc-agent
tags: [tdd, characterization-tests, wave-0, bill-01, failure-monitor]
dependency_graph:
  requires: []
  provides:
    - "FailureMonitorState.driving_state field (billing_guard.rs Wave 1 compile gate)"
    - "5 BILL-01 characterization tests (safety net for billing bot code)"
  affects:
    - "crates/rc-agent/src/failure_monitor.rs (struct field + Default)"
    - "crates/racecontrol/src/billing.rs (5 new tests in mod tests)"
tech_stack:
  added: []
  patterns:
    - "Characterization test pattern: pure HashMap/constant assertions documenting invariants"
key_files:
  modified:
    - crates/rc-agent/src/failure_monitor.rs
    - crates/racecontrol/src/billing.rs
decisions:
  - "driving_state field placed after recovery_in_progress — follows structural ordering convention"
  - "5 new tests use pure synchronous assertions (no async/tokio) — aligns with existing billing test style"
  - "stuck_session threshold 60s and idle drift threshold 300s documented as constants in test bodies — Wave 1 bot code must match these"
metrics:
  duration_min: 3
  completed_date: "2026-03-16"
  tasks_completed: 2
  files_modified: 2
---

# Phase 25 Plan 01: BILL-01 Characterization Tests + FailureMonitorState Field Summary

Wave 0 prerequisite gate satisfied: 5 characterization tests in billing.rs documenting bot-facing paths, plus `driving_state: Option<DrivingState>` field on `FailureMonitorState` enabling Wave 1 billing_guard.rs compilation.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add driving_state to FailureMonitorState | 730ad00 | crates/rc-agent/src/failure_monitor.rs |
| 2 | Write 5 BILL-01 characterization tests in billing.rs | c92ddb0 | crates/racecontrol/src/billing.rs |

## What Was Built

**Task 1 — FailureMonitorState.driving_state compile gate:**
- Added `use rc_common::types::DrivingState;` import to failure_monitor.rs
- Added `pub driving_state: Option<DrivingState>` field after `recovery_in_progress` with doc comment linking to billing_guard.rs Wave 1 use
- Updated `Default` impl to set `driving_state: None`
- Updated `test_failure_monitor_state_default` to assert `driving_state.is_none()`
- `cargo check -p rc-agent-crate` passes

**Task 2 — 5 BILL-01 characterization tests:**
1. `game_exit_while_billing_ends_session` — characterizes AcStatus::Off → active_timers lookup path: billing_active + game exits → session_id must be resolvable for end_session to fire
2. `idle_drift_condition_check` — documents 300s idle drift threshold (BILL-03); asserts DrivingState::Idle does NOT match Active
3. `end_session_removes_timer` — characterizes active_timers HashMap remove contract
4. `stuck_session_condition` — documents 60s stuck session threshold (BILL-02); asserts billing_active=true + game_pid=None satisfies condition
5. `start_session_inserts_timer` — characterizes start_session → active_timers insert required by recover_stuck_session(); verifies BillingTimer::dummy sets pod_id and session_id correctly

All 5 tests are pure synchronous assertions using `BillingTimer::dummy()` or direct constant/condition checks. No I/O, no async.

## Test Results

```
billing::tests: 50 passed (45 existing + 5 new)
failure_monitor::tests: 8 passed (7 existing + 1 updated)
racecontrol-crate total: 253 unit + 41 integration = 294 passed
rc-common: 112 passed
Zero regressions
```

## Verification

- [x] cargo check -p rc-agent-crate passes — driving_state field added
- [x] cargo test -p racecontrol-crate -- billing::tests — 50 tests green (5 new + 45 existing)
- [x] Full 3-crate suite — 294+ tests green, zero regressions
- [x] billing_guard.rs does NOT exist (Wave 0 adds no bot code)
- [x] bot_coordinator.rs does NOT exist (Wave 0 adds no bot code)
- [x] BILL-01 prerequisite gate satisfied

## Deviations from Plan

None — plan executed exactly as written.

The `start_session_inserts_timer` test uses `t.session_id.contains("pod_1")` rather than exact equality because `BillingTimer::dummy("pod_1")` produces `session_id = "test-session-pod_1"`. This is consistent with the plan note about verifying the dummy() helper's actual fields.

## Self-Check: PASSED

Files exist:
- crates/rc-agent/src/failure_monitor.rs — FOUND (modified)
- crates/racecontrol/src/billing.rs — FOUND (modified)

Commits exist:
- 730ad00 feat(25-01): add driving_state field to FailureMonitorState — FOUND
- c92ddb0 test(25-01): add 5 BILL-01 characterization tests in billing.rs — FOUND
