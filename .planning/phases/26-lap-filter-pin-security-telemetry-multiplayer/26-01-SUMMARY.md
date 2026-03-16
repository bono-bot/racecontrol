---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
plan: "01"
subsystem: racecontrol
tags: [tdd, red-stubs, lap-tracker, auth, bot-coordinator, wave-0]
dependency_graph:
  requires: []
  provides: [LAP-01-stub, LAP-02-stub, LAP-03-stub, PIN-01-stub, PIN-02-stub, TELEM-01-stub, MULTI-01-stub]
  affects: [lap_tracker.rs, auth/mod.rs, bot_coordinator.rs]
tech_stack:
  added: []
  patterns: [TDD Red-Green-Refactor, todo! stubs for behavioral contracts]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/bot_coordinator.rs
decisions:
  - Wave 0 gate enforced: all 11 RED stubs committed before any production code change
  - All stubs use todo!() (not #[should_panic]) — panics automatically on run, compiles cleanly
  - lap_tracker.rs had no prior test block — new #[cfg(test)] mod tests created
  - auth/mod.rs and bot_coordinator.rs had existing test blocks — stubs appended inside
metrics:
  duration_seconds: 200
  completed_date: "2026-03-16"
  tasks_completed: 3
  files_modified: 3
---

# Phase 26 Plan 01: RED Test Stubs (Wave 0 TDD Gate) Summary

**One-liner:** 11 RED test stubs across 3 files encoding behavioral contracts for LAP-01/02/03, PIN-01/02, TELEM-01, MULTI-01 — Wave 0 TDD gate before any production code changes.

## What Was Built

Wave 0 TDD gate for Phase 26. Wrote 11 failing test stubs that document the exact behavioral contracts the production implementations in Wave 1a/1b and Wave 2 must satisfy. No production code was modified.

### lap_tracker.rs (4 stubs — LAP-01/02/03)

| Test | Requirement | Contract |
|------|-------------|---------|
| `lap_invalid_flag_prevents_persist` | LAP-01 | valid=false must return false without DB write |
| `lap_review_required_below_min_floor` | LAP-02 | lap below track minimum floor sets review_required=true |
| `lap_not_flagged_above_min_floor` | LAP-02 | lap above minimum must NOT set review_required |
| `lap_data_carries_session_type` | LAP-03 | LapData must have session_type field from sim adapter |

### auth/mod.rs (3 stubs — PIN-01/02)

| Test | Requirement | Contract |
|------|-------------|---------|
| `customer_and_staff_counters_are_separate` | PIN-01 | customer failures increment customer counter only |
| `customer_failures_do_not_affect_staff_counter` | PIN-01 | 5 customer failures leave staff counter at 0 |
| `staff_pin_succeeds_when_customer_counter_maxed` | PIN-02 | staff always unlocks even when customer exhausted |

### bot_coordinator.rs (4 stubs — TELEM-01/MULTI-01)

| Test | Requirement | Contract |
|------|-------------|---------|
| `telemetry_gap_skipped_when_game_not_running` | TELEM-01 | handle_telemetry_gap is no-op when game_state != Running |
| `telemetry_gap_alerts_when_game_running_and_billing_active` | TELEM-01 | email fires when game=Running + billing_active |
| `multiplayer_failure_triggers_lock_end_billing_log_in_order` | MULTI-01 | teardown order: lock -> end billing -> log |
| `multiplayer_failure_noop_when_billing_inactive` | MULTI-01 | no teardown when billing inactive |

## Test Results

```
racecontrol-crate: 258 passed; 11 failed (all new stubs with todo!() panics)
rc-common:         112 passed; 0 failed
rc-agent-crate:    builds clean (no tests run — binary-only verification)
```

No compile errors (`error[E...]`) anywhere in the workspace.

## Commits

| Hash | Description |
|------|-------------|
| 21569dc | test(26-01): RED stubs for LAP-01/02/03 in lap_tracker.rs |
| a255c50 | test(26-01): RED stubs for PIN-01/02 in auth/mod.rs |
| c3f8ad7 | test(26-01): RED stubs for TELEM-01/MULTI-01 in bot_coordinator.rs |

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

Files verified:
- crates/racecontrol/src/lap_tracker.rs: FOUND (4 stubs added at end)
- crates/racecontrol/src/auth/mod.rs: FOUND (3 stubs added inside mod tests)
- crates/racecontrol/src/bot_coordinator.rs: FOUND (4 stubs added inside mod tests)

Commits verified:
- 21569dc: FOUND
- a255c50: FOUND
- c3f8ad7: FOUND
