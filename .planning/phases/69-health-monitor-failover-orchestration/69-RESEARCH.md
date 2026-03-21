# Phase 69: Health Monitor & Failover Orchestration - Research

**Researched:** 2026-03-21
**Domain:** Node.js health probe FSM + Rust HTTP endpoint + comms-link coordination
**Confidence:** HIGH

## Summary

Phase 69 is entirely in-project — no external libraries need research. All implementation
decisions are already locked in CONTEXT.md. The research task is to document the exact code
integration points so the planner produces tasks with precise file targets and line numbers.

The phase has three distinct components: (1) a Node.js health probe FSM added to
`comms-link/james/index.js`, (2) a new HTTP endpoint on Bono's racecontrol VPS to receive
the failover-activation POST and broadcast `SwitchController` to connected pods, and (3) a
LAN probe guard added to the rc-agent `SwitchController` handler at line 2572 of
`crates/rc-agent/src/main.rs`. The Bono secondary watchdog extends the existing
`HeartbeatMonitor` in `comms-link/bono/heartbeat-monitor.js`.

A critical gap discovered during research: there is **no existing HTTP endpoint on racecontrol
that broadcasts `SwitchController` to all connected agents**. The message type exists in
`rc-common/protocol.rs` and is handled by rc-agent, but no route in
`crates/racecontrol/src/api/routes.rs` sends it. Phase 69 must add this endpoint to Bono's
cloud racecontrol. The broadcast mechanism to use is `state.agent_senders` — the same
`mpsc::Sender<CoreToAgentMessage>` map used by billing, ac_server, and other racecontrol
modules.

**Primary recommendation:** Add the health probe loop as a new module `james/health-monitor.js`
(not inline in `james/index.js`) to keep the relay server file manageable. Wire it into
`james/index.js` the same way `ConfigWatcher` is wired — as a standalone module that emits
events. The rest of the orchestration flows through existing comms-link primitives.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Health Probe Loop (HLTH-01, HLTH-03)**
- Runs on James (.27) in comms-link (NOT on .23)
- Probes `http://192.168.31.23:8080/api/v1/health` every 5s (primary) and `http://192.168.31.23:8090/ping` (secondary) — both must fail for the cycle to count as down
- Tailscale fallback probe: `http://100.71.226.83:8090/ping` if LAN fails
- Probe timeout: 3s per request
- Tailscale fallback used to disambiguate total outage vs LAN-only failure

**Hysteresis State Machine (HLTH-02, HLTH-03)**
- Reuse cloud_sync.rs pattern — consecutive failure/success counters
- DOWN threshold: 12 consecutive failures at 5s = 60s sustained outage
- UP threshold: 2 consecutive successes (Phase 70 failback — not in scope here)
- States: Healthy → Degraded (1-11) → Down (12+, triggers failover)
- Single success resets failure counter to 0 (conservative, no false positives)
- Log every state transition

**Failover Orchestration Sequence (ORCH-01, ORCH-02)**
1. James sends `exec_request` with `activate_failover` command (or `task_request`) to Bono via comms-link
2. Bono runs `pm2 start racecontrol` on VPS (`cwd: /root/racecontrol`)
3. James waits for `task_response` / `exec_result` from Bono
4. James POSTs to cloud racecontrol broadcast endpoint (new endpoint)
5. Cloud racecontrol broadcasts `SwitchController { target_url: "ws://100.70.177.44:8080/ws/agent" }`
6. Retry steps 4-5 up to 3 times with 5s intervals if cloud not ready

**Pod-Side Split-Brain Guard (ORCH-03)**
- In rc-agent SwitchController handler (line 2572 of `crates/rc-agent/src/main.rs`), before the URL write: HTTP GET `http://192.168.31.23:8090/ping` with 2s timeout
- .23 responds → reject switch, log "split-brain guard: .23 still reachable, ignoring switch"
- .23 no response → accept switch, proceed normally
- Per-pod independent decision, no quorum

