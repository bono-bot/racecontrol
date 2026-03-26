---
phase: 198-on-track-billing
plan: 02
subsystem: billing
tags: [rust, billing, ac, waiting-for-game, multiplayer, configurable-timeouts, cancelled-no-playable]

# Dependency graph
requires:
  - phase: 198-on-track-billing plan 01
    provides: "CancelledNoPlayable variant, BillingConfig with 5 configurable timeout fields"
provides:
  - "WaitingForGame sessions broadcast BillingTick each tick — kiosk shows Loading state (BILL-05)"
  - "cancelled_no_playable DB records on launch timeout (attempt 2) and game-death-before-playable (BILL-06)"
  - "total_paused_seconds persisted for PausedGamePause state in sync_timers_to_db (BILL-07)"
  - "Single Utc::now() call for billing accuracy event timestamps — no dual-capture skew (BILL-09)"
  - "Multiplayer DB query failure rejects billing with error log instead of unwrap_or_default (BILL-10)"
  - "Multiplayer and launch timeouts configurable via BillingConfig fields (BILL-11, BILL-12)"
affects:
  - "kiosk Loading state display (consumes WaitingForGame BillingTick)"
  - "billing_sessions analytics — cancelled_no_playable records now tracked"
  - "Phase 199 WhatsApp staff alerts (TODO comment inserted at cancelled_no_playable paths)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WaitingForGame broadcast: separate loop over waiting_for_game map after active_timers loop — avoids status enum comparison complexity"
    - "Lock ordering: drop(mp) before acquiring waiting_for_game in BILL-10 error path — prevents potential deadlock"
    - "cancelled_no_playable INSERT uses pricing_tier_id from WaitingForGameEntry — FK constraint satisfied without hardcoded tier"
    - "Single Utc::now() for dual timestamp fields — eliminates sub-millisecond skew in billing accuracy events"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/billing.rs"

key-decisions:
  - "WaitingForGame ticks broadcast from a separate loop over waiting_for_game map (not active_timers) — WaitingForGame entries are never inserted into active_timers, so the active_timers for-loop cannot see them"
  - "cancelled_no_playable INSERT uses pricing_tier_id from the WaitingForGameEntry directly — avoids hardcoded 'tier_30min' fallback while satisfying the NOT NULL FK constraint"
  - "BILL-10 error path drops mp lock before acquiring waiting_for_game lock — consistent lock ordering (waiting_for_game is always acquired before multiplayer_waiting in the normal flow)"
  - "check_launch_timeouts_from_manager() now takes explicit timeout_secs: u64 — test callers pass 180 directly, production caller passes state.config.billing.launch_timeout_per_attempt_secs"

patterns-established:
  - "cancelled_no_playable pattern: both timeout and crash paths produce identical INSERT with driving_seconds=0, total_paused_seconds=0 — the no-charge contract is enforced by the DB record"
  - "Configurable timeout pattern: BillingConfig field read at spawn time, captured into closure — no config re-reads inside async timeout handler"

requirements-completed: [BILL-05, BILL-06, BILL-07, BILL-09, BILL-10, BILL-11]

# Metrics
duration: 10min
completed: 2026-03-26
---

# Phase 198 Plan 02: On-Track Billing Server-Side Logic Summary

**WaitingForGame tick broadcasts for kiosk Loading state, cancelled_no_playable DB records on timeout/crash, paused seconds persistence for game-pause, single-timestamp billing accuracy, and multiplayer error rejection — billing.rs fully wired to BillingConfig**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-26T07:16:23Z
- **Completed:** 2026-03-26T07:26:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added WaitingForGame broadcast loop in tick_all_timers() — kiosk receives BillingTick with status=WaitingForGame each second during game load (BILL-05)
- Added cancelled_no_playable DB INSERT on AcStatus::Off (game crashed before PlayableSignal) and on launch timeout attempt 2 (6 min total, no PlayableSignal) — zero charge to customer (BILL-06)
- Added PausedGamePause to sync_timers_to_db() — total_paused_seconds now persisted for game-pause state (was missing; only PausedDisconnect was persisted) (BILL-07)
- Fixed dual Utc::now() calls to single capture in both single-player and multiplayer billing accuracy event recording (BILL-09)
- Replaced unwrap_or_default() on group_session_members DB query with explicit error handling — billing REJECTED with tracing::error! on DB failure, entry re-inserted for retry (BILL-10)
- Made multiplayer and launch timeouts configurable: multiplayer uses state.config.billing.multiplayer_wait_timeout_secs, launch uses state.config.billing.launch_timeout_per_attempt_secs (BILL-11, BILL-12)

## Task Commits

Each task was committed atomically:

1. **Task 1: WaitingForGame tick broadcast + cancelled_no_playable on timeout/crash** - `f5189125` (feat)
2. **Task 2: AC timer sync + multiplayer error handling + configurable timeouts** - `f9b7be6b` (feat)

**Plan metadata:** (this summary commit)

## Files Created/Modified
- `crates/racecontrol/src/billing.rs` - All 6 requirement implementations: BILL-05 WaitingForGame broadcast, BILL-06 cancelled_no_playable records, BILL-07 PausedGamePause sync, BILL-09 single timestamp, BILL-10 multiplayer DB error handling, BILL-11/12 configurable timeouts

## Decisions Made
- WaitingForGame entries are never in active_timers — they live only in the waiting_for_game HashMap. Broadcast requires a separate loop, not a status check inside the existing active_timers for-loop.
- cancelled_no_playable INSERT uses pricing_tier_id from the WaitingForGameEntry (available on both timeout and crash paths) — avoids hardcoded fallback tier while satisfying billing_sessions FK constraint.
- BILL-10 error path drops multiplayer_waiting write lock before acquiring waiting_for_game write lock — prevents potential deadlock (normal flow always acquires waiting_for_game first, then multiplayer_waiting).
- check_launch_timeouts_from_manager() signature changed to accept timeout_secs: u64 — backward compatible: tests pass 180 explicitly, production passes config value.

## Deviations from Plan

None - plan executed exactly as written. All 6 requirements implemented without scope changes.

## Issues Encountered
None beyond the lock ordering concern in BILL-10 which was caught at implementation time and resolved by explicit drop(mp) before waiting_for_game acquisition.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All billing.rs changes are complete — Plan 03 (if any) can build on this foundation
- TODO Phase 199: WhatsApp staff alert for cancelled_no_playable sessions (TODO comment inserted at both INSERT paths)
- All 608 tests pass (538 unit + 4 main + 66 integration)

## Self-Check: PASSED
- SUMMARY.md: FOUND at .planning/phases/198-on-track-billing/198-02-SUMMARY.md
- Task 1 commit f5189125: FOUND
- Task 2 commit f9b7be6b: FOUND

---
*Phase: 198-on-track-billing*
*Completed: 2026-03-26*
