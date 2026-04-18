---
phase: 414-continuous-billing-session
plan: "00"
subsystem: billing
tags: [tdd, wave-0, stub-tests, billing-fsm, contract-tests]
dependency_graph:
  requires: []
  provides: [414-wave0-test-scaffold]
  affects: [billing_fsm.rs, billing_tests.rs, protocol.rs, types.rs, contract-tests]
tech_stack:
  added: []
  patterns: [ignore-attribute-for-pre-commit-compatibility, describe-skip-vitest]
key_files:
  created:
    - crates/racecontrol/tests/billing_session_e2e.rs
  modified:
    - crates/racecontrol/src/billing_fsm.rs
    - crates/racecontrol/src/billing_tests.rs
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs
    - packages/contract-tests/src/fixtures/ws-dashboard.json
    - packages/contract-tests/src/ws-dashboard.contract.test.ts
    - packages/contract-tests/src/billing.contract.test.ts
decisions:
  - "All Wave 0 Rust stubs use #[ignore] so pre-commit gate (cargo test --lib) passes between Wave 0 and Wave 1 commits"
  - "TS stubs use describe.skip so vitest shows 1 skipped (not failed)"
  - "BillingEvent::GameStopped added with #[allow(dead_code)] in Wave 0 so tests compile; TRANSITION_TABLE row lands in Plan 01"
  - "Negative FSM tests (completed/pending reject GameStopped) PASS under --ignored because validate_transition already rejects unknown transitions — this is correct"
metrics:
  duration: "~35 minutes"
  completed: "2026-04-18"
  tasks_completed: 3
  tasks_total: 3
  files_created: 1
  files_modified: 7
---

# Phase 414 Plan 00: Wave 0 TDD Scaffolding Summary

**One-liner:** 14 stubbed RED tests + 2 fixtures + 1 e2e file scaffolding Phase 414 continuous billing session with `#[ignore]`'d Rust stubs and `describe.skip` TS stubs so the pre-commit gate passes throughout the Wave 0 → Wave 5 build-out.

---

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Stub 5 FSM tests + 2 protocol/types round-trips | `92888a19` | billing_fsm.rs, protocol.rs, types.rs |
| 2 | Stub 7 timer/integration tests + e2e file | `18d52955` | billing_tests.rs, billing_session_e2e.rs (NEW) |
| 3 | TS contract test stubs + new fixtures | `ff74cad6` | ws-dashboard.json, ws-dashboard.contract.test.ts, billing.contract.test.ts |

---

## Stubbed Tests (14 total)

### FSM stubs — `crates/racecontrol/src/billing_fsm.rs` (5 tests)
All marked `#[ignore = "Wave 1 fills TRANSITION_TABLE — Plan 01 removes this attribute"]`

| Test | REQ-ID | Status under --ignored |
|------|--------|----------------------|
| `test_active_game_stopped_to_waiting` | 414-FSM-02 | FAIL (expected — no TRANSITION_TABLE row) |
| `test_waiting_end_to_completed` | 414-FSM-03 | FAIL (expected — no TRANSITION_TABLE row) |
| `test_waiting_end_early_to_ended_early` | 414-FSM-04 | FAIL (expected — no TRANSITION_TABLE row) |
| `test_completed_game_stopped_rejected` | 414-FSM-05 | PASS (correct — terminal states already reject unknown events) |
| `test_pending_game_stopped_rejected` | 414-FSM-05 | PASS (correct — terminal states already reject unknown events) |

Note: The 2 negative tests (`_rejected`) PASS under `--ignored` because `validate_transition` already returns `Err` for any event not in TRANSITION_TABLE — they test rejections which are already correct. These tests will continue to pass after Plan 01 adds the 3 new positive transitions.

### Protocol/types stubs — `crates/rc-common/src/protocol.rs` + `types.rs` (2 tests)
Both marked `#[ignore = "Phase 414 Plan 03 will add ..."]`

| Test | REQ-ID |
|------|--------|
| `test_idle_warning_serde_roundtrip` | 414-PROTOCOL-01 |
| `test_billing_info_idle_seconds_roundtrip` | 414-PROTOCOL-02 |

### Timer/integration stubs — `crates/racecontrol/src/billing_tests.rs` (7 tests)

