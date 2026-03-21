# Phase 70: Failback & Data Reconciliation - Research

**Researched:** 2026-03-21
**Domain:** Failover orchestration, session data reconciliation, FSM recovery transitions (Node.js/Rust)
**Confidence:** HIGH — all findings based on direct codebase analysis

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Recovery Detection (BACK-01)**
- HealthMonitor FSM already has 2-up threshold — when 2 consecutive probes succeed after being in `Down` state, emit `server_recovery` event
- Recovery detection is automatic — no manual confirmation from Uday required
- After `server_recovery` fires, wait 30s stabilization period before starting failback (prevents flapping)

**Session Data Merge (BACK-02)**
- During failover, billing sessions run on Bono VPS cloud racecontrol (SQLite DB on VPS)
- On recovery, James sends a `task_request` to Bono: "export failover sessions created since {failover_timestamp}"
- Bono queries cloud DB and returns as JSON payload via `task_response`
- James POSTs the session data to .23 via a new `POST /api/v1/sync/import-sessions` endpoint
- UUID strategy: import with `INSERT OR IGNORE` (if UUID already exists, skip; no overwrites)
- One-way sync: cloud to local for the failover window only
- If import fails: log error, notify Uday, but still proceed with pod switchback

**Failback Sequence (BACK-03)**
- Strict order:
  1. James detects recovery (2-up threshold)
  2. Wait 30s stabilization
  3. Re-probe .23 to confirm still up
  4. Send `task_request` to Bono: export failover sessions
  5. Import sessions to .23 via `/api/v1/sync/import-sessions`
  6. Broadcast `SwitchController { target_url: "ws://192.168.31.23:8080/ws/agent" }` via cloud racecontrol's broadcast endpoint
  7. Wait 30s for pods to reconnect
  8. Send `exec_request` to Bono: `deactivate_failover` (pm2 stop racecontrol)
  9. Notify Uday: all-clear + outage duration
- If step 4-5 fails: proceed to step 6 anyway
- If step 6 fails: retry 3 times, then alert Uday for manual intervention

**Outage Reporting (BACK-04)**
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

### Deferred Ideas (OUT OF SCOPE)
- Grafana dashboard for failover/failback history — Future requirement MON-01
- Automatic failback testing (scheduled .23 power-off drill) — Future
- Session data reconciliation UI for manual review — Future
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BACK-01 | Recovery detection: HealthMonitor FSM emits `server_recovery` when 2 consecutive probes succeed after `Down` state; 30s stabilization wait before starting failback sequence | HealthMonitor already tracks `#consecutiveSuccesses` and `UP_THRESHOLD=2`; `#updateState()` needs one new branch for `down -> healthy` transition |
| BACK-02 | Session data merge: James sends `task_request` to Bono for session export since failover timestamp; Bono responds with billing_sessions JSON; James POSTs to `.23 /api/v1/sync/import-sessions` using `INSERT OR IGNORE` | billing_sessions schema fully documented (26 columns); sync_push handler provides exact INSERT pattern to adapt; task_request/task_response protocol is wired in both james/index.js and bono/index.js |
| BACK-03 | Failback sequence: 9-step ordered sequence (detect, stabilize, re-probe, sync, import, broadcast, wait, deactivate, notify); mirrors FailoverOrchestrator.initiateFailover() structure | FailoverOrchestrator is the direct model to extend; `deactivate_failover` already in COMMAND_REGISTRY; failover_broadcast endpoint already exists for pod switchback; `#waitForExecResult()` pattern ready to reuse |
| BACK-04 | Outage reporting: WhatsApp + email notification with outage duration, session count, IST timestamp; `notify_failback` added to COMMAND_REGISTRY; 10-min cooldown | `notify_failover` entry in COMMAND_REGISTRY is the exact template to clone; buildSafeEnv() already passes Evolution API vars; send-email.js stdlib-only |
</phase_requirements>

---

## Summary

Phase 70 is the reverse of Phase 69. The infrastructure (HealthMonitor FSM, FailoverOrchestrator, COMMAND_REGISTRY, failover_broadcast endpoint) is already in place. Phase 70 adds three new capabilities: (1) recovery detection by extending the existing FSM, (2) session data reconciliation from cloud to local via a new import-sessions endpoint, and (3) pod switchback via a new `initiateFailback()` method on FailoverOrchestrator.