**Bono Secondary Watchdog (HLTH-04)**
- Bono monitors heartbeat gap: if James heartbeat stops for 5 minutes, probe `http://100.71.226.83:8090/ping`
- If both James heartbeat AND server .23 are down → Bono auto-activates cloud racecontrol
- Implement in `bono/index.js` heartbeat-gap detection (timer on `monitor.on('james_down')`)

**Notifications (ORCH-04)**
- Email via existing `email_alerts.rs` shell-out pattern (`node send_email.js`)
- WhatsApp via existing Evolution API (`sendEvolutionText` in `comms-link/bono/alert-manager.js`)
- Content: "FAILOVER ACTIVATED — Server .23 unreachable. Pods switched to cloud (100.70.177.44). Time: {IST}. Pods connected: {count}/8."
- Send AFTER pods have switched (so count is accurate)
- Rate limit: max 1 failover notification per 10 minutes

### Claude's Discretion
- Whether health probe runs as a standalone module or inline in james/index.js
- Exact hysteresis counter implementation (class vs plain object)
- Whether to use existing racecontrol SwitchController broadcast or build a new endpoint on Bono's VPS
- Retry strategy for failed SwitchController broadcasts
- WhatsApp message formatting (plain text vs template)
- Bono watchdog implementation details (timer-based vs heartbeat-gap detection)

### Deferred Ideas (OUT OF SCOPE)
- Failback (server recovery detection + switch back to .23) — Phase 70
- Session data reconciliation after failover — Phase 70
- Grafana dashboard for failover events — Future MON-01
- Automatic config sync on failover — already handled by Phase 67
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HLTH-01 | James probes server .23 every 5s via HTTP GET to detect outage | `health-monitor.js` module with setInterval, `node:http` or undici/fetch; probe both :8080/api/v1/health and :8090/ping; timeout via AbortController or manual socket timeout |
| HLTH-02 | Hysteresis FSM: 12 consecutive failures = Down (60s), 2 successes = Healthy | Plain object with `consecutiveFailures`, `consecutiveSuccesses`, state enum string; mirrors `cloud_sync.rs` RELAY_DOWN_THRESHOLD/UP_THRESHOLD pattern exactly |
| HLTH-03 | Tailscale fallback probe: if LAN fails, probe 100.71.226.83:8090/ping; if Tailscale also fails, real outage | Third probe in same cycle; only after both LAN probes fail; uses same 3s timeout |
| HLTH-04 | Bono secondary watchdog: if James heartbeat absent 5min AND .23 probe fails → auto-activate cloud | Add timer in `bono/index.js` `monitor.on('james_down')` handler; `HeartbeatMonitor` already emits `james_down` after 45s timeout; extend to 5min secondary check |
| ORCH-01 | James sends `exec_request` `activate_failover` to Bono; waits for `exec_result` confirming pm2 start | `activate_failover` already in `COMMAND_REGISTRY` in `exec-protocol.js` (line 112); `exec_request` flow fully established in `james/index.js` |
| ORCH-02 | James POSTs to cloud racecontrol `/api/v1/failover/broadcast`; cloud broadcasts SwitchController to all pods | New endpoint needed on racecontrol: reads `agent_senders`, iterates, sends `CoreToAgentMessage::SwitchController { target_url }`; James calls via `httpPost` utility |
| ORCH-03 | rc-agent split-brain guard: probe .23 before honoring SwitchController; reject if .23 responds | Insert `reqwest::Client::get("http://192.168.31.23:8090/ping").timeout(2s)` at line 2572 before `*active_url.write().await = target_url.clone()` |
| ORCH-04 | Notify Uday via email + WhatsApp after failover, with pod count; rate-limited 10min | Use `emailAlerter.send_alert()` pattern + `sendEvolutionText` from `comms-link/bono/alert-manager.js`; notification sent by James after receiving exec_result from Bono confirming cloud is up |
</phase_requirements>

---

## Standard Stack

### Core — No New Dependencies Required

All required libraries are already present in the project.

