---
phase: 04-remote-booking-pin-generation
plan: 02
subsystem: scheduler
tags: [sqlite, reservation-expiry, refund, debit-intent, scheduler]

# Dependency graph
requires:
  - phase: 03-sync-hardening
    provides: reservations and debit_intents tables with bidirectional sync
provides:
  - "Automatic reservation expiry cleanup (60s tick)"
  - "Refund debit_intent creation for completed debits on expired reservations"
  - "Pending debit_intent cancellation on expired reservations"
affects: [04-remote-booking-pin-generation, sync]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Negative amount_paise for refund debit_intents", "origin='local' to ensure cloud sync picks up refunds"]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/scheduler.rs

key-decisions:
  - "Refund debit_intents use origin='local' so cloud sync does not skip them"
  - "Both pending_debit and confirmed statuses can expire (pending_debit if sync never processed)"
  - "Pending/processing debit_intents are cancelled, completed ones get negative-amount refund"

patterns-established:
  - "Negative amount_paise signals refund in debit_intents table"
  - "Scheduler expire cleanup runs every tick (60s) alongside existing wake/analytics logic"

requirements-completed: [BOOK-06, BOOK-07]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 04 Plan 02: Reservation Expiry Summary

**Scheduler auto-expires past-TTL reservations every 60s with refund debit_intents for completed debits and cancellation for pending ones**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T12:46:11Z
- **Completed:** 2026-03-21T12:47:36Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `expire_reservations()` function to scheduler tick loop
- Expired confirmed reservations with completed debits automatically get refund via negative amount debit_intent
- Expired pending_debit reservations get their debit_intent cancelled (no charge occurred)
- Refund intents use `origin='local'` to ensure cloud sync picks them up

## Task Commits

Each task was committed atomically:

1. **Task 1: Add expire_reservations() to scheduler tick** - `c817d02` (feat)

## Files Created/Modified
- `crates/racecontrol/src/scheduler.rs` - Added expire_reservations() function and call from tick()

## Decisions Made
- Refund debit_intents use `origin='local'` so cloud sync picks them up (not 'cloud' which would be filtered out as already-synced)
- Both `pending_debit` and `confirmed` reservations can expire -- pending_debit covers the case where sync never processed the debit
- Pending/processing debit_intents are cancelled rather than refunded (nothing was actually debited)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Reservation expiry is operational; ready for Plan 03 (PIN redemption kiosk endpoint)
- Cloud sync will pick up refund debit_intents via origin='local' filter

---
*Phase: 04-remote-booking-pin-generation*
*Completed: 2026-03-21*
