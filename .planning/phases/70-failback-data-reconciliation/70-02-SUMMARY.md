---
phase: 70-failback-data-reconciliation
plan: 02
subsystem: comms-link / failover orchestration
tags: [failback, health-monitor, failover-orchestrator, exec-protocol, comms-link]
dependency_graph:
  requires: [69-01, 69-04]
  provides: [server_recovery event, initiateFailback 9-step sequence, export_failover_sessions, notify_failback]
  affects: [james/index.js event wiring, bono exec-protocol COMMAND_REGISTRY]
tech_stack:
  added: []
  patterns: [exec_request/exec_result async pair, AlertCooldown reuse for failback suppression, fire-and-forget email on recovery]
key_files:
  created: []
  modified:
    - C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js
    - C:/Users/bono/racingpoint/comms-link/james/health-monitor.js
    - C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
    - C:/Users/bono/racingpoint/comms-link/james/index.js
decisions:
  - server_recovery uses prev === 'down' guard — prevents spurious failback on degraded->healthy; only full outage recovery triggers failback sequence
  - initiateFailback reuses same alertCooldown as initiateFailover — prevents double-trigger if recovery fires while failover cooldown is active
  - sync failure does NOT block pod switchback — sessions missed during export/import are logged as syncError in Uday notify message
  - #httpGet uses imported http module (top-level ESM import) rather than inline require — consistent with ESM file style
  - export_failover_sessions placed in AUTO tier — read-only sqlite3 query, no side effects
  - notify_failback placed in AUTO tier — consistent with notify_failover pattern
metrics:
  duration_minutes: 3
  completed_date: "2026-03-21"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 4
---

# Phase 70 Plan 02: Failback Orchestration Summary

**One-liner:** Full failback automation — HealthMonitor server_recovery event + FailoverOrchestrator 9-step sequence (stabilize, re-probe, session export/import, broadcast LOCAL target, deactivate cloud, notify Uday with outage duration).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add COMMAND_REGISTRY entries + HealthMonitor server_recovery event | fa8a0be | exec-protocol.js, health-monitor.js |
| 2 | Add initiateFailback() + wire server_recovery in james/index.js | e1826d5 | failover-orchestrator.js, index.js |

## What Was Built

### HealthMonitor (health-monitor.js)
- Added `server_recovery` event emission in `#updateState()` with mandatory `prev === 'down'` guard
- Event fires only on down-to-healthy transition, not on degraded-to-healthy
- Updated JSDoc to document the new event

### COMMAND_REGISTRY (exec-protocol.js)
- `export_failover_sessions`: sqlite3 -json full billing_sessions dump from cloud DB at /root/racecontrol/racecontrol.db
- `notify_failback`: Evolution API WhatsApp node -e script identical in structure to notify_failover but with FAILBACK COMPLETE default reason

### FailoverOrchestrator (failover-orchestrator.js)
- Added `import http from 'node:http'` at top (ESM-compatible)
- Added `#failoverStartedAt = null` private field
- Records `this.#failoverStartedAt = Date.now()` as first action in `initiateFailover()` (after cooldown check)
- Added `initiateFailback()` async method implementing the 9-step sequence:
  1. Cooldown check (reuses alertCooldown — prevents retriggering)
  2. 30s stabilization wait
  3. Re-probe `http://192.168.31.23:8080/api/v1/health` via `#httpGet`
  4. Send `export_failover_sessions` exec_request to Bono
  5. Filter sessions to failover window, POST to `.23/api/v1/sync/import-sessions`
  6. Verify cloud racecontrol health; reactivate if stopped; broadcast `SwitchController` with `target_url: ws://192.168.31.23:8080/ws/agent` to cloud endpoint
  7. Wait 30s for pods to reconnect
  8. Send `deactivate_failover` exec_request (pm2 stop racecontrol on cloud)
  9. Notify Uday via `notify_failback` exec_request + fire-and-forget email
- Added `#httpGet(url)` private method (5s timeout, returns true on 2xx)
- Resets `#failoverStartedAt = null` after failback completes

### Event Wiring (james/index.js)
- Added `healthMonitor.on('server_recovery', ...)` handler calling `initiateFailback()`
- Updated startup log from `~60s` to `DOWN_THRESHOLD=12, UP_THRESHOLD=2`

## Key Design Decisions

**server_recovery guard:** `prev === 'down'` is mandatory. The condition `consecutiveFailures === 0 && prev !== 'healthy'` matches BOTH `degraded` and `down`. Without the inner guard, every 1-tick latency spike recovery would trigger a full failback including pm2 stop on cloud racecontrol.

**Sync failure non-blocking:** If `export_failover_sessions` fails or returns empty, the system logs the error in `syncError` and continues with pod switchback. Uday receives the sync warning in the WhatsApp/email notification. Pods switching back to local is more important than perfect session reconciliation.

**Broadcast endpoint vs target:** The broadcast HTTP POST goes to `http://100.70.177.44:8080/api/v1/failover/broadcast` (cloud racecontrol endpoint) but the `target_url` payload is `ws://192.168.31.23:8080/ws/agent` (local server). Cloud racecontrol fans this out to all connected pods via SwitchController.

**Outage duration calculation:** Uses `recoveryTimestamp - failoverStartedAt`. If `failoverStartedAt` is null (edge case: james restarted during outage), falls back to 1 hour ago for session filtering.

## Deviations from Plan

None — plan executed exactly as written.

## Verification Results

All 4 files pass `node --check` syntax validation:
- `node --check shared/exec-protocol.js` — PASSED
- `node --check james/health-monitor.js` — PASSED
- `node --check james/failover-orchestrator.js` — PASSED
- `node --check james/index.js` — PASSED

All acceptance criteria verified:
- `export_failover_sessions` in COMMAND_REGISTRY — CONFIRMED
- `notify_failback` in COMMAND_REGISTRY — CONFIRMED
- `server_recovery` emitted with `prev === 'down'` guard — CONFIRMED
- `initiateFailback()` method defined — CONFIRMED (3 references)
- `#failoverStartedAt` tracked in `initiateFailover()` — CONFIRMED
- Broadcast target_url is LOCAL (`ws://192.168.31.23:8080/ws/agent`) — CONFIRMED
- `server_recovery` handler wired in james/index.js — CONFIRMED

## Self-Check: PASSED

Files exist:
- FOUND: C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js
- FOUND: C:/Users/bono/racingpoint/comms-link/james/health-monitor.js
- FOUND: C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
- FOUND: C:/Users/bono/racingpoint/comms-link/james/index.js

Commits exist:
- fa8a0be: feat(70-02): add COMMAND_REGISTRY entries + HealthMonitor server_recovery event
- e1826d5: feat(70-02): add initiateFailback() to FailoverOrchestrator + wire server_recovery in index.js
