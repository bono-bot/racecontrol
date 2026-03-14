---
phase: 12-data-foundation
plan: 02
subsystem: database
tags: [sqlite, sqlx, car-class, lap-tracking, schema-migration, tdd]

# Dependency graph
requires:
  - phase: 12-data-foundation plan 01
    provides: "laps table, kiosk_experiences table with car_class, billing_sessions with experience_id"
provides:
  - "laps.car_class column populated from billing_sessions -> kiosk_experiences on lap persist"
  - "idx_laps_car_class index on (track, car_class) for event auto-entry matching"
  - "persist_lap() car_class lookup query"
affects: [14-events-and-championships]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "car_class lookup via JOIN billing_sessions + kiosk_experiences before lap INSERT"
    - "NULL car_class as sentinel for pre-v3.0 laps or laps without active billing session"

key-files:
  created: []
  modified:
    - "crates/rc-core/src/db/mod.rs"
    - "crates/rc-core/src/lap_tracker.rs"
    - "crates/rc-core/tests/integration.rs"

key-decisions:
  - "No backfill of historical laps: NULL car_class is explicit sentinel for pre-v3.0 data"
  - "car_class lookup uses driver_id + status='active' to find billing session, not pod_id"
  - "Added kiosk_experiences table to test migrations (was missing, needed for JOIN query in tests)"

patterns-established:
  - "Lap enrichment pattern: look up metadata from billing context before persisting"
  - "NULL sentinel for pre-migration data: no backfill, no crash"

requirements-completed: [DATA-06]

# Metrics
duration: 5min
completed: 2026-03-15
---

# Phase 12 Plan 02: Car Class on Laps Summary

**car_class column on laps table, auto-populated from billing_sessions -> kiosk_experiences JOIN in persist_lap(), with TDD**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-14T19:57:17Z
- **Completed:** 2026-03-14T20:02:12Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 3

## Accomplishments
- laps table now has car_class TEXT column with idx_laps_car_class index on (track, car_class)
- persist_lap() automatically looks up car_class from active billing session's kiosk_experience before INSERT
- Graceful NULL fallback when no active billing session exists (no crash)
- 2 new integration tests verify the car_class population chain and NULL fallback
- All 209 unit tests + 20 integration tests + 93 rc-common + 167 rc-agent tests pass

## Task Commits

Each task was committed atomically (TDD pattern):

1. **Task 1 RED: Write failing tests for car_class population** - `be3085a` (test)
2. **Task 1 GREEN: Implement car_class column and lookup** - `a733868` (feat)

## Files Created/Modified
- `crates/rc-core/src/db/mod.rs` - Added ALTER TABLE laps ADD COLUMN car_class TEXT + idx_laps_car_class index
- `crates/rc-core/src/lap_tracker.rs` - Added car_class lookup from billing_sessions JOIN kiosk_experiences before INSERT, added car_class bind parameter
- `crates/rc-core/tests/integration.rs` - Added kiosk_experiences table to run_test_migrations(), car_class column to laps CREATE, idx_laps_car_class index, 2 new test functions

## Decisions Made
- No backfill of historical laps: NULL car_class is the explicit sentinel for pre-v3.0 data, preserving data provenance
- car_class lookup uses driver_id + status='active' (not pod_id) to find the active billing session, matching the existing resolve_driver_for_pod pattern
- Added kiosk_experiences table to test migrations since it was missing and required for the JOIN query validation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added kiosk_experiences table to test migrations**
- **Found during:** Task 1 RED phase
- **Issue:** run_test_migrations() did not include kiosk_experiences table, but tests need it for the billing_sessions JOIN kiosk_experiences query
- **Fix:** Added CREATE TABLE IF NOT EXISTS kiosk_experiences to run_test_migrations() mirroring production schema
- **Files modified:** crates/rc-core/tests/integration.rs
- **Verification:** Tests compile and pass
- **Committed in:** be3085a (Task 1 RED commit)

**2. [Rule 1 - Bug] Fixed foreign key constraint on test lap inserts**
- **Found during:** Task 1 RED phase
- **Issue:** Test inserted laps with invalid session_id references ('bs-test-1', 'no-session') but PRAGMA foreign_keys=ON rejects invalid FK references
- **Fix:** Created proper sessions table entries before inserting laps
- **Files modified:** crates/rc-core/tests/integration.rs
- **Verification:** Tests pass without FK constraint errors
- **Committed in:** be3085a (Task 1 RED commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for test correctness. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 12 (Data Foundation) is now complete: all schema, indexes, and data enrichment in place
- Phase 13 (Leaderboard Core) can build leaderboard queries using idx_laps_leaderboard
- Phase 14 (Events and Championships) can use laps.car_class for auto-entry matching against hotlap_events.car_class
- car_class is populated on every new lap going forward; historical laps remain NULL

## Self-Check: PASSED

- [x] crates/rc-core/src/db/mod.rs exists
- [x] crates/rc-core/src/lap_tracker.rs exists
- [x] crates/rc-core/tests/integration.rs exists
- [x] .planning/phases/12-data-foundation/12-02-SUMMARY.md exists
- [x] Commit be3085a (Task 1 RED) found
- [x] Commit a733868 (Task 1 GREEN) found

---
*Phase: 12-data-foundation*
*Completed: 2026-03-15*
