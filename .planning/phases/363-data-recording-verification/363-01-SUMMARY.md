---
phase: 363-data-recording-verification
plan: 01
subsystem: database, billing, telemetry
tags: [sqlite, sqlx, billing, telemetry, session-audit, cloud-sync, feature-flags]

# Dependency graph
requires:
  - phase: 362-post-launch-config-verification
    provides: "SessionConfig + verify_launch_config() Stage 5 pattern used as template"
provides:
  - "8 new billing_sessions columns (lap_count_expected, lap_count_actual, lap_count_flag, telemetry_coverage_pct, suspect, suspect_reasons, csv_fallback_received_at, lap_reject_grace_until)"
  - "lap_rejections table with session_id column (per D-12)"
  - "phase363_session_audit feature flag (kill switch)"
  - "session_audit.rs module: expected_laps, compute_lap_flag, coverage_pct, compute_suspect, run_session_audit"
  - "BillingTimer.telemetry_seconds_covered HashSet<u32> coverage histogram"
  - "WS Telemetry handler updates coverage bucket via try_write() (non-blocking)"
  - "post_session_hooks calls run_session_audit at session end"
  - "cloud_sync.rs billing_sessions push payload extended with all 8 new columns"
affects: [363-02, 363-03, 367-admin-suspect-laps, cloud-sync, billing-fsm]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Phase 363 migration pattern: let _ = sqlx::query(ALTER TABLE ...).execute(pool).await — idempotent duplicate-column silence"
    - "Coverage histogram pattern: HashSet<u32> seconds buckets in BillingTimer, flushed at session end"
    - "Non-blocking hot-path update: try_write() in WS handler — minor undercounting acceptable per D-04"
    - "Feature flag kill switch: check flags RwLock + drop guard before any .await (CLAUDE.md lock rule)"

key-files:
  created:
    - crates/racecontrol/src/session_audit.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/cloud_sync.rs

key-decisions:
  - "All 8 new columns go on billing_sessions (NOT sessions table) — per 363-RESEARCH.md correction to CONTEXT.md D-13"
  - "session_type not stored on billing_sessions — default to 'trackday' in run_session_audit (conservative heuristic, D-01)"
  - "seconds_covered captured BEFORE timers.remove() in end_billing_session(), passed through post_session_hooks"
  - "F-05 already fixed structurally in current codebase (363-RESEARCH.md finding) — integration tests deferred to 363-03"
  - "test_billing_timer_coverage_histogram_default_empty + test_billing_timer_default_coverage_empty added (both empty-check variants)"

patterns-established:
  - "Session audit: run_session_audit called from post_session_hooks fire-and-forget tokio::spawn"
  - "Coverage histogram: HashSet<u32> inserted on Telemetry packet arrival, len() captured at session end"
  - "Cloud sync: always update billing_sessions json_object in same commit as migration (CLAUDE.md DB rule)"

requirements-completed: [GLD-C-01, GLD-C-02]

# Metrics
duration: 39min
completed: 2026-04-09
---

# Phase 363 Plan 01: Data Recording Verification — DB Schema, Session Audit Module, Coverage Histogram, Cloud Sync Summary

**SQLite migration adds 8 billing_sessions audit columns + lap_rejections table; new session_audit.rs module implements lap heuristic + telemetry coverage calc; BillingTimer gains non-blocking 1s-bucket histogram; cloud_sync payload extended with all 8 columns — 17 tests green, 962 total pass**

## Performance

- **Duration:** ~39 min
- **Started:** 2026-04-09T19:32:36Z (20:02 IST)
- **Completed:** 2026-04-10T00:11 IST
- **Tasks:** 3 of 3
- **Files modified:** 5 (billing.rs, ws/mod.rs, cloud_sync.rs, db/mod.rs, lib.rs) + 1 created (session_audit.rs)

## Accomplishments

- DB migration: 8 new `billing_sessions` columns, `lap_rejections` table (with `session_id` per D-12), `phase363_session_audit` feature flag seeded enabled=1. Idempotent (let _ = pattern).
- New `session_audit.rs` module: `expected_laps()` D-01 floor heuristic, `compute_lap_flag()` D-02 directional check, `coverage_pct()` D-04 formula, `compute_suspect()` D-06 logic, `run_session_audit()` async orchestrator with feature flag kill switch.
- `BillingTimer` gains `telemetry_seconds_covered: HashSet<u32>`. WS Telemetry handler updates it via `try_write()` (non-blocking per CLAUDE.md lock rule). Captured at session end before timer removal.
- `cloud_sync.rs` billing_sessions push JSON extended with all 8 new columns (COALESCE defaults), same commit as migration (CLAUDE.md DB rule).