The most uncertain area is session data reconciliation — specifically how Bono responds to a `task_request` for session export. Currently, Bono's `task_request` handler in `bono/index.js` only logs and ACKs; it does NOT execute DB queries or return data via `task_response`. The session export capability must be built on Bono's side. The recommended approach is to use `exec_request` to run a sqlite3 CLI query on Bono's VPS, returning results as JSON stdout via the existing exec_result protocol.

The failback notification (`notify_failback`) requires adding one entry to COMMAND_REGISTRY and three changes total: the new `initiateFailback()` method, the `server_recovery` event emission in `health-monitor.js`, and the event handler wiring in `james/index.js`. The Rust side needs one new endpoint (`POST /api/v1/sync/import-sessions`) which is a structural copy of the sync_push billing_sessions block with `INSERT OR IGNORE`.

**Primary recommendation:** Add `server_recovery` to HealthMonitor, add `initiateFailback()` to FailoverOrchestrator (mirrors `initiateFailover()`), add `/api/v1/sync/import-sessions` endpoint to Rust service_routes (structural copy of sync_push billing_sessions block), and add `notify_failback` to COMMAND_REGISTRY.

---

## Standard Stack

### Core
| Library/Module | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `health-monitor.js` | Phase 69 | FSM with `#consecutiveSuccesses` counter | Already tracks UP_THRESHOLD=2, just needs `server_recovery` event |
| `failover-orchestrator.js` | Phase 69 | Orchestration state machine | `initiateFailback()` mirrors `initiateFailover()` exactly |
| `shared/exec-protocol.js` | Phase 69 | COMMAND_REGISTRY | `deactivate_failover` already exists; `notify_failback` is a clone of `notify_failover` |
| `shared/protocol.js` | Phase 66 | task_request, task_response, exec_request, exec_result message types | Established comms protocol |
| `routes.rs` service_routes() | Phase 69 | Axum service tier (terminal_secret auth) | `/sync/import-sessions` follows same auth pattern as `sync_push` |
| SQLx | 0.7 | Async SQLite queries | Already in use for all DB operations |

### Supporting
| Library/Module | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `shared/send-email.js` | Phase 69 | Stdlib-only email (sendmail + SMTP fallback) | Failback email notification to Uday |
| `bono/alert-manager.js` AlertCooldown | Phase 69 | 10-min notification cooldown | Reuse same cooldown instance in `initiateFailback()` |
| `node:crypto` randomUUID | Node.js builtin | execId generation | Already used in `initiateFailover()` |

---

## Architecture Patterns

### Recommended Project Structure

Changes touch exactly 4 files:

```
comms-link/
  james/
    health-monitor.js         ADD: server_recovery event in #updateState()
    failover-orchestrator.js  ADD: initiateFailback() method, store failoverStartedAt
    index.js                  ADD: server_recovery event handler wiring

  shared/
    exec-protocol.js          ADD: notify_failback + export_failover_sessions entries

crates/racecontrol/src/api/
  routes.rs                   ADD: import_sessions handler + route in service_routes()
```

### Pattern 1: FSM Recovery Transition

**What:** Add `server_recovery` event to HealthMonitor when transitioning from `down` to `healthy`.

The existing `#updateState()` method handles `down` to `healthy` via the `consecutiveFailures === 0 && prev !== 'healthy'` branch. It transitions to `healthy` but emits only `state_change`. Phase 70 adds `server_recovery` emission in this same transition with a `prev === 'down'` guard.

**Key detail:** The FSM check `prev !== 'healthy'` fires for BOTH `degraded to healthy` AND `down to healthy`. The guard `prev === 'down'` is mandatory to prevent spurious failback triggers on minor blip recovery.

```javascript
// In health-monitor.js #updateState() — extend the existing if (next !== prev) block
if (next !== prev) {
  this.#state = next;
  this.emit('state_change', { from: prev, to: next });

  if (next === 'down') {
    this.emit('server_down');
  }

  // Phase 70: recovery from full outage only — NOT for degraded->healthy
  if (next === 'healthy' && prev === 'down') {
    this.emit('server_recovery');
  }
}
```

### Pattern 2: FailoverOrchestrator.initiateFailback()

