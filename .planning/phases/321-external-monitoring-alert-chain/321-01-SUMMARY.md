---
phase: 321-external-monitoring-alert-chain
plan: 01
subsystem: monitoring
tags: [watchdog, fsm, tasklist, dual-detection, process-liveness, rc-sentry]

requires:
  - phase: none
    provides: existing watchdog FSM and pod_healer TierOneRestart
provides:
  - "check_process_alive() function for process liveness detection via tasklist"
  - "Dual-detection FSM: health + process signals for <5s crash detection"
  - "MON-02 verified: sentry fallback already implemented in pod_healer"
  - "MON-03 verified: MAINTENANCE_MODE backoff already implemented (no-op)"
affects: [321-02, 321-03, rc-sentry-watchdog, pod-healer]

tech-stack:
  added: []
  patterns: ["dual-signal FSM (health poll + process liveness)", "fail-open tasklist check with CREATE_NO_WINDOW"]

key-files:
  created: []
  modified:
    - "crates/rc-sentry/src/watchdog.rs"
    - "crates/racecontrol/src/pod_healer.rs"

key-decisions:
  - "Fail-open on tasklist error: if tasklist fails, assume process alive to avoid false positives"
  - "Dual-detection respects restart_suppressed flag to prevent fast-crash during OTA/MAINTENANCE_MODE"
  - "MON-02 uses sc start RCWatchdog + taskkill (NOT schtasks) per Session 1 standing rule"
  - "MON-03 confirmed as no-op: RestartTracker + MAINTENANCE_MODE + auto-clear already implemented"

patterns-established:
  - "Dual-signal watchdog: health poll + tasklist process check for fast crash detection"
  - "Test mock via module-level AtomicBool (MOCK_PROCESS_ALIVE) for process liveness tests"
  - "fsm_dual_next() test helper mirrors production FSM for unit testing without thread spawning"

requirements-completed: [MON-01, MON-02, MON-03]

duration: 9min
completed: 2026-04-06
---

# Phase 321 Plan 01: Dual-Detection Watchdog Summary

**Dual-detection watchdog FSM: tasklist process check + health poll reduces crash detection from 15s to <5s while preserving hysteresis for unresponsive-but-alive scenarios**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-06T16:39:45Z
- **Completed:** 2026-04-06T16:48:50Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `check_process_alive()` using tasklist with CREATE_NO_WINDOW (fail-open on error)
- Extended FSM to 3-signal match: `(state, health, process_alive)` with 6 transition rules
- Dual-detection fast path: health DOWN + process DEAD = immediate Crashed (skip 15s hysteresis)
- Restart suppression check prevents fast-crash during OTA/MAINTENANCE_MODE deploys
- Verified MON-02 (sentry fallback via TierOneRestart) and MON-03 (backoff/auto-clear) as already implemented
- 7 new unit tests for dual-detection + 1 MON-02 verification test, all passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Add check_process_alive() and dual-detection FSM logic** - `edab945f` (feat)
2. **Task 2: Verify MON-02 and MON-03, add coverage tests** - `9b2ec450` (feat)

## Files Created/Modified
- `crates/rc-sentry/src/watchdog.rs` - Added check_process_alive(), dual-detection FSM, 7 tests
- `crates/racecontrol/src/pod_healer.rs` - Added MON-02 comment and verification test

## Decisions Made
- Fail-open on tasklist error: avoids false-positive crash detection when Windows is under load
- Dual-detection respects restart_suppressed: prevents MAINTENANCE_MODE fast-crash during deploys
- MON-03 is a confirmed no-op: all backoff and auto-clear mechanisms already exist
- Used module-level AtomicBool mock instead of trait-based DI for simpler test code

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all code paths are fully wired.

## Next Phase Readiness
- Watchdog dual-detection is complete and ready for Phase 322 (Core MI migration)
- check_process_alive() is available for any future process monitoring needs
- No blockers

---
*Phase: 321-external-monitoring-alert-chain*
*Completed: 2026-04-06*
