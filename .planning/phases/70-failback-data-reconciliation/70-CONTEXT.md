# Phase 70: Failback & Data Reconciliation - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

When server .23 recovers from an outage, James detects recovery (2-up threshold), syncs billing sessions from Bono's cloud DB back to local .23 DB, broadcasts SwitchController to return all pods to .23, deactivates cloud racecontrol, and notifies Uday with an all-clear + outage duration report. This is the reverse of Phase 69's failover sequence.

</domain>

<decisions>
## Implementation Decisions

### Recovery Detection (BACK-01)
- The HealthMonitor FSM from Phase 69 already has the 2-up threshold — when 2 consecutive probes succeed after being in `Down` state, emit a `server_recovery` event
- Recovery detection is automatic — no manual confirmation from Uday required
- After `server_recovery` fires, wait 30s stabilization period before starting failback (prevents flapping if server reboots and immediately crashes again)

### Session Data Merge (BACK-02)
- During failover, billing sessions run on Bono's VPS cloud racecontrol (SQLite DB on VPS)
- On recovery, James sends a `task_request` to Bono: "export failover sessions created since {failover_timestamp}"
- Bono queries cloud DB for `billing_sessions WHERE created_at > {failover_start}` and returns as JSON payload via `task_response`
- James POSTs the session data to .23's racecontrol via a new `POST /api/v1/sync/import-sessions` endpoint
- UUID strategy: cloud sessions use different UUIDs from local — import with `INSERT OR IGNORE` (if UUID already exists, skip; no overwrites)
- This is a ONE-WAY sync: cloud → local for the failover window only. Regular `cloud_sync.rs` handles ongoing bidirectional sync
- If import fails: log error, notify Uday, but still proceed with pod switchback (sessions can be manually reconciled)

### Failback Sequence (BACK-03)
- Strict order:
  1. James detects recovery (2-up threshold)
  2. Wait 30s stabilization
  3. Re-probe .23 to confirm it's still up
  4. Send `task_request` to Bono: export failover sessions
  5. Import sessions to .23 via `/api/v1/sync/import-sessions`
  6. Broadcast `SwitchController { target_url: "ws://192.168.31.23:8080/ws/agent" }` to pods via cloud racecontrol's broadcast endpoint
  7. Wait 30s for pods to reconnect
  8. Send `exec_request` to Bono: `deactivate_failover` (pm2 stop racecontrol)
  9. Notify Uday: all-clear + outage duration
- If step 4-5 fails (data sync): proceed to step 6 anyway — pod reconnection is more important than perfect data
- If step 6 fails (broadcast): retry 3 times, then alert Uday for manual intervention

### Outage Reporting (BACK-04)
- Email + WhatsApp notification to Uday (reuse Phase 69 notification pattern)
- Content: "FAILBACK COMPLETE — Server .23 recovered. Outage duration: {HH:MM}. Sessions synced: {count}. All pods back on local server. Time: {IST timestamp}."
- Include session count and any sync errors in the notification
- Reuse `notify_failover` COMMAND_REGISTRY entry pattern — add `notify_failback` command
- Rate limit: same 10-min cooldown as failover notifications

### Claude's Discretion
- Exact implementation of the session export query on Bono's side (task_request handler or exec_request)
- Whether to use task_request (structured coordination) or exec_request (command execution) for session export
- The `/api/v1/sync/import-sessions` endpoint implementation details (batch insert vs row-by-row)
- How to store the failover start timestamp (in-memory on James's HealthMonitor or persisted)
- Stabilization period duration (30s recommended, adjustable)
- Whether Bono watchdog (Phase 69) also handles failback when it was the one that triggered failover

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Data Sync
- `crates/racecontrol/src/cloud_sync.rs` lines 18, 325-338 — SYNC_TABLES list, billing_sessions push query (exact columns to sync back)
- `crates/racecontrol/src/api/routes.rs` — existing `/sync/push` endpoint pattern (adapt for `/sync/import-sessions`)

### Failover Infrastructure (Phase 69)
- `C:/Users/bono/racingpoint/comms-link/james/health-monitor.js` — HealthMonitor FSM, add `server_recovery` event
- `C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js` — FailoverOrchestrator, add failback sequence method
- `C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js` — COMMAND_REGISTRY (add deactivate_failover already exists, add notify_failback)
- `C:/Users/bono/racingpoint/comms-link/shared/send-email.js` — email sender (reuse for failback notification)

### Comms-Link Coordination
- `C:/Users/bono/racingpoint/comms-link/shared/protocol.js` — task_request, task_response, exec_request message types
- `C:/Users/bono/racingpoint/comms-link/bono/index.js` — Bono exec handler, secondary watchdog

### Research
- `.planning/research/ARCHITECTURE.md` — session sync cloud→local is the most uncertain area
- `.planning/research/PITFALLS.md` — sync-before-accept, failback data integrity concerns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cloud_sync.rs` billing_sessions push query (lines 325-338) — reverse this direction for cloud→local import
- `FailoverOrchestrator` in failover-orchestrator.js — add `initiateFailback()` method alongside existing `initiateFailover()`
- `HealthMonitor` FSM — already has UP_THRESHOLD=2, just needs to emit `server_recovery` when transitioning from Down→Healthy
- `deactivate_failover` already in COMMAND_REGISTRY (Phase 66)
- `notify_failover` pattern (Phase 69-04) — clone for `notify_failback`

### Established Patterns
- task_request/task_response for structured coordination (Phase 66)
- exec_request for command execution on Bono VPS (Phase 66)
- SwitchController broadcast via POST /api/v1/failover/broadcast (Phase 69)
- WhatsApp + email notification pattern (Phase 69-04)

### Integration Points
- HealthMonitor `server_recovery` event → FailoverOrchestrator `initiateFailback()`
- James → Bono: task_request for session export OR exec_request for DB query
- James → .23: POST /api/v1/sync/import-sessions for session import
- Cloud racecontrol → pods: broadcast SwitchController with .23 URL
- James → Bono: exec_request deactivate_failover after pods switch back

</code_context>

<specifics>
## Specific Ideas

- The failback sequence is the REVERSE of failover — but data sync happens BEFORE pod switch (sync-before-accept)
- UUID conflicts between cloud and local DBs are handled by INSERT OR IGNORE — no overwrites, no duplicates
- The failover start timestamp should be stored in FailoverOrchestrator when failover fires — passed to Bono for session export window
- If Bono's secondary watchdog triggered the failover (not James), failback still goes through James when James comes back online
- Outage duration = recovery timestamp - failover timestamp (both stored in FailoverOrchestrator)

</specifics>

<deferred>
## Deferred Ideas

- Grafana dashboard for failover/failback history — Future requirement MON-01
- Automatic failback testing (scheduled .23 power-off drill) — Future
- Session data reconciliation UI for manual review — Future

</deferred>

---

*Phase: 70-failback-data-reconciliation*
*Context gathered: 2026-03-21*
