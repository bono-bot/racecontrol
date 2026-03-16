---
phase: 04-deployment-pipeline
plan: 02
subsystem: infra
tags: [deploy, axum, tokio, async, websocket, binary-deploy]

# Dependency graph
requires:
  - phase: 04-01
    provides: DeployState enum, DeployProgress event, DashboardCommand::DeployPod/DeployRolling/CancelDeploy, pod_deploy_states in AppState
provides:
  - deploy_pod() async executor in crates/racecontrol/src/deploy.rs — full kill->wait-dead->download->size-check->start->verify sequence
  - POST /api/deploy/:pod_id endpoint — 202 Accepted, background deploy spawn, 409 Conflict guards
  - GET /api/deploy/status — snapshot of all pod deploy states
  - DashboardCommand::DeployPod/DeployRolling/CancelDeploy handlers in ws/mod.rs
  - Cancellation support via is_cancelled() check at each deploy step
affects: [04-03, rolling-deploy, kiosk-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Deploy executor: background tokio::spawn with sequential async steps, each updating AppState + broadcasting DashboardEvent"
    - "409 Conflict guard: check is_active() and billing before spawning background task"
    - "Cancellation via state mutation: CancelDeploy sets Failed state, deploy_pod() polls is_cancelled() at step boundaries"
    - "Binary safety: HEAD request to validate URL before killing old process — never leave pod without agent"
    - "5MB minimum binary size check guards against HTML error pages saved as .exe"

key-files:
  created:
    - crates/racecontrol/src/deploy.rs
  modified:
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "Binary URL validated via HEAD request before killing old process — prevents leaving pod without agent on 404/network error"
  - "CancelDeploy sets Failed state in AppState; deploy_pod() checks is_cancelled() at each step boundary (not every tick)"
  - "DeployRolling in WS handler sorts Pod 8 canary first, then ascending by pod number, with 5s inter-pod delay"
  - "Config write failure is non-fatal (warning log, proceed with existing config) — binary update is higher priority than config update"
  - "POST /api/deploy/:pod_id returns 409 for both active billing sessions and active deploys — protects pod during session"

patterns-established:
  - "Deploy executor: multi-step async pipeline with state broadcast at each step"
  - "Background task spawn + 202 Accepted pattern for long-running operations"

requirements-completed: [DEPLOY-02, DEPLOY-05]

# Metrics
duration: 6min
completed: 2026-03-13
---

# Phase 4 Plan 02: Deploy Executor Summary

**deploy_pod() async executor: kill->wait-dead->download(5MB guard)->start->verify-health with email alerts, 409 guards, and real-time progress via DashboardEvent::DeployProgress**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-03-13T02:01:37Z
- **Completed:** 2026-03-13T02:07:17Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Full deploy executor in deploy.rs: 9-step pipeline from URL validation to health verification
- POST /api/deploy/:pod_id API endpoint with concurrent deploy guard (409 Conflict) and billing session guard
- GET /api/deploy/status for snapshot of all pod deploy states
- WS handlers for DeployPod, DeployRolling (Pod 8 canary first), and CancelDeploy
- 17 pure-function tests covering all edge cases in validate_binary_size, parse_file_size_from_dir, deploy_step_label, generate_pod_config

## Task Commits

Each task was committed atomically:

1. **Task 1: Create deploy.rs module with pure helper functions and tests** - `31dcecf` (feat)
2. **Task 2: Add POST /api/deploy/:pod_id endpoint and wire dashboard command** - `7cd47bc` (feat)

**Plan metadata:** (included in final docs commit)

_Note: Task 1 used TDD — pure functions written and tested in RED/GREEN cycle_

## Files Created/Modified
- `crates/racecontrol/src/deploy.rs` - Deploy executor module: deploy_pod(), validate_binary_size(), parse_file_size_from_dir(), deploy_step_label(), generate_pod_config(), is_cancelled()
- `crates/racecontrol/src/lib.rs` - Added `pub mod deploy;`
- `crates/racecontrol/src/api/routes.rs` - Added POST /api/deploy/:pod_id and GET /api/deploy/status handlers
- `crates/racecontrol/src/ws/mod.rs` - Wired DashboardCommand::DeployPod, DeployRolling, CancelDeploy match arms

## Decisions Made
- Binary URL validated via HEAD request before killing old process — prevents leaving pod without agent on URL errors
- CancelDeploy sets Failed state in AppState; deploy_pod() checks is_cancelled() at each async step boundary
- DeployRolling sorts Pod 8 canary first, then ascending, with 5s inter-pod delay
- Config write failure is non-fatal — proceeds with existing config, only logs warning
- 409 Conflict for active billing sessions in addition to active deploys

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Deploy executor is complete and tested. POST /api/deploy/:pod_id is live.
- Plan 04-03 (rolling deploy + kiosk UI) can now wire up the rolling deploy UI using the DeployProgress events and GET /api/deploy/status endpoint.
- All 153 tests pass (100 racecontrol + 53 rc-common).

---
*Phase: 04-deployment-pipeline*
*Completed: 2026-03-13*

## Self-Check: PASSED

- deploy.rs: FOUND
- 04-02-SUMMARY.md: FOUND
- Commit 31dcecf (Task 1): FOUND
- Commit 7cd47bc (Task 2): FOUND
