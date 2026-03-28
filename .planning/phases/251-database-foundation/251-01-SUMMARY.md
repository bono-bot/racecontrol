---
phase: 251-database-foundation
plan: 01
subsystem: database
tags: [sqlite, wal, billing, timer-persistence, staggered-writes, crash-recovery]

# Dependency graph
requires: []
provides:
  - SQLite WAL mode fail-fast verification on server startup
  - elapsed_seconds + last_timer_sync_at columns on billing_sessions table
  - idx_billing_sessions_status_sync index for orphan detection
  - persist_timer_state() function with pod-staggered 60s writes
  - Timer state recovered from DB (COALESCE elapsed_seconds fallback) on server restart
  - Staggered timer-persist tokio task spawned at server startup
affects:
  - 251-02 (orphan detection uses last_timer_sync_at index)
  - 252 (financial atomicity builds on stable DB layer)
  - 253 (state machine hardening depends on accurate elapsed_seconds on recovery)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Snapshot under lock + drop before await: collect snapshot Vec inside { }, then iterate outside"
    - "Staggered pod writes: Pod N writes at second (N*7)%60 within minute to prevent 8-pod simultaneous SQLite writes"
    - "COALESCE(elapsed_seconds, driving_seconds) recovery: handles both old (no column) and new sessions"
    - "Fail-fast WAL verification: bail! if PRAGMA journal_mode != 'wal' at startup"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "WAL verification uses query_as fetch_one AFTER pragma set — fail-fast with bail! prevents server start on read-only filesystem"
  - "Two coexisting sync loops: 5s sync_timers_to_db (dashboard driving_seconds) + 60s persist_timer_state (crash recovery elapsed_seconds)"
  - "Stagger formula (N*7)%60 spreads 8 pods across 56 seconds with 7s gaps, no two pods write at same second"
  - "COALESCE fallback ensures old sessions (before migration) recover correctly using driving_seconds"

patterns-established:
  - "Lock snapshot pattern: { let guard = lock.read().await; guard.values().map(...).collect() } — guard dropped before DB writes"
  - "Pod number extraction: pod_id.trim_start_matches('pod_').parse::<u32>().unwrap_or(0)"

requirements-completed: [RESIL-01, RESIL-02, FSM-09]

# Metrics
duration: 15min
completed: 2026-03-28
---

# Phase 251 Plan 01: Database Foundation Summary

**SQLite WAL fail-fast verification + staggered 60s timer persistence with COALESCE crash recovery for 8-pod concurrent writes**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-28T18:50:00Z (IST: 2026-03-29T00:20 IST)
- **Completed:** 2026-03-28T19:05:06Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- WAL mode is now verified at startup — server will NOT start if WAL fails to activate (e.g., read-only filesystem)
- billing_sessions table gains elapsed_seconds + last_timer_sync_at columns via idempotent ALTER TABLE migrations
- idx_billing_sessions_status_sync index created for Phase 251-02 orphan detection queries
- persist_timer_state() function writes elapsed_seconds every 60s per pod, staggered by (pod_number * 7) % 60
- recover_active_sessions() now uses COALESCE(bs.elapsed_seconds, bs.driving_seconds) so elapsed_seconds survives server restarts
- 559 lib tests pass, 1 pre-existing integration test linker error unrelated to these changes

## Task Commits

Each task was committed atomically:

1. **Task 1: WAL verification + column migrations** - `08acee0c` (feat)
2. **Task 2: Staggered timer persistence + elapsed recovery** - `6babdd40` (feat)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` — WAL fail-fast verification query + elapsed_seconds/last_timer_sync_at ALTER TABLE migrations + status_sync index
- `crates/racecontrol/src/billing.rs` — persist_timer_state() function + COALESCE in recover_active_sessions() + elapsed_secs recovery variable
- `crates/racecontrol/src/main.rs` — timer-persist tokio::spawn with 1s tick and per-pod stagger check

## Decisions Made

- Two coexisting sync loops kept intentionally: the 5s `sync_timers_to_db` loop handles fast dashboard updates (driving_seconds), while the new 60s `persist_timer_state` handles crash recovery (elapsed_seconds). Removing the 5s loop would degrade live dashboard responsiveness.
- Stagger formula (N*7)%60 chosen: produces values 7, 14, 21, 28, 35, 42, 49, 56 — all distinct, spread across 56 seconds, no collision for 8 pods.
- COALESCE fallback (row.11.unwrap_or(row.6)) ensures backward compatibility for sessions created before migration.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo test -p racecontrol` failed with Windows LNK1104 linker error during stash experiment — the integration test binary was locked. Resolved by restoring stash and running `--lib` tests only. The 1 pre-existing crypto test failure (`load_keys_wrong_length`) confirmed to be unrelated to this plan's changes.

## User Setup Required

None - no external service configuration required. DB migrations are idempotent — columns silently skip if already exist on next server start.

## Next Phase Readiness

- Phase 251-02 (orphan detection) can now use `idx_billing_sessions_status_sync` and `last_timer_sync_at` column
- Phase 252 (financial atomicity) has a stable WAL-verified DB layer with 5000ms busy_timeout
- Phase 253 (state machine hardening) has accurate elapsed_seconds recovery from DB

## Self-Check: PASSED

- FOUND: .planning/phases/251-database-foundation/251-01-SUMMARY.md
- FOUND: commit 08acee0c (Task 1)
- FOUND: commit 6babdd40 (Task 2)

---
*Phase: 251-database-foundation*
*Completed: 2026-03-28*
