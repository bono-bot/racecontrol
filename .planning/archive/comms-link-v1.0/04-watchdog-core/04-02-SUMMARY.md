---
phase: 04-watchdog-core
plan: 02
subsystem: process-management
tags: [watchdog, task-scheduler, session-1, schtasks, entry-point]

# Dependency graph
requires:
  - phase: 04-watchdog-core
    provides: "ClaudeWatchdog class with detect/kill/restart lifecycle"
provides:
  - "Watchdog runner entry point for Task Scheduler invocation"
  - "One-time Task Scheduler registration script"
  - "Automatic watchdog startup on user login (Session 1)"
affects: [05-watchdog-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Task Scheduler /sc onlogon + /it for Session 1 process launch", "process.execPath + path.resolve for portable node paths"]

key-files:
  created:
    - james/watchdog-runner.js
    - scripts/register-watchdog.js
  modified: []

key-decisions:
  - "Old PowerShell watchdog (claude_watchdog.ps1) preserved until Phase 5 confirms stability"
  - "Task Scheduler uses /rl highest (elevated) + /it (interactive) for Session 1 launch"
  - "process.execPath and path.resolve used instead of hardcoded paths for portability"

patterns-established:
  - "Task Scheduler registration pattern: one-time script in scripts/ directory"
  - "Signal handling: SIGTERM/SIGINT for graceful shutdown, uncaughtException for crash logging"

requirements-completed: [WD-03]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 4 Plan 02: Watchdog Runner + Task Scheduler Summary

**Standalone watchdog-runner.js entry point with Task Scheduler registration (onlogon, Session 1, elevated) for automatic Claude Code crash recovery on boot**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T04:06:00Z
- **Completed:** 2026-03-12T04:11:59Z
- **Tasks:** 2 (1 auto + 1 human-verify checkpoint)
- **Files modified:** 2

## Accomplishments
- watchdog-runner.js imports ClaudeWatchdog, logs all events with ISO timestamps, handles graceful shutdown via SIGTERM/SIGINT
- register-watchdog.js creates 'CommsLink-Watchdog' scheduled task with /sc onlogon, /it (Session 1), /rl highest
- Task Scheduler verified: runs as user 'bono' (not SYSTEM), interactive only, at logon, enabled
- All 75 tests pass with zero regressions
- Human-verified: watchdog starts correctly and monitors Claude Code

## Task Commits

Each task was committed atomically:

1. **Task 1: Watchdog runner entry point + Task Scheduler registration script** - `ff46a80` (feat)
2. **Task 2: Verify watchdog end-to-end** - checkpoint:human-verify approved (no code change)

## Files Created/Modified
- `james/watchdog-runner.js` - Standalone entry point that imports ClaudeWatchdog, wires event logging, handles signals (55 lines)
- `scripts/register-watchdog.js` - One-time setup script that registers 'CommsLink-Watchdog' in Task Scheduler with onlogon trigger (83 lines)

## Decisions Made
- Old PowerShell watchdog (claude_watchdog.ps1) preserved until Phase 5 confirms new watchdog is stable -- avoids premature removal
- Task Scheduler uses /rl highest (elevated privileges) combined with /it (interactive token) to ensure Session 1 launch
- Used process.execPath and path.resolve instead of hardcoded paths for portability across Node.js installations

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - Task Scheduler registration is automated via `node scripts/register-watchdog.js`.

## Next Phase Readiness
- Phase 4 (Watchdog Core) is now complete: ClaudeWatchdog class + runner + Task Scheduler
- Ready for Phase 5 (Watchdog Hardening): escalating cooldown, self-test, WebSocket re-establishment, email notification
- Old PowerShell watchdog can be retired in Phase 5 after confirming new watchdog stability

## Self-Check: PASSED

- FOUND: james/watchdog-runner.js
- FOUND: scripts/register-watchdog.js
- FOUND: .planning/phases/04-watchdog-core/04-02-SUMMARY.md
- FOUND: ff46a80 (Task 1 commit)

---
*Phase: 04-watchdog-core*
*Completed: 2026-03-12*
