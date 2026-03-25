---
phase: 195-metrics-foundation
plan: "01"
subsystem: database
tags: [metrics, sqlite, jsonl, rust, telemetry, game-launcher, dual-write]

requires: []
provides:
  - "metrics.rs module: LaunchEvent, LaunchOutcome, ErrorTaxonomy structs with serde support"
  - "record_launch_event() dual-write: SQLite launch_events table + JSONL flat file"
  - "DB failure JSONL fallback with db_fallback=true flag (METRICS-02)"
  - "launch_events SQLite table with all METRICS-01 columns and 4 indexes"
  - "All game_launcher call sites write to new launch_events (launch, relaunch, stop, timeout, crash)"
  - "log_game_event() no longer swallows DB errors (METRICS-07)"
  - "extract_launch_fields() and classify_error_taxonomy() helpers"
affects:
  - "195-02 onwards: dynamic timeout, intelligence, crash recovery — all depend on this event data"
  - "game_launcher.rs consumers: any module reading launch_events table"

tech-stack:
  added: []
  patterns:
    - "Dual-write pattern: SQLite primary + JSONL fallback for zero data loss on DB failure"
    - "Pre-move extraction: extract needed fields from Option<String> before moving into message"
    - "Platform-conditional path: #[cfg(target_os)] for JSONL path on Windows vs Linux"

key-files:
  created:
    - "crates/racecontrol/src/metrics.rs"
  modified:
    - "crates/racecontrol/src/db/mod.rs"
    - "crates/racecontrol/src/lib.rs"
    - "crates/racecontrol/src/game_launcher.rs"

key-decisions:
  - "launch_events table is SEPARATE from legacy game_launch_events — richer schema, no backward compat breakage"
  - "DB errors logged via tracing::error, never swallowed — JSONL fallback ensures event is never lost"
  - "extract_launch_fields() called before launch_args is moved into CoreToAgentMessage to avoid borrow-after-move"
  - "classify_error_taxonomy() uses simple heuristics — keyword matching sufficient for Phase 195 (no ML needed)"
  - "stop_game records outcome=Success as informational event — informational state changes do not use Crash/Error"
  - "Only Error game state triggers metrics in handle_game_state_update — Running/Loading/Idle are too chatty"

patterns-established:
  - "Dual-write: always write to JSONL after SQLite, set db_fallback=true if DB failed"
  - "LaunchEvent ID: uuid::Uuid::new_v4() per call site, NOT reused from log_game_event"
  - "Timestamps: chrono::Utc::now() explicit format, not DB DEFAULT, per METRICS-07"

requirements-completed: [METRICS-01, METRICS-02, METRICS-07]

duration: 8min
completed: "2026-03-26"
---

# Phase 195 Plan 01: Metrics Foundation Summary

**SQLite launch_events table + JSONL dual-write infrastructure with LaunchEvent/LaunchOutcome/ErrorTaxonomy types wired into all 5 game_launcher call sites**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-25T22:58:54Z (IST: 2026-03-26T04:28:54)
- **Completed:** 2026-03-25T23:06:54Z
- **Tasks:** 2/2
- **Files modified:** 4

## Accomplishments
- Created `metrics.rs` with LaunchEvent, LaunchOutcome, ErrorTaxonomy, record_launch_event(), JSONL writer, hash_launch_args()
- Added `launch_events` SQLite table with all METRICS-01 columns and 4 performance indexes (pod, combo, outcome, created_at)
- Wired metrics at all 5+ game_launcher call sites: launch, relaunch, stop, timeout, crash/error state update
- Fixed log_game_event() error swallowing — DB failures now logged via tracing::error with JSONL fallback (METRICS-07)
- DB failure path sets db_fallback=true in JSONL event so data recovery is traceable (METRICS-02)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create metrics module with launch_events table and JSONL writer** - `176c2f4e` (feat)
2. **Task 2: Wire record_launch_event into game_launcher and fix log_game_event error swallowing** - `3135e7dc` (feat)

## Files Created/Modified
- `crates/racecontrol/src/metrics.rs` - New module: LaunchEvent struct, LaunchOutcome/ErrorTaxonomy enums, record_launch_event(), append_launch_jsonl(), hash_launch_args(), record_launch_event_jsonl_only()
- `crates/racecontrol/src/db/mod.rs` - Added launch_events table + 4 indexes to migrate() function
- `crates/racecontrol/src/lib.rs` - Added pub mod metrics
- `crates/racecontrol/src/game_launcher.rs` - Fixed log_game_event, added metrics import, wired all call sites, added extract_launch_fields() and classify_error_taxonomy() helpers

## Decisions Made
- `launch_events` is a new table, separate from `game_launch_events` — preserves backward compatibility with existing AI/error_aggregator queries while enabling richer schema
- Only `GameState::Error` triggers a metrics event in `handle_game_state_update` — Running/Loading/Idle are too chatty and already covered by launch/relaunch calls
- `stop_game` records `LaunchOutcome::Success` — graceful stop is informational, not an error
- `classify_error_taxonomy()` uses keyword heuristics — simple and fast, sufficient for Phase 195 baseline

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed borrow-after-move for launch_args in launch_game()**
- **Found during:** Task 2 (cargo check)
- **Issue:** `launch_args` moved into `CoreToAgentMessage::LaunchGame` at line 146, then borrowed again at line 182 for `extract_launch_fields()`
- **Fix:** Call `extract_launch_fields(&launch_args)` BEFORE the `tx.send(cmd)` call that consumes `launch_args`
- **Files modified:** `crates/racecontrol/src/game_launcher.rs`
- **Verification:** `cargo check` passes cleanly
- **Committed in:** `3135e7dc` (Task 2 commit)

**2. [Rule 1 - Bug] Fixed implicit-borrow warning in extract_launch_fields()**
- **Found during:** Task 2 (cargo check)
- **Issue:** `let Some(ref args_json) = launch_args` matched on `&Option<String>` with explicit `ref` — redundant and warned by rustc
- **Fix:** Changed to `let Some(args_json) = launch_args` (implicit borrow)
- **Files modified:** `crates/racecontrol/src/game_launcher.rs`
- **Verification:** `cargo check` passes with zero errors
- **Committed in:** `3135e7dc` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (Rule 1 - Bug, both compile-time issues caught by cargo check)
**Impact on plan:** Both fixes required for correctness. No scope creep. Both in same Task 2 commit.

## Issues Encountered
None beyond the two compile errors fixed above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Launch event data backbone is in place — all game starts/stops/timeouts/crashes are recorded
- Phase 195-02 (dynamic timeouts) can query `launch_events` table for historical timeout data
- Phase 195-03 (intelligence) has ErrorTaxonomy classification ready to extend
- DB schema is additive (new table, no ALTER on existing tables) — safe to deploy on live server

## Self-Check: PASSED

- metrics.rs: FOUND
- db/mod.rs: FOUND
- game_launcher.rs: FOUND
- SUMMARY.md: FOUND
- commit 176c2f4e: FOUND
- commit 3135e7dc: FOUND

---
*Phase: 195-metrics-foundation*
*Completed: 2026-03-26*
