---
phase: 303-multi-venue-schema-prep
plan: "01"
subsystem: database-schema
tags: [venue-id, schema-migration, sqlite, config]
dependency_graph:
  requires: []
  provides: [venue_id-column-all-major-tables, VenueConfig.venue_id, MULTI-VENUE-ARCHITECTURE.md]
  affects: [303-02-INSERT-threading, cloud-sync-cross-venue-queries]
tech_stack:
  added: []
  patterns: [idempotent-ALTER-let-underscore, serde-default-function, temp-file-test-pool]
key_files:
  created:
    - docs/MULTI-VENUE-ARCHITECTURE.md
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/db/mod.rs
decisions:
  - venue_id serde default 'racingpoint-hyd-001' — no TOML change required for existing deployments
  - let _ = ALTER pattern (ignore duplicate-column error) — idempotent on existing production DBs
  - Tests use temp file-based SQLite (WAL mode required by init_pool, not supported by :memory:)
  - 44 tables in single for-loop migration block (not one ALTER per table)
metrics:
  duration: 25min
  completed_date: "2026-04-02"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 2
---

# Phase 303 Plan 01: VenueConfig venue_id + 44-Table ALTER Migrations Summary

**One-liner:** Added `venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'` to all 44 major operational tables via idempotent ALTER migrations and `VenueConfig.venue_id` field with serde default.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| RED | Failing venue_id migration tests | c72c27c3 | db/mod.rs |
| 1 (GREEN) | VenueConfig venue_id + 44-table ALTER migrations | 16ebec9a | config.rs, db/mod.rs |
| 2 | MULTI-VENUE-ARCHITECTURE.md design document | 547cc67c | docs/MULTI-VENUE-ARCHITECTURE.md |

## What Was Built

### Task 1: VenueConfig venue_id field + ALTER migrations

**config.rs changes:**
- Added `pub venue_id: String` to `VenueConfig` with `#[serde(default = "default_venue_id")]`
- Added `fn default_venue_id() -> String { "racingpoint-hyd-001".to_string() }`
- Updated `default_config()` to include `venue_id: default_venue_id()`

**db/mod.rs changes:**
- Added 44-table `for table in &[...] { let _ = sqlx::query(...ALTER TABLE...).execute(pool).await; }` block at end of `migrate()`, before `Ok(())`
- All 44 tables listed (see plan for full list)
- Tables already having `venue_id` (model_evaluations, metrics_rollups, fleet_solutions) excluded

**Tests added (db/mod.rs):**
- `test_venue_id_migration_billing_sessions` — pragma_table_info check
- `test_venue_id_migration_laps` — pragma_table_info check
- `test_venue_id_migration_drivers` — pragma_table_info check
- `test_venue_id_migration_wallets` — pragma_table_info check
- `test_venue_id_migration_system_events` — pragma_table_info check
- `test_venue_id_migration_idempotent` — migrate() twice must not error
- `test_venue_config_default_venue_id` — serde default check
- All 7 tests pass. 3 pre-existing email_alerts venue tests also pass (10 total).

### Task 2: MULTI-VENUE-ARCHITECTURE.md

Created `docs/MULTI-VENUE-ARCHITECTURE.md` (134 lines) covering:
1. Current State — 44 tables + 3 pre-existing, zero behavioral impact
2. Trigger Conditions — business (second location) + technical (new TOML) + operational
3. Schema Strategy — sovereign DB per venue, cloud aggregation point
4. Sync Model — venue-push (billing), cloud-push (drivers), LWW (metrics)
5. Breaking Points — wallet scoping, leaderboard cross-venue, INSERT threading
6. Migration Checklist — day-of checklist for Venue 2 launch
7. Implementation History — phase-by-phase changes

## Verification Results

```
running 10 tests
test db::venue_id_tests::test_venue_config_default_venue_id ... ok
test db::venue_id_tests::test_venue_id_migration_billing_sessions ... ok
test db::venue_id_tests::test_venue_id_migration_drivers ... ok
test db::venue_id_tests::test_venue_id_migration_idempotent ... ok
test db::venue_id_tests::test_venue_id_migration_laps ... ok
test db::venue_id_tests::test_venue_id_migration_system_events ... ok
test db::venue_id_tests::test_venue_id_migration_wallets ... ok
test result: ok. 10 passed; 0 failed; 0 ignored
```

`cargo build --bin racecontrol` — Finished without errors.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed VenueConfig initializer missing venue_id**
- **Found during:** Task 1 GREEN phase
- **Issue:** `default_config()` in config.rs had a struct literal initializing `VenueConfig` — adding the `venue_id` field to the struct caused a compile error (missing field)
- **Fix:** Added `venue_id: default_venue_id()` to the `VenueConfig` literal in `default_config()`
- **Files modified:** crates/racecontrol/src/config.rs
- **Commit:** 16ebec9a (included in same commit)

**2. [Rule 1 - Bug] Fixed test pool: :memory: does not support WAL mode**
- **Found during:** Task 1 GREEN phase (first test run)
- **Issue:** `init_pool(":memory:")` fails with "CRITICAL: SQLite WAL mode failed to activate — got 'memory'" because in-memory SQLite cannot use WAL journal mode (WAL requires a real filesystem). The plan specified `:memory:` for tests.
- **Fix:** Replaced `init_pool(":memory:")` in tests with `test_pool()` helper that creates a temp file-based DB (using `std::env::temp_dir()` + unique name) and cleans up after each test
- **Files modified:** crates/racecontrol/src/db/mod.rs
- **Commit:** 16ebec9a (included in same commit)

## Known Stubs

None. Plan 01 scope is schema migration only. INSERT threading (explicitly adding `venue_id` to INSERT statements) is Plan 303-02 — this is documented and intentional, not a stub.

The `venue_id` column is present in all 44 tables with `DEFAULT 'racingpoint-hyd-001'`. Existing rows silently return the default. New rows will also get the default until Plan 303-02 threads `state.config.venue.venue_id` through INSERT statements.

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| docs/MULTI-VENUE-ARCHITECTURE.md exists | FOUND |
| crates/racecontrol/src/config.rs exists | FOUND |
| crates/racecontrol/src/db/mod.rs exists | FOUND |
| 303-01-SUMMARY.md exists | FOUND |
| commit c72c27c3 (RED tests) | FOUND |
| commit 16ebec9a (GREEN implementation) | FOUND |
| commit 547cc67c (design doc) | FOUND |

GATES TRIGGERED: [G0, G1, G4] | PROOFS: G0=plan block above, G1=10/10 tests + cargo build clean, G4=tests PASS + build PASS | SKIPPED: G2 (no fleet deploy), G3 (no new info shared during execution), G5 (no anomalous data), G6 (no context switch), G7 (no tool selection needed), G8 (VenueConfig is not a shared dependency causing downstream breakage), G9 (no multi-exchange debug session)
