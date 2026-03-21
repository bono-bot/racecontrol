---
phase: 89-psychology-foundation
plan: 89-03
subsystem: api
tags: [psychology, badges, streaks, notifications, axum, sqlx]

# Dependency graph
requires:
  - phase: 89-01
    provides: psychology.rs types, DB schema (achievements, driver_achievements, streaks, nudge_queue)
  - phase: 89-02
    provides: evaluate_badges, update_streak, queue_notification, spawn_dispatcher, is_whatsapp_budget_exceeded
provides:
  - Psychology hooks in post_session_hooks (evaluate_badges + update_streak auto-fire on every session end)
  - psychology::spawn_dispatcher launched on RaceControl boot
  - 5 seed badges in achievements table with JSON criteria
  - GET /psychology/badges -- list active badge definitions
  - GET /psychology/badges/{driver_id} -- earned badges per driver
  - GET /psychology/streaks/{driver_id} -- streak data per driver
  - GET /psychology/nudge-queue -- admin nudge queue listing
  - POST /psychology/test-nudge -- manual notification testing
affects: [phase-90-customer-progression, billing.rs consumers, admin dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns: [fire-and-forget async hooks in post_session_hooks, INSERT OR IGNORE seed pattern for badge definitions, staff-protected psychology admin endpoints]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Psychology routes placed in staff_routes (JWT-protected) since badge queries and nudge admin are staff-facing; customer-facing badge display deferred to Phase 90"
  - "evaluate_badges and update_streak called sequentially at end of post_session_hooks -- already inside tokio::spawn so non-blocking to billing response"
  - "count variable extracted before into_iter().map() for driver_badges and list_nudge_queue handlers to avoid use-after-move (Rust borrow)"

patterns-established:
  - "Psychology hooks pattern: add new post-session analysis as numbered steps at end of post_session_hooks"
  - "Badge seed pattern: INSERT OR IGNORE with static badge IDs -- idempotent across DB migrations"

requirements-completed: [FOUND-02, FOUND-03, FOUND-04, FOUND-05]

# Metrics
duration: 18min
completed: 2026-03-21
---

# Phase 89 Plan 03: Psychology Foundation Wiring Summary

**Psychology engine fully integrated: badges and streaks auto-fire on every session end, dispatcher starts on boot, 5 seed badges in DB, and 5 API endpoints expose psychology data to staff**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-03-21T02:20:00Z
- **Completed:** 2026-03-21T02:38:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- post_session_hooks in billing.rs now calls evaluate_badges + update_streak as steps 5 and 6 (after WhatsApp receipt)
- psychology::spawn_dispatcher is launched at RaceControl boot alongside other background tasks in main.rs
- 5 seed badge definitions inserted into achievements table via INSERT OR IGNORE (idempotent)
- 5 psychology API endpoints added to staff_routes with full handler implementations

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire psychology hooks into billing lifecycle and server startup** - `8440601` (feat)
2. **Task 2: Seed badge definitions and add psychology API endpoints** - `9b69b77` (feat)

**Plan metadata:** (this SUMMARY + state updates)

## Files Created/Modified

- `crates/racecontrol/src/billing.rs` - Added evaluate_badges + update_streak calls in post_session_hooks (steps 5 and 6)
- `crates/racecontrol/src/main.rs` - Added psychology to imports, psychology::spawn_dispatcher call after scheduler::spawn
- `crates/racecontrol/src/db/mod.rs` - Added INSERT OR IGNORE seed for 5 badge definitions
- `crates/racecontrol/src/api/routes.rs` - Added use crate::psychology, 5 route entries in staff_routes, 5 handler functions

## Decisions Made

- Psychology routes placed in staff_routes (staff JWT protected) since the endpoints serve admin/debug workflows. Customer-facing badge display (PWA profile page) is deferred to Phase 90.
- evaluate_badges and update_streak are awaited sequentially at end of post_session_hooks. This is acceptable because post_session_hooks itself runs inside tokio::spawn from end_billing_session — errors are logged internally in each psychology function and do not affect billing.
- count variable extracted before into_iter().map() in driver_badges and list_nudge_queue to avoid Rust use-after-move on the Vec.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. Two pre-existing test failures (`server_ops::tests::test_exec_echo` and `config::tests::config_fallback_preserved_when_no_env_vars` in parallel test runs due to env var collision) were present before these changes and are unrelated to the psychology wiring.

## Next Phase Readiness

- Complete: Psychology foundation fully wired. Every billing session end triggers badge evaluation and streak update. Dispatcher runs. 5 seed badges defined.
- Ready for: Phase 90 (Customer Progression) — can query driver badges/streaks via API, add more badge types, and surface badges in the PWA customer profile.
- No blockers.

## Self-Check

---
*Phase: 89-psychology-foundation*
*Completed: 2026-03-21*