| Test | REQ-ID | Unblocked by |
|------|--------|-------------|
| `timer_idle_counter_advances_only_in_waiting` | 414-TIMER-01 | Plan 02 |
| `timer_idle_counter_resets_on_resume` | 414-TIMER-02 | Plan 02 |
| `idle_warning_fires_at_600s_once` | 414-TIMER-03 | Plan 02 |
| `idle_auto_ends_at_900s_completed` | 414-TIMER-04 | Plan 04 |
| `cumulative_snap_25_5_yields_pkg_30` | 414-INTEGRATION-01 | Plan 04 |
| `idle_auto_end_completes_with_cumulative_cost` | 414-INTEGRATION-02 | Plan 04 |
| `pod_offline_in_waiting_auto_ends_completed` | 414-INTEGRATION-03 | Plan 04 |

### E2E integration stub — `crates/racecontrol/tests/billing_session_e2e.rs` (1 test)

| Test | REQ-ID | Unblocked by |
|------|--------|-------------|
| `stop_billing_branches_on_elapsed` | 414-INTEGRATION-04 | Plan 04 |

### TS contract stubs — `packages/contract-tests/` (3 tests, 1 skipped)

| Test | REQ-ID | Status |
|------|--------|--------|
| `idle_warning fixture has all 5 required fields` | 414-CONTRACT-02 | PASSING (fixture-only) |
| `billing_tick_between_games has elapsed_seconds > 0...` | 414-CONTRACT-02 | PASSING (fixture-only) |
| `Phase 414 — BillingSession.between_games_idle_seconds field` (describe.skip) | 414-CONTRACT-01 | SKIPPED (Plan 03 removes skip) |

---

## Pre-Commit Gate Verification

| Check | Result |
|-------|--------|
| `cargo build -p rc-common` | PASS (1 pre-existing warning) |
| `cargo build -p racecontrol-crate` | PASS (1 pre-existing warning) |
| `cargo build -p racecontrol-crate --tests` | PASS |
| `cargo test -p rc-common --lib` | PASS — 252 passed, 2 ignored, 0 failed |
| `cargo test -p racecontrol-crate --lib billing_fsm` | PASS — 30 passed, 5 ignored, 0 failed |
| `cargo test -p racecontrol-crate --lib billing_fsm -- --ignored` | RED scaffold confirmed — 3 FAILED (positive transitions), 2 PASSED (negative/rejection tests) |
| `cargo test --test billing_session_e2e -- --list` | stop_billing_branches_on_elapsed discovered |
| `cd packages/contract-tests && npx vitest run` | PASS — 53 passed, 1 skipped, 0 failed |

---

## Acceptance Criteria Status

| Criterion | Status |
|-----------|--------|
| All 14 stub tests exist in source | PASS |
| All Rust stubs marked `#[ignore]` | PASS |
| TS stubs use `describe.skip` | PASS |
| Pre-commit gate passes (cargo test --lib exits 0) | PASS |
| Stubbed tests are RED when run with --ignored | PASS (3 FAIL as expected; 2 negative tests correctly PASS) |
| cargo build -p rc-common exits 0 | PASS |
| cargo build -p racecontrol-crate exits 0 | PASS |
| cargo build -p racecontrol-crate --tests exits 0 | PASS |
| vitest run exits 0 | PASS |
| No .unwrap() in new code | PASS |
| No `any` in TypeScript | PASS |

---

## Plan 01 Pointer

Plan 01 Wave 1 should:
1. Remove `#[ignore]` from the 5 FSM tests in `billing_fsm.rs`
2. Add TRANSITION_TABLE rows for `Active + GameStopped → WaitingForGame`, `WaitingForGame + End → Completed`, `WaitingForGame + EndEarly → EndedEarly`
3. Verify all 5 FSM tests pass (including the 2 that already passed in Wave 0)

---

## Deviations from Plan

None — plan executed exactly as specified. One note: the plan's verification comment said `grep -c "FAILED"` should return 5, but the 2 negative tests (`test_completed_game_stopped_rejected`, `test_pending_game_stopped_rejected`) already PASS because `validate_transition` returns `Err` for ANY unknown transition — which is exactly what those tests assert. This is correct behavior, not a deviation. Plan 01's removal of `#[ignore]` from all 5 tests will not change the test results for these 2 — they already pass.

---

## Known Stubs

All 14 stub tests are intentional Wave 0 scaffolding. Each is documented with its target plan. No stubs prevent Phase 414 Plan 00's goal (scaffolding) from being achieved.

---

## Self-Check

Files created:
- `.planning/phases/414-continuous-billing-session/414-00-SUMMARY.md` — this file

Commits verified:
- `92888a19` — present in `git log origin/main`
- `18d52955` — present in `git log origin/main`
- `ff74cad6` — present in `git log origin/main`

## Self-Check: PASSED
