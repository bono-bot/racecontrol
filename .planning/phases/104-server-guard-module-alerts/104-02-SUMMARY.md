---
phase: 104-server-guard-module-alerts
plan: 02
subsystem: api
tags: [rust, axum, process-guard, sysinfo, violations, server-guard]

# Dependency graph
requires:
  - phase: 104-server-guard-module-alerts/104-01
    provides: ViolationStore type + state.pod_violations RwLock<HashMap<String, ViolationStore>> in AppState
  - phase: 103-pod-guard-module
    provides: rc-agent process_guard.rs scan loop pattern (spawn_blocking sysinfo, grace period, CRITICAL_BINARIES, log rotation)
  - phase: 102-whitelist-schema-config-fetch-endpoint
    provides: merge_for_machine() in racecontrol process_guard.rs; ProcessGuardConfig with enabled/poll_interval_secs/violation_action

provides:
  - spawn_server_guard(Arc<AppState>) in crates/racecontrol/src/process_guard.rs
  - SERVER_CRITICAL_BINARIES constant (rc-agent.exe = zero grace on server)
  - is_server_critical() fn for CRITICAL binary detection (case-insensitive)
  - log_server_guard_event() with 512KB rotation at C:\RacingPoint\process-guard.log
  - Server scan loop running every poll_interval_secs, violations pushed to pod_violations["server"]
  - spawn_server_guard() wired into main.rs after fleet_health::start_probe_loop

affects:
  - 105-port-scan-audit (server guard violations now in pod_violations["server"] alongside pod data)
  - pwa/dashboard displaying violation counts (server violations now included in fleet/health API response)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Server guard mirrors rc-agent guard pattern: spawn_blocking sysinfo, two-cycle grace, CRITICAL zero-grace, 512KB log rotation"
    - "sysinfo 0.33 API: System::new() + refresh_processes(ProcessesToUpdate::All, true) — NOT System::new_all() + refresh_processes()"
    - "PID identity verification before kill: re-check name + start_time in spawn_blocking to guard against PID reuse"
    - "Self-exclusion: own PID inline + racecontrol.exe name check (own binary on server)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/process_guard.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "sysinfo 0.33 uses System::new() + refresh_processes(ProcessesToUpdate::All, true), NOT System::new_all() — plan snippet was simplified but actual API matches rc-agent pattern"
  - "spawn_server_guard() wired in main.rs (not lib.rs) — start_probe_loop is also in main.rs, plan context was correct"
  - "process_guard module added to main.rs use racecontrol_crate::{ ... } block to make spawn_server_guard callable"

patterns-established:
  - "Server guard startup: no-op if enabled=false, then tokio::spawn + interval.tick() consume immediate first tick (fire after full interval)"
  - "Grace period cleanup: grace_counts.retain() keyed on violations_this_cycle HashSet after each cycle to prevent unbounded growth"

requirements-completed: [DEPLOY-02]

# Metrics
duration: 22min
completed: 2026-03-21
---

# Phase 104 Plan 02: Server Guard Module Alerts Summary

**Server-side process guard scan loop using sysinfo 0.33 spawn_blocking pattern, CRITICAL detection of rc-agent.exe with zero grace, 512KB-rotating log at C:\RacingPoint\process-guard.log, violations fed into pod_violations["server"] ViolationStore.**

## Performance

- **Duration:** 22 min
- **Started:** 2026-03-21T11:10:00Z (IST: 16:40)
- **Completed:** 2026-03-21T11:32:00Z (IST: 17:02)
- **Tasks:** 1 of 1
- **Files modified:** 2

## Accomplishments
- spawn_server_guard(Arc<AppState>) added to process_guard.rs: starts tokio scan loop on poll_interval_secs interval
- SERVER_CRITICAL_BINARIES = ["rc-agent.exe"] with zero grace period enforcement
- is_server_critical() for case-insensitive CRITICAL binary detection
- log_server_guard_event() with 512KB rotation (matches rc-agent pattern exactly)
- TDD: test_is_server_critical_rc_agent() confirms rc-agent.exe, RC-AGENT.EXE detected, svchost.exe and racecontrol.exe correctly excluded
- All 14 process_guard tests pass; cargo build -p racecontrol-crate succeeds with zero errors

## Task Commits

Each task was committed atomically:

1. **Task 1: spawn_server_guard() + test_is_server_critical_rc_agent** - `c8f8324` (feat)

## Files Created/Modified
- `crates/racecontrol/src/process_guard.rs` - Added SERVER_CRITICAL_BINARIES, is_server_critical(), log_server_guard_event(), spawn_server_guard(); TDD test added
- `crates/racecontrol/src/main.rs` - Added process_guard to use block; spawn_server_guard(state.clone()) call after fleet_health::start_probe_loop

## Decisions Made
- Used `System::new()` + `refresh_processes(ProcessesToUpdate::All, true)` (sysinfo 0.33 actual API) rather than plan snippet's `System::new_all()` + `refresh_processes()` — matches rc-agent pattern and compiles correctly
- spawn_server_guard() wired in main.rs (not lib.rs) because that is where start_probe_loop is called; lib.rs only has pub mod declarations

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Used correct sysinfo 0.33 API in spawn_blocking**
- **Found during:** Task 1 (implementation)
- **Issue:** Plan snippet used `System::new_all()` + `sys.refresh_processes()` (older API). sysinfo 0.33 uses `System::new()` + `refresh_processes(ProcessesToUpdate::All, true)`
- **Fix:** Used rc-agent's established pattern which already compiles correctly with sysinfo 0.33
- **Files modified:** crates/racecontrol/src/process_guard.rs
- **Verification:** cargo build -p racecontrol-crate succeeds; 14 tests pass
- **Committed in:** c8f8324 (Task 1 commit)

**2. [Rule 3 - Blocking] Added process_guard to main.rs use block**
- **Found during:** Task 1 (wiring spawn call)
- **Issue:** Plan said add spawn call after fleet_health::start_probe_loop in lib.rs, but that function is in main.rs. process_guard was not in the use racecontrol_crate::{ ... } block in main.rs
- **Fix:** Added process_guard to the use block and placed spawn_server_guard call in main.rs
- **Files modified:** crates/racecontrol/src/main.rs
- **Verification:** cargo build succeeds; grep confirms call present
- **Committed in:** c8f8324 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 API correctness, 1 blocking import)
**Impact on plan:** Both auto-fixes necessary for the code to compile. No scope creep.

## Issues Encountered

None beyond the sysinfo API deviation documented above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Server guard is fully operational: spawns on startup when process_guard.enabled=true, scans every 60s, CRITICAL log for rc-agent.exe, violations in pod_violations["server"]
- fleet/health API already includes violation_count_24h + last_violation_at from Plan 01 — server violations are now included automatically
- Phase 104 complete — ready for Phase 105 (port scan audit)
- Pre-work still needed before Phase 103/105 enforcement: run sysinfo inventory dump on all 8 pods to capture full legitimate process set

---
*Phase: 104-server-guard-module-alerts*
*Completed: 2026-03-21*
