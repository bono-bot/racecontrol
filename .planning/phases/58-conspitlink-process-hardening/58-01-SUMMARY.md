---
phase: 58-conspitlink-process-hardening
plan: 01
subsystem: process-management
tags: [conspit-link, ffb, crash-recovery, config-backup, json-verification, atomics]

# Dependency graph
requires:
  - phase: 57-session-end-safety
    provides: "safe_session_end(), close_conspit_link(), restart_conspit_link() in ffb_controller.rs"
provides:
  - "restart_conspit_link_hardened() with crash count, config backup, verify, minimize retry"
  - "backup_conspit_configs() / verify_conspit_configs() with auto-restore from .bak"
  - "SESSION_END_IN_PROGRESS AtomicBool guard for watchdog race prevention"
  - "CONSPIT_CRASH_COUNT AtomicU32 tracking watchdog-triggered restarts"
  - "minimize_conspit_window_with_retry() polling loop (500ms x 16 = 8s max)"
affects: [58-02, 62-fleet-config-distribution, 63-fleet-monitoring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "AtomicU32/AtomicBool for lock-free cross-thread state"
    - "Testable _impl(Option<&Path>) pattern for filesystem-dependent functions"
    - "Config backup skips corrupt source to preserve good .bak"

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/ffb_controller.rs"

key-decisions:
  - "backup_conspit_configs() validates JSON before overwriting .bak -- prevents Pitfall 2 (corrupt overwrites good backup)"
  - "verify_conspit_configs() auto-restores from .bak on corruption, returns false only if no backup exists"
  - "Crash count increments only on watchdog path (is_crash_recovery=true), not session-end"
  - "Single post-restart thread handles minimize-retry then verify (not two separate threads)"
  - "Testable _impl functions accept Option<&Path> base_dir, with #[cfg(test)] pub wrappers"

patterns-established:
  - "Corrupt-source-skip backup: always parse JSON before overwriting .bak"
  - "_in_dir() test wrappers for filesystem functions that use hardcoded production paths"

requirements-completed: [PROC-01, PROC-02, PROC-03, PROC-04]

# Metrics
duration: 5min
completed: 2026-03-20
---

# Phase 58 Plan 01: ConspitLink Process Hardening Summary

**Hardened ConspitLink restart with crash-count tracking, JSON config backup/verify with auto-restore, and polling window minimize retry**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-20T09:29:38Z
- **Completed:** 2026-03-20T09:34:24Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Centralized all ConspitLink restart logic into restart_conspit_link_hardened() replacing the old restart_conspit_link()
- Config backup validates JSON before overwriting .bak (prevents corrupt backup chain)
- Post-restart verification checks all 3 config files + runtime Global.json with auto-restore from .bak
- SESSION_END_IN_PROGRESS flag prevents watchdog/session-end race condition
- 9 unit tests covering crash count, backup skip/copy, verify restore, and session-end guard

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement hardened restart with backup/verify/crash-count/minimize-retry** - `ef339f0` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ffb_controller.rs` - Added hardened restart, backup, verify, crash count, minimize retry, SESSION_END_IN_PROGRESS flag, 9 unit tests

## Decisions Made
- backup_conspit_configs() validates JSON before overwriting .bak -- prevents Pitfall 2 (corrupt overwrites good backup)
- verify_conspit_configs() auto-restores from .bak on corruption, returns false only if no backup exists
- Crash count increments only on watchdog path (is_crash_recovery=true), not session-end
- Single post-restart thread handles minimize-retry then verify (not two separate threads like old code)
- Testable _impl functions accept Option<&Path> base_dir, with #[cfg(test)] pub wrappers for unit tests

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- restart_conspit_link_hardened() is ready for Plan 02 to wire into ensure_conspit_link_running() watchdog
- SESSION_END_IN_PROGRESS flag is ready for watchdog to check before restarting CL
- All unit tests pass, release build clean

## Self-Check: PASSED

- ffb_controller.rs: FOUND
- 58-01-SUMMARY.md: FOUND
- Commit ef339f0: FOUND

---
*Phase: 58-conspitlink-process-hardening*
*Completed: 2026-03-20*
