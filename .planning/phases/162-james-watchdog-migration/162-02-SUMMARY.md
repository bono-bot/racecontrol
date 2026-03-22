---
phase: 162-james-watchdog-migration
plan: "02"
subsystem: rc-watchdog
tags: [watchdog, james-monitor, task-scheduler, deploy, registration, windows]

dependency_graph:
  requires:
    - phase: 162-01
      provides: rc-watchdog binary with james_monitor mode built and tested
  provides:
    - rc-watchdog.exe deployed to deploy-staging on James (.27)
    - register-james-watchdog.bat one-shot registration script
    - CommsLink-DaemonWatchdog Task Scheduler task running rc-watchdog.exe every 2 min
    - watchdog-state.json created at C:\Users\bono\.claude\watchdog-state.json
  affects: [james-watchdog-migration, rc-watchdog, james-monitor]

tech-stack:
  added: []
  patterns:
    - "One-shot Task Scheduler registration via schtasks /Create with /RU SYSTEM /RL HIGHEST"
    - "Dual registration: Task Scheduler (primary) + HKLM Run (boot-start fallback)"
    - "bat script uses goto labels for error handling (never parentheses in if/else per standing rules)"

key-files:
  created:
    - scripts/register-james-watchdog.bat
    - deploy-staging/rc-watchdog.exe
  modified: []

key-decisions:
  - "HKLM Run key requires admin elevation — Task Scheduler is primary persistence mechanism; HKLM Run failed on this run (acceptable)"
  - "Immediate schtasks /Run after registration provides first check-run verification without waiting 2 minutes"
  - "watchdog-state.json created with empty counts — all services healthy at time of first run"

patterns-established:
  - "Registration script pattern: delete old task silently, register new binary, add Run key, trigger immediate run"

requirements-completed: [JWAT-01]

duration: 25min
completed: "2026-03-22"
---

# Phase 162 Plan 02: James Watchdog Migration — Registration and Deployment Summary

**rc-watchdog.exe deployed to deploy-staging and registered as CommsLink-DaemonWatchdog Task Scheduler task on James (.27), replacing james_watchdog.ps1 with Rust-based monitoring running every 2 minutes**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-22T21:30:00+05:30
- **Completed:** 2026-03-22T22:00:00+05:30
- **Tasks:** 1 auto task + 1 checkpoint (human-verified)
- **Files modified:** 2

## Accomplishments

- Binary deployed: rc-watchdog.exe copied from target/release to deploy-staging (3.7MB, static CRT)
- Registration script created: scripts/register-james-watchdog.bat with CRLF line endings, no BOM, goto-label error handling
- Task Scheduler registered: CommsLink-DaemonWatchdog runs rc-watchdog.exe every 2 minutes as SYSTEM/HIGHEST
- First check run completed: watchdog-state.json created with empty counts (all 5 services healthy)
- HKLM Run key registration attempted — failed due to missing admin elevation (acceptable; Task Scheduler is primary)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create registration script and copy binary to deploy-staging** - `b7cad99c` (feat)

**Plan metadata:** pending (this summary commit)

## Files Created/Modified

- `scripts/register-james-watchdog.bat` - One-shot registration: delete old PS1 task, register rc-watchdog.exe, add HKLM Run key, trigger immediate run
- `deploy-staging/rc-watchdog.exe` - Deployed binary (3.7MB, static CRT, dual-mode: --service for pods, no-args for james_monitor)

## Decisions Made

- HKLM Run key failure (needs admin) is acceptable — Task Scheduler via SYSTEM account is the primary and sufficient persistence mechanism. The Run key is a secondary boot-start fallback; its absence does not affect 2-minute polling.
- watchdog-state.json created with empty counts confirms all 5 monitored services were healthy at first run (ollama, comms-link relay, kiosk, webterm, claude-code).

## Deviations from Plan

None — plan executed exactly as written. HKLM Run key failure was documented in the plan as acceptable (checkpoint verification noted it as such).

## Issues Encountered

- HKLM Run key registration failed (exit code non-zero) — requires Administrator elevation. Task Scheduler registration succeeded as SYSTEM which provides equivalent persistence. No fix required; documented in checkpoint outcome.

## User Setup Required

None — registration script was run directly by the user as the checkpoint action.

## Next Phase Readiness

- Phase 162 (james-watchdog-migration) is complete
- rc-watchdog.exe runs every 2 minutes as CommsLink-DaemonWatchdog on James (.27)
- james_watchdog.ps1 task has been replaced (deleted and re-registered as rc-watchdog.exe)
- recovery-log.jsonl receives james_monitor entries each run
- watchdog-state.json persists failure counts across invocations
- No blockers — ready for v17.1 phase completion

---
*Phase: 162-james-watchdog-migration*
*Completed: 2026-03-22*

## Self-Check: PASSED

| Item | Status |
|------|--------|
| scripts/register-james-watchdog.bat | FOUND (committed b7cad99c) |
| deploy-staging/rc-watchdog.exe | FOUND (committed b7cad99c) |
| Commit b7cad99c | FOUND |
