# Phase 14: Graceful Degradation - Research

**Researched:** 2026-03-20
**Domain:** Connection mode management, email fallback, offline buffering
**Confidence:** HIGH

## Summary

Phase 14 adds a ConnectionMode state machine that orchestrates automatic fallthrough between three delivery modes: REALTIME (WebSocket), EMAIL_FALLBACK (send_email.js), and OFFLINE_QUEUE (disk-buffered WAL). The existing codebase has all the building blocks already implemented across prior phases -- CommsClient tracks WebSocket state (CONNECTED/RECONNECTING/DISCONNECTED), MessageQueue provides WAL-backed persistence, the email infrastructure exists via send_email.js (used by daily-summary.js), and MetricsCollector exports health snapshots. The work is integration: creating a ConnectionMode manager that reacts to CommsClient state changes and routes critical messages through the correct delivery path.

The key design insight is that this is James-side only. James is the one who sends critical messages (alerts, exec results, task requests) outbound to Bono. When WebSocket drops, James needs to decide whether to use email or buffer to disk. Bono does not need graceful degradation -- if James disconnects, Bono's HeartbeatMonitor already detects it and alerts Uday via WhatsApp. The ConnectionMode state is exposed in the existing `/relay/metrics` endpoint and heartbeat payload for observability.

**Primary recommendation:** Build a `shared/connection-mode.js` module that wraps CommsClient state events + email availability probing into a three-state mode manager. Wire it into james/index.js to intercept `sendTracked()` calls and route critical message types through the appropriate delivery path.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| GD-01 | When WebSocket is down, critical messages (alerts, commands) fall back to email | ConnectionMode listens to CommsClient 'state' events; when state is RECONNECTING/DISCONNECTED, routes alert/exec_result/task_request types through send_email.js child process |
| GD-02 | When email is also unavailable, messages buffer to disk queue (offline mode) | MessageQueue (WAL-backed) already exists from Phase 9; ConnectionMode enqueues to WAL when both WS and email are down; drain loop replays on mode upgrade |
| GD-03 | Explicit connection mode visible: REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE | ConnectionMode exposes `.mode` property; wired into MetricsCollector.snapshot() and heartbeat payload; exposed via /relay/metrics endpoint |
</phase_requirements>

## Standard Stack

### Core

No new dependencies needed. Phase 14 is pure integration of existing modules.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `node:child_process` (execFile) | Node 22.14.0 | Email sending via send_email.js | Already used in daily-summary.js for email delivery |
| `node:events` (EventEmitter) | Node 22.14.0 | ConnectionMode event emission | Project pattern -- all modules use EventEmitter |
| `node:fs/promises` | Node 22.14.0 | WAL persistence (via MessageQueue) | Already in use from Phase 9 |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `ws` | ^8.19.0 | WebSocket transport | Already installed, no change needed |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| execFile for email | Nodemailer / SMTP direct | Would add a dependency; send_email.js is already proven and handles Gmail OAuth |
| WAL file for offline | In-memory buffer | Would lose messages on crash -- WAL already handles this |

**Installation:** No new packages needed.

## Architecture Patterns

### Recommended Project Structure

```
shared/
  connection-mode.js     # NEW: ConnectionMode state machine + email probe
james/
  index.js               # MODIFY: wire ConnectionMode, intercept sendTracked()
james/
  metrics-collector.js   # MODIFY: add connectionMode to snapshot()
james/
  system-metrics.js      # MODIFY: add connectionMode to heartbeat payload
test/
  connection-mode.test.js # NEW: unit tests for ConnectionMode
  graceful-degradation.test.js # NEW: integration tests for fallthrough
```

### Pattern 1: ConnectionMode State Machine

**What:** A three-state machine (REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE) that reacts to external signals (WS state changes, email probe results) and emits mode transitions.

**When to use:** Always active in james/index.js. Mode determines message routing.

**State transitions:**
```
REALTIME ----[WS disconnects]----> EMAIL_FALLBACK
EMAIL_FALLBACK --[email probe OK, WS reconnects]--> REALTIME
EMAIL_FALLBACK --[email probe fails]--> OFFLINE_QUEUE
OFFLINE_QUEUE --[email probe OK]--> EMAIL_FALLBACK
OFFLINE_QUEUE --[WS reconnects]--> REALTIME
```

