---
phase: 14-events-and-championships
plan: 05
subsystem: api
tags: [rust, axum, sqlite, sqlx, public-api, events, championships, leaderboard, group-racing]

# Dependency graph
requires:
  - phase: 14-01
    provides: schema for hotlap_events, hotlap_event_entries, championships, championship_rounds, group_sessions.hotlap_event_id, multiplayer_results
  - phase: 14-03
    provides: auto_enter_event, recalculate_event_positions — entries already exist with badges/positions when public endpoints read them
  - phase: 14-04
    provides: score_group_event, f1_points_for_position, compute_championship_standings, assign_championship_positions
provides:
  - GET /public/events — pageable events list with entry_count, cancelled excluded, sorted by status priority
  - GET /public/events/{id} — event leaderboard with per-entry badges, 107% flags, gap-to-leader, PII-safe display names
  - GET /public/events/{id}/sessions — group session results with F1 points and gap-to-leader per driver
  - GET /public/championships — championships list (non-cancelled, active first)
  - GET /public/championships/{id} — championship metadata + live standings + per-round breakdown
affects: [kiosk-frontend, admin-dashboard, cloud-sync, phase-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - BTreeMap round grouping for per-round championship breakdown (ordered by round_number)
    - PII exclusion pattern: CASE WHEN show_nickname_on_leaderboard=1 AND nickname IS NOT NULL THEN nickname ELSE name END
    - EventsListQuery struct with optional status/sim_type filters applied as inline WHERE string building
    - f1_points_for_position(position, dnf) called inline in event_sessions handler for per-driver F1 points

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/tests/integration.rs

key-decisions:
  - "GET /public/championships/{id} is a NEW full-detail endpoint (standings + rounds) separate from Plan 04's GET /public/championships/{id}/standings (standings-only) — both routes remain registered"
  - "EventsListQuery filters use inline WHERE string building with manual escaping — parameterized query would require sqlx query builder, out of scope for simple string filters"
  - "public_event_sessions computes gap_to_leader_ms inline from multiplayer_results.best_lap_ms min — consistent with score_group_event's approach in Plan 04"
  - "Task 1 and Task 2 handlers committed in the same atomic commit — all routes were written together in one edit session and verified clean before commit"

patterns-established:
  - "Public endpoints exclude PII by construction: no SELECT email/phone/wallet anywhere in public handlers"
  - "BTreeMap<round_number, Value> for grouping championship round results — natural sort order by integer key"

requirements-completed: [EVT-03, EVT-04, EVT-07, GRP-02, GRP-03, CHP-03]

# Metrics
duration: 35min
completed: 2026-03-17
---

# Phase 14 Plan 05: Public Read Endpoints Summary

**5 public GET endpoints for events listing, event leaderboard with badges/107%/gap, group session F1 results, championships listing, and championship standings with per-round breakdown — 331 tests GREEN**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-16T19:53:30Z
- **Completed:** 2026-03-17T00:28:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- `GET /public/events` returns all non-cancelled events sorted by status priority (active → upcoming → scoring → completed), with per-event entry_count subquery
- `GET /public/events/{id}` returns event metadata + leaderboard entries with badges (gold/silver/bronze), 107% flags, gap-to-leader-ms, and PII-safe display names (nickname or real name per driver preference)
- `GET /public/events/{id}/sessions` returns group session results linked to a hotlap event, with F1 2010 points per position and gap-to-leader calculated from best_lap_ms
- `GET /public/championships` returns all non-cancelled championships sorted by status priority with full metadata
- `GET /public/championships/{id}` returns championship metadata + live-computed standings (total_points, wins, p2/p3 counts, best_result, rounds_entered) + per-round breakdown grouped by round_number

## Task Commits

Each task was committed atomically:

1. **Task 1 + Task 2: All 5 public endpoints with tests** - `3cc531f` (feat)

**Plan metadata:** committed after SUMMARY

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - Added EventsListQuery struct, 5 new handlers (public_events_list, public_event_leaderboard, public_championships_list, public_championship_standings, public_event_sessions), 5 new routes registered under /public/*
- `crates/racecontrol/tests/integration.rs` - Added test_public_events_list (cancelled exclusion, active-first ordering, entry_count) and test_public_event_leaderboard (PII exclusion, nickname logic, badge/gap/107% assertions)

## Decisions Made
- `GET /public/championships/{id}` is a new full-detail endpoint distinct from Plan 04's `GET /public/championships/{id}/standings` (standings-only). Both remain registered. Plan 05's version adds description, car_class, sim_type, and per-round breakdown.
- Tasks 1 and 2 were committed atomically in one commit — all handler code was written and verified in a single edit session.
- `public_event_sessions` computes gap_to_leader_ms from the minimum best_lap_ms among non-DNF drivers, consistent with the approach in `score_group_event()`.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - clean implementation. Pre-existing warnings in routes.rs are unchanged from prior phases.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 5 public API endpoints are live and ready for PWA consumption
- The PWA events browsing page can call `GET /public/events` for the events list with status badges
- Event leaderboard page can call `GET /public/events/{id}` for per-class leaderboard with badges and 107% flags
- Championship standings page can call `GET /public/championships/{id}` for full standings with per-round breakdown
- Group event result page can call `GET /public/events/{id}/sessions` for race results with F1 points
- Phase 14 (events-and-championships) is COMPLETE — all 5 plans delivered

---
*Phase: 14-events-and-championships*
*Completed: 2026-03-17*
