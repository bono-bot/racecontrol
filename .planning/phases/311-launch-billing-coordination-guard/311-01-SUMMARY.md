---
phase: 311-launch-billing-coordination-guard
plan: 01
subsystem: billing
tags: [billing, game-launcher, stale-cancel, gametracker, rwlock]

requires:
  - phase: 310-session-trace-id
    provides: GameTracker with billing_session_id for tracing
provides:
  - Game-aware stale cancel logic in tick_all_timers
  - Sessions with alive games extended up to 10 min absolute max
  - Revenue-loss prevention for game-loading scenarios
affects: [312-ws-ack-protocol, 313-game-state-resilience, 314-billing-atomicity]

tech-stack:
  added: []
  patterns: [game-state-snapshot-before-async, per-session-cancel-decision]

key-files:
  created: []
  modified: [crates/racecontrol/src/billing.rs]

key-decisions:
  - "Added status column to stale sessions SELECT to avoid per-session re-query"
  - "Treat Loading state as game-alive alongside Launching and Running"
  - "Unparseable created_at timestamps treated as very old (age=99 min) to ensure cancel"

patterns-established:
  - "Game-state snapshot pattern: read RwLock, collect into HashMap, drop guard, then iterate with async DB calls"
  - "Per-session cancel decision with logged reason codes (LBILL-02/03)"

requirements-completed: [LBILL-01, LBILL-02, LBILL-03]

duration: 10min
completed: 2026-04-03
---

# Phase 311 Plan 01: Game-Aware Stale Cancel Summary

**Billing stale-cancel now checks GameTracker before cancelling waiting_for_game sessions -- prevents free play when game loads slowly**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-02T20:46:26Z
- **Completed:** 2026-04-02T20:57:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- tick_all_timers checks GameTracker state before cancelling waiting_for_game sessions (LBILL-01)
- Sessions with alive games (Launching/Loading/Running) extended up to 10 min absolute max (LBILL-02)
- Sessions with dead games or >10 min cancelled with full wallet refund (LBILL-03)
- Every stale cancel decision logged with LBILL-02/LBILL-03 reason codes
- RwLock snapshot pattern: no lock held across .await (standing rule compliance)
- 5 new async tests, 807 lib tests pass with 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Game-aware stale cancel logic in tick_all_timers** - `4488f48a` (feat)

## Files Created/Modified
- `crates/racecontrol/src/billing.rs` - Modified stale cancel block in tick_all_timers: now queries pod_id/created_at/status, snapshots GameTracker, makes per-session cancel/extend decision. Added 5 async tests.
- `LOGBOOK.md` - Added entry for this commit.

## Decisions Made
- Added `status` column to the stale sessions SELECT query to avoid a per-session re-query (was in initial implementation, optimized out)
- Included `Loading` state as "game alive" alongside `Launching` and `Running` -- Loading means game process is detected but PlayableSignal not yet received, which is a valid loading state
- Treat unparseable `created_at` timestamps as very old (age=99 min) to ensure they are cancelled rather than silently extended

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Package name is `racecontrol-crate` not `racecontrol` (standard project naming)
- 8 pre-existing integration test failures in lap/notification tests (unrelated to this change, all in `tests/integration.rs`)

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all logic is fully wired. The game-state check uses the existing `active_games` RwLock on `GameManager` which is populated by the game launcher pipeline.

## Next Phase Readiness
- Phase 312 (WS ACK Protocol) can proceed -- this phase's billing guard is independent
- The game-aware stale cancel logic will benefit from Phase 313's GameTracker resilience (stuck Launching states)

## Self-Check: PASSED

- billing.rs: FOUND
- 311-01-SUMMARY.md: FOUND
- Commit 4488f48a: FOUND

---
*Phase: 311-launch-billing-coordination-guard*
*Completed: 2026-04-03*