**What:** New public method on FailoverOrchestrator mirroring `initiateFailover()`.

**Critical details:**
- `failoverStartedAt` must be stored on the orchestrator when `initiateFailover()` runs (not in the HealthMonitor). Add `this.#failoverStartedAt = Date.now()` at the start of `initiateFailover()`.
- `initiateFailback()` uses `this.#failoverStartedAt` for the session export window.
- If `#failoverStartedAt` is null (Bono watchdog triggered failover without James), fall back to `Date.now() - 3_600_000` (1 hour ago). INSERT OR IGNORE handles any duplicates.

**Sequence sketch:**
```javascript
async initiateFailback() {
  // Cooldown check — same alertCooldown as initiateFailover()
  if (!this.#alertCooldown.canSend()) return;

  const recoveryTimestamp = Date.now();

  // Step 2: 30s stabilization
  await sleep(30_000);

  // Step 3: Re-probe .23 — abort if still down
  const stillUp = await this.#httpGet('http://192.168.31.23:8080/api/v1/health');
  if (!stillUp) return;

  // Step 4+5: Session export + import (see Session Export pattern below)

  // Step 6: Broadcast SwitchController back to .23
  // POST to cloud racecontrol broadcast endpoint with target_url = ws://192.168.31.23:8080/ws/agent
  // Retry up to 3 times (same pattern as initiateFailover broadcast)

  // Step 7: Wait 30s for pods to reconnect
  await sleep(30_000);

  // Step 8: Deactivate cloud racecontrol
  const deactivateExecId = randomUUID().slice(0, 8);
  this.#client.send('exec_request', {
    execId: `ex_${deactivateExecId}`,
    command: 'deactivate_failover',
    reason: 'failback: pods have reconnected to .23',
    requestedBy: 'james',
  });
  await this.#waitForExecResult(`ex_${deactivateExecId}`, 30_000).catch(() => null);

  // Step 9: Notify Uday
  this.#alertCooldown.recordSent();
  // exec_request notify_failback + email (fire-and-forget)
}
```

### Pattern 3: Session Export — exec_request Approach

**Recommendation: Use `exec_request` with sqlite3 CLI (not task_request).**

**Why exec_request wins:**
- `task_request` handler in `bono/index.js` only ACKs and logs (lines 203-215). It returns no data. Adding real data-return logic requires a new response protocol.
- `exec_request` + `exec_result` is the established pattern for getting data back from Bono. ExecHandler captures stdout and returns it in `exec_result`.
- A sqlite3 CLI query exports billing_sessions as JSON natively via the `-json` flag.

**New COMMAND_REGISTRY entry (add to shared/exec-protocol.js under Failover commands):**
```javascript
export_failover_sessions: {
  binary: 'sqlite3',
  args: [
    '/root/racecontrol/racecontrol.db',
    '-json',
    "SELECT id, driver_id, pod_id, pricing_tier_id, allocated_seconds, " +
    "driving_seconds, status, custom_price_paise, notes, started_at, ended_at, " +
    "created_at, experience_id, car, track, sim_type, split_count, " +
    "split_duration_minutes, wallet_debit_paise, discount_paise, coupon_id, " +
    "original_price_paise, discount_reason, pause_count, total_paused_seconds, " +
    "refund_paise FROM billing_sessions ORDER BY created_at ASC",
  ],
  tier: ApprovalTier.AUTO,
  timeoutMs: 30000,
  description: 'Export all billing sessions from cloud SQLite DB as JSON',
},
```

**Timestamp filtering:** COMMAND_REGISTRY args are static — dynamic timestamps cannot be injected. Export all sessions, then filter client-side in James: parse the JSON stdout array, drop entries where `created_at < failoverStartIso`. This is safe because the failover window produces few sessions in practice.

**Fallback if sqlite3 CLI unavailable:** Use `node -e` inline script with `require('better-sqlite3')` if the package is available in comms-link node_modules. Check availability with a health_check exec_request first.

### Pattern 4: import-sessions Endpoint (Rust)

**What:** New endpoint `POST /api/v1/sync/import-sessions` in `service_routes()`. Structural copy of the billing_sessions block in `sync_push` with `INSERT OR IGNORE` instead of `ON CONFLICT DO UPDATE`.