| Component | Location | Version | Purpose |
|-----------|----------|---------|---------|
| Node.js http module | `node:http` | stdlib | HTTP probes in James health monitor |
| comms-link protocol | `shared/protocol.js` | in-repo | `exec_request`, `task_request`, `exec_result` message types |
| exec-protocol | `shared/exec-protocol.js` | in-repo | `activate_failover` already in COMMAND_REGISTRY |
| reqwest (Rust) | `crates/rc-agent/Cargo.toml` | already present | HTTP probe for split-brain guard in rc-agent |
| sendEvolutionText | `comms-link/bono/alert-manager.js` | in-repo | WhatsApp notification to Uday |
| EmailAlerter | `crates/racecontrol/src/email_alerts.rs` | in-repo | Email notification pattern |
| HeartbeatMonitor | `comms-link/bono/heartbeat-monitor.js` | in-repo | Already emits `james_down` after 45s |

**No new npm packages or Cargo crates needed.**

---

## Architecture Patterns

### Pattern 1: Health Probe FSM (James — `comms-link/james/`)

Modeled exactly on `cloud_sync.rs` hysteresis (lines 122-162):

```javascript
// james/health-monitor.js
import { EventEmitter } from 'node:events';
import http from 'node:http';

const PROBE_INTERVAL_MS = 5_000;
const PROBE_TIMEOUT_MS = 3_000;
const DOWN_THRESHOLD = 12;   // 12 × 5s = 60s
const UP_THRESHOLD = 2;

// States: 'healthy' | 'degraded' | 'down'
export class HealthMonitor extends EventEmitter {
  #state = 'healthy';
  #consecutiveFailures = 0;
  #consecutiveSuccesses = 0;
  #timer = null;

  start() { this.#timer = setInterval(() => this.#probe(), PROBE_INTERVAL_MS); }
  stop() { clearInterval(this.#timer); }

  async #probe() {
    const lanOk = await this.#httpGet('http://192.168.31.23:8080/api/v1/health')
                  || await this.#httpGet('http://192.168.31.23:8090/ping');
    // If LAN fails, try Tailscale
    const tailscaleOk = lanOk ? true : await this.#httpGet('http://100.71.226.83:8090/ping');
    const cycleOk = lanOk || tailscaleOk; // down only if BOTH LAN and Tailscale fail

    if (cycleOk) {
      this.#consecutiveFailures = 0;
      this.#consecutiveSuccesses++;
    } else {
      this.#consecutiveSuccesses = 0;
      this.#consecutiveFailures++;
    }

    const prevState = this.#state;
    if (this.#state !== 'down' && this.#consecutiveFailures >= DOWN_THRESHOLD) {
      this.#state = 'down';
    } else if (this.#state === 'healthy' && this.#consecutiveFailures > 0) {
      this.#state = 'degraded';
    } else if (this.#consecutiveFailures === 0 && this.#state !== 'healthy') {
      this.#state = 'healthy';
    }

    if (prevState !== this.#state) {
      console.log(`[HEALTH] State: ${prevState} -> ${this.#state}`);
      this.emit('state_change', { from: prevState, to: this.#state });
    }
    if (this.#state === 'down' && prevState !== 'down') {
      this.emit('server_down');
    }
  }

  #httpGet(url) {
    return new Promise((resolve) => {
      const req = http.get(url, { timeout: PROBE_TIMEOUT_MS }, (res) => {
        resolve(res.statusCode >= 200 && res.statusCode < 500);
        res.resume(); // drain
      });
      req.on('error', () => resolve(false));
      req.on('timeout', () => { req.destroy(); resolve(false); });
    });
  }
}
```

### Pattern 2: Wiring Health Monitor into james/index.js

Use the same pattern as `ConfigWatcher` (lines 556-578 of `james/index.js`):

```javascript
// james/index.js — add after ConfigWatcher section
import { HealthMonitor } from './health-monitor.js';

