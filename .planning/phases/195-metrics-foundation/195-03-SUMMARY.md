---
phase: 195-metrics-foundation
plan: "03"
subsystem: racecontrol-api
tags: [metrics, api, axum, sqlx, launch-stats, billing-accuracy]
requirements: [METRICS-05, METRICS-06]

dependency_graph:
  requires: [195-01, 195-02]
  provides: [launch_stats_handler, billing_accuracy_handler, /api/v1/metrics/launch-stats, /api/v1/metrics/billing-accuracy]
  affects: [admin-dashboard-phase-201, self-improving-intelligence-phase-200]

tech_stack:
  added: []
  patterns:
    - Dynamic WHERE clause with sqlx .bind() — safe parameterized queries without ORM
    - P95 computation via sorted-fetch + index (SQLite has no NTILE window function)
    - Trend detection: 15-day vs 15-day success rate comparison

key_files:
  created:
    - crates/racecontrol/src/api/metrics.rs
  modified:
    - crates/racecontrol/src/api/mod.rs
    - crates/racecontrol/src/api/routes.rs

key_decisions:
  - Routes placed in public_routes() (no auth) — consistent with fleet/health pattern; admin dashboard needs unauthenticated SSR access
  - P95 computed by fetching sorted durations and picking 95th percentile index — SQLite lacks NTILE, approach works for thousands of events
  - Trend uses 15/15-day split with 5% threshold to classify improving/stable/degrading

metrics:
  duration: 6 minutes
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 2
  completed_date: "2026-03-26"
---

# Phase 195 Plan 03: Metrics API Endpoints Summary

Axum REST handlers for launch statistics and billing accuracy, querying the SQLite tables from Plans 01 and 02, with filterable aggregates and a 30-day rolling window.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create metrics API handlers | d941ff68 | crates/racecontrol/src/api/metrics.rs (created), api/mod.rs |
| 2 | Register metrics routes | 6d17f271 | crates/racecontrol/src/api/routes.rs |

## What Was Built

### GET /api/v1/metrics/launch-stats

Accepts `pod`, `game`, `car`, `track` query params. Returns:
- `success_rate` — fraction of successful launches in last 30 days
- `avg_time_to_track_ms` — average duration_to_playable_ms (nullable)
- `p95_time_to_track_ms` — 95th percentile of durations (nullable, sorted-fetch approach)
- `total_launches` — count of all events in window
- `common_failure_modes` — top-5 error_taxonomy values with counts
- `last_30d_trend` — "improving" / "stable" / "degrading" based on 15-day rate comparison

Dynamic WHERE clause built from optional params, values bound via sqlx `.bind()` — no user input interpolated into SQL.

### GET /api/v1/metrics/billing-accuracy

No filter params. Returns:
- `avg_delta_ms` — average launch-to-billing delta (event_type='start', 30d)
- `max_delta_ms` — maximum delta in window
- `sessions_with_zero_delta` — sessions where billing started instantly (delta=0)
- `sessions_where_billing_never_started` — launch_command_at set but billing_start_at NULL
- `false_playable_signals` — discrepancy events tagged `false_playable` in details

Both endpoints placed in `public_routes()` — no auth required, consistent with existing operational endpoints pattern (fleet/health, debug/db-stats).

## Deviations from Plan

None — plan executed exactly as written. The only adjustment was that routes.rs is a 16000+ line file so the insertion point was confirmed precisely before editing.

## Verification

- `cargo check -p racecontrol-crate`: passes (3 pre-existing warnings unrelated to this plan)
- `cargo test -p racecontrol-crate`: 66 passed, 0 failed, 0 regressions
- `/metrics/launch-stats` and `/metrics/billing-accuracy` routes confirmed in public_routes()
- `grep -c "/metrics/" routes.rs` = 2
- `grep -c "pub async fn.*handler" metrics.rs` = 2

## Self-Check: PASSED

- crates/racecontrol/src/api/metrics.rs: FOUND
- crates/racecontrol/src/api/mod.rs (pub mod metrics): FOUND
- /metrics/launch-stats in routes.rs: FOUND at line 120
- /metrics/billing-accuracy in routes.rs: FOUND at line 121
- Commit d941ff68: FOUND
- Commit 6d17f271: FOUND
