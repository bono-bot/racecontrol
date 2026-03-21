---
phase: 100-staff-visibility-kiosk-badge-fleet-health-manual-clear
plan: 01
subsystem: api
tags: [fleet-health, maintenance, websocket, preflight, rust, axum]

# Dependency graph
requires:
  - phase: 99-system-network-billing-checks-handler-wiring
    provides: PreFlightFailed/PreFlightPassed AgentMessage variants and ClearMaintenance CoreToAgentMessage in rc-common protocol
provides:
  - FleetHealthStore with in_maintenance and maintenance_failures fields
  - PodFleetStatus serializes in_maintenance and maintenance_failures to fleet health JSON API
  - PreFlightFailed WS handler sets in_maintenance=true with failure check names on FleetHealthStore
  - PreFlightPassed WS handler clears in_maintenance=false on FleetHealthStore
  - POST /api/v1/pods/{id}/clear-maintenance endpoint sends ClearMaintenance WS message and optimistically clears server state
affects: [staff-visibility, kiosk-badge, dashboard, fleet-health-api]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Optimistic server-side state update after sending WS command to agent"
    - "FleetHealthStore used as single source of truth for per-pod maintenance state"
    - "clear_on_disconnect() clears maintenance state — offline != in-maintenance from server perspective"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/fleet_health.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "clear_on_disconnect() clears in_maintenance=false because offline pods are not in maintenance from the server's perspective — maintenance is a live WS-connected state"
  - "clear_maintenance_pod() does optimistic server-side clear immediately rather than waiting for PreFlightPassed confirmation from agent, to give staff instant visual feedback"
  - "log_pod_activity called via crate::activity_log:: path (same pattern as other routes) since the function is not re-exported via use statement in routes.rs"

patterns-established:
  - "Fleet health fields pattern: add to FleetHealthStore (Default), add to PodFleetStatus (Serialize), populate in fleet_health_handler None and Some branches"

requirements-completed: [STAFF-02, STAFF-03]

# Metrics
duration: 18min
completed: 2026-03-21
---

# Phase 100 Plan 01: Fleet Health Maintenance State + Clear Endpoint Summary

**Server-side maintenance tracking via PreFlightFailed/Passed WS events with in_maintenance field on fleet health JSON and POST /pods/{id}/clear-maintenance endpoint sending ClearMaintenance to pod agent**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-21T06:10:00Z
- **Completed:** 2026-03-21T06:28:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- FleetHealthStore and PodFleetStatus gain in_maintenance + maintenance_failures fields; fleet health API JSON now includes them per pod
- PreFlightFailed WS handler sets in_maintenance=true with failure check names; PreFlightPassed clears it; clear_on_disconnect clears it (offline != in maintenance)
- POST /api/v1/pods/{id}/clear-maintenance sends ClearMaintenance via WS and optimistically clears server state; registered in staff_routes()
- 2 new unit tests added; all 16 fleet_health tests pass; cargo build --bin racecontrol 0 errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Add maintenance state to FleetHealthStore and PodFleetStatus** - `65f6c4e` (feat)
2. **Task 2: Wire PreFlightFailed/Passed handlers + add clear-maintenance endpoint** - `f9688ec` (feat)

**Plan metadata:** (docs commit to follow)

## Files Created/Modified

- `crates/racecontrol/src/fleet_health.rs` - Added in_maintenance + maintenance_failures to FleetHealthStore and PodFleetStatus; clear_on_disconnect clears them; fleet_health_handler populates them; 2 new unit tests
- `crates/racecontrol/src/ws/mod.rs` - PreFlightFailed sets in_maintenance=true + maintenance_failures; PreFlightPassed clears in_maintenance=false
- `crates/racecontrol/src/api/routes.rs` - Added clear_maintenance_pod() handler and /pods/{id}/clear-maintenance route registration in staff_routes()

## Decisions Made

- clear_on_disconnect() clears in_maintenance because offline pods are not "in maintenance" from the server's perspective — maintenance is a live connected-agent state
- Optimistic server-side clear on clear_maintenance_pod() for instant staff visual feedback without waiting for PreFlightPassed roundtrip
- log_pod_activity accessed as crate::activity_log::log_pod_activity (not re-exported via use in routes.rs scope)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The cargo package name is `racecontrol-crate` (not `racecontrol`) so tests were run with `-p racecontrol-crate`. Build and tests passed first attempt.

## User Setup Required

None - no external service configuration required. The new endpoint is accessible via existing staff JWT auth.

## Next Phase Readiness

- Fleet health API now exposes in_maintenance and maintenance_failures per pod — ready for STAFF-03 kiosk badge display
- POST /pods/{id}/clear-maintenance ready for STAFF-02 staff dashboard button integration
- Both STAFF-02 and STAFF-03 server-side requirements complete

---
*Phase: 100-staff-visibility-kiosk-badge-fleet-health-manual-clear*
*Completed: 2026-03-21*