const healthMonitor = new HealthMonitor();
healthMonitor.on('server_down', async () => {
  console.log('[HEALTH] Server .23 confirmed down — initiating failover');
  await initiateFailover();
});
healthMonitor.start();
```

`initiateFailover()` is a new async function in `james/index.js` that:
1. Sends `exec_request` with command `activate_failover` via the existing `client.send('exec_request', ...)` pattern
2. Waits for `exec_result` by checking `wss.on('exec_result', ...)` — or uses a Promise resolved by the exec_result handler
3. POSTs to cloud racecontrol broadcast endpoint
4. Sends notification

### Pattern 3: New Broadcast Endpoint on Cloud Racecontrol (Bono's VPS)

The endpoint iterates `state.agent_senders` — the same `HashMap<String, mpsc::Sender<CoreToAgentMessage>>` used everywhere:

```rust
// crates/racecontrol/src/api/routes.rs — new route handler
pub async fn failover_broadcast(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FailoverBroadcastRequest>,
) -> impl IntoResponse {
    let target_url = body.target_url;
    let agent_senders = state.agent_senders.read().await;
    let mut sent = 0usize;

    for (_pod_id, sender) in agent_senders.iter() {
        if sender.send(CoreToAgentMessage::SwitchController {
            target_url: target_url.clone(),
        }).await.is_ok() {
            sent += 1;
        }
    }

    tracing::info!("[failover] Broadcast SwitchController to {}/{} agents", sent, agent_senders.len());
    Json(serde_json::json!({ "ok": true, "sent": sent, "total": agent_senders.len() }))
}

#[derive(serde::Deserialize)]
pub struct FailoverBroadcastRequest {
    pub target_url: String,
}
```

Route registration pattern (same as other routes in routes.rs):
```rust
.route("/api/v1/failover/broadcast", post(failover_broadcast))
```

**Security:** Protect with `x-terminal-secret` header check (same middleware used on `/sync/push`). The endpoint is only called by James via internal network or Tailscale.

### Pattern 4: Split-Brain Guard in rc-agent (ORCH-03)

Insert before line 2584 (`*active_url.write().await = target_url.clone()`):

```rust
// crates/rc-agent/src/main.rs — inside SwitchController handler, after allowlist check
// Phase 69: Split-brain guard — verify .23 is actually unreachable before switching
let lan_probe = reqwest::Client::builder()
    .timeout(Duration::from_secs(2))
    .build()
    .ok()
    .and_then(|c| Some(c.get("http://192.168.31.23:8090/ping").send()));

let server_reachable = if let Some(probe) = lan_probe {
    matches!(probe.await, Ok(r) if r.status().is_success())
} else {
    false
};

if server_reachable {
    tracing::warn!(
        "[switch] split-brain guard: .23 still reachable, ignoring SwitchController to {}",
        target_url
    );
    // Do NOT break — stay on current connection
    // ... continue inner loop
} else {
    // .23 unreachable — proceed with switch
    tracing::info!("[switch] split-brain guard passed — .23 unreachable, accepting switch");
    *active_url.write().await = target_url.clone();
    // ... rest of existing switch logic
}
```

Note: `reqwest` is already in rc-agent's Cargo.toml (used in `remote_ops.rs`). No new dependency.

### Pattern 5: Bono Secondary Watchdog (HLTH-04)

`HeartbeatMonitor` emits `james_down` after 45s timeout. The secondary watchdog adds a 5-minute probe on top in `bono/index.js`:

```javascript
// bono/index.js — in wireBono() or isMainModule block
// After: monitor.on('james_down', ...) handler
monitor.on('james_down', async (evt) => {
  // Existing: alertManager.handleJamesDown(evt)
  // New: start secondary watchdog timer
  setTimeout(async () => {
    // If James still down after 5 minutes total
    if (!monitor.isUp) {
      const serverReachable = await httpProbe('http://100.71.226.83:8090/ping', 5000);
      if (!serverReachable) {
        console.log('[WATCHDOG] James AND server .23 both down — auto-activating cloud racecontrol');
        // Use bonoExecHandler or direct pm2 start — Bono is ON the VPS
        // pm2 start racecontrol directly, then POST to localhost:8080/api/v1/failover/broadcast
        await activateCloudRacecontrol();
      }
    }
  }, 5 * 60 * 1000 - 45_000); // 5min total from first miss, minus 45s already elapsed
});
```

`httpProbe` is a small helper identical to `HealthMonitor.#httpGet`. Since Bono IS the VPS,
`activate_failover` runs directly via local pm2 — no comms-link round-trip needed.

