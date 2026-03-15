---
phase: 19-watchdog-service
plan: 01
subsystem: infra
tags: [windows-service, winapi, watchdog, process-monitor, session-1-spawn, crash-report]

# Dependency graph
requires:
  - phase: 18-startup-self-healing
    provides: start-rcagent.bat + HKLM Run key pattern for Session 1 rc-agent startup
provides:
  - rc-watchdog crate (Windows SYSTEM service binary)
  - WatchdogCrashReport type in rc-common
  - Session 1 spawn via WTSQueryUserToken + CreateProcessAsUser
  - tasklist-based process polling with grace window
  - Fire-and-forget HTTP crash reporting to rc-core
affects: [20-deploy-pipeline, 21-fleet-dashboard, install.bat, pod-deploy]

# Tech tracking
tech-stack:
  added: [windows-service 0.8, reqwest blocking, winapi wtsapi32/processthreadsapi/userenv/errhandlingapi]
  patterns: [SYSTEM service with SCM lifecycle, Session 0 to Session 1 bridge, fire-and-forget HTTP reporting]

key-files:
  created:
    - crates/rc-watchdog/Cargo.toml
    - crates/rc-watchdog/src/main.rs
    - crates/rc-watchdog/src/service.rs
    - crates/rc-watchdog/src/session.rs
    - crates/rc-watchdog/src/reporter.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/rc-common/src/types.rs

key-decisions:
  - "Read rc-agent.toml for pod_id/server_url with COMPUTERNAME fallback (no separate watchdog config)"
  - "tasklist polling (not sysinfo crate) for process detection — simpler, no extra dependency"
  - "reqwest blocking client (no tokio) — watchdog is a synchronous poll loop"
  - "winapi::um::winbase::WTSGetActiveConsoleSessionId (not wtsapi32) — correct location in winapi 0.3"
  - "errhandlingapi feature added for GetLastError in WinAPI error reporting"

patterns-established:
  - "Session 1 spawn pattern: WTSGetActiveConsoleSessionId + WTSQueryUserToken + DuplicateTokenEx + CreateProcessAsUserW with winsta0\\default desktop"
  - "Windows service poll loop: mpsc channel for stop/shutdown, 5s sleep interval, conservative default (assume running on error)"
  - "Restart grace window: track last_restart_at Instant, skip polling for 15s after spawn"

requirements-completed: [SVC-01, SVC-02, SVC-03]

# Metrics
duration: 10min
completed: 2026-03-15
---

# Phase 19 Plan 01: Watchdog Service Crate Summary

**rc-watchdog Windows SYSTEM service with SCM lifecycle, tasklist process polling, Session 1 spawn via WTSQueryUserToken + CreateProcessAsUser, and fire-and-forget HTTP crash reporting**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-15T09:56:50Z
- **Completed:** 2026-03-15T10:06:52Z
- **Tasks:** 1
- **Files modified:** 8

## Accomplishments
- Created rc-watchdog crate as a proper Windows SYSTEM service using windows-service 0.8 with define_windows_service! macro
- Implemented Session 1 process spawn bridging Session 0 isolation via WTSQueryUserToken + DuplicateTokenEx + CreateProcessAsUserW
- Added WatchdogCrashReport type to rc-common with serde roundtrip tests
- 16 unit tests across rc-watchdog (13) and rc-common (3), all existing 556 tests still green

## Task Commits

Each task was committed atomically:

