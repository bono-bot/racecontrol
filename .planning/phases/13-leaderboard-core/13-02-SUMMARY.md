---
phase: 13-leaderboard-core
plan: 02
subsystem: api
tags: [axum, leaderboard, sim-type-filter, circuit-records, vehicle-records, sqlite]

# Dependency graph
requires:
  - phase: 13-leaderboard-core
    provides: suspect column on laps table, suspect computation in persist_lap()
provides:
  - sim_type filtering on public_track_leaderboard (defaults to assetto_corsa)
  - show_invalid toggle (includes valid=0 but still hides suspect=1)
  - /public/circuit-records endpoint (one record per track+car+sim_type)
  - /public/vehicle-records/{car} endpoint (best per track for a given car)
  - LeaderboardQuery struct with sim_type, car, show_invalid params
affects: [13-leaderboard-core, 14-events-and-championships, pwa-leaderboard-pages]

# Tech tracking
tech-stack:
  added: []
  patterns: [sim-type-default-assetto-corsa, suspect-always-hidden, show-invalid-toggle]

key-files:
  created: []
  modified:
    - crates/rc-core/src/api/routes.rs
    - crates/rc-core/tests/integration.rs

key-decisions:
  - "sim_type defaults to assetto_corsa for backward compatibility with existing PWA consumers"
  - "Suspect laps are ALWAYS hidden from public endpoints regardless of show_invalid toggle"
  - "Circuit records query from laps table (not track_records) to include sim_type dimension"
  - "Vehicle records grouped by (track, sim_type) to avoid cross-sim contamination"

patterns-established:
  - "Leaderboard queries: WHERE sim_type = ? AND (suspect IS NULL OR suspect = 0)"
  - "show_invalid=true drops valid=1 but keeps suspect filter"
  - "Circuit/vehicle records use correlated subquery for driver display name (nickname-aware)"

requirements-completed: [LB-01, LB-02, LB-03, LB-04, LB-06]

# Metrics
duration: 8min
completed: 2026-03-15
---

# Phase 13 Plan 02: Leaderboard sim_type Filtering + Circuit/Vehicle Records Summary

**sim_type filtering on track leaderboard (defaults assetto_corsa), /public/circuit-records and /public/vehicle-records/{car} endpoints with suspect-always-hidden policy**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-14T21:37:48Z
- **Completed:** 2026-03-14T21:46:14Z
- **Tasks:** 2 (TDD RED + GREEN)
- **Files modified:** 2

## Accomplishments
- Modified public_track_leaderboard to accept sim_type, car, and show_invalid query params with suspect filtering
- Created /public/circuit-records endpoint returning one record per (track, car, sim_type) combo
- Created /public/vehicle-records/{car} endpoint returning best lap per track for a given car
- 6 integration tests covering sim_type isolation, suspect hiding, invalid toggle, circuit records, vehicle records
- All 209 library tests + 93 rc-common tests pass

## Task Commits

Each task was committed atomically:

1. **TDD RED: Failing tests for leaderboard query patterns** - `33ae2d7` (test)
2. **TDD GREEN: Implement handlers + route registration** - `a0b28d3` (feat)

_TDD REFACTOR phase not needed -- handlers are clean and follow existing patterns._

## Files Created/Modified
- `crates/rc-core/src/api/routes.rs` - Added LeaderboardQuery struct, modified public_track_leaderboard with sim_type+suspect+show_invalid, new public_circuit_records and public_vehicle_records handlers, registered 2 new routes
- `crates/rc-core/tests/integration.rs` - Added insert_test_lap helper, 6 new tests (sim_type_filter, no_cross_sim, suspect_hidden, invalid_toggle, circuit_records, vehicle_records)

## Decisions Made
- sim_type defaults to "assetto_corsa" when absent, ensuring backward compatibility with existing PWA consumers that don't yet pass this parameter.
- Suspect laps are always hidden from all public endpoints. The show_invalid toggle only controls whether valid=0 laps appear -- suspect=1 are never shown.
- Circuit records query the laps table directly (not track_records) because track_records has no sim_type column. At venue scale (<50k laps) this is acceptable performance.
- Vehicle records group by (track, sim_type) to prevent cross-sim contamination in per-car record lists.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing uncommitted change to lap_tracker.rs (stale import) caused stash conflict during build verification. Resolved by reverting the unrelated change. No impact on plan execution.
- Integration test compilation includes failing tests from Plan 13-03 (TDD RED for `get_previous_record_holder`). These are expected -- they are the RED phase for the next plan and do not affect this plan's tests.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All leaderboard endpoints now filter by sim_type and exclude suspect laps
- PWA can consume /public/leaderboard/{track}?sim_type=f1_25 and /public/circuit-records?sim_type=assetto_corsa
- Foundation ready for 13-03 (track record notifications) and downstream PWA pages

## Self-Check: PASSED

- Both modified files exist on disk
- Both commit hashes (33ae2d7, a0b28d3) verified in git log
- All 209 library tests + 6 new integration tests + 93 rc-common tests pass

---
*Phase: 13-leaderboard-core*
*Completed: 2026-03-15*
