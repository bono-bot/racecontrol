---
phase: 195-metrics-foundation
plan: 02
subsystem: database
tags: [sqlite, metrics, billing, crash-recovery, sqlx, rust]

# Dependency graph
requires:
  - phase: 195-01
    provides: metrics.rs module with record_launch_event(), launch_events table, metrics:: namespace

provides:
  - billing_accuracy_events SQLite table with 3 indexes (session, pod, created_at)
  - recovery_events SQLite table with 3 indexes (pod, failure_mode, created_at)
  - BillingAccuracyEvent struct and record_billing_accuracy_event() in metrics.rs
  - RecoveryOutcome enum, RecoveryEvent struct and record_recovery_event() in metrics.rs
  - billing.rs wired: records billing accuracy event at every billing start (single-player + multiplayer)
  - game_launcher.rs wired: records recovery event at auto-relaunch attempt and relaunch exhaustion

affects:
  - 195-03 (billing-accuracy API — reads billing_accuracy_events table)
  - 199 (history-informed recovery — reads recovery_events table)
  - Phase 195 success criteria 4 (BILLING ACCURACY) and 5 (CRASH RECORDING)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "fire-and-forget metrics recording — async fn returns (), logs errors via tracing::error"
    - "delta_ms from waiting_since.elapsed() — measures launch-command to billing-start gap without wall-clock timestamps"
    - "RecoveryOutcome::Success for relaunch initiated (action taken), Failed for exhausted (all attempts spent)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/metrics.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/game_launcher.rs

key-decisions:
  - "delta_ms uses entry.waiting_since.elapsed() (Instant) — measures time from defer_billing_start() call (launch command moment) to billing start on AcStatus::Live; no wall-clock timestamps needed"
  - "RecoveryOutcome::Success records that the relaunch action was taken, not that the game succeeded — actual game outcome tracked by subsequent handle_game_state_update LaunchEvent"
  - "Multiplayer billing accuracy event includes details='multiplayer' to distinguish from single-player in queries"

patterns-established:
  - "Metrics recording pattern: build struct inline at call site, call record_*() as fire-and-forget — matches record_launch_event() pattern from Plan 01"
  - "Tables follow Plan 01 migration convention: CREATE TABLE IF NOT EXISTS inside migrate() function, after prior metrics tables"

requirements-completed: [METRICS-03, METRICS-04]

# Metrics
duration: 15min
completed: 2026-03-26
---

# Phase 195 Plan 02: Metrics Foundation Summary

**SQLite billing_accuracy_events and recovery_events tables with recording functions wired into billing.rs and game_launcher.rs, producing real rows on every billing start and every Race Engineer crash recovery.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-26T~17:30:00Z
- **Completed:** 2026-03-26T~17:45:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Two new SQLite tables (billing_accuracy_events, recovery_events) with 3 indexes each, migrated idempotently via CREATE TABLE IF NOT EXISTS
- BillingAccuracyEvent and RecoveryEvent structs with recording functions in metrics.rs — all DB errors logged, never swallowed
- billing.rs wired at both single-player and multiplayer billing start points — delta_ms captures launch-command to billing-start gap
- game_launcher.rs wired at auto-relaunch attempt (RecoveryOutcome::Success) and relaunch exhaustion (RecoveryOutcome::Failed) — Phase 199 history-informed recovery now has real training data

## Task Commits

Each task was committed atomically:

1. **Task 1: Add billing_accuracy_events and recovery_events tables + recording functions** - `503ef7c0` (feat)
2. **Task 2: Wire billing accuracy recording into billing.rs and recovery recording into game_launcher.rs** - `2ec92cb5` (feat)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` — Added billing_accuracy_events and recovery_events table migrations with 6 indexes total
- `crates/racecontrol/src/metrics.rs` — Added BillingAccuracyEvent, RecoveryOutcome, RecoveryEvent structs and record_billing_accuracy_event(), record_recovery_event() functions
- `crates/racecontrol/src/billing.rs` — Wired record_billing_accuracy_event() at single-player and multiplayer billing start in handle_game_status_update()
- `crates/racecontrol/src/game_launcher.rs` — Wired record_recovery_event() at auto-relaunch attempt and relaunch exhaustion in handle_game_state_update()

## Decisions Made

- delta_ms uses `entry.waiting_since.elapsed()` — a `std::time::Instant` set at defer_billing_start() (the moment the launch command fires). No wall-clock timestamps needed; elapsed gives the precise launch-command to billing-start gap that METRICS-03 requires.
- RecoveryOutcome::Success is recorded when a relaunch is *initiated*, not when the game reaches Live. The actual post-relaunch success/failure is captured by the existing LaunchEvent recording path (Plan 01). This avoids double-tracking and keeps each event type's responsibility clear.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- billing_accuracy_events and recovery_events tables live in DB migration; will be created on next server startup
- Phase 195 verification criteria 4 and 5 (billing accuracy + crash recording) will now produce real rows
- Plan 03 (billing-accuracy API) can read from billing_accuracy_events immediately
- Phase 199 (history-informed recovery) has a real recovery_events feed

---
*Phase: 195-metrics-foundation*
*Completed: 2026-03-26*