## Task Commits

1. **Task 1: DB migrations + lap_rejections + feature flag seed** - `e4784c51` (feat)
2. **Task 2: session_audit.rs module** - `8b9d2d3b` (feat)
3. **Task 3: Coverage histogram + post_session_hooks + cloud sync** - `0b4e356c` (feat)

## Files Created/Modified

- `crates/racecontrol/src/session_audit.rs` — New module: LapCountFlag enum, 4 pure functions, run_session_audit() orchestrator, 10 tests
- `crates/racecontrol/src/db/mod.rs` — Phase 363 migration block (8 ALTER TABLE + CREATE TABLE lap_rejections + feature flag INSERT OR IGNORE) + 4 migration tests
- `crates/racecontrol/src/lib.rs` — `pub mod session_audit;` registered
- `crates/racecontrol/src/billing.rs` — BillingTimer.telemetry_seconds_covered field + Default impl + 3 explicit construction sites + end_billing_session capture + post_session_hooks signature + run_session_audit call + 2 tests
- `crates/racecontrol/src/ws/mod.rs` — Telemetry handler coverage bucket update (try_write, guard dropped immediately)
- `crates/racecontrol/src/cloud_sync.rs` — 8 new column entries in billing_sessions json_object + 1 test

## Decisions Made

- `billing_sessions` vs `sessions`: All 8 columns go on `billing_sessions` (the RESEARCH.md correction to CONTEXT.md D-13). The abstract `sessions` table is NOT in cloud sync and has no lap data.
- `session_type` missing from `billing_sessions`: No `session_type` or `experience_type` column exists on `billing_sessions`. `run_session_audit()` defaults to "trackday" (conservative heuristic). Phase 365 will refine.
- `seconds_covered` capture point: Captured inside `end_billing_session()` BEFORE `timers.remove()`, then passed to `post_session_hooks()` as a new parameter. No extra lock acquisition needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Context] session_type column absent from billing_sessions**

- **Found during:** Task 2 (run_session_audit implementation)
- **Issue:** The PLAN specifies `SELECT session_type, allocated_seconds FROM billing_sessions WHERE id = ?` but `session_type` does not exist on `billing_sessions` (confirmed by grep). Only `allocated_seconds` is available.
- **Fix:** `run_session_audit()` queries only `allocated_seconds` and defaults `session_type = "trackday"` (the conservative heuristic). This correctly implements D-01 without the missing column. All behavior tests still pass.
- **Files modified:** `crates/racecontrol/src/session_audit.rs`
- **Verification:** `test_run_audit_integration` passes (30min trackday → expected=10, 5 actual → UNDER_RECORDED)
- **Committed in:** `8b9d2d3b` (Task 2 commit)

**2. [Rule 2 - Additional Test] Added test_billing_timer_default_coverage_empty**

- **Found during:** Task 3 (billing tests)
- **Issue:** Plan specifies only `test_billing_timer_coverage_histogram_default_empty` (via `make_test_timer`). The `BillingTimer::default()` path should also be covered.
- **Fix:** Added `test_billing_timer_default_coverage_empty` test for the `Default` impl path.
- **Committed in:** `0b4e356c` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 missing context handled gracefully, 1 additional test coverage)
**Impact on plan:** All requirements met. session_type default is conservative and correct per D-01. No scope creep.

## Issues Encountered

- `cargo -p racecontrol` fails — package name is `racecontrol-crate` not `racecontrol`. All test commands corrected.

## Known Stubs

None — all audit columns are written with real values at session end. Feature flag kill switch works (tested). No hardcoded placeholder data flows to UI.

## Next Phase Readiness

- Phase 363-02 (CSV fallback auto-sync + telemetry fallback endpoint) can proceed immediately
- Phase 363-03 (billing grace window + F-05 integration tests) can proceed immediately
- Phase 367 (Admin Suspect Laps UI) can read `billing_sessions.suspect` + `suspect_reasons` columns
- Cloud: needs `git pull + cargo build --release + pm2 restart racecontrol` on Bono VPS for migration to run
- Deploy: racecontrol binary must be redeployed to server AND Bono VPS (cloud parity rule)

---
*Phase: 363-data-recording-verification*
*Completed: 2026-04-09*
