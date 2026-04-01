---
phase: 302-structured-event-archive
plan: 01
subsystem: database
tags: [sqlite, jsonl, scp, chrono-tz, sha256, event-archive, tokio]

# Dependency graph
requires:
  - phase: 300-backup-pipeline
    provides: SCP pattern (Steps A-E), SHA256 verify, IST window, NaiveDate dedup — copied verbatim
provides:
  - system_events SQLite table with 3 indexes (type, pod, timestamp)
  - EventArchiveConfig struct with serde defaults (no TOML changes at deploy)
  - event_archive.rs module: append_event, spawn, export_daily_jsonl, purge_old_events, transfer_jsonl_to_remote
  - Module wired in lib.rs and main.rs
  - 5 unit tests: insert, JSONL export, idempotency, purge, config defaults
affects:
  - 302-02 (REST API plan — depends on system_events table existing)
  - Any future call site using append_event()

# Tech tracking
tech-stack:
  added: []
  patterns:
    - fire-and-forget DB insert via tokio::spawn (same as activity_log.rs)
    - hourly background tick with idempotent export + purge + time-gated SCP
    - IST 02:00-03:59 window + NaiveDate deduplication for nightly SCP
    - format!() for SQLite datetime() modifier parameter (bind params not supported)

key-files:
  created:
    - crates/racecontrol/src/event_archive.rs
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "event_archive.rs is a NEW module separate from activity_log.rs — different concerns (system-wide archive vs pod real-time log)"
  - "system_events table NOT named events (collision with hotlap competition table) or scheduler_events (WoL log)"
  - "retention_days uses format!() in SQL string — SQLite datetime() modifier cannot use bind parameters"
  - "last_remote_transfer owned by event_archive, NOT shared with backup_pipeline — each module tracks independently"
  - "insert_event_direct() is pub(crate) helper so tests can bypass the fire-and-forget tokio::spawn"
  - "unwrap() allowed in #[cfg(test)] block only — all production code uses ?, .ok(), unwrap_or_else"

patterns-established:
  - "Pattern: format retention days into SQL string for SQLite datetime() modifiers"
  - "Pattern: pub(crate) inner helper for testability with fire-and-forget public wrapper"

requirements-completed:
  - EVENT-01
  - EVENT-02
  - EVENT-03
  - EVENT-04

# Metrics
duration: 25min
completed: 2026-04-01
---

# Phase 302 Plan 01: Structured Event Archive — Module + Data Layer Summary

**SQLite system_events table with EventArchiveConfig, hourly JSONL export + 90-day purge + nightly SCP to Bono VPS using backup_pipeline SCP pattern verbatim**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-01T16:30:00Z
- **Completed:** 2026-04-01T16:55:00Z
- **Tasks:** 2 (+ 1 deviation fix)
- **Files modified:** 5

## Accomplishments

- system_events SQLite table created in db/mod.rs migrate() with 3 indexes (type, pod, timestamp)
- EventArchiveConfig added to config.rs with serde defaults — no racecontrol.toml change at deploy
- event_archive.rs module with all 5 required functions: append_event, spawn, export_daily_jsonl, purge_old_events, transfer_jsonl_to_remote
- SCP transfer reuses backup_pipeline.rs Steps A-E verbatim with StrictHostKeyChecking=no, 120s timeout, SHA256 verify
- 5 unit tests pass: insert, JSONL export, idempotency, purge, config defaults
- event_archive::spawn wired after backup_pipeline::spawn in main.rs

## Task Commits

Each task was committed atomically:

1. **Task 1: EventArchiveConfig + system_events table + event_archive.rs module** - `3bc332ec` (feat)
2. **Task 2: Wire event_archive::spawn into main.rs** - `dca4988a` (feat)
3. **Deviation fix: add event_archive to main.rs use imports** - `c47e23c3` (fix)

## Files Created/Modified

- `crates/racecontrol/src/event_archive.rs` — NEW: 5 public/private functions, 5 unit tests, 420+ lines
- `crates/racecontrol/src/config.rs` — EventArchiveConfig struct + Default impl + field in Config + default_config()
- `crates/racecontrol/src/db/mod.rs` — system_events table + 3 indexes at end of migrate()
- `crates/racecontrol/src/lib.rs` — pub mod event_archive (after error_rate, alphabetical)
- `crates/racecontrol/src/main.rs` — event_archive import + event_archive::spawn call

## Decisions Made

- `event_archive.rs` is a separate module from `activity_log.rs` — different audiences (system-wide archive vs pod real-time log)
- Table named `system_events` to avoid collision with `events` (hotlap competition) and `scheduler_events` (WoL log)
- `retention_days` formatted inline into SQL string because SQLite `datetime()` modifier cannot use bind parameters
- `last_remote_transfer` tracked independently per module — event archive and DB backup transfer tracking are separate
- `insert_event_direct()` marked `pub(crate)` so tests can call it directly without going through fire-and-forget tokio::spawn
- `Default` impl for `EventArchiveConfig` added and `default_config()` in Config updated — avoids compile error on missing field

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing event_archive in main.rs use imports**
- **Found during:** Release build verification after Task 2
- **Issue:** `event_archive::spawn` referenced in main.rs body but `event_archive` not in the `use racecontrol_crate::{...}` import block — `cargo build --release` failed with E0433
- **Fix:** Added `event_archive` to the use block in main.rs
- **Files modified:** `crates/racecontrol/src/main.rs`
- **Verification:** `cargo build --release --bin racecontrol` now compiles cleanly
- **Committed in:** `c47e23c3` (separate fix commit)

---

**Total deviations:** 1 auto-fixed (blocking import error)
**Impact on plan:** Necessary for release build. Zero scope creep.

## Issues Encountered

- Pre-existing integration test failures (`BillingTimer` missing `nonce` field) — out of scope, not caused by this plan. Tests in `tests/integration.rs` were already broken before Phase 302. Used `--lib` flag to run only unit tests.

## Known Stubs

None — all functions are fully implemented and wired. append_event() is ready to be called from billing.rs, deploy.rs, etc. (Phase 302-02 instruments those call sites).

## Next Phase Readiness

- `system_events` table exists in DB — Phase 302-02 (REST API) can build on it immediately
- `append_event()` is callable from any module that has access to `&state.db`
- Phase 302-02 needs to add: GET /api/v1/events route handler with EventsQuery filters, registered in staff_routes()

---

*Phase: 302-structured-event-archive*
*Completed: 2026-04-01*