1. **Task 1: Create rc-watchdog crate with WatchdogCrashReport type and service skeleton** - `bd0f414` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `Cargo.toml` - Added rc-watchdog to workspace members
- `Cargo.lock` - Updated with windows-service 0.8 + widestring deps
- `crates/rc-watchdog/Cargo.toml` - New crate manifest with windows-service, winapi, reqwest blocking
- `crates/rc-watchdog/src/main.rs` - Service entry point with define_windows_service! macro, tracing to watchdog.log
- `crates/rc-watchdog/src/service.rs` - SCM lifecycle, 5s poll loop, tasklist process detection, 15s grace window, config loading
- `crates/rc-watchdog/src/session.rs` - Session 1 spawn: WTSGetActiveConsoleSessionId + WTSQueryUserToken + DuplicateTokenEx + CreateProcessAsUserW with proper handle cleanup
- `crates/rc-watchdog/src/reporter.rs` - HTTP POST crash report to rc-core with 5s timeout, fire-and-forget semantics
- `crates/rc-common/src/types.rs` - Added WatchdogCrashReport struct with pod_id, exit_code, crash_time, restart_count, watchdog_version

## Decisions Made
- Read rc-agent.toml for pod_id and server_url rather than a separate watchdog config file — watchdog is a companion to rc-agent
- Used tasklist polling (not sysinfo crate) for process detection — simpler, no extra dependency, consistent with pod_monitor.rs pattern
- Used reqwest blocking (not async) since watchdog is a synchronous poll loop with no tokio runtime
- WTSGetActiveConsoleSessionId found in winapi::um::winbase (not wtsapi32) — corrected during build
- Added errhandlingapi feature to winapi for proper GetLastError reporting
- Conservative is_rc_agent_running: returns true on error (assume running if can't check, prevents false restarts)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] WTSGetActiveConsoleSessionId import location**
- **Found during:** Task 1 (initial build)
- **Issue:** Plan referenced wtsapi32 module, but in winapi 0.3 WTSGetActiveConsoleSessionId is in winbase
- **Fix:** Changed import from winapi::um::wtsapi32 to winapi::um::winbase
- **Files modified:** crates/rc-watchdog/src/session.rs
- **Verification:** cargo build -p rc-watchdog succeeds

**2. [Rule 3 - Blocking] Missing errhandlingapi feature flag**
- **Found during:** Task 1 (initial build)
- **Issue:** GetLastError requires errhandlingapi feature in winapi, not listed in plan's Cargo.toml template
- **Fix:** Added "errhandlingapi" to winapi features list in Cargo.toml
- **Files modified:** crates/rc-watchdog/Cargo.toml
- **Verification:** cargo build -p rc-watchdog succeeds

**3. [Rule 3 - Blocking] c_void type mismatch (std vs winapi)**
- **Found during:** Task 1 (initial build)
- **Issue:** Used std::ffi::c_void for env_block but winapi functions expect winapi::ctypes::c_void
- **Fix:** Changed to winapi::ctypes::c_void for CreateEnvironmentBlock/DestroyEnvironmentBlock/CreateProcessAsUserW
- **Files modified:** crates/rc-watchdog/src/session.rs
- **Verification:** cargo build -p rc-watchdog succeeds

---

**Total deviations:** 3 auto-fixed (all Rule 3 - Blocking)
**Impact on plan:** All auto-fixes necessary for compilation. No scope creep. Winapi module location and feature flags are common gotchas documented in research pitfalls section.

## Issues Encountered
None beyond the auto-fixed blocking issues above.

## User Setup Required
None - no external service configuration required. Service installation (sc create) is part of Phase 19 Plan 02 (install.bat updates).

## Next Phase Readiness
- rc-watchdog.exe compiles and is ready for deployment
- Plan 02 (service installation + deployment) can proceed: update install.bat with sc create/failure commands
- rc-core endpoint for WatchdogCrashReport (POST /api/v1/pods/:pod_id/watchdog-crash) not yet implemented — needed in a future plan
- Binary size: ~7.8 MB debug (release build will be smaller)

## Self-Check: PASSED

- All 7 key files confirmed present on disk
- Commit bd0f414 confirmed in git log
- 16 new tests pass (13 rc-watchdog + 3 rc-common WatchdogCrashReport)
- All 556 existing tests still pass (rc-common 103, rc-agent 199, rc-core 254)
- rc-watchdog.exe binary confirmed at target/debug/rc-watchdog.exe (7.8 MB)

---
*Phase: 19-watchdog-service*
*Completed: 2026-03-15*
