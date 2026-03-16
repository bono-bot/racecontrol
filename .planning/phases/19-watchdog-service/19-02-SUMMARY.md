---
phase: 19-watchdog-service
plan: 02
subsystem: infra
tags: [axum-endpoint, crash-report, install-script, windows-service, pod-canary]

# Dependency graph
requires:
  - phase: 19-watchdog-service/01
    provides: rc-watchdog crate + WatchdogCrashReport type in rc-common
provides:
  - POST /api/v1/pods/{pod_id}/watchdog-crash endpoint in racecontrol
  - install-watchdog.bat for fleet SCM registration
  - Release binary rc-watchdog.exe (3.6 MB)
affects: [20-deploy-pipeline, 21-fleet-dashboard, pod-deploy]

# Tech tracking
tech-stack:
  added: []
  patterns: [fire-and-forget crash report ingestion, sc.exe failure actions]

key-files:
  created: [deploy/install-watchdog.bat]
  modified: [crates/racecontrol/src/api/routes.rs]

key-decisions:
  - "Handler returns StatusCode::OK (no JSON body) -- watchdog is fire-and-forget, no response parsing needed"
  - "log_pod_activity source='watchdog' to distinguish from agent/core/race_engineer sources in activity feed"

patterns-established:
  - "Watchdog crash report: POST to /api/v1/pods/{pod_id}/watchdog-crash with WatchdogCrashReport JSON body"

requirements-completed: [SVC-03, SVC-04]

# Metrics
duration: 9min
completed: 2026-03-15
---

# Phase 19 Plan 02: Crash Report Endpoint + Install Script Summary

**racecontrol POST /api/v1/pods/{pod_id}/watchdog-crash endpoint with WARN-level structured logging, activity recording, and install-watchdog.bat for fleet SCM registration with failure restart actions**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-15T10:10:39Z
- **Completed:** 2026-03-15T10:19:25Z
- **Tasks:** 1 of 2 (Task 2 is checkpoint:human-verify -- awaiting Pod 8 canary)
- **Files modified:** 2

## Accomplishments
- POST /api/v1/pods/{pod_id}/watchdog-crash returns 200 for valid WatchdogCrashReport JSON
- Handler logs at WARN level with structured fields: pod_id, exit_code, restart_count, crash_time, watchdog_version
- Pod activity recorded via log_pod_activity (category=system, action="Watchdog Crash Report", source=watchdog)
- install-watchdog.bat: sc.exe create with auto-start, SYSTEM account, failure restart actions (5s/10s/30s)
- rc-watchdog.exe release binary built successfully (3.6 MB)
- All 571 tests pass across 4 crates (no regressions)

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Failing tests for watchdog crash report** - `faea0d6` (test)
2. **Task 1 GREEN: Implement handler + install script** - `6578e45` (feat)
3. **Task 2: Pod 8 canary verification** - PENDING (checkpoint:human-verify)

_TDD task: RED produced compilation failure (E0425: function not found), GREEN made all 3 tests pass._

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - Added watchdog_crash_report handler + route + 3 unit tests
- `deploy/install-watchdog.bat` - Windows service installer with sc.exe create + failure actions

## Decisions Made
- Handler returns bare StatusCode::OK (no JSON body) since watchdog is fire-and-forget -- no response parsing needed
- log_pod_activity source set to "watchdog" to distinguish from agent/core/race_engineer in activity feed
- install-watchdog.bat uses sc.exe (not sc) to avoid PowerShell alias conflict

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Crash report endpoint is live (once racecontrol is rebuilt/restarted)
- install-watchdog.bat is ready for fleet deployment
- rc-watchdog.exe release binary ready for Pod 8 canary test
- Pod 8 canary verification pending (Task 2 checkpoint)
- After canary approval: fleet rollout via Phase 20 deploy pipeline

## Self-Check: PASSED

- [x] `crates/racecontrol/src/api/routes.rs` -- FOUND
- [x] `deploy/install-watchdog.bat` -- FOUND
- [x] `target/release/rc-watchdog.exe` -- FOUND (3.6 MB)
- [x] Commit `faea0d6` (RED) -- FOUND
- [x] Commit `6578e45` (GREEN) -- FOUND

---
*Phase: 19-watchdog-service*
*Completed: 2026-03-15*
