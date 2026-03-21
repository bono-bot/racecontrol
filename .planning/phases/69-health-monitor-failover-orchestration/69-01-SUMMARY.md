---
phase: 69-health-monitor-failover-orchestration
plan: 01
subsystem: comms-link/james
tags: [health-monitor, failover, fsm, hysteresis, orchestration]
dependency_graph:
  requires: []
  provides: [HLTH-01, HLTH-02, HLTH-03, ORCH-01, ORCH-04]
  affects: [comms-link/james/index.js, Phase 70 failback]
tech_stack:
  added: []
  patterns:
    - EventEmitter FSM with hysteresis counters (consecutive failures/successes)
    - Pending exec-promise map for correlating exec_request/exec_result by execId
    - node:http.get with explicit timeout + res.resume() drain pattern
key_files:
  created:
    - C:/Users/bono/racingpoint/comms-link/james/health-monitor.js
    - C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/index.js
decisions:
  - ONE cycleOk boolean per 5s tick — consecutiveFailures increments by 1 per cycle, not per probe
  - Tailscale fallback (100.71.226.83) fires only when both LAN probes fail (conservative)
  - activateFailover continues after exec_result timeout — cloud pm2 start may succeed without James getting the result
  - notify_failover via exec_request to Bono (server .23 is down, can't use its email_alerts)
  - healthMonitor.stop() added to shutdown() to cleanly drain the probe interval
metrics:
  duration: "2 min"
  completed: "2026-03-21"
  tasks_completed: 2
  files_changed: 3
---

# Phase 69 Plan 01: Health Monitor & Failover Orchestration Summary

**One-liner:** HealthMonitor FSM with 12-tick/60s hysteresis on James (.27) probing server .23, wired to FailoverOrchestrator that sends activate_failover exec_request, waits for exec_result, polls cloud health, broadcasts SwitchController, and notifies Uday — all rate-limited by 10-min AlertCooldown.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create HealthMonitor FSM with hysteresis probe loop | 0849580 | james/health-monitor.js (new) |
| 2 | Create FailoverOrchestrator and wire into james/index.js | 4545729 | james/failover-orchestrator.js (new), james/index.js |

## What Was Built

### Task 1: HealthMonitor (`james/health-monitor.js`)

- `HealthMonitor extends EventEmitter` with private state `'healthy' | 'degraded' | 'down'`
- Constants: `PROBE_INTERVAL_MS=5000`, `PROBE_TIMEOUT_MS=3000`, `DOWN_THRESHOLD=12`, `UP_THRESHOLD=2`
- Probe cycle fires every 5s: LAN primary (`:8080/api/v1/health`) → LAN secondary (`:8090/ping`, only if primary fails) → Tailscale (`100.71.226.83:8090/ping`, only if both LAN fail)
- `cycleOk` is ONE boolean per tick — `consecutiveFailures` always increments by exactly 1 per 5s, guaranteeing 12 × 5s = 60s to `Down`
- A single `cycleOk=true` resets `consecutiveFailures` to 0 (conservative — avoids false failover from AC-launch CPU spikes)
- Emits `state_change { from, to }` on every transition; emits `server_down` exactly once on first entry into `'down'`
- `node:http.get` with `timeout` option + `res.resume()` body drain — no fetch/undici

### Task 2: FailoverOrchestrator (`james/failover-orchestrator.js`)

- `FailoverOrchestrator` constructor: `{ client, httpPost, alertCooldown }`
- `initiateFailover()` orchestration sequence:
  1. `alertCooldown.canSend()` gate — suppresses within 10-min window
  2. `activate_failover` exec_request → Bono (pm2 start racecontrol on VPS)
  3. Await exec_result via pending Map keyed by execId (30s timeout, continues on timeout)
  4. Poll cloud health via `racecontrol_health` exec_request (3 attempts × 5s)
  5. POST to `http://100.70.177.44:8080/api/v1/failover/broadcast` with `x-terminal-secret` header (3 attempts × 5s)
  6. `alertCooldown.recordSent()`
  7. `notify_failover` exec_request → Bono with IST timestamp + pod count
- `handleExecResult(payload)` resolves pending awaiter by execId

### james/index.js wiring

- Imports: `HealthMonitor`, `FailoverOrchestrator`, `AlertCooldown` (from `../bono/alert-manager.js`)
- After `configWatcher.start()`: creates `failoverCooldown`, `failoverOrchestrator`, `healthMonitor`, wires `server_down` listener, calls `healthMonitor.start()`
- `exec_result` handler: existing logging preserved + `failoverOrchestrator.handleExecResult(msg.payload)` added
- `shutdown()`: `healthMonitor.stop()` added before relayServer.close()

## Verification Results

```
node --check james/health-monitor.js       → SYNTAX_OK
node --check james/failover-orchestrator.js → SYNTAX_OK
node --check james/index.js               → SYNTAX_OK

node -e "import('./james/health-monitor.js').then(m => { const h = new m.HealthMonitor(); console.log(h.state); h.stop(); console.log('OK'); })"
→ state: healthy
→ OK

grep DOWN_THRESHOLD james/health-monitor.js → const DOWN_THRESHOLD = 12
grep activate_failover failover-orchestrator.js → command: 'activate_failover'
grep failover/broadcast failover-orchestrator.js → 'http://100.70.177.44:8080/api/v1/failover/broadcast'
grep server_down james/index.js → healthMonitor.on('server_down', ...)
```

## Decisions Made

1. **ONE cycleOk boolean per tick** — `consecutiveFailures++` once per 5s cycle, not per probe attempt. This is critical: if each of the 3 probe attempts counted separately, 12 "failures" would only be 20s, not 60s.
2. **Continue after exec_result timeout** — If Bono's pm2 start runs but the exec_result message doesn't arrive in 30s (network hiccup), the orchestration continues to the broadcast step rather than aborting. pm2 may have succeeded.
3. **notify_failover via exec_request to Bono** — Server .23 is down, so James cannot use .23's email_alerts. Delegation to Bono (who handles `notify_failover` commands) is the correct path.
4. **healthMonitor.stop() in shutdown()** — Prevents the setInterval from holding the event loop open after SIGTERM/SIGINT.
5. **UP_THRESHOLD=2 stored but not acted on** — Phase 70 (failback) will use it; declared in health-monitor.js as a constant so it's visible in the codebase without being wired yet.

## Deviations from Plan

None — plan executed exactly as written. The one discretionary choice (whether to add `healthMonitor.stop()` to shutdown) was taken as a correctness requirement (Rule 2).

## Self-Check: PASSED

- `C:/Users/bono/racingpoint/comms-link/james/health-monitor.js` — created, 187 lines
- `C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js` — created, 219 lines
- `C:/Users/bono/racingpoint/comms-link/james/index.js` — modified
- Commit 0849580 (Task 1) — confirmed in git log
- Commit 4545729 (Task 2) — confirmed in git log
- Both pushed to github.com/james-racingpoint/comms-link main