**Auth pattern — exact copy from sync_push (routes.rs lines 7175-7181):**
```rust
if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
    let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
    if provided != Some(secret) {
        return Json(json!({ "error": "Unauthorized" }));
    }
}
```

**INSERT pattern — use INSERT OR IGNORE:**
```rust
// Routes.rs line 7281 uses ON CONFLICT DO UPDATE — replace with INSERT OR IGNORE
"INSERT OR IGNORE INTO billing_sessions (
    id, driver_id, pod_id, pricing_tier_id,
    allocated_seconds, driving_seconds, status, custom_price_paise, notes,
    started_at, ended_at, created_at, experience_id, car, track, sim_type,
    split_count, split_duration_minutes,
    wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason,
    pause_count, total_paused_seconds, refund_paise)
 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
```

**Response format:**
```rust
Json(json!({
    "imported": imported,  // rows_affected() > 0
    "skipped": skipped,    // INSERT OR IGNORE skipped (UUID already exists)
    "synced_at": chrono::Utc::now().to_rfc3339(),
}))
```

**Request body:** `{ "sessions": [...array of session objects...] }`

**Route registration — add to service_routes():**
```rust
.route("/sync/import-sessions", post(import_sessions))
```

### Pattern 5: notify_failback COMMAND_REGISTRY Entry

Clone of `notify_failover`. Same Evolution API delivery inline script. EXEC_REASON contains the formatted all-clear message.

```javascript
notify_failback: {
  binary: 'node',
  args: ['-e', [
    "const https = require('https');",
    "const http = require('http');",
    "const reason = process.env.EXEC_REASON || 'FAILBACK COMPLETE';",
    "const url = process.env.EVOLUTION_URL;",
    "const instance = process.env.EVOLUTION_INSTANCE;",
    "const apiKey = process.env.EVOLUTION_API_KEY;",
    "const number = process.env.UDAY_WHATSAPP;",
    "if (!url || !number || !instance) { process.exit(1); }",
    "const body = JSON.stringify({ number, text: reason });",
    "const parsed = new URL('/message/sendText/' + instance, url);",
    "const transport = parsed.protocol === 'https:' ? https : http;",
    "const req = transport.request(parsed, { method: 'POST', headers: { 'Content-Type': 'application/json', apikey: apiKey } }, (res) => { res.resume(); process.exit(res.statusCode < 400 ? 0 : 1); });",
    "req.on('error', (e) => { process.exit(1); });",
    "req.setTimeout(10000, () => { req.destroy(new Error('timeout')); });",
    "req.write(body); req.end();",
  ].join(' ')],
  tier: ApprovalTier.AUTO,
  timeoutMs: 15000,
  description: 'Send failback all-clear WhatsApp notification to Uday via Evolution API',
},
```

EXEC_REASON format: `"FAILBACK COMPLETE — Server .23 recovered. Outage duration: {HH}h {MM}m. Sessions synced: {N}. All pods back on local server. Time: {IST}"`

### Pattern 6: Wiring in james/index.js

Two-line addition alongside the existing `server_down` handler (lines 595-600):

```javascript
healthMonitor.on('server_recovery', () => {
  console.log('[HEALTH-MONITOR] server_recovery event — triggering failback orchestration');
  failoverOrchestrator.initiateFailback().catch((err) => {
    console.error(`[HEALTH-MONITOR] initiateFailback error: ${err.message}`);
  });
});
```

Also: store `failoverStartedAt` on FailoverOrchestrator by adding `this.#failoverStartedAt = Date.now()` at the top of `initiateFailover()`. Declare `#failoverStartedAt = null` as a private field.

### Anti-Patterns to Avoid