**Example:**
```javascript
// shared/connection-mode.js
import { EventEmitter } from 'node:events';

export const Mode = Object.freeze({
  REALTIME: 'REALTIME',
  EMAIL_FALLBACK: 'EMAIL_FALLBACK',
  OFFLINE_QUEUE: 'OFFLINE_QUEUE',
});

export class ConnectionMode extends EventEmitter {
  #mode = Mode.REALTIME;
  #wsConnected = false;
  #emailAvailable = true; // optimistic default
  #probeEmailFn;
  #probeIntervalMs;
  #probeTimer = null;

  constructor({ probeEmailFn, probeIntervalMs = 60000 }) {
    super();
    this.#probeEmailFn = probeEmailFn;
    this.#probeIntervalMs = probeIntervalMs;
  }

  get mode() { return this.#mode; }

  onWsStateChange(state) {
    this.#wsConnected = (state === 'CONNECTED');
    this.#recalculate();
  }

  onEmailProbeResult(available) {
    this.#emailAvailable = available;
    this.#recalculate();
  }

  #recalculate() {
    let next;
    if (this.#wsConnected) {
      next = Mode.REALTIME;
    } else if (this.#emailAvailable) {
      next = Mode.EMAIL_FALLBACK;
    } else {
      next = Mode.OFFLINE_QUEUE;
    }
    if (next !== this.#mode) {
      const prev = this.#mode;
      this.#mode = next;
      this.emit('mode', { mode: next, previous: prev, ts: Date.now() });
    }
  }
}
```

### Pattern 2: Critical Message Router

**What:** A wrapper around `sendTracked()` that checks ConnectionMode and routes accordingly.

**When to use:** For message types that must not be lost: `exec_result`, `task_request`, alert-class messages.

**Example:**
```javascript
// Critical message types that need fallback routing
const CRITICAL_TYPES = new Set([
  'exec_result', 'task_request', 'recovery',
]);

function sendCritical(type, payload) {
  switch (connectionMode.mode) {
    case Mode.REALTIME:
      return sendTracked(type, payload);
    case Mode.EMAIL_FALLBACK:
      return sendViaEmail(type, payload);
    case Mode.OFFLINE_QUEUE:
      return messageQueue.enqueue({ id: randomUUID(), type, payload });
  }
}
```

### Pattern 3: Email Probe

**What:** Periodic check of email availability by verifying send_email.js exists and is executable. Does NOT send an actual email -- just validates the infrastructure is present.

**When to use:** When mode is EMAIL_FALLBACK or OFFLINE_QUEUE, probe every 60s to detect recovery.

**Example:**
```javascript
async function probeEmail() {
  try {
    await access(sendEmailPath, constants.R_OK | constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
```

### Pattern 4: Offline Queue Drain

**What:** When mode upgrades from OFFLINE_QUEUE to EMAIL_FALLBACK or REALTIME, drain buffered messages from WAL and send via the now-available path.

**When to use:** On every mode upgrade event.

**Example:**
```javascript
connectionMode.on('mode', async ({ mode, previous }) => {
  if (previous === Mode.OFFLINE_QUEUE && mode !== Mode.OFFLINE_QUEUE) {
    const pending = messageQueue.getPending();
    for (const msg of pending) {
      if (mode === Mode.REALTIME) {
        sendTracked(msg.type, msg.payload);
      } else {
        sendViaEmail(msg.type, msg.payload);
      }
      await messageQueue.acknowledge(msg.id);
    }
    await messageQueue.compact();
  }
});
```

### Anti-Patterns to Avoid

- **Polling WS state on a timer:** Use event-driven state changes from CommsClient 'state' events, never poll `client.state` in a setInterval.
- **Sending test emails as probes:** Probing email availability should be a file-existence check, not sending real emails. Sending emails costs API quota and creates noise.
- **Falling back for ALL messages:** Only critical message types need fallback routing. Heartbeats, echoes, and status queries are meaningless when WS is down -- just skip them.
- **Blocking on email sends:** Email via execFile is fire-and-forget. Never await the email send before continuing -- treat email as best-effort in the fallback path.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Disk-persistent queue | Custom append-to-file logic | `shared/message-queue.js` (MessageQueue) | Already handles WAL, crash recovery, compaction, dedup |
| Email delivery | SMTP client, Nodemailer | `send_email.js` via `execFile` | Already handles Gmail OAuth, proven in daily-summary.js |
| Connection state tracking | Custom WS state polling | `CommsClient` state events | Already emits 'state' with CONNECTED/RECONNECTING/DISCONNECTED |
| Retry with backoff | Custom timer logic | `AckTracker` | Already implements exponential backoff retries |
| Metrics export | Custom JSON builder | `MetricsCollector.snapshot()` | Already provides structured snapshot; just add mode field |

