---
phase: 04-remote-booking-pin-generation
plan: 01
subsystem: api
tags: [rust, axum, reservation, pin-generation, whatsapp, sqlite, debit-intent]

# Dependency graph
requires:
  - phase: 03-sync-hardening
    provides: reservations and debit_intents tables, bidirectional sync
provides:
  - reservation.rs module with PIN generation, CRUD, WhatsApp delivery
  - 4 REST endpoints for remote booking (POST create, GET view, PUT modify, DELETE cancel)
  - debit_intent creation for wallet debit flow
affects: [04-remote-booking-pin-generation, kiosk-pin-redemption, cloud-sync]

# Tech tracking
tech-stack:
  added: []
  patterns: [debit-intent-pattern, fire-and-forget-whatsapp, cancel-rebook-modify]

key-files:
  created:
    - crates/racecontrol/src/reservation.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Used separate route paths for body-accepting handlers (POST /create, PUT /modify) due to Axum MethodRouter chaining limitations with Json extractors"
  - "Table is kiosk_experiences not experiences - queries corrected to match actual schema"
  - "ThreadRng scoped to non-async block to avoid Send trait issues across await boundaries"

patterns-established:
  - "Debit intent pattern: never modify wallet directly, create debit_intent with origin='cloud'"
  - "Cancel+rebook for modify: cancel old reservation, create new one preserving original expires_at"
  - "Fire-and-forget WhatsApp: tokio::spawn for non-blocking PIN delivery"

requirements-completed: [BOOK-01, BOOK-02, BOOK-03, BOOK-04, API-04]

# Metrics
duration: 7min
completed: 2026-03-21
---

# Phase 04 Plan 01: Remote Booking PIN Generation Summary

**Reservation module with 6-char PIN generation, debit_intent wallet flow, and fire-and-forget WhatsApp delivery via Evolution API**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-21T12:46:03Z
- **Completed:** 2026-03-21T12:53:43Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created reservation.rs module with 6 public async functions (generate_unique_pin, create_reservation, get_active_reservation, cancel_reservation, modify_reservation, send_pin_whatsapp)
- Wired 4 customer-facing REST endpoints for remote booking CRUD
- Enforced one-active-reservation-per-customer constraint
- Implemented cancel-with-refund logic (pending cancels intent, completed creates negative refund intent)
- Modify preserves original TTL via cancel+rebook pattern

## Task Commits

Each task was committed atomically:

1. **Task 1: Create reservation.rs module with PIN generation, CRUD logic, and WhatsApp delivery** - `8d7bcf4` (feat)
2. **Task 2: Wire reservation API routes into customer_routes()** - `80bc9f1` (feat)

## Files Created/Modified
- `crates/racecontrol/src/reservation.rs` - New module: PIN generation (31-char unambiguous charset), reservation CRUD, WhatsApp delivery
- `crates/racecontrol/src/lib.rs` - Added `pub mod reservation;`
- `crates/racecontrol/src/api/routes.rs` - Added 4 route handlers + registration in customer_routes(), added `use crate::reservation`

## Decisions Made
- Used separate route paths (/reservation, /reservation/create, /reservation/modify) instead of single path with all HTTP methods due to Axum 0.8 MethodRouter generic resolution issues with Json body extractors
- Corrected experience table name from `experiences` to `kiosk_experiences` to match actual schema
- Scoped ThreadRng to non-async block to satisfy Send bound requirement for Axum handlers

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ThreadRng not Send across await boundary**
- **Found during:** Task 2 (route wiring)
- **Issue:** `rand::thread_rng()` held across `.await` in generate_unique_pin made the future non-Send, causing Axum Handler trait bound failure
- **Fix:** Scoped RNG creation and PIN generation to a synchronous block within the loop
- **Files modified:** crates/racecontrol/src/reservation.rs
- **Verification:** cargo check passes
- **Committed in:** 80bc9f1 (Task 2 commit)

**2. [Rule 1 - Bug] Experience table name mismatch**
- **Found during:** Task 1 (reservation module creation)
- **Issue:** Plan referenced `experiences` table but actual schema uses `kiosk_experiences`
- **Fix:** Used `kiosk_experiences` in all queries
- **Files modified:** crates/racecontrol/src/reservation.rs
- **Verification:** cargo check passes
- **Committed in:** 8d7bcf4 (Task 1 commit)

**3. [Rule 3 - Blocking] Route path splitting for Axum compatibility**
- **Found during:** Task 2 (route wiring)
- **Issue:** Axum 0.8 MethodRouter chaining (.post()/.put() on get() result) fails Handler trait resolution for handlers with Json body extractors
- **Fix:** Split into separate route paths: GET+DELETE on /reservation, POST on /reservation/create, PUT on /reservation/modify
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** cargo check passes
- **Committed in:** 80bc9f1 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (2 bugs, 1 blocking)
**Impact on plan:** All fixes necessary for compilation. Route paths slightly differ from plan (separate paths vs single path with all methods) but functionality is identical.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Reservation CRUD endpoints ready for PWA integration (Plan 04-02)
- Debit intent processing already handled by sync hardening (Phase 03)
- WhatsApp delivery uses existing Evolution API configuration

---
*Phase: 04-remote-booking-pin-generation*
*Completed: 2026-03-21*
