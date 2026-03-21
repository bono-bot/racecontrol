---
phase: 88-leaderboard-integration
plan: "02"
subsystem: api
tags: [rust, axum, sqlite, sqlx, leaderboard, sim_type, multi-game]

# Dependency graph
requires:
  - phase: 88-leaderboard-integration plan 01
    provides: sim_type column in track_records and personal_bests tables

provides:
  - sim_type query param on public_leaderboard endpoint (GET /public/leaderboard?sim_type=)
  - available_sim_types array in public_leaderboard response for frontend game picker
  - sim_type field in every leaderboard record response (all 4 endpoints)
  - sim_type query param on public_track_leaderboard (removes hardcoded assetto_corsa default)
  - sim_type query param on staff track_leaderboard (GET /leaderboard/{track}?sim_type=)
  - sim_type query param on bot_leaderboard (GET /bot/leaderboard?sim_type=)

affects: [frontend-leaderboard, dashboard, bot-integration, 88-leaderboard-integration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Optional sim_type: if Some(st) use WHERE sim_type = ? else omit clause — all query branches explicit, no format! string injection"
    - "Dynamic sqlx query binding: bind conditionally after building query string with format! for optional clauses"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "sim_type=None means all games on all endpoints (backward compatible) — old hardcoded assetto_corsa default removed from public_track_leaderboard"
  - "public_leaderboard returns available_sim_types array for frontend game picker (queried from laps WHERE valid=1)"
  - "bot_leaderboard track-specific branch uses laps table; all-tracks branch uses track_records table — both get sim_type filter"
  - "sim_type included as field in every leaderboard record so frontend can display which game each record belongs to"

patterns-established:
  - "Optional filter pattern: duplicate query branches (filtered/unfiltered) rather than building SQL strings with conditional appending where possible"

requirements-completed: [LB-03]

# Metrics
duration: 6min
completed: 2026-03-21
---

# Phase 88 Plan 02: Leaderboard sim_type Filtering Summary

**Optional sim_type query param added to all 4 leaderboard endpoints with available_sim_types discovery array and per-record sim_type field in responses**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-21T08:12:31Z
- **Completed:** 2026-03-21T08:18:40Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- `public_leaderboard` accepts `?sim_type=` and returns only that game's records; without filter returns all games
- `public_leaderboard` includes `available_sim_types` array (DISTINCT from laps WHERE valid=1) for frontend game picker
- `public_track_leaderboard` removes hardcoded `assetto_corsa` default — `sim_type=None` now means all games, all queries use conditional SQL branches
- `track_leaderboard` (staff, `GET /leaderboard/{track}`) accepts `?sim_type=` via new `StaffTrackLeaderboardQuery` struct
- `bot_leaderboard` adds `sim_type: Option<String>` to `BotLeaderboardQuery` and filters both track-specific and all-tracks query branches
- All 4 endpoints include `"sim_type"` field per record in JSON response

## Task Commits

Each task was committed atomically:

1. **Task 1 + Task 2: Add sim_type filtering to all leaderboard endpoints** - `d88f422` (feat)

**Plan metadata:** to be added in final docs commit

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - PublicLeaderboardQuery struct, sim_type filtering in public_leaderboard, public_track_leaderboard default fix, StaffTrackLeaderboardQuery struct, track_leaderboard update, BotLeaderboardQuery sim_type field, bot_leaderboard conditional filtering

## Decisions Made
- `sim_type=None` means all games on all endpoints (backward compatible): the previous `unwrap_or("assetto_corsa")` in `public_track_leaderboard` was pre-multi-game and inappropriate for a venue serving AC, F1 25, iRacing, and LMU
- `available_sim_types` queries `laps WHERE valid=1` rather than a config table, so it reflects what's actually been driven
- Tasks 1 and 2 committed in a single atomic commit since both modified the same file and were verified together in one `cargo build --release` run

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All leaderboard endpoints are now sim_type-aware
- Frontend can use `available_sim_types` from `/public/leaderboard` response to build a game picker dropdown
- Bot can request `GET /bot/leaderboard?sim_type=f125` to get F1 25-only data
- Staff can use `GET /leaderboard/{track}?sim_type=iracing` for iRacing-specific track records

---
*Phase: 88-leaderboard-integration*
*Completed: 2026-03-21*
