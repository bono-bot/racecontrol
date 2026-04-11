---
phase: 366-fleet-intelligence
plan: 01
subsystem: api
tags: [rust, axum, sqlite, health-scoring, metrics, fleet]

# Dependency graph
requires:
  - phase: 363-data-recording
    provides: billing_sessions table with suspect, telemetry_coverage_pct, status columns
provides:
  - fleet_intelligence.rs module with compute_pod_health_score, compute_time_patterns, fleet_intelligence_handler
  - GET /api/v1/fleet/intelligence endpoint returning composite 0-100 health scores per pod
  - METRIC_POD_HEALTH_SCORE upgraded from binary 0/1 to composite 0-100 in TSDB
affects: [366-02, 366-03, 366-04, metrics_producers, fleet_health]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composite health scoring: 40/30/20/10 weights for session_success_rate/telemetry/config_mismatch/crash_penalty"
    - "Insufficient data guard: score=null when <3 completed sessions in 7-day window"
    - "Time-of-day failure analysis: SQL GROUP BY strftime('%H') with HAVING failure_rate >= 0.30 AND sample_count >= 3"

key-files:
  created:
    - crates/racecontrol/src/fleet_intelligence.rs
  modified:
    - crates/racecontrol/src/metrics_producers.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "null score (not 0) when fewer than 3 sessions in 7-day window to avoid misleading metrics"
  - "config_mismatch_rate defaults to 0.0 (Phase 362 data not yet wired) with 20pt weight preserved for future"
  - "METRIC_POD_HEALTH_SCORE uses score=0.0 for insufficient_data pods in TSDB (not null) for time-series continuity"

patterns-established:
  - "Fleet intelligence module pattern: compute_X function + handler function + unit tests in same file"
  - "Composite health score formula: (success_rate*40) + (avg_coverage/100*30) + (clean_config*20) + ((1-crash_penalty)*10)"

requirements-completed: [GLD-F-01, GLD-F-02]

# Metrics
duration: 30min
completed: 2026-04-11
---

# Phase 366 Plan 01: Fleet Intelligence Summary

**Composite 0-100 per-pod health score from billing_sessions with time-of-day failure pattern analysis, replacing binary TSDB metric**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-04-11T01:59:00Z
- **Completed:** 2026-04-11T02:01:00Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- Created `fleet_intelligence.rs` module with `compute_pod_health_score` (7-day window, 40/30/20/10 composite formula), `compute_time_patterns` (30-day window, 30% failure threshold), and `fleet_intelligence_handler`
- Wired `GET /api/v1/fleet/intelligence` into `staff_routes()` in routes.rs and declared `pub mod fleet_intelligence` in lib.rs
- Upgraded `METRIC_POD_HEALTH_SCORE` in `metrics_producers.rs` from binary 0.0/1.0 to composite 0-100 using `compute_pod_health_score`
- 4 unit tests: `insufficient_data` when <3 sessions, score=100 for clean pod, score=80 for 50% suspect rate, time_patterns flagged above 30% threshold

## Task Commits

1. **Task 1: Create fleet_intelligence.rs module** - `c1b647e5` (feat)
2. **Task 2: Wire module into lib.rs and routes.rs** - `c1b647e5` (feat)
3. **Task 3: Upgrade METRIC_POD_HEALTH_SCORE** - `c1b647e5` (feat)

## Files Created/Modified
- `crates/racecontrol/src/fleet_intelligence.rs` — New module: composite health scoring, time-of-day analysis, HTTP handler
- `crates/racecontrol/src/metrics_producers.rs` — Pod health score block upgraded from binary to composite compute_pod_health_score
- `crates/racecontrol/src/lib.rs` — Added `pub mod fleet_intelligence;`
- `crates/racecontrol/src/api/routes.rs` — Added `/fleet/intelligence` route in staff_routes()

## Decisions Made
- Score returns `null` (not `0`) when `sessions_in_window < 3` — avoids misleading "healthy" or "unhealthy" signals for new pods with no data
- `config_mismatch_rate` defaults to `0.0` (contributing full 20pts) since Phase 362 live data not yet wired — weight preserved for future phases
- TSDB emission uses `0.0` for insufficient_data pods (not null) for time-series graph continuity

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `fleet_intelligence.rs` module ready for import by 366-02 content drift and 366-04 integration gate
- `/fleet/intelligence` endpoint live in staff_routes, requires staff JWT
- METRIC_POD_HEALTH_SCORE in TSDB now carries composite signal for monitoring dashboards

---
*Phase: 366-fleet-intelligence*
*Completed: 2026-04-11*
