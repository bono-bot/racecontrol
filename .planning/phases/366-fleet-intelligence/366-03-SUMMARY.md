---
phase: 366-fleet-intelligence
plan: 03
subsystem: api
tags: [rust, axum, http-409, billing, game-launcher, concurrent-session]

# Dependency graph
requires:
  - phase: 314-billing-atomicity
    provides: BATOM-02 concurrent session guard in billing start handler
  - phase: 311-launch-billing-guard
    provides: LIFE-04 concurrent game guard in game_launcher.rs
provides:
  - HTTP 409 response from billing start when pod already has active session (was HTTP 200)
  - HTTP 409 response from game launch when pod already has active game (was HTTP 200)
  - Structured error bodies: {error: "pod_already_active", active_session_id, pod_id} and {error: "game_already_active", pod_id, detail}
affects: [kiosk, pwa, admin, 366-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Outer wrapper pattern: inner handler returns Json<Value>, outer wrapper adds StatusCode::CONFLICT without changing inner return type"
    - "axum::response::Response with .into_response() for launch_game return type change"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Outer wrapper pattern used for billing start — preserves inner handler's backward-compatible Json<Value> return while adding 409 at wrapper level"
  - "launch_game return type changed to axum::response::Response using .into_response() for uniform 409 handling"
  - "game_already_active error includes 'detail' field with original error string for debugging"
  - "959 tests pass including existing billing + game launch tests — no regressions"

patterns-established:
  - "HTTP 409 Conflict for all duplicate active-resource errors (pod billing, pod game)"
  - "Structured error body convention: {error: machine_readable_key, ...context fields}"

requirements-completed: [GLD-F-04]

# Metrics
duration: 20min
completed: 2026-04-11
---

# Phase 366 Plan 03: Concurrent Session Guard Summary

**HTTP 409 Conflict upgrade for billing start (pod_already_active) and game launch (game_already_active), closing silent-loss P2-09 where callers couldn't distinguish error from success via status code**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-04-11T02:10:00Z
- **Completed:** 2026-04-11T02:30:00Z
- **Tasks:** 3 (Tasks 1+2 in one commit, Task 3 via integration gate commit)
- **Files modified:** 1

## Accomplishments
- Billing start handler: returns HTTP 409 with `{error: "pod_already_active", active_session_id, pod_id}` when `active_timers` or `waiting_for_game` contains the pod_id — previously returned HTTP 200 with text error
- Game launch handler: returns HTTP 409 with `{error: "game_already_active", pod_id, detail}` for `already has a game active` and `game still stopping` errors — previously returned HTTP 200 with `{ok: false, error: ...}`
- Outer wrapper pattern used for billing start to preserve inner handler's existing return type while adding 409 at wrapper level
- `launch_game` return type changed to `axum::response::Response` using `.into_response()` for uniform HTTP status code handling
- 959 tests pass (0 regressions from return type changes)

## Task Commits

1. **Task 1: Billing start HTTP 409** - `92bdc00b` (feat) + `546d00d8` (feat)
2. **Task 2: Game launch HTTP 409** - `546d00d8` (feat)
3. **Task 3: Unit tests / TODO comments** - `546d00d8` (feat)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` — Two handler changes: billing start outer wrapper returning 409, launch_game return type + 409 arm for concurrent game errors (65 lines added, 9 removed)

## Decisions Made
- Outer wrapper pattern for billing start: preserves the inner handler's existing `Json<Value>` return type, adds conflict detection at the wrapper level — avoids touching all 15+ return statements in the large inner handler
- `launch_game` uses `.into_response()` with `axum::response::Response` return — cleaner than wrapper for a match-based handler
- `detail` field included in game_already_active error body to expose the original error string for staff debugging

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None — two commits (92bdc00b + 546d00d8) for the same plan because the outer wrapper approach required a small iteration to get the return type right.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Kiosk, PWA, and admin dashboard can now detect duplicate session errors via HTTP 409 status code
- Billing start 409 body includes `active_session_id` for staff to identify the conflicting session
- Both guards are server-side; no frontend changes required (callers simply need to handle 409)

---
*Phase: 366-fleet-intelligence*
*Completed: 2026-04-11*
