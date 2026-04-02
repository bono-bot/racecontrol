---
phase: 304-fleet-deploy-automation
plan: 02
subsystem: infra
tags: [rust, deploy, fleet, api, routes, appstate, superadmin]

# Dependency graph
requires:
  - phase: 304-01
    provides: FleetDeploySession, FleetDeployRequest, DeployScope, run_fleet_deploy, create_session

provides:
  - fleet_deploy_session field on AppState (Arc<RwLock<Option<FleetDeploySession>>>)
  - POST /api/v1/fleet/deploy — fleet_deploy_handler (202 + background tokio::spawn)
  - GET /api/v1/fleet/deploy/status — fleet_deploy_status_handler (session JSON or idle)
  - 409 Conflict guard for concurrent deploy prevention
  - Deploy window lock enforcement in fleet_deploy_handler

affects:
  - racecontrol server (state.rs, routes.rs)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - check-and-set with write lock (write guard dropped before any .await)
    - 202-Accepted + background tokio::spawn with Arc<RwLock<>> shared to background task
    - superadmin tier route registration (require_role_superadmin layer)

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "fleet_deploy_session uses Arc<RwLock<>> (not plain RwLock) so background task holds an independent Arc reference"
  - "Handler extracts deploy_id string before dropping write guard — no lock held across spawn"
  - "fleet_deploy_handler uses axum::Extension<StaffClaims> (not tuple extractor) — matches existing deploy_rolling_handler pattern"

requirements-completed: [DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05, DEPLOY-06]

# Metrics
duration: 12min
completed: 2026-04-02
---

# Phase 304 Plan 02: AppState Field + Route Handlers Summary

**POST /api/v1/fleet/deploy (202 + background orchestration) and GET /api/v1/fleet/deploy/status wired into the superadmin router via fleet_deploy_session Arc<RwLock<>> on AppState**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-02T12:09:00+05:30
- **Completed:** 2026-04-02T12:21:00+05:30
- **Tasks:** 1 of 1
- **Files modified:** 2

## Accomplishments

- Added `fleet_deploy_session: Arc<RwLock<Option<FleetDeploySession>>>` to `AppState` struct and initialized in `AppState::new()`
- Added `fleet_deploy_handler` (68 lines): 409 guard, window lock check, check-and-set session, tokio::spawn orchestration, 202 response
- Added `fleet_deploy_status_handler` (14 lines): read lock, serialize session or return idle JSON
- Registered both routes in superadmin tier (`.route("/fleet/deploy", post(...))` + `.route("/fleet/deploy/status", get(...))`)
- Route uniqueness test: PASS
- Full test suite: 792 tests PASS (all green)

## Task Commits

1. **Task 1: Add AppState field and route handlers** - `17c18f75` (feat)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - Added `fleet_deploy_session` field declaration and initialization
- `crates/racecontrol/src/api/routes.rs` - Added `fleet_deploy_handler`, `fleet_deploy_status_handler`, route registrations

## Decisions Made

- `fleet_deploy_session` uses `Arc<RwLock<>>` (not plain `RwLock`) so the background `tokio::spawn` task can hold an independent `Arc` clone — if we used a plain `RwLock` field, the background task would need to hold the full `Arc<AppState>` reference through the session lifetime
- Write guard dropped before `tokio::spawn` by extracting `deploy_id: String` in a tight `{ }` block — no lock held across async boundary
- Handler uses `axum::Extension<StaffClaims>` extractor (matching `deploy_rolling_handler` pattern) rather than the tuple syntax shown in the plan spec

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] StaffClaims extractor syntax corrected**
- **Found during:** Task 1 (cargo check)
- **Issue:** Plan spec showed `claims: StaffClaims` as a bare extractor. The existing `deploy_rolling_handler` pattern uses `axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>`. The bare syntax would fail to compile — `StaffClaims` is injected by middleware as an Extension, not via a direct extractor.
- **Fix:** Used `axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>` in `fleet_deploy_handler`. For `fleet_deploy_status_handler` (where claims are not used), used `_claims: axum::Extension<...>` to satisfy the extractor requirement while naming the variable as unused.
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Verification:** `cargo check -p racecontrol-crate` clean
- **Committed in:** `17c18f75`

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug, from cargo check)
**Impact on plan:** No scope creep. Fix was purely to match existing auth extractor pattern.

## Issues Encountered

Cargo package name is `racecontrol-crate`, not `racecontrol` — `cargo check -p racecontrol` fails. Used correct name throughout (same as Plan 01).

## Known Stubs

None — both handlers are fully functional and read/write the real `fleet_deploy_session` field on AppState. The background orchestration logic is complete from Plan 01 (`run_fleet_deploy`). No placeholder data flows.

## Next Phase Readiness

Phase 304 (fleet-deploy-automation) is now complete:
- Plan 01: `fleet_deploy.rs` module with all types and orchestration (11 unit tests)
- Plan 02: AppState field + superadmin route handlers (route_uniqueness PASS, 792 tests PASS)

All 6 DEPLOY requirements satisfied (DEPLOY-01 through DEPLOY-06).

## Self-Check: PASSED

- `crates/racecontrol/src/state.rs` — FOUND, contains `fleet_deploy_session`
- `crates/racecontrol/src/api/routes.rs` — FOUND, contains `fleet/deploy` and `fleet_deploy_handler` and `fleet_deploy_status_handler`
- commit `17c18f75` — FOUND in git log
- route_uniqueness test — PASS (1 passed)
- full suite — 792 tests PASS

---
*Phase: 304-fleet-deploy-automation*
*Completed: 2026-04-02*
