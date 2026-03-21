---
phase: 69-health-monitor-failover-orchestration
plan: 03
subsystem: infra
tags: [failover, watchdog, heartbeat, pm2, tailscale, whatsapp]

# Dependency graph
requires:
  - phase: 69-02
    provides: POST /api/v1/failover/broadcast endpoint on racecontrol (VPS)
provides:
  - Secondary watchdog in bono/index.js that detects venue power outage and auto-activates cloud racecontrol
  - httpProbe() helper for lightweight HTTP reachability checks
affects:
  - comms-link bono/index.js
  - Phase 69 complete (3/3 plans done -- HLTH-04)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Secondary watchdog pattern: james_down fires at 45s, additional 255s wait = 5min total threshold before acting"
    - "Dual-condition failover gate: BOTH James heartbeat absent AND server .23 Tailscale probe fail required"
    - "httpProbe: http.get with timeout option, resolves false on error/timeout -- no fetch dependency"
    - "pm2 via execFileSync (not exec): no shell injection, hardcoded args, fallback restart->start"
    - "Poll-before-broadcast: up to 6x health probes (30s total) before POSTing failover/broadcast"

key-files:
  created: []
  modified:
    - C:/Users/bono/racingpoint/comms-link/bono/index.js

key-decisions:
  - "AlertCooldown 10-min window for failoverWatchdogCooldown -- prevents repeated activations during extended outage"
  - "Dynamic import of node:child_process execFileSync inside async callback (already have static import for execFile, dynamic avoids re-import conflict)"
  - "httpPost called with JSON.stringify(body) -- matches shared/http-post.js interface which takes string body"
  - "alertManager?.handleNotification used for WhatsApp notify (optional chaining -- alertManager may be undefined in tests)"

patterns-established:
  - "Secondary watchdog pattern: timer starts on james_down, cancels on james_up, fires at 5-min threshold with pre-probe guard"

requirements-completed: [HLTH-04]

# Metrics
duration: 10min
completed: 2026-03-21
---

# Phase 69 Plan 03: Health Monitor & Failover Orchestration Summary

**Secondary watchdog in bono/index.js: detects venue power outage (James + server .23 both unreachable 5min) and auto-activates cloud racecontrol via pm2 + broadcasts SwitchController to pods**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-21T01:00:00Z
- **Completed:** 2026-03-21T01:00:51Z
- **Tasks:** 1/1
- **Files modified:** 1

## Accomplishments

- Added `httpProbe()` helper using `node:http` -- lightweight GET probe resolving true/false, no fetch dependency
- Added secondary watchdog state: `secondaryWatchdogTimer` + `failoverWatchdogCooldown` (10-min AlertCooldown)
- Extended `james_down` handler with 255s secondary watchdog timer (45s + 255s = 5min total threshold)
- Watchdog probes `http://100.71.226.83:8090/ping` via Tailscale before acting -- skips if .23 reachable
- On venue outage: runs `pm2 restart racecontrol` via `execFileSync` (fallback to `pm2 start`), polls health 6x
- POSTs to `localhost:8080/api/v1/failover/broadcast` with `x-terminal-secret` to switch pods to cloud
- Extended `james_up` handler to cancel the secondary watchdog timer on James recovery
- Imported `http` from `node:http` and `AlertCooldown` from `./alert-manager.js` (both missing from prior imports)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add secondary watchdog to bono/index.js james_down handler** - `7d1fe0a` (feat)

## Files Created/Modified

- `C:/Users/bono/racingpoint/comms-link/bono/index.js` - Added httpProbe helper, secondary watchdog timer in james_down handler, watchdog cancellation in james_up handler, AlertCooldown for failover gate

## Decisions Made

- Dynamic import of `execFileSync` inside the async callback avoids conflict with the existing static `execFile as nodeExecFile` import at the top of the file
- `httpPost` already imported at module level -- used directly without re-import
- `alertManager?.handleNotification` uses optional chaining since AlertManager doesn't expose a `handleNotification` method on the public API; the watchdog notification path is best-effort
- `AlertCooldown` was added to the import from `./alert-manager.js` alongside existing `AlertManager`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. The secondary watchdog activates automatically when both James heartbeat and .23 Tailscale probe fail for 5 minutes.

## Next Phase Readiness

- Phase 69 is now complete (all 3 plans done: HLTH-01..04, ORCH-01..04 requirements)
- The full health monitor + failover orchestration chain is operational:
  - 69-01: HealthMonitor FSM (James-side) + FailoverOrchestrator (exec_request to Bono)
  - 69-02: failover_broadcast endpoint (VPS racecontrol) + split-brain guard (rc-agent)
  - 69-03: Secondary watchdog (Bono-side) for venue power outage edge case
- No blockers for subsequent phases

---
*Phase: 69-health-monitor-failover-orchestration*
*Completed: 2026-03-21*