### Anti-Patterns to Avoid

- **Do NOT build quorum logic** — each pod independently decides. No counting connected pods before accepting switch.
- **Do NOT notify before broadcast** — pod count in message would be 0. Send notification after the broadcast POST returns.
- **Do NOT add hysteresis to Bono watchdog** — Bono's check is a one-shot secondary. James already has 60s hysteresis.
- **Do NOT create a new reqwest Client per probe** — create once outside the loop; reuse with `.clone()` if needed.
- **Do NOT use exec_request for Bono's own watchdog action** — Bono is already on the VPS; call pm2/HTTP directly.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Message delivery reliability | Custom ACK tracking | Existing `AckTracker` in `shared/ack-tracker.js` | Already handles retries, dedup, timeout |
| exec_request to Bono | Custom WS message | `activate_failover` in `COMMAND_REGISTRY` + `client.send('exec_request', ...)` | Registry enforces binary+args, approval tier=NOTIFY |
| WhatsApp notification | New HTTP client | `sendEvolutionText` in `comms-link/bono/alert-manager.js` | Already handles Evolution API, timeout, error |
| Email notification | New Node script | `emailAlerter.send_alert()` shell-out to `send_email.js` | Existing pattern in `email_alerts.rs` + racecontrol |
| HTTP probe timeout | Custom socket | `http.get` with `timeout` option + `req.on('timeout', ...)` | Native Node.js; no extra deps |
| Alert rate limiting | Custom timer | `AlertCooldown` class in `comms-link/bono/alert-manager.js` | `canSend()` + `recordSent()` already implemented |

---

## Common Pitfalls

### Pitfall 1: Probe Counts Both Paths as Separate Failures

**What goes wrong:** Coding the probe so that failure of `:8080/health` AND failure of `:8090/ping` each increment the failure counter by 1 (giving 2 per cycle) reaches DOWN_THRESHOLD=12 in only 6 cycles (30s) instead of 12 (60s).

**Why it happens:** Treating each HTTP call as an independent probe rather than a two-check guard for one cycle.

**How to avoid:** The cycle result is ONE boolean: `cycleOk = lanOk || tailscaleOk`. ONE failure count per 5s interval tick. The counter increments by 1 per tick, not per probe attempt.

### Pitfall 2: exec_result Handler Race Condition

**What goes wrong:** James sends `exec_request` for `activate_failover`, then immediately POSTs to the cloud broadcast endpoint before Bono's `exec_result` arrives. Cloud racecontrol is not yet running.

**Why it happens:** `exec_request` is fire-and-forget in the current `james/index.js` flow (line 504). The existing `exec_result` handler only logs (line 369). No promise-based waiting is wired.

**How to avoid:** Add a pending exec map in `initiateFailover()`: store a Promise resolve function keyed by `execId`. In the `exec_result` handler section (lines 369-374 of `james/index.js`), emit an event or resolve the promise when `exitCode === 0` for the failover execId. Only then POST to cloud broadcast.

### Pitfall 3: Split-Brain Guard Creates New reqwest Client Per Message

**What goes wrong:** Calling `reqwest::Client::new()` inside the SwitchController handler (inside the inner event loop) creates a new client on every message. This is harmless for a rare failover event but wasteful pattern-wise.

**How to avoid:** Create the probe client once before the outer reconnect loop and clone it:
```rust
let probe_client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build()
    .unwrap_or_default();
```
Then clone inside the handler: `probe_client.clone().get(...).send().await`.

