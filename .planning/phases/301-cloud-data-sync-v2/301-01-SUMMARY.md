---
phase: 301-cloud-data-sync-v2
plan: 01
subsystem: database
tags: [sqlite, cloud-sync, rust, sqlx, lww, conflict-resolution]

# Dependency graph
requires:
  - phase: 300-sqlite-backup-pipeline
    provides: backup infrastructure and DB stability required before extending sync
provides:
  - model_evaluations table (CREATE TABLE migration)
  - metrics_rollups updated_at + venue_id columns (ALTER TABLE migration)
  - sync_state conflict_count column (ALTER TABLE migration)
  - fleet_solutions push/receive/pull in cloud sync pipeline
  - model_evaluations push/receive/pull in cloud sync pipeline
  - metrics_rollups push/receive/pull in cloud sync pipeline
  - LWW conflict resolution with venue_id tiebreaker for all 3 tables
  - conflict_count tracking in sync_state + sync_health endpoint
  - SCHEMA_VERSION bumped to 4
affects:
  - 301-02 (admin dashboard sync panel — depends on conflict_count in sync_health)
  - cloud sync between venue (.23) and Bono VPS
  - fleet_kb.rs (fleet_solutions now synced to cloud)
  - multi-model audit results now persisted and synced cross-venue

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "LWW upsert: ON CONFLICT DO UPDATE WHERE excluded.updated_at > table.updated_at OR (equal AND venue_id tiebreaker)"
    - "normalize_timestamp() made pub(crate) for reuse in routes.rs"
    - "metrics_rollups conflict key: UNIQUE(resolution, metric_name, pod_id, period_start), NOT id (AUTOINCREMENT)"
    - "conflict_count: increment sync_state row when LWW rejects an incoming write"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/cloud_sync.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "normalize_timestamp made pub(crate) rather than inlining — single source of truth for timestamp normalization"
  - "metrics_rollups conflict resolution uses max-sample-count wins for avg_value (more data = more authoritative)"
  - "metrics_rollups LWW WHERE clause includes OR metrics_rollups.updated_at IS NULL to handle existing rows without updated_at"
  - "SCHEMA_VERSION bumped 3->4 to signal new table keys in push payload"

patterns-established:
  - "Phase 301 LWW pattern: ON CONFLICT DO UPDATE SET ... WHERE excluded.updated_at > table.updated_at OR (equal AND smaller venue_id)"
  - "Conflict counting: increment sync_state.conflict_count when rows_affected == 0 after LWW upsert attempt"
  - "sync_changes arm pattern: SELECT json_object(all cols except AUTOINCREMENT id) WHERE updated_at > ? LIMIT ?"

requirements-completed: [SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05]

# Metrics
duration: 30min
completed: 2026-04-01
---

# Phase 301 Plan 01: Cloud Data Sync v2 — Intelligence Tables Summary

**Bidirectional cloud sync extended to fleet_solutions, model_evaluations, and metrics_rollups with SQLite LWW conflict resolution (updated_at + venue_id tiebreaker), SCHEMA_VERSION=4, and conflict_count tracking in sync_health**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-04-01T16:00:00Z
- **Completed:** 2026-04-01T16:30:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- DB migrations: model_evaluations CREATE TABLE, metrics_rollups + 2 columns (updated_at, venue_id), sync_state + conflict_count column — all idempotent via `let _ =` pattern
- Cloud sync push path: collect_push_payload extended with fleet_solutions, model_evaluations, metrics_rollups push blocks (SYNC-01/02/03)
- Cloud sync receive path: sync_push blocks with LWW + venue_id tiebreaker for all 3 tables (SYNC-04/05)
- Cloud sync pull path: sync_changes dispatch arms for all 3 tables (SYNC-04)
- sync_health: conflict_count per table now exposed in response (SYNC-05)
- SYNC_TABLES constant updated with 3 new table names
- normalize_timestamp made pub(crate) to eliminate code duplication in routes.rs

## Task Commits

1. **Task 1: DB migrations (model_evaluations table, metrics_rollups columns, sync_state conflict_count)** - `fccf3ba3` (feat)
2. **Task 2: Extend cloud_sync.rs push + routes.rs receive/pull for 3 tables with LWW conflict resolution** - `7c743151` (feat)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` - model_evaluations CREATE TABLE, ALTER TABLE metrics_rollups (updated_at + venue_id), ALTER TABLE sync_state (conflict_count)
- `crates/racecontrol/src/cloud_sync.rs` - SCHEMA_VERSION 4, normalize_timestamp pub(crate), SYNC_TABLES updated, 3 push blocks in collect_push_payload
- `crates/racecontrol/src/api/routes.rs` - 3 sync_changes arms, 3 sync_push blocks with LWW, sync_health with conflict_count (5-tuple query)

## Decisions Made

- normalize_timestamp made `pub(crate)` rather than inlined — single source of truth, avoids drift
- metrics_rollups uses max-sample-count wins for avg_value: more data points = more authoritative than simple LWW
- metrics_rollups LWW WHERE clause includes `OR metrics_rollups.updated_at IS NULL` to handle rows created before the migration (no updated_at value yet)
- SCHEMA_VERSION bumped from 3 to 4 to signal new table keys in push payload to receiving side

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Pre-existing test failure in `crates/racecontrol/tests/integration.rs` (BillingTimer missing `nonce` field — unrelated to this plan). Confirmed pre-existing by stashing our changes and running tests on the base commit. Our changes did not introduce any new test failures; `cargo check` passes cleanly.

## Known Stubs

None — model_evaluations table is created with correct schema. The table will be empty until AI diagnosis code writes to it (v35.0/Phase 290). Empty table sync is a valid no-op. No frontend or business logic depends on populated data from this plan.

## User Setup Required

None — DB migrations run automatically on server start. Both venue (.23) and Bono VPS require the new binary deployed to apply the migrations.

## Next Phase Readiness

- Plan 301-02 (admin dashboard sync panel) can now read conflict_count from `GET /sync/health`
- Bono VPS deployment needed to apply migrations on cloud side before inbound sync pushes succeed
- fleet_solutions, model_evaluations, metrics_rollups will sync bidirectionally on the next sync cycle after deploy

---
*Phase: 301-cloud-data-sync-v2*
*Completed: 2026-04-01*