**Key insight:** Phase 14 is 90% wiring and 10% new code. The ConnectionMode state machine is the only genuinely new module. Everything else -- MessageQueue, send_email.js, MetricsCollector, CommsClient state events -- is already built and tested.

## Common Pitfalls

### Pitfall 1: Email Probe Races with Mode Change

**What goes wrong:** WS reconnects while an email probe is in-flight. Probe returns "available" and triggers a mode recalculation that briefly flashes EMAIL_FALLBACK before settling on REALTIME.
**Why it happens:** Async probe results arrive after WS state changes.
**How to avoid:** In `#recalculate()`, always check `#wsConnected` first. If WS is connected, mode is REALTIME regardless of email probe result. The probe result only matters when WS is down.
**Warning signs:** Mode flickering in logs between REALTIME and EMAIL_FALLBACK during reconnect.

### Pitfall 2: Offline Queue Drain Storms

**What goes wrong:** Mode upgrades from OFFLINE_QUEUE, drain loop sends 100+ buffered messages simultaneously, overwhelming the WS or email path.
**Why it happens:** No rate limiting on drain.
**How to avoid:** Drain with a small delay between messages (e.g., 100ms). For WS, AckTracker handles backpressure. For email, batch messages into a single email body.
**Warning signs:** WS connection drops immediately after reconnect due to send flood.

### Pitfall 3: Gmail OAuth Expired

**What goes wrong:** send_email.js exists and is executable (probe passes), but actual email send fails because Gmail OAuth tokens are expired.
**Why it happens:** Known issue documented in STATE.md -- OAuth tokens expired, renewal is a separate ops task.
**How to avoid:** The email send function should catch execFile errors. If email send fails, downgrade to OFFLINE_QUEUE. Log the error clearly for ops diagnosis. Do NOT keep retrying email in a tight loop -- use a cooldown (e.g., 5 minutes before next email attempt).
**Warning signs:** `[DailySummary] Email send failed` in logs.

### Pitfall 4: Double-Sending on Mode Transition

**What goes wrong:** Message is sent via email, then WS reconnects and the same message is replayed via AckTracker.
**Why it happens:** Message was tracked in AckTracker AND sent via email. On reconnect, AckTracker replays it.
**How to avoid:** When routing via email or offline queue, do NOT also track in AckTracker. The fallback path is a separate delivery mechanism. Use the DeduplicatorCache on Bono's side to handle any rare duplicates.
**Warning signs:** Bono receives the same message twice -- once via email, once via WS.

### Pitfall 5: send_email.js Path Differs Between James and Bono

**What goes wrong:** SEND_EMAIL_PATH is configured for Bono's VPS but Phase 14 email fallback runs on James.
**Why it happens:** James and Bono have different filesystem layouts. Bono's path is `/root/racingpoint-google/send_email.js`, James's path is `C:/Users/bono/racingpoint/racecontrol/send_email.js`.
**How to avoid:** Use the already-configured `SEND_EMAIL_PATH` env var from start-comms-link.bat. It's already set to the correct James path.
**Warning signs:** `ENOENT` when trying to exec send_email.js.

## Code Examples

### Email Fallback Send (from daily-summary.js pattern)

```javascript
// Source: bono/daily-summary.js lines 243-253
// This is the proven pattern for sending email via execFile
function sendViaEmail(type, payload, sendEmailPath, execFileFn) {
  const subject = `[COMMS-LINK] ${type} (fallback)`;
  const body = JSON.stringify({ type, payload, ts: Date.now() }, null, 2);
  execFileFn('node', [
    sendEmailPath,
    '--to', 'bono@racingpoint.in',
    '--subject', subject,
    '--body', body,
  ], (err) => {
    if (err) console.error(`[EMAIL-FALLBACK] Send failed: ${err.message}`);
    else console.log(`[EMAIL-FALLBACK] Sent ${type} via email`);
  });
}
```

### Wiring ConnectionMode into CommsClient State Events

```javascript
// Source: james/index.js existing pattern (lines 143-149)
// CommsClient already emits 'state' events -- just listen
client.on('state', (evt) => {
  connectionMode.onWsStateChange(evt.state);
});
```

### Exposing Mode in Metrics Snapshot

```javascript
// Source: james/index.js lines 435-442 (existing /relay/metrics handler)
if (req.method === 'GET' && req.url === '/relay/metrics') {
  const snapshot = metricsCollector.snapshot();
  snapshot.queueDepth = messageQueue.size;
  snapshot.ackPending = ackTracker.pendingCount;
  snapshot.wsState = client.state;
  snapshot.connectionMode = connectionMode.mode; // NEW
  jsonResponse(res, 200, snapshot);
}
```