### Pitfall 4: Bono Watchdog Fires on Routine James Restart

**What goes wrong:** James's comms-link process restarts for a deploy, heartbeats gap for 2 minutes. Bono's watchdog fires, activates cloud racecontrol unnecessarily.

**Why it happens:** `HeartbeatMonitor` fires `james_down` at 45s. If Bono's secondary check is scheduled for exactly 5min total, a 2-minute James restart window crosses the threshold.

**How to avoid:** Bono's secondary check must verify BOTH conditions: James still down AND .23 Tailscale probe fails. If .23 (server) is reachable, it is NOT a failover scenario regardless of James's status.

### Pitfall 5: Notification Sent to Wrong Entity

**What goes wrong:** The notification is triggered from racecontrol's broadcast endpoint response handler. But if cloud racecontrol is on Bono's VPS, and notifications use `email_alerts.rs` (Rust, on server .23), the server is DOWN — notification fails.

**How to avoid:** Notifications for failover must originate from James (comms-link Node.js on .27) or from Bono's comms-link. NOT from server racecontrol. Use `sendEvolutionText` from `comms-link/bono/alert-manager.js` — Bono's VPS is online during the failover.

### Pitfall 6: SwitchController Sent Before pm2 Start Completes

**What goes wrong:** Bono's exec_result arrives with exitCode 0 (pm2 returns 0 even if racecontrol needs a few seconds to bind its port). James immediately broadcasts SwitchController. Pods try to connect to `ws://100.70.177.44:8080/ws/agent` — racecontrol is still starting. Pods hit reconnect backoff.

**Why it happens:** `pm2 start` is non-blocking — the process starts, pm2 exits 0, but racecontrol binds asynchronously.

**How to avoid:** After `exec_result` for `activate_failover`, James calls `racecontrol_health` exec_request (also in COMMAND_REGISTRY) to poll `http://localhost:8080/api/v1/health` on Bono's VPS. Wait for 200 before POSTing to broadcast endpoint. The 3-retry-with-5s-interval strategy in CONTEXT.md handles this.

---

## Code Examples

### Existing Hysteresis Pattern (cloud_sync.rs lines 122-162)

```rust
// Source: crates/racecontrol/src/cloud_sync.rs
let mut effective_relay_up = false;
let mut consecutive_up: u32 = 0;
let mut consecutive_down: u32 = 0;

// On each tick:
if raw_up {
    consecutive_up += 1;
    consecutive_down = 0;
} else {
    consecutive_down += 1;
    consecutive_up = 0;
}

if effective_relay_up && consecutive_down >= RELAY_DOWN_THRESHOLD {
    effective_relay_up = false;
} else if !effective_relay_up && consecutive_up >= RELAY_UP_THRESHOLD {
    effective_relay_up = true;
}
```

Phase 69 adapts this with `DOWN_THRESHOLD=12`, `UP_THRESHOLD=2` (for Phase 70 failback).

### Existing exec_request Send Pattern (james/index.js lines 497-513)

```javascript
// Source: comms-link/james/index.js
if (req.method === 'POST' && req.url === '/relay/exec/send') {
  const execId = `ex_${randomUUID().slice(0, 8)}`;
  const sent = client.send('exec_request', {
    execId,
    command: payload.command,    // 'activate_failover'
    reason: payload.reason || 'relay-api',
    requestedBy: 'james',
  });
}
```

### Existing SwitchController Handler (rc-agent/src/main.rs lines 2572-2598)

```rust
// Source: crates/rc-agent/src/main.rs
rc_common::protocol::CoreToAgentMessage::SwitchController { target_url } => {
    let is_primary = target_url == primary_url;
    let is_failover = failover_url.as_ref().map_or(false, |f| target_url == *f);

    if !is_primary && !is_failover {
        tracing::warn!("[switch] Rejected — not in allowlist");
    } else {
        tracing::info!("[switch] Switching to {}", target_url);
        *active_url.write().await = target_url.clone();
        heartbeat_status.last_switch_ms.store(now_ms, Ordering::Relaxed);
        self_monitor::log_event(&format!("SWITCH: target={}", target_url));
        let _ = ws_tx.send(tokio_tungstenite::tungstenite::Message::Close(None)).await;
        break; // outer loop picks up new URL
    }
}
```

