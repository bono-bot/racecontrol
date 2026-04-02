---
phase: 286-metrics-query-api
plan: 01
subsystem: api
tags: [metrics, tsdb, query-api, rest, sqlite]
dependency_graph:
  requires: [285-01]
  provides: [metrics/query endpoint, metrics/names endpoint, metrics/snapshot endpoint]
  affects: [Phase 287 dashboard, any consumer of time-series metric data]
tech_stack:
  added: []
  patterns: [dynamic sqlx::query (no compile-time DB), inner functions for testability, select_resolution auto-tier]
key_files:
  created:
    - crates/racecontrol/src/api/metrics_query.rs
  modified:
    - crates/racecontrol/src/api/mod.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - Used dynamic sqlx::query instead of sqlx::query! macros — metrics_samples/metrics_rollups tables do not exist in dev DB, and existing codebase pattern (metrics.rs) also uses query_as
  - Extracted inner query functions (query_time_series, query_metric_names, query_snapshot) to enable unit testing without AppState
  - Tests use #[tokio::test] with in-memory SQLite (sqlite::memory:) matching existing billing.rs/metrics.rs pattern
metrics:
  duration: ~30min
  completed_date: "2026-04-01T11:00:00Z"
  tasks_completed: 2
  files_modified: 3
---

# Phase 286 Plan 01: Metrics Query API Summary

Three REST endpoints giving operators and Phase 287 dashboard programmatic access to TSDB data: time-series query with auto-resolution, metric name listing, and current snapshot.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create metrics_query.rs with three handlers and inline tests | f86ee5d2 | metrics_query.rs, api/mod.rs |
| 2 | Register routes in staff_routes and verify full build | fffd6ac2 | routes.rs |

## What Was Built

**Three handlers in `crates/racecontrol/src/api/metrics_query.rs`:**

- `query_handler` — GET /api/v1/metrics/query?metric=X&from=T1&to=T2[&pod=N][&resolution=raw|hourly|daily]
  - Validates from < to (400 on invalid range)
  - Auto-resolution: <24h=raw (metrics_samples), 24h-7d=hourly (metrics_rollups), >7d=daily (metrics_rollups)
  - Optional `?pod=N` filter converts to `pod-N` for DB binding
  - Uses avg_value from rollups as the point value

- `names_handler` — GET /api/v1/metrics/names
  - UNION of DISTINCT metric_name from both tables, sorted

- `snapshot_handler` — GET /api/v1/metrics/snapshot[?pod=N]
  - Self-join to get MAX(recorded_at) per metric+pod group
  - COALESCE(pod_id, '') in join condition handles NULL pod_id correctly
  - CAST(strftime('%s', recorded_at) AS INTEGER) for correct unix epoch ts

**Route registration in `staff_routes()` (staff JWT required — business intelligence data):**
```
GET /api/v1/metrics/query
GET /api/v1/metrics/names
GET /api/v1/metrics/snapshot
```

## Test Results

- 8 unit tests: all pass
- Full cargo test suite: 725 passed, 0 failed (8 new from this plan)
- cargo build --release --bin racecontrol: success
- No duplicate routes (uniq -d check empty)
- No .unwrap() in production code

## Deviations from Plan

**1. [Rule 1 - Pattern Match] Used dynamic sqlx::query instead of sqlx::query! macros**
- Found during: Task 1 implementation
- Issue: sqlx::query! macros require compile-time DB verification. metrics_samples/metrics_rollups tables do not exist in dev DB. Test module compiled but 0 tests registered because the macros silently failed.
- Fix: Switched to sqlx::query (dynamic) with .try_get() row extraction, matching the existing pattern in api/metrics.rs (which also uses query_as not query!)
- Files modified: crates/racecontrol/src/api/metrics_query.rs

**2. [Rule 1 - Architecture] Extracted inner functions for test isolation**
- Found during: Task 1 — AppState is massive (50+ fields), cannot be instantiated in tests without full Config + FieldCipher
- Fix: Extracted query_time_series(), query_metric_names(), query_snapshot() as pub functions taking &SqlitePool. Handlers call these inner functions. Tests call inner functions directly with in-memory pool.
- This matches the existing metrics.rs pattern (query_alternatives etc.)

## Known Stubs

None — all three endpoints query live DB tables. Phase 287 dashboard will consume these endpoints.

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/api/metrics_query.rs
- FOUND: commit f86ee5d2
- FOUND: commit fffd6ac2
- 8 tests pass, full suite 725/0 pass/fail
- No .unwrap() in production code
- 3 routes in staff_routes, no duplicates
