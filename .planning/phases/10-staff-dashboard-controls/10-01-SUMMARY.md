---
phase: 10-staff-dashboard-controls
plan: 01
subsystem: api
tags: [rust, axum, wol, kiosk, lockdown, billing, unit-tests]

# Dependency graph
requires:
  - phase: 07-server-pinning
    provides: racecontrol running on server at :8080 with agent_senders + billing infrastructure
provides:
  - POST /pods/{id}/lockdown route with billing guard and disconnected sender guard
  - POST /pods/lockdown-all route that iterates all senders skipping billing-active and closed
  - 6 parse_mac unit tests (colon, dash, lowercase, too-few-parts, invalid-hex, empty)
  - 4 lockdown route unit tests (billing active, missing sender, closed sender, bulk skip)
  - BillingTimer::dummy() test helper for racecontrol unit tests
affects: [10-staff-dashboard-controls plans 02+, frontend UI wiring for lockdown]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Targeted agent send via agent_senders lookup instead of broadcast_settings for per-pod ephemeral settings"
    - "Billing guard pattern: check active_timers before sending to agent (mirrors pod_monitor:274)"
    - "Disconnected sender guard: is_closed() check before send, return error not silent success"
    - "BillingTimer::dummy() test helper with cfg(test) for lightweight AppState test setup"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/wol.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Lockdown toggle is ephemeral (no DB write) — resets to rc-agent config default on restart; correct behavior since default is locked"
  - "parse_mac changed to pub(crate) visibility for unit testability without making it part of public API"
  - "BillingTimer::dummy() added as cfg(test) method in billing.rs to avoid duplicating 20-field struct literal across route tests"
  - "lockdown-all route registered with other static bulk routes before {id} dynamic routes to prevent Axum routing conflict"

patterns-established:
  - "Agent sender check pattern: read agent_senders, get by pod_id, check is_closed(), then send — always return explicit error on not-connected"
  - "Billing guard for write operations: read active_timers, contains_key check before any destructive/intrusive operation on pod"

requirements-completed: [KIOSK-01, KIOSK-02, PWR-01, PWR-02, PWR-03]

# Metrics
duration: 25min
completed: 2026-03-14
---

# Phase 10 Plan 01: Staff Dashboard Controls — Lockdown API Routes Summary

**Axum lockdown routes for per-pod and bulk kiosk toggle with billing guard + 10 new unit tests covering parse_mac and lockdown logic**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-14T00:00:00Z
- **Completed:** 2026-03-14T00:25:00Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- Added `POST /pods/{id}/lockdown` route: guards against billing-active pods and closed/missing agent senders, sends `CoreToAgentMessage::SettingsUpdated { kiosk_lockdown_enabled }` to targeted pod only
- Added `POST /pods/lockdown-all` route: iterates all agent senders, skips closed senders (not_connected) and billing-active pods (when locking), sends lockdown command to all eligible pods
- Added 6 `parse_mac` unit tests in wol.rs covering colon/dash/lowercase happy paths and 3 error cases (too-few-parts, invalid-hex, empty)
- Added 4 lockdown route unit tests in routes.rs verifying billing guard, missing sender, closed sender, and bulk skip logic
- Test count grew from 178 to 188 (165+13 → 175+13); all passing

## Task Commits

1. **Task 1: Add parse_mac unit tests, lockdown route handlers, and lockdown unit tests to racecontrol** - `564b8ee` (feat)

**Plan metadata:** (committed in final docs commit)

## Files Created/Modified

- `crates/racecontrol/src/wol.rs` - Changed parse_mac to pub(crate), added 6 unit tests
- `crates/racecontrol/src/billing.rs` - Added BillingTimer::dummy() test helper (cfg(test))
- `crates/racecontrol/src/api/routes.rs` - Added lockdown_pod and lockdown_all_pods handlers + route registration + 4 unit tests

## Decisions Made

- Lockdown is ephemeral (in-memory only, no DB write): rc-agent default config always starts locked, so ephemeral commands are the correct behavior; no per-pod persistence needed
- Used `parse_mac` → `pub(crate)` instead of adding a separate test-only wrapper: minimal change, preserves non-public interface
- Registered `/pods/lockdown-all` with the other static bulk routes (before `{id}` dynamic routes) to prevent Axum routing conflict

## Deviations from Plan

None — plan executed exactly as written. The `BillingTimer::dummy()` helper was added to billing.rs to support clean test construction; this is within the spirit of the plan's "construct AppState with billing timer entry" instruction.

## Issues Encountered

None. All tests passed on first compile.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Backend lockdown API is complete and tested
- Frontend wiring for lockdown buttons (plan 02+): `POST /pods/{id}/lockdown` and `POST /pods/lockdown-all` are ready to call
- Both routes follow the same pattern as existing wake/shutdown/restart routes — easy to wire up in api.ts
- No blockers

---
*Phase: 10-staff-dashboard-controls*
*Completed: 2026-03-14*