Phase 69 adds the LAN probe BEFORE `*active_url.write().await = ...`.

### Existing agent_senders Broadcast Pattern (state.rs lines 253-282)

```rust
// Source: crates/racecontrol/src/state.rs
let agent_senders = self.agent_senders.read().await;
for (pod_id, sender) in agent_senders.iter() {
    // apply per-pod logic...
    let _ = sender.send(CoreToAgentMessage::SettingsUpdated { settings: pod_settings }).await;
}
```

Phase 69 follows this exact pattern for the failover broadcast endpoint.

### sendEvolutionText Pattern (comms-link/bono/alert-manager.js)

```javascript
// Source: comms-link/bono/alert-manager.js
await sendEvolutionText({
  url: process.env.EVOLUTION_URL,
  instance: process.env.EVOLUTION_INSTANCE,
  apiKey: process.env.EVOLUTION_API_KEY,
  number: process.env.UDAY_WHATSAPP,
  text: 'FAILOVER ACTIVATED — Server .23 unreachable. Pods switched to cloud (100.70.177.44). Time: ... Pods: .../8.',
});
```

### AlertCooldown Pattern (comms-link/bono/alert-manager.js)

```javascript
// Source: comms-link/bono/alert-manager.js
const cooldown = new AlertCooldown({ windowMs: 10 * 60 * 1000 }); // 10 min for failover
if (cooldown.canSend()) {
  cooldown.recordSent();
  await sendEvolutionText({ ... });
}
```

---

## State of the Art

| Current State | Phase 69 Adds | Why Needed |
|--------------|----------------|-----------|
| No server health probe | James health probe loop (5s interval, FSM) | Can't self-monitor; needs external watcher |
| SwitchController handled but no HTTP trigger | New `/api/v1/failover/broadcast` endpoint on cloud racecontrol | James needs to trigger pod switch via HTTP |
| activate_failover in COMMAND_REGISTRY (Phase 66) | Wire it into health monitor state machine | Registry existed but wasn't triggered automatically |
| HeartbeatMonitor emits james_down at 45s | Add 5-min secondary watchdog in bono/index.js | James machine itself could be offline |
| split-brain: no guard | LAN probe before SwitchController acceptance | rc-agent needs to independently verify .23 is actually down |

---

## Open Questions

1. **exec_result Promise Resolution**
   - What we know: `exec_result` handler in `james/index.js` (lines 369-374) only logs — no callback/promise
   - What's unclear: cleanest way to wire promise resolution without large refactor
   - Recommendation: Add a `pendingExecPromises = new Map()` in `initiateFailover()` scope; in the `exec_result` handler block, check `pendingExecPromises.has(msg.payload.execId)` and resolve/reject. EventEmitter-based would also work.

2. **Failover Broadcast Endpoint Security**
   - What we know: Should use `x-terminal-secret` header check (consistent with `/sync/push`)
   - What's unclear: Whether the terminal_secret on Bono's VPS is the same as server .23
   - Recommendation: Yes, use same `TERMINAL_SECRET` env var. James already knows it (used in `james/index.js` line 36).

3. **pm2 Start Idempotency**
   - What we know: `pm2 start racecontrol` errors if process is already running
   - What's unclear: Bono's VPS pm2 state — racecontrol may or may not be running
   - Recommendation: Use `pm2 restart racecontrol || pm2 start racecontrol` OR add a `racecontrol_restart` command to COMMAND_REGISTRY as `pm2 restart --update-env racecontrol`. Planner should decide.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + manual Node.js test script |
