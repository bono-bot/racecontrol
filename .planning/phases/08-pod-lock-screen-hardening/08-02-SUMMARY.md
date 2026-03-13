---
phase: 08-pod-lock-screen-hardening
plan: 02
subsystem: infra
tags: [watchdog, batch, scheduled-task, rc-agent, pod-deploy]

# Dependency graph
requires:
  - phase: 06-diagnosis
    provides: confirmed that rc-agent crash recovery was needed
provides:
  - deploy-staging/watchdog-rcagent.bat — minimal scheduled-task watchdog for rc-agent crash recovery
affects:
  - 08-03 (watchdog deployment to pods)
  - 09-edge-hardening (pod stability)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Scheduled-task watchdog: runs every 1min via schtasks, exits immediately (no loop), idempotent"
    - "tasklist /NH + find /i pattern for reliable process detection across Windows versions"

key-files:
  created:
    - deploy-staging/watchdog-rcagent.bat
  modified: []

key-decisions:
  - "Use scheduled task (not loop watchdog) so the watchdog exits cleanly and schtasks controls retry interval"
  - "Call start-rcagent.bat for restarts (existing script handles taskkill + start atomically)"
  - "Append to watchdog.log (not overwrite) to preserve restart history for debugging"
  - "Use tasklist /NH + find /i pattern (not /FI filter alone) for reliable errorlevel on all Windows versions"

patterns-established:
  - "Watchdog pattern: check-only script invoked by schtasks /SC MINUTE, no internal loop"

requirements-completed: [LOCK-03]

# Metrics
duration: 5min
completed: 2026-03-14
---

# Phase 8 Plan 02: RC-Agent Watchdog Script Summary

**Minimal scheduled-task watchdog for rc-agent that detects crashes via tasklist+find and restarts via start-rcagent.bat, logging events to C:\RacingPoint\watchdog.log**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-13T23:55:00Z
- **Completed:** 2026-03-13T23:59:00Z
- **Tasks:** 1 of 1
- **Files modified:** 1

## Accomplishments
- Created watchdog-rcagent.bat as a one-shot scheduled-task script (not a looping process)
- Script detects rc-agent.exe absence reliably using tasklist /NH + find /i (avoids /FI exit-code quirks)
- Calls existing start-rcagent.bat for restart (handles taskkill + 2s wait + start)
- Appends timestamped entries to C:\RacingPoint\watchdog.log when restart is triggered
- Idempotent: exits without action when rc-agent is already running

## Task Commits

Each task was committed atomically:

1. **Task 1: Create watchdog-rcagent.bat** - `04a9744` (chore)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `deploy-staging/watchdog-rcagent.bat` - One-shot watchdog for rc-agent; deployed as schtasks /SC MINUTE on each pod

## Decisions Made

**Scheduled task vs. looping watchdog:** Previous watchdog (deploy/watchdog-rc-agent.cmd) used `goto loop` with timeout. New design uses a one-shot script invoked by Windows Task Scheduler every minute. This is cleaner: no persistent process, OS controls the schedule, script exits immediately when rc-agent is running.

**30-second recovery analysis:** Scheduled task fires every 60s. Average case: 30s wait + 5s startup = ~35s. Worst case: 65s. The plan notes this and reserves the strict 30s decision for Plan 03 checkpoint. A second staggered task at +30s offset could close the gap if required.

**Call not Start:** Using `call C:\RacingPoint\start-rcagent.bat` so the watchdog waits for the startup script to complete before the watchdog task exits.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## Deployment Guide (for Pod-Agent)

To deploy watchdog to a pod:

**Step 1: Copy script to pod via HTTP server**
```
curl -s -X POST http://<pod_ip>:8090/exec -H "Content-Type: application/json" \
  -d "{\"cmd\": \"curl -s -o C:\\RacingPoint\\watchdog-rcagent.bat http://192.168.31.27:9998/watchdog-rcagent.bat\"}"
```

**Step 2: Create scheduled task (runs every 1 minute as SYSTEM)**
```
schtasks /create /TN "RCAgentWatchdog" /TR "C:\RacingPoint\watchdog-rcagent.bat" /SC MINUTE /MO 1 /RU SYSTEM /RL HIGHEST /F
```

**Step 3: Verify task was created**
```
schtasks /query /TN "RCAgentWatchdog" /FO LIST
```

**Note:** The HTTP server must be running on James's PC at port 9998:
`python3 -m http.server 9998 --directory /c/Users/bono/racingpoint/deploy-staging --bind 0.0.0.0`

However, the actual file is now in the racecontrol repo's deploy-staging/. Copy it to the external deploy-staging at `C:\Users\bono\racingpoint\deploy-staging\` before serving.

## User Setup Required

None — script is ready for deployment via pod-agent. Deployment is documented above and will be executed in Plan 03.

## Next Phase Readiness
- watchdog-rcagent.bat ready to deploy to all 8 pods via pod-agent
- Plan 03 will handle actual deployment and scheduled task creation on each pod
- No blockers

---
*Phase: 08-pod-lock-screen-hardening*
*Completed: 2026-03-14*
