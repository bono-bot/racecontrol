---
phase: 13-leaderboard-core
plan: 01
subsystem: database
tags: [sqlite, lap-validation, leaderboard, integrity, tdd]

# Dependency graph
requires:
  - phase: 12-data-foundation
    provides: laps table schema with car_class column, competitive tables
provides:
  - suspect column on laps table (production ALTER + test CREATE)
  - suspect computation logic in persist_lap() (sector sum + sanity checks)
  - 5 integration tests for suspect flagging
affects: [13-leaderboard-core, 14-events-and-championships, 15-telemetry-and-driver-rating]

# Tech tracking
tech-stack:
  added: []
  patterns: [lap-validity-hardening, suspect-flag-pattern]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "Suspect flag is orthogonal to valid: a lap can be valid=1 (game says ok) AND suspect=1 (sector mismatch or impossible time)"
  - "Zero sectors treated as absent (not flagged) since some sims do not report sector times"
  - "Sector sum tolerance is 500ms to account for minor rounding differences across sim telemetry"
  - "Pre-migration laps get suspect=0 via DEFAULT, treating historical data as clean"

patterns-established:
  - "Suspect flagging: compute before INSERT, bind as i32 (0 or 1)"
  - "Leaderboard queries should filter on suspect=0 AND valid=1"

requirements-completed: [LB-05]

# Metrics
duration: 6min
completed: 2026-03-15
---

# Phase 13 Plan 01: Lap Validity Hardening Summary

**Suspect column on laps table with sector-sum and sanity-time checks computed in persist_lap() before INSERT**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-14T21:27:57Z
- **Completed:** 2026-03-14T21:34:08Z
- **Tasks:** 2 (TDD RED + GREEN)
- **Files modified:** 3

## Accomplishments
- Added suspect INTEGER DEFAULT 0 column to laps table via idempotent ALTER TABLE migration
- Implemented suspect computation in persist_lap(): sanity check (lap_time_ms >= 20000) and sector sum check (|s1+s2+s3 - lap_time_ms| <= 500ms when all sectors present and > 0)
- 5 integration tests covering: sector sum mismatch, impossibly fast time, valid lap, null sectors, zero sectors
- Full test suite green: rc-common (93), rc-agent (167), racecontrol (25) -- 285 total tests pass

## Task Commits

Each task was committed atomically:

1. **TDD RED: Failing suspect tests** - `514f67c` (test)
2. **TDD GREEN: Suspect column + computation logic** - `d0f6e17` (feat)

_TDD REFACTOR phase not needed -- implementation is minimal and clean._

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` - Added ALTER TABLE laps ADD COLUMN suspect INTEGER NOT NULL DEFAULT 0 migration
- `crates/racecontrol/src/lap_tracker.rs` - Added suspect computation (sanity_ok + sector_sum_ok) before INSERT, added suspect bind
- `crates/racecontrol/tests/integration.rs` - Added suspect column to test CREATE TABLE, added 5 test functions

## Decisions Made
- Suspect flag is orthogonal to the valid flag: a lap can be valid=1 (game considers it clean) AND suspect=1 (impossible time or sector mismatch). This allows leaderboard filtering without losing data.
- Zero sectors (Some(0)) treated same as absent sectors (None) -- not flagged. Some sims report zeros instead of null.
- 500ms tolerance for sector sum vs lap time accounts for rounding across different sim telemetry systems.
- Pre-migration laps receive suspect=0 via DEFAULT 0, treating historical data as clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed LapData struct fields in tests**
- **Found during:** TDD RED (writing tests)
- **Issue:** Plan's interface section listed SimType::AssettoCorsaSHM (does not exist) and omitted the required created_at field on LapData
- **Fix:** Used SimType::AssettoCorsa and added created_at: chrono::Utc::now() to all test LapData constructors
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** Tests compile and run (fail on missing suspect column as expected)
- **Committed in:** 514f67c (TDD RED commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor correction to match actual type definitions. No scope change.

## Issues Encountered
None -- plan executed cleanly after interface corrections.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- suspect column is ready for all Phase 13 leaderboard queries to filter on suspect=0
- All future leaderboard endpoints should include WHERE suspect = 0 AND valid = 1
- Foundation complete for 13-02 (leaderboard API endpoints)

## Self-Check: PASSED

- All 3 modified files exist on disk
- Both commit hashes (514f67c, d0f6e17) verified in git log
- All 285 tests pass across 3 crates (rc-common: 93, rc-agent: 167, racecontrol: 25)

---
*Phase: 13-leaderboard-core*
*Completed: 2026-03-15*