| Config file | `.cargo/config.toml` (workspace) |
| Quick run command | `cargo test -p rc-agent -- switch` |
| Full suite command | `cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HLTH-01 | Health probe fires every 5s, counts failures | unit (JS) | `node james/health-monitor.test.js` | ❌ Wave 0 |
| HLTH-02 | FSM: 12 failures → Down, 1 success resets | unit (JS) | `node james/health-monitor.test.js` | ❌ Wave 0 |
| HLTH-03 | Tailscale fallback probe used only after LAN fails | unit (JS) | `node james/health-monitor.test.js` | ❌ Wave 0 |
| HLTH-04 | Bono watchdog: 5min gap + .23 unreachable → activate | manual | observe bono/index.js logs | manual-only (timing) |
| ORCH-01 | exec_request activate_failover sent to Bono | manual | observe comms-link logs | manual-only |
| ORCH-02 | broadcast endpoint sends SwitchController to all agent_senders | unit (Rust) | `cargo test -p racecontrol -- failover_broadcast` | ❌ Wave 0 |
| ORCH-03 | split-brain guard rejects switch if .23 responds | unit (Rust) | `cargo test -p rc-agent -- split_brain` | ❌ Wave 0 |
| ORCH-04 | notification sent after pod count confirmed | manual | observe logs + Uday's phone | manual-only |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent -- switch` (covers SwitchController + split-brain)
- **Per wave merge:** `cargo test -p rc-agent && cargo test -p racecontrol && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `comms-link/james/health-monitor.test.js` — covers HLTH-01, HLTH-02, HLTH-03 (simple Node.js unit test using mock http server)
- [ ] `crates/racecontrol/src/api/routes.rs::test::failover_broadcast_sends_to_all_agents` — covers ORCH-02
- [ ] `crates/rc-agent/src/main.rs::test::split_brain_guard_rejects_when_server_reachable` — covers ORCH-03

---

## Sources

### Primary (HIGH confidence — read directly from codebase)

- `crates/racecontrol/src/cloud_sync.rs` lines 26-27, 122-162 — hysteresis FSM pattern (RELAY_DOWN_THRESHOLD, UP_THRESHOLD, consecutive counters)
- `crates/rc-agent/src/main.rs` lines 2572-2598 — SwitchController handler exact code
- `crates/rc-agent/src/self_monitor.rs` lines 80-103 — last_switch_ms guard pattern
- `crates/rc-common/src/protocol.rs` lines 400-406 — SwitchController variant definition
- `comms-link/james/index.js` lines 497-513, 160-173 — exec_request send + sendTaskRequest patterns
- `comms-link/bono/index.js` lines 81-99, 253-263 — sendExecRequest, james_down handler
- `comms-link/shared/exec-protocol.js` lines 104-127 — activate_failover, deactivate_failover in COMMAND_REGISTRY
- `comms-link/shared/protocol.js` — MessageType enum (exec_request, exec_result, task_request, task_response)
- `comms-link/bono/heartbeat-monitor.js` — HeartbeatMonitor, TIMEOUT_MS=45s, james_down event
- `comms-link/bono/alert-manager.js` — sendEvolutionText, AlertCooldown, AlertManager
- `crates/racecontrol/src/email_alerts.rs` — EmailAlerter shell-out pattern
- `crates/racecontrol/src/whatsapp_alerter.rs` — send_whatsapp, ist_now_string, Evolution API
- `crates/racecontrol/src/state.rs` lines 113, 253-282 — agent_senders broadcast pattern
- `crates/racecontrol/src/api/routes.rs` — confirmed NO existing failover/broadcast endpoint

### Secondary (MEDIUM confidence)

- None required — all implementation is in-project code.

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all verified by reading source files directly
- Architecture: HIGH — all patterns copied from existing working code in the repo
- Pitfalls: HIGH — derived from reading the actual handler code and identifying gaps (exec_result not promise-resolved, pm2 start idempotency, notification location)
- Missing endpoint: HIGH — confirmed by grepping all racecontrol routes; no `SwitchController` reference exists in `routes.rs`

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-20 (stable internal codebase; no external dependencies)
