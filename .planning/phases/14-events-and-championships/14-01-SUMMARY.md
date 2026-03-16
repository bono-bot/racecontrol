---
phase: 14-events-and-championships
plan: 01
subsystem: testing
tags: [sqlite, sqlx, tdd, schema-migrations, hotlap-events, championships, group-sessions]

# Dependency graph
requires:
  - phase: 12-data-foundation
    provides: hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, driver_ratings tables
  - phase: 13-public-pwa
    provides: integration test infrastructure (create_test_db, run_test_migrations, seed helpers)
provides:
  - 19 failing RED tests covering all Phase 14 core business logic
  - Schema migrations: group_sessions.hotlap_event_id, championship_standings.p2_count, championship_standings.p3_count
  - group_sessions + multiplayer_results tables added to run_test_migrations()
  - Complete test scaffold for Plans 14-02 through 14-05 to implement against
affects:
  - 14-02-PLAN.md (auto-entry logic — test_auto_event_entry etc. define the contract)
  - 14-03-PLAN.md (group scoring — test_f1_points_scoring, test_gap_to_leader etc.)
  - 14-04-PLAN.md (championship standings — test_championship_standings_sum etc.)
  - 14-05-PLAN.md (cloud sync — test_sync_competitive_tables, test_sync_targeted_telemetry)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent ALTER TABLE: let _ = sqlx::query(...).execute(pool).await — swallows duplicate column errors"
    - "RED test scaffold: insert setup data, assert post-implementation state, fail because no logic exists yet"
    - "pod_id omitted in test laps inserts (nullable FK) — avoids seeding pods for competitive logic tests"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "pod_id omitted in Phase 14 test laps inserts — pod context irrelevant for event/championship logic, nullable FK allows NULL"
  - "group_sessions + multiplayer_results added to run_test_migrations() — required for GRP-01/GRP-04 test data setup"
  - "7 of 19 tests pass vacuously (negative cases: no-match, expired event, slower lap, no-reference badge, sync queries) — valid RED because test_auto_event_entry proves auto-entry logic is missing"
  - "p2_count/p3_count as ALTER TABLE not schema change — avoids rewriting CREATE TABLE, matches production idempotent pattern"

patterns-established:
  - "Phase 14 RED tests: direct SQL assertions against expected post-state; implementation fills the gap in Plans 02-04"
  - "Test isolation: each test uses unique IDs (ae-drv-1, nm-drv-1 etc.) — no cross-test interference in parallel execution"

requirements-completed: [EVT-02, EVT-05, EVT-06, GRP-01, GRP-04, CHP-02, CHP-04, CHP-05]

# Metrics
duration: 6min
completed: 2026-03-17
---

# Phase 14 Plan 01: Events and Championships Summary

**19 failing RED test stubs covering auto-entry, 107% rule, badges, F1 scoring, gap-to-leader, and championship tiebreaker — with 3 schema migrations closing the p2/p3 tiebreaker gap**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-03-17T19:11:39Z
- **Completed:** 2026-03-17T19:17:33Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added 3 idempotent ALTER TABLE migrations to db/mod.rs: group_sessions.hotlap_event_id (GRP-01 F1 scoring link), championship_standings.p2_count and p3_count (CHP-04 tiebreaker)
- Extended run_test_migrations() with group_sessions, group_session_members, multiplayer_results tables and Phase 14 ALTER TABLE statements
- Wrote 19 test stubs covering EVT-02 (5 tests), EVT-05 (2), EVT-06 (4), GRP-01 (2), GRP-04 (1), CHP-02 (1), CHP-04 (2), SYNC-01 (1), SYNC-02 (1)
- 12 tests FAIL RED (auto-entry, badges, F1 scoring, gap, standings, tiebreaker), 7 pass vacuously (correct negative cases)

## Task Commits

Each task was committed atomically:

1. **Task 1: Schema migrations** - `73e6a42` (feat)
2. **Task 2: 19 failing RED tests** - `27ea6ba` (test)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` - 3 Phase 14 ALTER TABLE migrations added after existing idempotent block
- `crates/racecontrol/tests/integration.rs` - group_sessions/multiplayer_results in run_test_migrations() + 19 RED test stubs appended

## Decisions Made

- Omitted pod_id from test lap inserts — pod context not needed for competitive logic tests, FK is nullable
- Added group_sessions and multiplayer_results to run_test_migrations() as a Rule 3 auto-fix (missing tables would block GRP test compilation)
- 7 of 19 tests pass vacuously (negative cases: no entry expected) — acceptable since test_auto_event_entry is the canonical RED failure proving implementation is absent

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added group_sessions + multiplayer_results to run_test_migrations()**
- **Found during:** Task 2 (writing GRP-01 test_f1_points_scoring)
- **Issue:** group_sessions and multiplayer_results tables not in test setup — FK constraints would fail on INSERT
- **Fix:** Added both CREATE TABLE statements plus group_session_members to run_test_migrations() before Phase 14 ALTER TABLE block
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** Tests compile and run without FK errors
- **Committed in:** 27ea6ba (Task 2 commit)

**2. [Rule 1 - Bug] Removed pod_id from Phase 14 test lap inserts**
- **Found during:** Task 2 — first test run showed FOREIGN KEY constraint failed (code 787)
- **Issue:** Test laps used 'ae-pod-1' etc. as pod_id but pods table was empty — FK violation
- **Fix:** Omitted pod_id column from INSERT statements (nullable FK, NULL is valid)
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** FK errors eliminated, all pre-existing 269 tests still pass
- **Committed in:** 27ea6ba (Task 2 commit)

**3. [Rule 1 - Bug] Removed broken syntax in test_championship_tiebreaker_wins**
- **Found during:** Task 2 — draft had invalid INSERT with missing quote and duplicate inserts
- **Fix:** Removed malformed multi-row INSERT with unwrap_or_default, kept two separate clean INSERTs
- **Files modified:** crates/racecontrol/tests/integration.rs
- **Verification:** Compiles cleanly
- **Committed in:** 27ea6ba (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 bugs)
**Impact on plan:** All auto-fixes necessary for tests to compile and run. No scope creep.

## Issues Encountered

None beyond the auto-fixed issues above.

## Next Phase Readiness

- Plan 14-02 can now implement auto_enter_event() with clear test contract: test_auto_event_entry (happy path), test_auto_entry_no_match, test_auto_entry_date_range, test_auto_entry_faster_lap, test_auto_entry_no_replace_slower
- Plan 14-03 can implement score_group_event() targeting test_f1_points_scoring, test_dns_dnf_zero_points, test_gap_to_leader, and the 107%/badge tests
- Plan 14-04 can implement compute_championship_standings() targeting test_championship_standings_sum, test_championship_tiebreaker_wins, test_championship_tiebreaker_p2
- Plan 14-05 can extend cloud_sync targeting test_sync_competitive_tables, test_sync_targeted_telemetry

---
*Phase: 14-events-and-championships*
*Completed: 2026-03-17*
