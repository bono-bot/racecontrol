---
phase: 285-metrics-ring-buffer
plan: 01
subsystem: database
tags: [sqlite, tsdb, metrics, time-series, rollups]

requires: []
provides:
  - metrics_samples and metrics_rollups SQLite tables
  - metrics_tsdb.rs module with record/rollup functions
  - 7 metric name constants
affects: [285-02, dashboard, alerting]

tech-stack:
  added: []
  patterns: [INSERT OR IGNORE idempotent rollups, UNIQUE constraint dedup]

key-files:
  created: [crates/racecontrol/src/metrics_tsdb.rs]
  modified: [crates/racecontrol/src/db/mod.rs, crates/racecontrol/src/lib.rs]

key-decisions:
  - "Hourly rollups aggregate previous full hour, daily rollups aggregate previous full day"
  - "INSERT OR IGNORE with UNIQUE constraint for idempotent rollup computation"

patterns-established:
  - "TSDB pattern: raw samples -> hourly rollups -> daily rollups with chrono time windowing"

requirements-completed: [TSDB-01, TSDB-03, TSDB-04, TSDB-05]

duration: 2min
completed: 2026-04-01
---

# Phase 285 Plan 01: TSDB Schema and Core Module Summary

**SQLite TSDB foundation with metrics_samples/metrics_rollups tables, record_sample insert, and hourly/daily rollup aggregation functions using idempotent INSERT OR IGNORE**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-01T10:17:28Z
- **Completed:** 2026-04-01T10:19:30Z
- **Tasks:** 1/1
- **Files modified:** 3

## Accomplishments

### Task 1: TSDB tables and metrics_tsdb.rs module
- Added `metrics_samples` table with metric_name, pod_id, value, recorded_at columns plus lookup index
- Added `metrics_rollups` table with resolution, metric_name, pod_id, min/max/avg/count, period_start plus UNIQUE constraint and lookup index
- Created `metrics_tsdb.rs` with MetricSample/MetricRollup structs, record_sample(), record_samples_batch(), compute_hourly_rollups(), compute_daily_rollups()
- Defined 7 metric name constants: cpu_usage, gpu_temp, fps, billing_revenue, ws_connections, pod_health_score, game_session_count
- Registered module in lib.rs
- **Commit:** `7b9ab9b0`

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED
