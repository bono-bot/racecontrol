---
phase: 14-events-and-championships
plan: 04
subsystem: api
tags: [rust, axum, sqlite, sqlx, f1-scoring, championship, events, racing]

# Dependency graph
requires:
  - phase: 14-01
    provides: schema for hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, group_sessions.hotlap_event_id, multiplayer_results
  - phase: 14-02
    provides: staff CRUD endpoints for events and championships
  - phase: 14-03
    provides: auto_enter_event, recalculate_event_positions in lap_tracker.rs
provides:
  - score_group_event() — reads multiplayer_results, assigns F1 2010 points, writes hotlap_event_entries
  - f1_points_for_position() — pure F1 2010 point lookup (25/18/15/12/10/8/6/4/2/1, DNF=0)
  - POST /staff/group-sessions/{id}/complete — staff endpoint to complete session and trigger scoring
  - compute_championship_standings() — aggregates points from hotlap_event_entries, upserts championship_standings
  - assign_championship_positions() — sorts championship_standings by F1 tiebreaker, updates position column
  - GET /public/championships/{id}/standings — live-computed public standings endpoint
affects: [14-05, cloud-sync, kiosk-frontend, admin-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - F1 2010 points lookup via const array indexed by (position-1)
    - score_group_event / compute_championship_standings made pub for direct test invocation
    - Championship tiebreaker: total_points DESC, wins DESC, p2_count DESC, p3_count DESC
    - assign_championship_positions as separate pub fn — allows tiebreaker tests without seeding hotlap_event_entries
    - Live standings computation at read time (not materialized) for HTTP handler

key-files:
  created: []
  modified:
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "score_group_event writes gap_to_leader_ms at score time (not via recalculate_event_positions) — group racing uses best_lap_ms not hotlap order"
  - "assign_championship_positions is a separate pub fn from compute_championship_standings — tiebreaker tests pre-insert standings rows directly, no hotlap_event_entries needed"
  - "compute_championship_standings upserts into championship_standings table (persisted) — tests query table directly after call"
  - "Public standings endpoint computes live from hotlap_event_entries at read time (not from championship_standings cache)"

patterns-established:
  - "Scoring functions pub for direct test invocation — avoids full AppState construction in integration tests"
  - "Two-phase standings: compute_championship_standings fills the table, assign_championship_positions orders it"

requirements-completed: [GRP-01, GRP-02, GRP-03, GRP-04, CHP-02, CHP-03, CHP-04]

# Metrics
duration: 35min
completed: 2026-03-17
---

# Phase 14 Plan 04: F1 Group Event Scoring and Championship Standings Summary

**F1 2010 point scoring from multiplayer_results into hotlap_event_entries, with live-computed championship standings using wins/P2/P3 tiebreaker — 6 new tests GREEN, 329 total passing**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-17T00:00:00Z
- **Completed:** 2026-03-17T00:35:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `score_group_event()` function processes multiplayer_results into hotlap_event_entries with F1 2010 points, gap_to_leader_ms, and result_status (dnf/finished)
- `POST /staff/group-sessions/{id}/complete` staff endpoint marks session complete and triggers scoring, returning `{ status: "completed", scored_event: event_id }`
- `compute_championship_standings()` aggregates points/wins/p2_count/p3_count across all championship rounds and persists to championship_standings table
- `assign_championship_positions()` applies F1 tiebreaker sort (points DESC, wins DESC, p2 DESC, p3 DESC) and writes 1-indexed position column
- `GET /public/championships/{id}/standings` computes standings live from hotlap_event_entries at read time

## Task Commits

Each task was committed atomically:

1. **Task 1 + Task 2: F1 scoring and championship standings** - `ea97697` (feat)

**Plan metadata:** committed after SUMMARY

## Files Created/Modified
- `crates/racecontrol/src/lap_tracker.rs` - Added F1_2010_POINTS, f1_points_for_position(), score_group_event(), compute_championship_standings(), assign_championship_positions()
- `crates/racecontrol/src/api/routes.rs` - Added complete_group_session handler, POST /staff/group-sessions/{id}/complete route, public_championship_standings_handler, GET /public/championships/{id}/standings route
- `crates/racecontrol/tests/integration.rs` - Updated import to include new pub functions, added function calls to 6 tests (score_group_event, recalculate_event_positions, compute_championship_standings, assign_championship_positions)

## Decisions Made
- `assign_championship_positions` is a separate pub function from `compute_championship_standings` so that the two tiebreaker tests (which pre-insert standings rows directly without seeding hotlap_event_entries) can call it in isolation.
- `score_group_event` computes gap_to_leader_ms inline (not via `recalculate_event_positions`) because group racing uses `best_lap_ms` from multiplayer_results rather than the hotlap leaderboard order.
- The public standings HTTP handler computes live from `hotlap_event_entries` JOIN `championship_rounds` — not from the persisted `championship_standings` cache — so it reflects real-time data without requiring an explicit compute step.

## Deviations from Plan

None - plan executed exactly as written. The plan noted the public handler function should be added here even though the route would be registered in Plan 05; both were added here since tests compile as a single binary.

## Issues Encountered
None - TDD flow was clean. Tests were already written as RED stubs from Plan 01; adding function calls to each test was the only change needed alongside the implementation.

## Next Phase Readiness
- F1 scoring pipeline complete: lap -> auto_enter_event -> recalculate_event_positions (hotlap events) and multiplayer_results -> score_group_event (group racing events)
- Championship standings computation and position assignment ready for Plan 05 (cloud sync extension)
- Public championships standings endpoint live at GET /public/championships/{id}/standings

---
*Phase: 14-events-and-championships*
*Completed: 2026-03-17*
