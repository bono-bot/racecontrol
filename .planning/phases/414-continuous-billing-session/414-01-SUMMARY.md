---
phase: 414-continuous-billing-session
plan: "01"
subsystem: billing
tags: [tdd, wave-1, billing-fsm, fsm-transitions, continuous-billing]
dependency_graph:
  requires:
    - phase: 414-00
      provides: "5 #[ignore]'d FSM tests + BillingEvent::GameStopped stub variant"
  provides:
    - "3 new TRANSITION_TABLE rows: Active+GameStopped→WaitingForGame, WaitingForGame+End→Completed, WaitingForGame+EndEarly→EndedEarly"
    - "All 5 Wave-0 FSM tests now GREEN (no longer #[ignore]'d)"
    - "BillingEvent::GameStopped variant now live in TRANSITION_TABLE (dead_code suppression removed)"
  affects: [414-02, 414-03, 414-04, 414-05, billing_fsm.rs, billing_game_status.rs]
tech-stack:
  added: []
  patterns:
    - "TRANSITION_TABLE-first: all billing status changes MUST have a TRANSITION_TABLE entry before any call site can use them"
    - "W3 closure comment: explicit explanation when a transition is intentionally absent (WaitingForGame + Disconnect)"
key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing_fsm.rs
key-decisions:
  - "Active + GameStopped → WaitingForGame: mid-stream game-stop moves billing to between-games state, meter pauses (D-FSM-01)"
  - "WaitingForGame + End → Completed: 15-min idle auto-end uses End event not EndEarly, signals natural session completion (D-IDLE-AUTOEND)"
  - "WaitingForGame + EndEarly → EndedEarly: staff-triggered stop_billing mid-stream uses EndEarly, edge case 4 in CONTEXT.md"
  - "No WaitingForGame + Disconnect transition added (W3 closure): meter already paused, disconnect does not change billing"
patterns-established:
  - "Wave-N completion removes #[ignore] from the stub tests Wave-(N-1) added — TDD RED→GREEN across plan boundaries"
requirements-completed: [414-FSM-01, 414-FSM-02, 414-FSM-03, 414-FSM-04, 414-FSM-05]
duration: "~8 minutes"
completed: "2026-04-18"
---

# Phase 414 Plan 01: Wave 1 FSM Table Extension Summary

**3 TRANSITION_TABLE rows added to billing_fsm.rs (Active+GameStopped→WaitingForGame, WaitingForGame+End→Completed, WaitingForGame+EndEarly→EndedEarly) plus 5 Wave-0 #[ignore] attributes removed — 5 RED tests now GREEN**

---

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-18T02:56:00Z
- **Completed:** 2026-04-18T02:56:00Z
- **Tasks:** 1
- **Files modified:** 1 (crates/racecontrol/src/billing_fsm.rs)

---

## Accomplishments

- Appended 3 Phase 414 rows to `TRANSITION_TABLE` const array in `billing_fsm.rs`
- Removed `#[ignore = "Wave 1 fills TRANSITION_TABLE..."]` attribute from all 5 Wave-0 FSM tests
- Removed `#[allow(dead_code)]` from `BillingEvent::GameStopped` (now referenced in TRANSITION_TABLE)
- Added W3 closure comment explaining why `WaitingForGame + Disconnect` is intentionally absent
- Test result: **35 passed; 0 failed; 0 ignored** (was 30 passed; 0 failed; 5 ignored in Wave 0)

---

## Test Results

```
running 35 tests
test billing_fsm::tests::test_active_game_stopped_to_waiting ... ok
test billing_fsm::tests::test_completed_game_stopped_rejected ... ok
test billing_fsm::tests::test_pending_game_stopped_rejected ... ok
test billing_fsm::tests::test_waiting_end_early_to_ended_early ... ok
test billing_fsm::tests::test_waiting_end_to_completed ... ok
... [30 pre-existing tests all still ok]

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 945 filtered out
```

All 5 Wave-0 stub tests (3 previously RED when run with --ignored, 2 correctly passing) are now actively GREEN without any `--ignored` flag needed.

