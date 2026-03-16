---
phase: 01-billing-game-lifecycle
plan: 01
subsystem: billing
tags: [rust, axum, billing-gate, double-launch-guard, game-launcher, tdd]

# Dependency graph
requires: []
provides:
  - "Billing validation gate in launch_game() — rejects launch when no active billing session"
  - "Expanded double-launch guard — blocks both Launching and Running states"
  - "4 unit tests for billing gate and double-launch guard"
affects: [02-game-crash-recovery, 03-launch-resilience]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Billing presence check: state.billing.active_timers.read().await.contains_key(pod_id)"
    - "Game state guard: matches!(tracker.game_state, GameState::Launching | GameState::Running)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/game_launcher.rs

key-decisions:
  - "Billing gate placed after catalog validation but before double-launch guard — validates billing before checking game state"
  - "Double-launch guard error message generalized to 'already has a game active' covering both Launching and Running"

patterns-established:
  - "In-module test pattern: #[cfg(test)] mod tests with make_state() helper using in-memory SQLite + Config::default_test()"

requirements-completed: [LIFE-01, LIFE-02, LIFE-04]

# Metrics
duration: 3min
completed: 2026-03-15
---

# Phase 1 Plan 01: Billing Gate + Double-Launch Guard Summary

**Billing validation gate and expanded double-launch guard in launch_game() with 4 TDD unit tests (LIFE-02, LIFE-04)**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-15T07:27:06Z
- **Completed:** 2026-03-15T07:30:15Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- launch_game() now rejects requests when pod has no active billing session (LIFE-02)
- launch_game() now blocks both Launching AND Running states, preventing double-launch (LIFE-04)
- LIFE-01 (game killed on billing end) confirmed already working via StopGame + SessionEnded flow
- 4 unit tests added via TDD (RED-GREEN), all 213 racecontrol tests + 41 integration tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add billing gate and fix double-launch guard** (TDD)
   - RED: `157f8f9` (test: add failing tests for billing gate + double-launch guard)
   - GREEN: `675a2bc` (feat: add billing gate + expand double-launch guard in launch_game())

## Files Created/Modified
- `crates/racecontrol/src/game_launcher.rs` - Added billing gate (LIFE-02), expanded double-launch guard to block Running state (LIFE-04), added 4 unit tests

## Decisions Made
- Billing gate uses the same `active_timers.read().await.contains_key(pod_id)` pattern already established in pod_healer.rs and ws/mod.rs
- Error message for double-launch guard generalized from "already launching a game" to "already has a game active" to cover both states
- Tests use `BillingTimer::dummy()` helper from billing.rs for clean test state construction

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Billing gate and double-launch guard are in place for racecontrol
- Plan 01-02 (rc-agent: arm 15s blank_timer in SessionEnded + fix BillingStopped billing_active flag) is ready to execute
- Phase 2 (Game Crash Recovery) depends on Phase 1 completion

## Self-Check: PASSED

- [x] game_launcher.rs exists and contains billing gate + expanded guard
- [x] Commit 157f8f9 (RED) exists in git log
- [x] Commit 675a2bc (GREEN) exists in git log
- [x] SUMMARY.md created at correct path
- [x] All 213 racecontrol unit tests + 41 integration tests + 93 rc-common tests pass

---
*Phase: 01-billing-game-lifecycle*
*Completed: 2026-03-15*
