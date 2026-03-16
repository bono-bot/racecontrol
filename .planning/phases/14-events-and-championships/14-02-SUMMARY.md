---
phase: 14-events-and-championships
plan: 02
subsystem: api
tags: [rust, axum, sqlite, sqlx, events, championships, staff-api]

# Dependency graph
requires:
  - phase: 14-events-and-championships Plan 01
    provides: "hotlap_events, championships, championship_rounds schema migrations + 19 RED test stubs"
provides:
  - "POST /staff/events — create hotlap event with full field set"
  - "GET /staff/events — list all events"
  - "GET /staff/events/{id} — fetch single event"
  - "PUT /staff/events/{id} — update event fields via COALESCE"
  - "POST /staff/championships — create championship"
  - "GET /staff/championships — list all championships"
  - "GET /staff/championships/{id} — championship + rounds detail"
  - "POST /staff/championships/{id}/rounds — link event as round"
  - "POST /staff/events/{id}/link-session — link group session to event"
affects: [14-03, 14-04, 14-05, event-auto-entry, public-leaderboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "COALESCE-based partial UPDATE: bind Option<T> per field; omitted fields keep existing values"
    - "sqlx raw row via try_get::<Type, _> for dynamic column mapping into serde_json::Value"

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/api/routes.rs — 9 new handler functions + 6 route registrations"

key-decisions:
  - "COALESCE UPDATE pattern for update_hotlap_event: bind all 6 optional fields, only provided ones take effect — avoids dynamic SQL string building which doesn't compile with sqlx query() type"
  - "list_staff_events/list_staff_championships use raw sqlx::query() + try_get() mapping to Value — avoids query_as struct definition overhead for simple listing"
  - "add_championship_round updates both championship_rounds table AND hotlap_events.championship_id AND championships.total_rounds atomically — 3 separate statements, errors in steps 2-3 logged but not fatal"
  - "Tasks 1 and 2 committed together as 3897038 — both implemented in single atomic edit session; task split is logical not physical"

patterns-established:
  - "Staff endpoint pattern: check_terminal_auth() first, return Json(json!({ error })) on auth fail, then logic"
  - "COALESCE UPDATE: bind Option<String>/Option<i64> per column, SQL uses COALESCE(?, col) for non-destructive partial updates"

requirements-completed: [EVT-01, CHP-01, CHP-05]

# Metrics
duration: 7min
completed: 2026-03-17
---

# Phase 14 Plan 02: Events and Championships Staff API Summary

**9 staff CRUD endpoints for hotlap events and championships in routes.rs, using check_terminal_auth() and COALESCE-based partial updates**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-03-16T19:20:00Z
- **Completed:** 2026-03-16T19:27:52Z
- **Tasks:** 2 (committed together)
- **Files modified:** 1

## Accomplishments
- 4 hotlap event endpoints: create (POST), list (GET), get-by-id (GET), update (PUT with COALESCE)
- 5 championship endpoints: create (POST), list (GET), get-with-rounds (GET + JOIN), add-round (POST with 3 cascading UPDATEs), link-group-session (POST)
- All 9 endpoints gate on check_terminal_auth() — same terminal PIN/secret auth as other staff routes
- cargo build -p racecontrol-crate succeeds with zero errors, only pre-existing warnings

## Task Commits

1. **Task 1: Staff hotlap event CRUD endpoints** - `3897038` (feat) — also includes Task 2 championship handlers
2. **Task 2: Staff championship CRUD and round assignment** — committed with Task 1 as single atomic edit

**Plan metadata:** (pending)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` — 9 new handler functions + 6 route registrations + 513 lines added

## Decisions Made

- COALESCE UPDATE pattern for `update_hotlap_event`: bind all 6 optional fields as `Option<T>`, SQL uses `COALESCE(?, column)` — this compiles cleanly with sqlx whereas dynamic SQL string building (concatenating SET clauses) requires rebinding which doesn't work with sqlx's type system
- `list_staff_events` and `list_staff_championships` use raw `sqlx::query()` + `row.try_get::<Type, _>(col)` mapping to avoid defining intermediate structs for one-off listing queries
- `add_championship_round` performs 3 SQL statements: INSERT into championship_rounds, UPDATE hotlap_events.championship_id, UPDATE championships.total_rounds — steps 2 and 3 use `let _ =` (fire-and-forget errors) since the round was already inserted
- Tasks 1 and 2 were committed in a single commit because all 9 handlers were written in one atomic edit session

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed dead sqlx::query_as first-pass in list_staff_events**
- **Found during:** Task 1 (writing list_staff_events)
- **Issue:** Initial draft had two fetch calls — a typed query_as with wrong tuple signature (unused variable), followed by the actual raw query
- **Fix:** Removed the dead query_as call before compilation
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Verification:** Build passes, no dead code warnings from new handlers
- **Committed in:** 3897038

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor cleanup before first compile. No scope creep.

## Issues Encountered
None — build succeeded on first attempt after cleanup.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 9 staff write endpoints are live; staff can now set up the competitive structure
- Plan 14-03 can begin: auto-entry logic (persist_lap inserts matching laps into hotlap_event_entries)
- Plan 14-04 public leaderboard endpoint ready to build against events table
- 19 RED tests from Plan 14-01 are still failing — they will turn GREEN as Plans 03-05 implement the logic

---
*Phase: 14-events-and-championships*
*Completed: 2026-03-17*