---

## Task Commits

1. **Task 1: Append 3 rows to TRANSITION_TABLE; remove #[ignore] from the 5 Wave 0 tests** — `5b5f9304` (feat)

---

## Files Modified

- `crates/racecontrol/src/billing_fsm.rs` — +21 lines, -8 lines:
  - 3 new TRANSITION_TABLE rows with detailed explanatory comments (lines ~106-129)
  - Removed 5 `#[ignore]` attributes from FSM tests
  - Removed `#[allow(dead_code)]` from `BillingEvent::GameStopped`
  - Updated `GameStopped` doc comment to reference D-FSM-01

---

## Acceptance Criteria Verification

| Criterion | Status |
|-----------|--------|
| `TRANSITION_TABLE` contains `BillingEvent::GameStopped, BillingSessionStatus::WaitingForGame` | PASS (grep: 1 match) |
| `TRANSITION_TABLE` contains `WaitingForGame, BillingEvent::End, BillingSessionStatus::Completed` | PASS (grep: 1 match) |
| `TRANSITION_TABLE` contains `WaitingForGame, BillingEvent::EndEarly, BillingSessionStatus::EndedEarly` | PASS (grep: 1 match) |
| `grep -c "Wave 1 fills TRANSITION_TABLE" billing_fsm.rs` returns 0 | PASS (grep: 0 matches) |
| `cargo test -p racecontrol-crate --lib billing_fsm` passes | PASS (35/35 ok, 0 ignored) |
| `test_active_game_stopped_to_waiting` passes | PASS |
| `test_waiting_end_to_completed` passes | PASS |
| `test_waiting_end_early_to_ended_early` passes | PASS |
| `test_completed_game_stopped_rejected` passes | PASS |
| `test_pending_game_stopped_rejected` passes | PASS |
| No regression in pre-existing 30 transition tests | PASS (35 = 30 + 5) |
| `cargo build -p racecontrol-crate` exits 0 | PASS |

---

## Decisions Made

- **W3 closure (explicit non-transition):** Per CONTEXT.md edge case discussion, `WaitingForGame + Disconnect` is intentionally absent — the meter is already paused in WaitingForGame so a disconnect does not change billing state. Added a comment documenting this intent at the end of the Phase 414 block.
- **Removed `#[allow(dead_code)]`:** With the GameStopped variant now referenced in TRANSITION_TABLE, the dead_code suppression attribute is no longer needed and was removed to avoid misleading future readers.

---

## Deviations from Plan

None — plan executed exactly as written. The only minor cleanup was removing the `#[allow(dead_code)]` attribute from `BillingEvent::GameStopped` (Rule 1 — would have been a spurious lint suppression once the variant is used; the plan did not mention this attribute removal but it is a necessary correctness cleanup).

---

## Known Stubs

None introduced in this plan. The 9 remaining Wave-0 stubs (7 timer/integration tests in billing_tests.rs + 1 e2e test + 1 TS describe.skip) are tracked in the 414-00-SUMMARY.md Known Stubs section and are intentional scaffolding for Plans 02-05.

---

## Plan 02 Pointer

Plan 02 (Wave 2 — BillingTimer field + tick branch) can now proceed. With Wave 1 landed:
- `validate_transition(Active, GameStopped)` returns `Ok(WaitingForGame)` — Plan 02's tick branch can fire this transition when game state goes Off
- `validate_transition(WaitingForGame, End)` returns `Ok(Completed)` — Plan 04's 15-min idle loop can use this
- `validate_transition(WaitingForGame, EndEarly)` returns `Ok(EndedEarly)` — Plan 04's stop_billing branch + Plan 05's kiosk button can use this

Without Wave 1, every downstream `validate_transition` call on these paths would have returned `Err` and silently dropped the state change.

---

## Self-Check

Files modified:
- `crates/racecontrol/src/billing_fsm.rs` — verified via `git show 5b5f9304 --stat`

Commits verified:
- `5b5f9304` — `feat(414-01): wave 1 — add 3 FSM transitions for continuous billing...`

## Self-Check: PASSED
