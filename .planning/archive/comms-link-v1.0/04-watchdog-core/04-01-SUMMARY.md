---
phase: 04-watchdog-core
plan: 01
subsystem: process-management
tags: [watchdog, tasklist, taskkill, spawn, eventemitter, tdd]

# Dependency graph
requires:
  - phase: 01-websocket-connection
    provides: "EventEmitter + DI + private fields conventions"
provides:
  - "ClaudeWatchdog class with detect/kill/restart lifecycle"
  - "findClaudeExe standalone export for version discovery"
affects: [04-02 watchdog-runner, 05-watchdog-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Polling loop with restart guard flag", "DI for all process-interaction functions", "try/finally for flag cleanup"]

key-files:
  created:
    - james/watchdog.js
    - test/watchdog.test.js
  modified: []

key-decisions:
  - "Null detection treated as running -- graceful degradation, no restart on tasklist failure"
  - "2-second delay between kill and spawn for OS handle cleanup"
  - "3-second post-spawn verification to detect immediate process death"
  - "try/finally ensures #restarting flag is always cleared, even on unexpected errors"

patterns-established:
  - "Restart guard: boolean flag prevents concurrent restart sequences"
  - "Semver directory sorting: split('.').map(Number) for version comparison"

requirements-completed: [WD-01, WD-02]

# Metrics
duration: 4min
completed: 2026-03-12
---

# Phase 4 Plan 01: ClaudeWatchdog Summary

**ClaudeWatchdog class with 3-second polling, zombie tree kill, detached spawn, and latest-version discovery via DI-injectable functions**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T03:56:49Z
- **Completed:** 2026-03-12T04:00:48Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments
- ClaudeWatchdog class detecting Claude crashes within one 3-second poll cycle
- Full restart sequence: kill zombies -> 2s delay -> find exe -> spawn detached -> 3s verify
- Restart guard prevents double-restart when poll fires during in-progress restart
- findClaudeExe discovers latest version directory via semver sort (handles Claude auto-updates)
- 18 new tests covering detection, kill ordering, restart, version discovery, restart guard, lifecycle
- All 75 tests pass (57 existing + 18 new, zero regressions)

## Task Commits

Each task was committed atomically (TDD):

1. **RED: Failing tests for ClaudeWatchdog** - `b568438` (test)
2. **GREEN: Implement ClaudeWatchdog passing all tests** - `63ba0de` (feat)

_No REFACTOR commit needed -- implementation was clean on first pass._

## Files Created/Modified
- `james/watchdog.js` - ClaudeWatchdog class + findClaudeExe export (229 lines)
- `test/watchdog.test.js` - 18 unit tests across 5 describe blocks (547 lines)

## Decisions Made
- Null detection (tasklist error/timeout) treated as running -- avoids false-positive restarts when OS is busy
- 2-second delay between kill and spawn gives Windows time to release process handles
- 3-second post-spawn verification catches immediate process death (restart_failed with reason 'process_died_after_spawn')
- try/finally for #restarting flag cleanup ensures the watchdog never gets permanently stuck

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Mock timer test for 2-second delay initially used Date.now() timestamp comparison, which doesn't work reliably with node:test mock timers (Date.now() doesn't advance with tick). Fixed by using behavioral verification (spawn not called at 1s, called at 2s).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- ClaudeWatchdog ready for integration in 04-02 (watchdog-runner.js + Task Scheduler registration)
- findClaudeExe exported as standalone for reuse by watchdog-runner.js
- All DI functions have sensible defaults for production use

## Self-Check: PASSED

- FOUND: james/watchdog.js
- FOUND: test/watchdog.test.js
- FOUND: .planning/phases/04-watchdog-core/04-01-SUMMARY.md
- FOUND: b568438 (RED commit)
- FOUND: 63ba0de (GREEN commit)

---
*Phase: 04-watchdog-core*
*Completed: 2026-03-12*