### Heartbeat Payload Extension

```javascript
// Source: james/system-metrics.js collectMetrics pattern
// Add connectionMode to the base metrics object
base.connectionMode = connectionModeFn?.() ?? 'UNKNOWN';
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fire-and-forget over WS | AckTracker with retries (Phase 9) | 2026-03-20 | Messages have delivery confirmation |
| In-memory queue only | WAL-backed MessageQueue (Phase 9) | 2026-03-20 | Messages survive crashes |
| No email fallback | send_email.js exists but untested E2E (Phase 13) | 2026-03-20 | Infrastructure ready, OAuth needs renewal |
| No mode visibility | wsState in /relay/metrics (Phase 13) | 2026-03-20 | Partial visibility; Phase 14 adds explicit mode |

**Current blocker:** Gmail OAuth tokens are expired. Email fallback code path is structurally correct but actual email delivery will fail until OAuth is renewed. Phase 14 should be designed to handle this gracefully (catch errors, downgrade to OFFLINE_QUEUE).

## Open Questions

1. **Email recipient for fallback messages**
   - What we know: Daily summary emails go to usingh@racingpoint.in. Bono's email is bono@racingpoint.in.
   - What's unclear: Should fallback messages go to Bono (the intended recipient), Uday (the boss), or both?
   - Recommendation: Send to bono@racingpoint.in -- these are AI-to-AI coordination messages. Bono is the intended recipient. Uday gets alerts via WhatsApp separately.

2. **Gmail OAuth renewal timeline**
   - What we know: OAuth tokens are expired. This is documented as a known issue.
   - What's unclear: When will this be fixed? Should Phase 14 block on it?
   - Recommendation: Do NOT block. Build the fallback path, handle email errors gracefully. When OAuth is renewed, the fallback path will "just work."

3. **Email format for machine-readable recovery**
   - What we know: Email is a human-readable channel. But Bono could potentially parse incoming emails.
   - What's unclear: Should email fallback messages be structured (JSON) for Bono to auto-process, or plain text for human reading?
   - Recommendation: Use JSON body. Even if Bono can't auto-parse emails today, structured data is more useful. Subject line includes message type for quick triage.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node 22.14.0) |
| Config file | none -- run directly with `node --test` |
| Quick run command | `node --test test/connection-mode.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GD-01 | WS down -> critical messages route to email | unit | `node --test test/connection-mode.test.js` | Wave 0 |
| GD-01 | Email send via execFile actually fires | unit | `node --test test/graceful-degradation.test.js` | Wave 0 |
| GD-02 | WS+email down -> messages buffer to WAL | unit | `node --test test/connection-mode.test.js` | Wave 0 |
| GD-02 | Mode upgrade -> drain WAL | unit | `node --test test/graceful-degradation.test.js` | Wave 0 |
| GD-03 | connectionMode visible in /relay/metrics | integration | `node --test test/metrics-endpoint.test.js` | Exists (extend) |
| GD-03 | connectionMode in heartbeat payload | unit | `node --test test/system-metrics.test.js` | Exists (extend) |

### Sampling Rate
- **Per task commit:** `node --test test/connection-mode.test.js test/graceful-degradation.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/connection-mode.test.js` -- covers GD-01, GD-02, GD-03 (ConnectionMode state machine unit tests)
- [ ] `test/graceful-degradation.test.js` -- covers GD-01, GD-02 (integration: email fallback send + WAL drain)

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis -- james/index.js, bono/index.js, shared/message-queue.js, shared/ack-tracker.js, james/comms-client.js, shared/state.js, shared/protocol.js, james/metrics-collector.js, james/system-metrics.js, bono/daily-summary.js, bono/alert-manager.js
- Existing test suite -- 32 test files covering all existing modules
- .planning/STATE.md -- accumulated decisions from Phases 9-13
- .planning/REQUIREMENTS.md -- GD-01, GD-02, GD-03 requirement definitions
- .planning/research/SUMMARY.md -- v2.0 research summary with architecture patterns

### Secondary (MEDIUM confidence)
- Email fallback infrastructure validated in Phase 13 (OBS-04) -- structurally correct but OAuth expired

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all building blocks exist in codebase
- Architecture: HIGH -- ConnectionMode pattern is straightforward state machine, all integration points identified via code inspection
- Pitfalls: HIGH -- based on direct analysis of existing code paths and known issues (OAuth, path differences)

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, no external dependencies to go stale)
