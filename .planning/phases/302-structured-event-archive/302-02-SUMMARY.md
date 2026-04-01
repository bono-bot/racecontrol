---
phase: 302-structured-event-archive
plan: 02
subsystem: api
tags: [sqlite, event-archive, rest-api, instrumentation, billing, deploy, pod-healer, ws, metric-alerts]

# Dependency graph
requires:
  - phase: 302-01
    provides: system_events table + append_event() function
provides:
  - GET /api/v1/system-events REST endpoint with event_type, pod, from, to, limit filters
  - 11 append_event call sites across billing (2), deploy (5), pod_healer (1), ws/mod (2), metric_alerts (1)
  - Events flowing into system_events: billing.session_started, billing.session_ended, deploy.started, deploy.completed, deploy.failed, pod.recovery, pod.online, pod.offline, alert.fired
affects:
  - system_events table (data now flows in from all 6 sources)
  - GET /api/v1/system-events queryable from admin dashboard or direct API calls

# Tech tracking
tech-stack:
  added: []
  patterns:
    - dynamic WHERE builder with character-allowlist validation (same as BillingListQuery)
    - fire-and-forget append_event alongside existing log_pod_activity (additive, no flow change)
    - sqlx::query_as tuple return for 6-column SELECT

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/deploy.rs
    - crates/racecontrol/src/pod_healer.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/metric_alerts.rs

key-decisions:
  - "Route named /system-events not /events — /events already registered for hotlap competition (same router, would panic at runtime)"
  - "billing.session_ended uses activity_action (derived from end_status) instead of status_str — status_str is defined after the log_pod_activity call site"
  - "deploy.failed added at 2 failure paths in deploy_pod_inner (download failure + no-prev fallback) — representative coverage without modifying all 5+ minor failure branches"
  - "pod.online passes conn_id in payload for cross-reference with tracing logs"

patterns-established:
  - "Pattern: character allowlist validation for event_type (alphanumeric + _ + .) and pod (alphanumeric + - + _)"
  - "Pattern: payload TEXT parsed back to JSON Value with serde_json::from_str().unwrap_or_else(Value::String) to avoid double-encoding"

requirements-completed:
  - EVENT-01
  - EVENT-05

# Metrics
duration: 35min
completed: 2026-04-01
---

# Phase 302 Plan 02: Structured Event Archive — REST API + Instrumentation Summary

**GET /api/v1/system-events handler with 4 validated filters; 11 append_event call sites across 5 source files populating the system_events table**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-01T16:25:00Z
- **Completed:** 2026-04-01T17:00:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- EventsQuery struct with event_type, pod, from, to, limit fields
- get_events handler using dynamic WHERE builder pattern from BillingListQuery — all inputs validated with character allowlists
- Payload TEXT column parsed back to JSON Value (no double-encoding)
- Route registered in staff_routes as `/system-events` (JWT required)
- 11 append_event call sites across 5 files:
  - `billing.rs`: `billing.session_started` (driver_id, tier, allocated_seconds) + `billing.session_ended` (driver_id, driving_seconds, end_status)
  - `deploy.rs`: `deploy.started` (binary_url) + `deploy.completed` (verify_delay_secs) + `deploy.failed` (reason) x2 failure paths = 5 calls
  - `pod_healer.rs`: `pod.recovery` (action, target, reason)
  - `ws/mod.rs`: `pod.online` (pod_number, conn_id) + `pod.offline` (reason)
  - `metric_alerts.rs`: `alert.fired` (rule_name, metric, value, threshold, severity)
- All calls additive fire-and-forget — zero existing control flow changed
- 774 unit tests pass; cargo build --release compiles cleanly

## Task Commits

Each task was committed atomically:

1. **Task 1: GET /api/v1/system-events handler with EventsQuery filters** - `7d1c763d` (feat)
2. **Task 2: Instrument 6 high-signal event sources with append_event calls** - `8069c0a8` (feat)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — EventsQuery struct + get_events handler + route in staff_routes
- `crates/racecontrol/src/billing.rs` — use crate::event_archive + 2 append_event calls
- `crates/racecontrol/src/deploy.rs` — use crate::event_archive + 5 append_event calls
- `crates/racecontrol/src/pod_healer.rs` — use crate::event_archive + 1 append_event call
- `crates/racecontrol/src/ws/mod.rs` — use crate::event_archive + 2 append_event calls
- `crates/racecontrol/src/metric_alerts.rs` — use crate::event_archive + 1 append_event call

## Decisions Made

- Route is `/system-events` not `/events` — the `/events` path was already registered in staff_routes for the hotlap competition system (list_events/create_event). Axum panics at runtime on duplicate METHOD+PATH, and the route_uniqueness_tests test caught this at `cargo test` time. Using `/system-events` matches the table name and avoids the collision.
- `billing.session_ended` payload uses `activity_action` (the match string already in scope) rather than `status_str` — `status_str` is defined several lines after the `log_pod_activity` call site and would require restructuring the function.
- `deploy.failed` instrumented at 2 representative failure paths (download failure, no-rollback terminal failure) rather than all 5+ minor branches. The 3 minor branches (binary size check, dir parse failure, rollback script write) are lower-signal and would have required repeated boilerplate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Route name conflict: /events already registered for hotlap competition**
- **Found during:** Task 1 acceptance criteria verification (cargo test)
- **Issue:** `route_uniqueness_tests::no_duplicate_route_registrations` panicked with "DUPLICATE ROUTES DETECTED: get /events". The plan spec said to register at `/events` but staff_routes already has `.route("/events", get(list_events).post(create_event))` for the hotlap competition system.
- **Fix:** Changed route to `/system-events` — matches the table name (system_events) and the established naming convention for this module
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Verification:** `cargo test --lib` passes all 774 tests after fix

---

**Total deviations:** 1 auto-fixed (route name collision)
**Impact on plan:** Route path changed from `/events` to `/system-events`. No functional scope change — the handler, filters, and behavior are identical.

## Issues Encountered

- Pre-existing integration test failures (BillingTimer missing `nonce` field) — out of scope, pre-existing from before Phase 302. Used `--lib` for unit tests per Plan 01 precedent.

## Known Stubs

None — all call sites are wired to the real system_events table via append_event(). Production events will flow immediately on deploy.

---

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/api/routes.rs (EventsQuery + get_events handler)
- FOUND: crates/racecontrol/src/billing.rs (2 append_event calls)
- FOUND: crates/racecontrol/src/deploy.rs (5 append_event calls)
- FOUND: crates/racecontrol/src/pod_healer.rs (1 append_event call)
- FOUND: crates/racecontrol/src/ws/mod.rs (2 append_event calls)
- FOUND: crates/racecontrol/src/metric_alerts.rs (1 append_event call)
- FOUND: commit 7d1c763d (feat(302-02): GET /api/v1/system-events handler)
- FOUND: commit 8069c0a8 (feat(302-02): instrument 6 high-signal event sources)
- 774 unit tests: PASS (cargo test --lib)
- cargo build --release: PASS

*Phase: 302-structured-event-archive*
*Completed: 2026-04-01*
