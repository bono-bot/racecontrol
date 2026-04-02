---
phase: 304-fleet-deploy-automation
plan: 01
subsystem: infra
tags: [rust, deploy, ota, fleet, rollback, canary, billing-drain]

# Dependency graph
requires:
  - phase: 303-venue-schema
    provides: AppState structure, state.rs field layout
  - phase: ota_pipeline (v22.0)
    provides: rollback_wave, set_ota_sentinel, set_kill_switch, health_check_pod, wave constants
  - phase: deploy (v22.0)
    provides: deploy_pod, is_deploy_window_locked, WaitingSession + pending_deploys pattern

provides:
  - FleetDeploySession struct (deploy_id, waves, rollback_events, overall_status)
  - FleetDeployRequest + DeployScope types with serde round-trip
  - run_fleet_deploy() orchestration: canary-first, wave delay, billing drain, per-pod/canary rollback
  - create_session() helper: pre-populates waves from scope (All/Canary/Pods)
  - 11 unit tests covering all scope variants, serde, lifecycle, status serialization

affects:
  - 304-02 (wires fleet_deploy_session into AppState and adds route handlers)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - snapshot-drop lock pattern (no RwLock held across .await)
    - 202-Accepted + background tokio::spawn deploy orchestration
    - canary-halt vs non-canary-continue rollback semantics
    - IST RFC3339 timestamp helper matching existing routes.rs pattern

key-files:
  created:
    - crates/racecontrol/src/fleet_deploy.rs
  modified:
    - crates/racecontrol/src/lib.rs

key-decisions:
  - "deploy_id format: {binary_hash[..8]}-{unix_timestamp} — human-readable + unique"
  - "DeployScope::Pods creates a single wave (wave 1) — no multi-wave for custom pod sets"
  - "canary failure halts entire deploy; non-canary failure triggers per-pod rollback but continues"
  - "sentry_service_key accessed via state.config.pods.sentry_service_key (not state.config directly)"
  - "IST offset fallback uses same pattern as routes.rs:21121 (.unwrap_or(east_opt(0).unwrap()))"

patterns-established:
  - "Lock discipline: always { let mut g = lock.write().await; mutate; } before any .await"
  - "deploy_pod() is infallible — read pod_deploy_states IMMEDIATELY after it returns"
  - "WaitingSession deferral: write DeployState::WaitingSession + pending_deploys entry, skip pod in loop"

requirements-completed: [DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05, DEPLOY-06]

# Metrics
duration: 25min
completed: 2026-04-02
---

# Phase 304 Plan 01: Fleet Deploy Orchestration Module Summary

**FleetDeploySession struct and run_fleet_deploy() canary-first orchestrator with wave-aware rollback, billing drain, and 11 unit tests in a new fleet_deploy.rs module**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-02T06:30:00+05:30
- **Completed:** 2026-04-02T11:57:00+05:30
- **Tasks:** 1 of 1
- **Files modified:** 2

## Accomplishments

- Created `fleet_deploy.rs` (639 lines) with all types, orchestration, and tests
- `run_fleet_deploy()` implements canary-first (Pod 8), then Wave 2 (Pods 1-4), then Wave 3 (Pods 5-7)
- Canary failure halts entire deploy and triggers rollback; non-canary failure is pod-local and non-fatal
- Billing drain: pods with active sessions get `WaitingSession` state + `pending_deploys` entry (auto-fires on session end)
- All 11 unit tests green; `cargo check -p racecontrol-crate` clean (no errors from new module)

## Task Commits

1. **Task 1: Create fleet_deploy.rs with types, orchestration, and tests** - `161746ec` (feat)

## Files Created/Modified

- `crates/racecontrol/src/fleet_deploy.rs` - New module: FleetDeploySession, FleetDeployRequest, DeployScope, WaveStatus, PodDeployResult, RollbackEvent, run_fleet_deploy(), create_session(), now_ist_rfc3339(), 11 unit tests
- `crates/racecontrol/src/lib.rs` - Added `pub mod fleet_deploy;` in alphabetical position (after `flags`, before `fleet_alert`)

## Decisions Made

- `deploy_id` format: `{binary_hash[..8]}-{unix_timestamp}` — human-readable, unique, matches existing deploy_id patterns
- `DeployScope::Pods(ids)` creates a single wave (wave 1) — these are ad-hoc sets, not canonical wave order
- `sentry_service_key` accessed as `state.config.pods.sentry_service_key` — the field is nested in a `PodConfig` sub-struct, not directly on `Config`
- IST offset fallback matches existing routes.rs pattern: `.unwrap_or(east_opt(0).unwrap())` — east_opt(0) is always Some

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed wrong field path for sentry_service_key**
- **Found during:** Task 1 (cargo check)
- **Issue:** Plan spec said `state.config.sentry_service_key` but actual Config struct has it at `state.config.pods.sentry_service_key`
- **Fix:** Used correct field path `state.config.pods.sentry_service_key.as_deref()`
- **Files modified:** `crates/racecontrol/src/fleet_deploy.rs`
- **Verification:** `cargo check -p racecontrol-crate` clean
- **Committed in:** `161746ec` (part of task commit)

**2. [Rule 1 - Bug] Fixed wrong PodInfo field name**
- **Found during:** Task 1 (cargo check)
- **Issue:** Plan spec and research used `p.ip` but `PodInfo` struct has `ip_address` (not `ip`)
- **Fix:** Changed all `p.ip.clone()` to `p.ip_address.clone()`
- **Files modified:** `crates/racecontrol/src/fleet_deploy.rs`
- **Verification:** `cargo check -p racecontrol-crate` clean
- **Committed in:** `161746ec` (part of task commit)

**3. [Rule 1 - Bug] Fixed FixedOffset default construction**
- **Found during:** Task 1 (cargo check)
- **Issue:** `FixedOffset` does not implement `Default` so `.unwrap_or_default()` fails to compile
- **Fix:** Used `.unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap())` — matches existing routes.rs:21121 pattern
- **Files modified:** `crates/racecontrol/src/fleet_deploy.rs`
- **Verification:** `cargo check -p racecontrol-crate` clean
- **Committed in:** `161746ec` (part of task commit)

---

**Total deviations:** 3 auto-fixed (all Rule 1 - Bug, all from cargo check)
**Impact on plan:** All fixes were for field names/types that differ from plan spec. No scope creep.

## Issues Encountered

Cargo name is `racecontrol-crate`, not `racecontrol` — `cargo check -p racecontrol` fails with "package ID specification did not match any packages". Used correct name throughout.

## Known Stubs

None — all types are fully defined and orchestration logic is complete. No placeholder data flows.

## Next Phase Readiness

Plan 02 can proceed immediately:
- `FleetDeploySession` type exported from `fleet_deploy` module
- `run_fleet_deploy()` signature is finalized: `(state: Arc<AppState>, session_lock: Arc<RwLock<Option<FleetDeploySession>>>)`
- Plan 02 needs to: add `fleet_deploy_session: RwLock<Option<FleetDeploySession>>` to `AppState`, add `fleet_deploy_handler` and `fleet_deploy_status_handler` route handlers, register routes in superadmin tier

---
*Phase: 304-fleet-deploy-automation*
*Completed: 2026-04-02*
