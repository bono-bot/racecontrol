---
phase: 200-self-improving-intelligence
plan: 01
subsystem: metrics
tags: [intel, combo-reliability, launch-api, game-launcher, sqlite, tdd]
dependency_graph:
  requires: []
  provides: [combo_reliability_table, query_combo_reliability, update_combo_reliability, reliability_warning_api, max_auto_relaunch]
  affects: [record_launch_event, launch_game_handler, GameTracker]
tech_stack:
  added: [combo_reliability SQLite table, ComboReliability struct, FailureMode struct]
  patterns: [rolling 30-day window, NULL-safe IS NULL comparison, DELETE+INSERT upsert, Option<ComboReliability> minimum guard]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/metrics.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ws/mod.rs
decisions:
  - "No COALESCE in PRIMARY KEY (SQLite limitation) — used UNIQUE INDEX on COALESCE(car,'') + COALESCE(track,'') with DELETE+INSERT upsert pattern"
  - "FailureMode defined locally in metrics.rs to avoid circular import with api::metrics module"
  - "Used crate::metrics:: prefix in routes.rs to disambiguate from local super::metrics (api::metrics)"
  - "All test GameTracker construction sites updated with max_auto_relaunch: 2 default (no breaking change)"
metrics:
  duration_minutes: 36
  completed_date: "2026-03-26T09:58:00Z"
  tasks_completed: 2
  files_modified: 5
---

# Phase 200 Plan 01: Combo Reliability Foundation Summary

Combo reliability scoring foundation: materialized combo_reliability table updated after every launch event, reliability warning injection in the launch API response, and auto-tuning retry cap for low-reliability combos.

## What Was Built

### Task 1: combo_reliability table + query/update functions (TDD)

**RED phase:** 5 failing tests written first covering upsert, rate calculation, minimum threshold, rolling window, and NULL car/track handling.

**GREEN phase:**

- `combo_reliability` table added to `db/mod.rs` after the `recovery_events` block with a UNIQUE INDEX on `COALESCE(car,'')` + `COALESCE(track,'')` for NULL-safe composite key enforcement
- `ComboReliability` struct with all fields including `common_failure_modes: Vec<FailureMode>`
- `FailureMode` struct defined locally in `metrics.rs` (not imported from `api::metrics` to avoid circular deps)
- `update_combo_reliability()`: rolling 30-day window, NULL-safe IS NULL comparison pattern, avg/p95 time_to_track from sorted successful durations, top 3 failure modes from error_taxonomy, DELETE+INSERT upsert
- `query_combo_reliability()`: returns `None` for `total_launches < 5` (INTEL-02 minimum threshold), parses `common_failure_modes` JSON string to `Vec<FailureMode>`
- `update_combo_reliability()` wired into `record_launch_event()` after both SQLite insert and JSONL write (Pitfall 5 — crash recovery relaunches also update reliability)

All 5 tests pass.

### Task 2: Warning injection + auto-retry cap tuning

**routes.rs launch handler:**
- Parses car/track from already-injected `launch_args` JSON before `handle_dashboard_command` call
- Calls `crate::metrics::query_combo_reliability()` (uses full crate path to disambiguate from `super::metrics` which is `api::metrics`)
- Injects `"warning"` field in JSON response when `success_rate < 0.70` with format: `"This combination has a X% success rate on this pod (Y/Z launches)"`
- No warning returned when `>= 70%` success or `< 5 launches` (insufficient data)
- `handle_dashboard_command` return type unchanged (Pitfall 3 avoided)

**game_launcher.rs:**
- `max_auto_relaunch: u32` field added to `GameTracker` struct (after `exit_codes`)
- `query_combo_reliability()` called after `query_dynamic_timeout()` in `launch_game()`
- `max_relaunch_cap = 3` when `success_rate < 0.50 && total_launches >= 5`, otherwise `2`
- Hardcoded `< 2` and `>= 2` checks replaced with `tracker.max_auto_relaunch`
- All 17 `GameTracker` construction sites updated with `max_auto_relaunch: 2` (tests + ws/mod.rs)

## Tests

- 5 new combo_reliability tests all passing
- 549 pre-existing tests pass (pre-existing crypto + config failures confirmed unrelated — both fail before any of our changes)
- `cargo build --release --bin racecontrol` compiles cleanly

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] SQLite does not support COALESCE in PRIMARY KEY definitions**
- **Found during:** Task 1 GREEN phase (tests failed silently — no rows inserted)
- **Issue:** Plan specified `PRIMARY KEY (pod_id, sim_type, COALESCE(car, ''), COALESCE(track, ''))` but SQLite only supports column names in PRIMARY KEY, not expressions
- **Fix:** Changed to no explicit PRIMARY KEY + `CREATE UNIQUE INDEX idx_combo_rel_pk ON combo_reliability(pod_id, sim_type, COALESCE(car, ''), COALESCE(track, ''))`. SQLite supports expressions in unique indexes. Used DELETE+INSERT upsert pattern instead of `INSERT OR REPLACE` (which requires a PRIMARY KEY for conflict detection).
- **Files modified:** `crates/racecontrol/src/db/mod.rs`, `crates/racecontrol/src/metrics.rs`
- **Commit:** ce4550db

**2. [Rule 1 - Bug] `metrics` name conflict in routes.rs**
- **Found during:** Task 2 — `use crate::metrics;` conflicts with existing `use super::metrics;` (which is `api::metrics`)
- **Fix:** Removed the `use crate::metrics;` import and used the fully-qualified `crate::metrics::query_combo_reliability` at the call site
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Commit:** 161c929a

**3. [Rule 2 - Missing] GameTracker construction sites needed max_auto_relaunch field**
- **Found during:** Task 2 — 17 construction sites in tests and ws/mod.rs missing the new field
- **Fix:** Bulk-replaced all GameTracker construction patterns using `replace_all` edit to add `max_auto_relaunch: 2`
- **Files modified:** `crates/racecontrol/src/game_launcher.rs`, `crates/racecontrol/src/ws/mod.rs`
- **Commit:** 161c929a

## Self-Check: PASSED

- FOUND: `crates/racecontrol/src/db/mod.rs` — combo_reliability CREATE TABLE on line 416
- FOUND: `crates/racecontrol/src/metrics.rs` — `pub async fn query_combo_reliability` on line 459
- FOUND: `crates/racecontrol/src/metrics.rs` — `pub async fn update_combo_reliability` on line 294
- FOUND: `crates/racecontrol/src/metrics.rs` — `update_combo_reliability` called inside `record_launch_event` on line 103
- FOUND: commit `ce4550db` — test(200-01): add failing combo_reliability tests (TDD RED)
- FOUND: commit `161c929a` — feat(200-01): warning injection in launch response + auto-retry cap tuning
- 5 combo_reliability tests pass, 549 other tests pass (pre-existing failures confirmed unrelated)
- `cargo build --release --bin racecontrol` — Finished `release` profile [optimized]