- **task_request for session export:** Bono's task_request handler (bono/index.js lines 203-215) only ACKs — returns no data. Use exec_request with sqlite3 CLI instead.
- **ON CONFLICT DO UPDATE on import:** Cloud data must never overwrite local sessions. Use INSERT OR IGNORE exclusively.
- **Emitting server_recovery on degraded to healthy:** Must guard with `prev === 'down'`. Without the guard, every minor recovery triggers a full 70-second failback sequence.
- **Re-using cloud URL for pod switchback broadcast:** The failback broadcast target is `ws://192.168.31.23:8080/ws/agent` (local), not the cloud URL. Copy-paste from initiateFailover() broadcast code would use the wrong URL.
- **Blocking pod switchback on data sync failure:** Per CONTEXT.md, if session import fails, proceed to step 6 broadcast anyway. Pod reconnection is more important than perfect data.
- **Broadcasting to cloud racecontrol when it is already stopped:** Always probe cloud racecontrol health before the broadcast attempt. If it is stopped, run activate_failover first, then broadcast.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Session export from cloud SQLite | Custom HTTP endpoint on Bono | sqlite3 CLI via exec_request | ExecHandler already captures stdout |
| Recovery event debouncing | Custom timer logic | UP_THRESHOLD=2 already in HealthMonitor | Already implemented — just emit event |
| Notification delivery | WhatsApp API client | notify_failback in COMMAND_REGISTRY | Evolution API script from notify_failover |
| Pod switchback broadcast | New broadcast mechanism | Existing `POST /api/v1/failover/broadcast` | Same endpoint, different target_url |
| exec_result correlation | Callback map | `#waitForExecResult()` in FailoverOrchestrator | Already built and tested |

---

## Common Pitfalls

### Pitfall 1: FSM Emits server_recovery on degraded to healthy
**What goes wrong:** server_recovery fires on minor blip recovery, not just full outage recovery. initiateFailback() runs spuriously.
**Why it happens:** `prev !== 'healthy'` covers both degraded and down states.
**How to avoid:** Add `prev === 'down'` guard in the server_recovery emission. This is the single most important correctness constraint in Phase 70.

### Pitfall 2: failoverStartedAt Is Null When Bono Watchdog Triggered the Failover
**What goes wrong:** Both James and .23 went down (power outage). James's FailoverOrchestrator never ran initiateFailover(). `#failoverStartedAt` is null. Session export window defaults to "all time," returning thousands of historical sessions.
**How to avoid:** Fallback to `Date.now() - 3_600_000` (1 hour ago) when `#failoverStartedAt` is null. INSERT OR IGNORE handles any resulting duplicates safely.

### Pitfall 3: Broadcast Uses Cloud URL Instead of Local URL
**What goes wrong:** Copy-paste error from initiateFailover() uses `ws://100.70.177.44:8080/ws/agent` (cloud) instead of `ws://192.168.31.23:8080/ws/agent` (local server).
**How to avoid:** Both URLs must be explicit constants. The failback broadcast target is ALWAYS the local server URL.

### Pitfall 4: end_reason Column Missing from Local billing_sessions Schema
**What goes wrong:** cloud_sync.rs export includes `end_reason` but local racecontrol.db may not have this column. Import INSERT fails with schema mismatch error.
**Why it happens:** The existing sync_push handler (routes.rs line 7281) omits `end_reason` — schema drift.
**How to avoid:** Omit `end_reason` from the import INSERT. Follow the same precedent as sync_push. If the column needs to be included later, add a migration first.

### Pitfall 5: sqlite3 CLI Not Available on Bono VPS
**What goes wrong:** exec_request for export_failover_sessions fails with "sqlite3: command not found."
**How to avoid:** First implementation task should verify availability. Fallback: Node.js inline script using `better-sqlite3` npm package (check comms-link node_modules). If neither is available, escalate to Bono for package installation.

### Pitfall 6: Broadcast Fails If Cloud racecontrol Already Stopped
**What goes wrong:** If deactivate_failover was run (manually or by a previous partial failback), cloud racecontrol is already stopped. The broadcast POST to `100.70.177.44:8080` returns connection refused. Pods never receive SwitchController.
**How to avoid:** Probe cloud racecontrol health before step 6. If down, send activate_failover exec_request to restart it, then proceed with broadcast.

---

## Code Examples

Verified patterns from direct codebase analysis:

### HealthMonitor server_recovery emission
```javascript
// Source: health-monitor.js #updateState() — extend the existing if block (lines 150-159)
if (next !== prev) {
  this.#state = next;
  console.log(`[HEALTH] State: ${prev} -> ${next} (failures=${this.#consecutiveFailures})`);
  this.emit('state_change', { from: prev, to: next });

  if (next === 'down') {
    this.emit('server_down');
  }
  // Phase 70: only emit on full down->healthy recovery
  if (next === 'healthy' && prev === 'down') {
    this.emit('server_recovery');
  }
}
```

### import_sessions Rust handler skeleton
```rust
// Source: Adapted from routes.rs sync_push handler — billing_sessions block lines 7275-7330
// Key difference: INSERT OR IGNORE instead of ON CONFLICT DO UPDATE
async fn import_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    let mut imported = 0u64;
    let mut skipped = 0u64;

    if let Some(sessions) = body.get("sessions").and_then(|v| v.as_array()) {
        for s in sessions {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let result = sqlx::query(
                "INSERT OR IGNORE INTO billing_sessions (
                    id, driver_id, pod_id, pricing_tier_id,
                    allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                    started_at, ended_at, created_at, experience_id, car, track, sim_type,
                    split_count, split_duration_minutes, wallet_debit_paise, discount_paise,
                    coupon_id, original_price_paise, discount_reason,
                    pause_count, total_paused_seconds, refund_paise)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            )
            // .bind() all 26 values from JSON fields
            .execute(&state.db)
            .await;
            match result {
                Ok(r) if r.rows_affected() > 0 => imported += 1,
                Ok(_) => skipped += 1,
                Err(e) => tracing::warn!("[import-sessions] skip {}: {}", id, e),
            }
        }
    }

    Json(json!({ "imported": imported, "skipped": skipped,
                 "synced_at": chrono::Utc::now().to_rfc3339() }))
}
```

### Failback broadcast call (pod switchback)
```javascript
// Source: failover-orchestrator.js initiateFailover() lines 163-190 — exact retry pattern
// Only the target_url changes: local server instead of cloud
const resp = await this.#httpPost(
  'http://100.70.177.44:8080/api/v1/failover/broadcast',  // cloud racecontrol sends it
  JSON.stringify({ target_url: 'ws://192.168.31.23:8080/ws/agent' }),  // back to local
  { 'x-terminal-secret': TERMINAL_SECRET },
);
```

### Outage duration calculation
```javascript
// Both timestamps are stored on FailoverOrchestrator
const outageMs = recoveryTimestamp - (this.#failoverStartedAt || recoveryTimestamp);
const totalMinutes = Math.round(outageMs / 60_000);
const hours = Math.floor(totalMinutes / 60);
const minutes = totalMinutes % 60;
const outageFormatted = `${hours}h ${minutes}m`;
```

### deactivate_failover — already in COMMAND_REGISTRY
```javascript
// Source: shared/exec-protocol.js lines 120-126 — exists, no change needed
deactivate_failover: {
  binary: 'pm2',
  args: ['stop', 'racecontrol'],
  tier: ApprovalTier.NOTIFY,
  timeoutMs: 15000,
  description: 'Stop cloud racecontrol process (deactivate failover mode)',
},
```

---

## State of the Art

| Old Approach | Current Approach | Phase Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual failback (Uday restarts server, pods reconnect manually) | Automatic failback via HealthMonitor FSM + FailoverOrchestrator | Phase 70 | Zero-touch recovery |
| Session data lost during failover window | Cloud sessions imported to local DB via import-sessions | Phase 70 | No billing data loss |
| No outage duration tracking | failoverStartedAt stored on orchestrator, duration in notification | Phase 70 | Uday sees full incident report |

---

## Open Questions

1. **sqlite3 CLI availability on Bono VPS**
   - What we know: Bono VPS runs Linux (Ubuntu). sqlite3 CLI is a separate package from the SQLite library.
   - What is unclear: Is `sqlite3` installed? Is `better-sqlite3` available in comms-link node_modules?
   - Recommendation: First plan task should run a health_check exec_request to verify. Document both paths in the plan.

2. **end_reason column in local billing_sessions schema**
   - What we know: cloud_sync.rs export includes `end_reason`; sync_push INSERT in routes.rs omits it (schema drift).
   - What is unclear: Does the local .23 SQLite schema have this column?
   - Recommendation: Omit end_reason from the import INSERT, following the sync_push precedent. Safe — no data loss.

3. **Failback when Bono watchdog triggered failover (failoverStartedAt is null)**
   - What we know: If both James and .23 went down, `#failoverStartedAt` is null on the FailoverOrchestrator.
   - What is unclear: Should the fallback window extend beyond 1 hour?
   - Recommendation: 1h fallback is safe. INSERT OR IGNORE handles duplicates. Accept up to 1h of extra sessions in the import — all will be deduplicated.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Jest/Node (JS modules) / cargo test (Rust) |
| Config file | `package.json` test script in comms-link |
| Quick run command | `cargo test -p racecontrol` |
| Full suite command | `cargo test -p racecontrol && cd C:/Users/bono/racingpoint/comms-link && npm test` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BACK-01 | HealthMonitor emits `server_recovery` on `down` to `healthy` transition | Unit | `npm test -- health-monitor` | Wave 0 gap |
| BACK-01 | HealthMonitor does NOT emit `server_recovery` on `degraded` to `healthy` | Unit | Same | Wave 0 gap |
| BACK-02 | import_sessions inserts new sessions and returns `imported: N` | Integration | `cargo test -p racecontrol import_sessions` | Wave 0 gap |
| BACK-02 | import_sessions skips duplicate UUIDs and returns `skipped: M` | Integration | Same | Wave 0 gap |
| BACK-02 | import_sessions returns 401 for missing x-terminal-secret | Integration | Same | Wave 0 gap |
| BACK-03 | initiateFailback() sends deactivate_failover exec_request after broadcast | Unit (mock) | Manual verification | Wave 0 gap |
| BACK-04 | notify_failback exec_request sent with outage duration in EXEC_REASON | Unit (mock) | Manual verification | Wave 0 gap |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol` for Rust changes; unit test for the specific JS module changed
- **Per wave merge:** Full suite
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `comms-link/james/health-monitor.test.js` — BACK-01: server_recovery on down-to-healthy, NOT on degraded-to-healthy
- [ ] `crates/racecontrol/src/api/routes.rs` inline `#[cfg(test)] mod import_sessions_tests` — BACK-02: INSERT OR IGNORE behavior, auth check

---

## Sources

### Primary (HIGH confidence)
- `comms-link/james/health-monitor.js` — FSM state machine, UP_THRESHOLD, `#updateState()` method (direct read)
- `comms-link/james/failover-orchestrator.js` — `initiateFailover()` method, `#waitForExecResult()`, `#pending` map (direct read)
- `comms-link/shared/exec-protocol.js` — COMMAND_REGISTRY (deactivate_failover, notify_failover), buildSafeEnv() (direct read)
- `comms-link/shared/protocol.js` — task_request, task_response, exec_request, exec_result MessageTypes (direct read)
- `comms-link/bono/index.js` — task_request handler (ACK-only, confirmed no data return, lines 203-215) (direct read)
- `comms-link/james/index.js` — HealthMonitor wiring lines 589-601, exec_result routing to failoverOrchestrator line 377 (direct read)
- `crates/racecontrol/src/cloud_sync.rs` lines 311-340 — billing_sessions push query, 26 JSON columns (direct read)
- `crates/racecontrol/src/api/routes.rs` lines 7275-7330 — sync_push billing_sessions INSERT pattern (direct read)
- `crates/racecontrol/src/api/routes.rs` lines 11832-11884 — failover_broadcast endpoint auth + SwitchController dispatch (direct read)
- `crates/racecontrol/src/api/routes.rs` lines 348-382 — service_routes() structure (direct read)

### Secondary (MEDIUM confidence)
- `.planning/research/PITFALLS.md` — sync-before-accept principle; confirmed that data sync before pod switch is correct
- `.planning/research/ARCHITECTURE.md` — comms-link topology, terminal_secret auth pattern

---

## Metadata

**Confidence breakdown:**
- FSM changes (HealthMonitor): HIGH — `#updateState()` is simple and well-understood
- FailoverOrchestrator pattern: HIGH — `initiateFailback()` mirrors `initiateFailover()` structurally
- Session export via exec_request: MEDIUM — depends on sqlite3 availability on VPS (open question 1)
- import-sessions Rust endpoint: HIGH — structural copy of sync_push, all patterns known
- COMMAND_REGISTRY additions: HIGH — clone of existing entries
- Bono watchdog failback path: MEDIUM — no code changes on Bono side needed; timing behavior when James comes back online needs attention

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain, no external dependencies)
