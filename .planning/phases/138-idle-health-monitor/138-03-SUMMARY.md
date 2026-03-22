---
phase: 138-idle-health-monitor
plan: "03"
subsystem: fleet-health
tags: [rust, websocket, fleet-health, idle-health, protocol, rc-common]

# Dependency graph
requires:
  - phase: 138-01
    provides: "AgentMessage::IdleHealthFailed variant in rc-common protocol.rs"
provides:
  - "AgentMessage::IdleHealthFailed match arm in server ws/mod.rs"
  - "FleetHealthStore.idle_health_fail_count and idle_health_failures fields"
  - "PodFleetStatus.idle_health_fail_count and idle_health_failures in GET /api/v1/fleet/health"
affects: [139-pod-healer, fleet-health-api-consumers]

# Tech tracking
tech-stack:
  added: []
  patterns: ["IdleHealthFailed WS handler follows PreFlightFailed pattern: warn log + log_pod_activity + fleet write lock"]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/fleet_health.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "IdleHealthFailed handler placed after ProcessGuardStatus arm, before catch-all, to group all health-related handlers"
  - "consecutive_count dereferences to *consecutive_count because match arm binds by reference"
  - "No email alerts or healer triggers in this plan — those belong to Phase 139"

patterns-established:
  - "Idle health fail state pattern: store.idle_health_fail_count = *consecutive_count; store.idle_health_failures = failures.clone()"

requirements-completed: [IDLE-03]

# Metrics
duration: 25min
completed: 2026-03-22
---

# Phase 138 Plan 03: Idle Health Monitor — Server Handler Summary

**IdleHealthFailed WS handler added to racecontrol: logs warn + activity_log + updates FleetHealthStore, exposed in GET /api/v1/fleet/health via idle_health_fail_count and idle_health_failures per pod**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-22T04:45:00Z
- **Completed:** 2026-03-22T05:10:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `idle_health_fail_count: u32` and `idle_health_failures: Vec<String>` to `FleetHealthStore` (Phase 138 doc comment)
- Added same fields to `PodFleetStatus` API response struct
- `fleet_health_handler` populates both fields from store for registered pods; unregistered slots return 0 / empty vec
- Added `AgentMessage::IdleHealthFailed` match arm in `ws/mod.rs` after `ProcessGuardStatus`
- Handler logs `tracing::warn!` with pod_id, failures, consecutive_count, and timestamp
- Calls `log_pod_activity` with category "system", action "Idle Health Failed"
- Updates `FleetHealthStore` via `pod_fleet_health.write().await` write lock
- `cargo build --release --bin racecontrol` exits 0

## Task Commits

Each task was committed atomically:

1. **Task 1: Add idle health fields to FleetHealthStore and PodFleetStatus** - `e811db7` (feat) — Note: this commit was created in an earlier session during Phase 138 setup work and is already in HEAD
2. **Task 2: Handle IdleHealthFailed in server WS handler** - `825a0a3` (feat)

**Plan metadata:** (pending docs commit)

## Files Created/Modified

- `crates/racecontrol/src/fleet_health.rs` — Added idle_health_fail_count and idle_health_failures to FleetHealthStore and PodFleetStatus; handler populates from store
- `crates/racecontrol/src/ws/mod.rs` — Added IdleHealthFailed match arm with warn log, activity log, and fleet health store update

## Decisions Made

- `consecutive_count` must be dereferenced (`*consecutive_count`) in the match arm because Rust binds struct fields by reference when pattern matching on `&AgentMessage` — rule 1 auto-fix during Task 2
- Handler placed after `ProcessGuardStatus` arm and before `_ => {}` catch-all for logical grouping with other health handlers
- No email alerts or healer triggers added — those are Phase 139 scope

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed mismatched type: &u32 vs u32 for consecutive_count**
- **Found during:** Task 2 (Handle IdleHealthFailed in server WS handler)
- **Issue:** `store.idle_health_fail_count = consecutive_count` failed — E0308 mismatched types, expected u32 found &u32 because match arm binds struct fields by reference
- **Fix:** Changed to `store.idle_health_fail_count = *consecutive_count` to dereference
- **Files modified:** crates/racecontrol/src/ws/mod.rs
- **Verification:** cargo check -p racecontrol-crate passed clean after fix
- **Committed in:** 825a0a3 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - bug)
**Impact on plan:** Fix necessary for correctness. No scope creep.

## Issues Encountered

- Pre-existing test failure: `config::tests::config_fallback_preserved_when_no_env_vars` — unrelated to Phase 138 changes, pre-existing from earlier phases
- Pre-existing linker error for `rc-sentry-ai` binary (OnnxRuntime/DirectML linking issue) — unrelated to racecontrol binary, which builds clean

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Server now receives and stores idle health failure state per pod
- GET /api/v1/fleet/health includes `idle_health_fail_count` and `idle_health_failures` for each pod
- Ready for Phase 139: pod healer can read idle_health_fail_count from fleet health to trigger remediation actions
- No blockers

---
*Phase: 138-idle-health-monitor*
*Completed: 2026-03-22*
