# Phase 6: Alerting - Research

**Researched:** 2026-03-12
**Domain:** WhatsApp alerting via Evolution API, email fallback, flapping suppression, WebSocket protocol extension
**Confidence:** HIGH

## Summary

Phase 6 adds alerting to Uday when James goes down or comes back online. The architecture splits responsibilities across both sides: Bono (VPS) owns WhatsApp alerting via Evolution API since he detects James-down through heartbeat timeout, while James owns email fallback for when the WebSocket link itself is down. Flapping suppression operates independently on both sides -- Bono suppresses WhatsApp, James suppresses email.

The existing codebase provides all the building blocks. HeartbeatMonitor on Bono already emits `james_down` and `james_up` events. EscalatingCooldown on James already implements the exact gating pattern needed for alert suppression. The protocol layer (shared/protocol.js) already defines a `status` message type that is currently unused -- this is the natural carrier for recovery signals. The only genuinely new code is: (1) an AlertManager on Bono that sends WhatsApp via Evolution API HTTP calls, (2) alert suppression wrappers on both sides, (3) a recovery message type in the protocol, and (4) email fallback logic in watchdog-runner.js that fires when CommsClient is DISCONNECTED.

**Primary recommendation:** Create an AlertManager class on Bono that listens to HeartbeatMonitor events and calls Evolution API, add a recovery/status message from James to Bono over WebSocket, extend watchdog-runner.js with email-to-Uday fallback gated by CommsClient state and cooldown cap, and add flapping suppression on both sides using the existing EscalatingCooldown pattern (or a simpler fixed-window cooldown).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Bono orchestrates ALL WhatsApp alerting -- both down-alerts (heartbeat timeout) and back-online (receives recovery signal from James)
- James sends recovery signal + status updates (crash count, cooldown state) to Bono over WebSocket
- Email fallback goes to BOTH Uday (usingh@racingpoint.in) AND Bono (bono@racingpoint.in)
- James sends down-email only after hitting 5min cooldown cap (final escalation step) -- not on every crash detection
- Evolution API details (instance name, API key, Uday's WhatsApp number) not yet available -- plan with env var placeholders, coordinate with Bono before execution
- Minimal one-liner WhatsApp messages with status emoji (down: crash attempt info, back-online: downtime + restart count)
- Both sides suppress independently: James suppresses email alerts locally, Bono suppresses WhatsApp alerts
- Email fallback alerts are also suppressed (one "James is down" email per suppression window)
- Check CommsClient state at event time: if DISCONNECTED -> email immediately, if CONNECTED -> send via WebSocket
- No try-then-fallback -- use current state to decide channel upfront
- Email has MORE detail than WhatsApp: include system metrics, cooldown history, log context
- Subject lines typed for inbox scanning: `[ALERT] James DOWN` / `[RECOVERED] James UP`
- Use existing send_email.js via execFile pattern (from Phase 5)

### Claude's Discretion
- Recovery signal format: new message type vs enriched heartbeat (pick what fits existing protocol)
- Status update delivery: part of heartbeat payload vs separate messages on state change
- Flapping suppression implementation: EscalatingCooldown reuse vs simple fixed window
- Whether to queue recovery signal for WS delivery when connection resumes after email fallback
- Alert suppression window duration(s)

### Deferred Ideas (OUT OF SCOPE)
- Daily health summary (uptime %, restart count, connection stability) -- Phase 8 (AL-05)
- Web dashboard for Uday -- v2 (EM-01)
- WhatsApp notification for pod issues at Racing Point (not comms-link scope)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AL-01 | WhatsApp notification to Uday when James goes down (via Bono's Evolution API) | Bono AlertManager listens to HeartbeatMonitor `james_down`, calls Evolution API POST /message/sendText/{instance} with env var config |
| AL-02 | WhatsApp notification to Uday when James comes back online | James sends `recovery` message over WebSocket; Bono AlertManager receives it and calls Evolution API |
| AL-03 | Email fallback -- same alert info sent via email when WebSocket is down | James watchdog-runner checks CommsClient.state at crash time; if DISCONNECTED + at cooldown cap, sends email to Uday + Bono via send_email.js |
| AL-04 | Flapping suppression -- suppress repeated alerts during rapid crash/restart cycles | AlertCooldown on Bono suppresses WhatsApp; James uses cooldown cap check (step >= last) to gate email sending |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:events | v22.14.0 built-in | EventEmitter base for AlertManager | Already used by HeartbeatMonitor, ClaudeWatchdog |
| node:child_process | v22.14.0 built-in | execFile for send_email.js | Already used in watchdog-runner.js |
| node:test | v22.14.0 built-in | Test runner | Already used across all 97 tests |
| ws | ^8.19.0 | WebSocket transport | Already a dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| node:assert/strict | built-in | Test assertions | All test files |
| node:http (or node:https) | built-in | Evolution API HTTP calls from Bono | AlertManager sends POST to Evolution API |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw http.request for Evolution API | node-fetch / axios | Adding a dependency for a single POST call is overkill; http.request is 15 lines and zero deps |
| New `recovery` message type | Enriched heartbeat payload | Separate message type is cleaner -- heartbeat is periodic, recovery is a one-time event signal |
| Fixed-window alert cooldown | Reuse EscalatingCooldown | Fixed window is simpler for alerts (no need for escalation -- just suppress duplicates); EscalatingCooldown is for restart gating |

**Installation:**
```bash
# No new dependencies needed -- everything is built-in or already installed
```

## Architecture Patterns

### Recommended Project Structure
```
james/
  watchdog-runner.js    # EXTEND: add alert email fallback on crash_detected + cooldown cap
  watchdog.js           # UNCHANGED (EscalatingCooldown already has the cap check we need)
  comms-client.js       # UNCHANGED (state check for fallback decision)
  heartbeat-sender.js   # UNCHANGED
  system-metrics.js     # UNCHANGED (metrics included in email body)
bono/
  alert-manager.js      # NEW: WhatsApp alerting via Evolution API + suppression
  index.js              # EXTEND: wire AlertManager to HeartbeatMonitor + WS messages
  comms-server.js       # UNCHANGED (already emits 'message' on wss)
  heartbeat-monitor.js  # UNCHANGED (already emits james_down/james_up)
shared/
  protocol.js           # EXTEND: add 'recovery' and 'alert_status' message types
test/
  alerting.test.js      # NEW: AlertManager unit tests + email fallback tests
```

### Pattern 1: Recovery Signal via New Message Type
**What:** James sends a `recovery` message over WebSocket after successful self-test, carrying crash count, downtime estimate, and restart count. Bono receives it and sends the back-online WhatsApp.
**When to use:** When James comes back online and WebSocket is connected.
**Recommendation:** Use a dedicated `recovery` message type rather than enriching heartbeat. Heartbeats are periodic background signals; recovery is a discrete event. The protocol already has an unused `status` type, but creating a specific `recovery` type is more explicit and self-documenting.

```javascript
// shared/protocol.js -- add to MessageType
export const MessageType = Object.freeze({
  echo: 'echo',
  echo_reply: 'echo_reply',
  heartbeat: 'heartbeat',
  heartbeat_ack: 'heartbeat_ack',
  status: 'status',
  recovery: 'recovery',       // NEW: James -> Bono after self-test pass
  file_sync: 'file_sync',
  file_ack: 'file_ack',
  message: 'message',
});

// james/watchdog-runner.js -- in self_test_passed handler
client.send('recovery', {
  crashCount: attemptCount,
  downtimeMs: Date.now() - crashTimestamp,  // tracked from crash_detected
  restartCount: attemptCount,
  pid,
  exePath,
});
```

### Pattern 2: AlertManager on Bono (Evolution API)
**What:** A class that listens to HeartbeatMonitor events and sends WhatsApp messages via Evolution API HTTP POST. Manages its own flapping suppression cooldown.
**When to use:** Bono-side alerting for both down and recovery events.

```javascript
// bono/alert-manager.js
import { EventEmitter } from 'node:events';
import https from 'node:https';

export class AlertManager extends EventEmitter {
  #evolutionUrl;   // e.g., 'https://evo.example.com'
  #instanceName;   // e.g., 'racingpoint'
  #apiKey;         // Evolution API key
  #udayNumber;     // e.g., '919876543210'
  #cooldownMs;     // suppression window (e.g., 300000 = 5 min)
  #lastAlertTime = 0;
  #downSince = null;

  constructor({ evolutionUrl, instanceName, apiKey, udayNumber, cooldownMs = 300_000, nowFn = Date.now }) {
    super();
    this.#evolutionUrl = evolutionUrl;
    this.#instanceName = instanceName;
    this.#apiKey = apiKey;
    this.#udayNumber = udayNumber;
    this.#cooldownMs = cooldownMs;
    this._nowFn = nowFn;  // for testability
  }

  /** Send WhatsApp text via Evolution API. Fire-and-forget with error logging. */
  #sendWhatsApp(text) { /* HTTP POST to /message/sendText/{instance} */ }

  /** Called when HeartbeatMonitor emits james_down */
  handleJamesDown({ timestamp, lastHeartbeat }) { /* suppression check + send */ }

  /** Called when recovery message arrives from James via WebSocket */
  handleRecovery({ crashCount, downtimeMs, restartCount }) { /* send back-online */ }
}
```

### Pattern 3: Email Fallback on James Side
**What:** When James detects a crash AND CommsClient is DISCONNECTED AND the cooldown has reached the 5-minute cap, James sends an email directly to Uday + Bono. This is the "last resort" escalation.
**When to use:** WebSocket is down (James cannot notify Bono), and crashes have been persistent (cooldown at max step).

```javascript
// In watchdog-runner.js, inside crash_detected handler:
watchdog.on('crash_detected', ({ timestamp }) => {
  // Only send email fallback if:
  // 1. WebSocket is DISCONNECTED (can't reach Bono)
  // 2. Cooldown is at the final step (5min cap = persistent failure)
  if (client !== null && client.state !== 'DISCONNECTED') return;
  if (watchdog.cooldown.attemptCount < 5) return;  // Not at cap yet

  // Send to both Uday and Bono
  const subject = '[ALERT] James DOWN';
  const body = buildDetailedAlertBody(timestamp, watchdog.cooldown);
  sendAlertEmail('usingh@racingpoint.in', subject, body);
  sendAlertEmail('bono@racingpoint.in', subject, body);
});
```

### Pattern 4: Flapping Suppression (Fixed Window)
**What:** A simple cooldown that prevents the same alert from being sent more than once within a time window. Unlike EscalatingCooldown (which gates restart attempts with escalating delays), alert suppression just needs a fixed "don't repeat for N minutes" window.
**When to use:** Preventing alert floods to Uday's WhatsApp.
**Recommendation:** Use a simple fixed-window approach rather than reusing EscalatingCooldown. Alert suppression has different semantics -- you want "one alert per window" not "escalating delays between alerts."

```javascript
// Simple alert cooldown (not escalating)
export class AlertCooldown {
  #windowMs;
  #lastAlertTime = 0;
  #nowFn;

  constructor({ windowMs = 300_000, nowFn = Date.now } = {}) {
    this.#windowMs = windowMs;
    this.#nowFn = nowFn;
  }

  /** Returns true if enough time has elapsed since last alert. */
  canSend() {
    return (this.#nowFn() - this.#lastAlertTime) >= this.#windowMs;
  }

  /** Record that an alert was sent. */
  recordSent() {
    this.#lastAlertTime = this.#nowFn();
  }

  /** Reset (e.g., when James comes back up and state changes). */
  reset() {
    this.#lastAlertTime = 0;
  }
}
```

### Pattern 5: Evolution API HTTP Call (No Dependencies)
**What:** Use Node.js built-in `http`/`https` module to POST to Evolution API. No need for fetch or axios for a single endpoint.
**When to use:** Bono-side WhatsApp message sending.

```javascript
import https from 'node:https';
import http from 'node:http';
import { URL } from 'node:url';

/**
 * Send a text message via Evolution API.
 * @param {{ url: string, instance: string, apiKey: string, number: string, text: string }} opts
 * @returns {Promise<{ ok: boolean, status: number, body: string }>}
 */
export function sendEvolutionText({ url, instance, apiKey, number, text }) {
  return new Promise((resolve) => {
    const parsed = new URL(`/message/sendText/${instance}`, url);
    const transport = parsed.protocol === 'https:' ? https : http;
    const body = JSON.stringify({ number, text });

    const req = transport.request(parsed, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'apikey': apiKey,
      },
    }, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => {
        resolve({ ok: res.statusCode >= 200 && res.statusCode < 300, status: res.statusCode, body: data });
      });
    });

    req.on('error', (err) => {
      resolve({ ok: false, status: 0, body: err.message });
    });

    req.setTimeout(10_000, () => {
      req.destroy(new Error('Evolution API timeout'));
    });

    req.write(body);
    req.end();
  });
}
```

### Anti-Patterns to Avoid
- **Try-then-fallback for channel selection:** Do NOT try WebSocket first and fall back to email on failure. Check `CommsClient.state` upfront and pick the channel. This avoids timeout delays.
- **Sending email on every crash detection:** Only send email when cooldown reaches the 5-minute cap AND WebSocket is disconnected. Early crash/restart cycles should NOT trigger email.
- **Blocking watchdog loop on alert sending:** Both WhatsApp API calls and email sends must be fire-and-forget. Never await them in the critical restart path.
- **Merging alert suppression with restart cooldown:** These are different concerns. Restart cooldown gates process restart attempts (EscalatingCooldown). Alert cooldown gates notification sending (fixed window). Keep them separate.
- **Adding Evolution API dependency to James:** James never calls Evolution API directly. Bono owns WhatsApp. James only sends recovery signals over WebSocket or email fallback.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email sending | Custom SMTP/Gmail client | `execFile('node', [send_email.js, ...])` | send_email.js already handles OAuth, token refresh, Gmail API |
| WebSocket reconnection | Custom reconnect logic | `CommsClient.connect()` | CommsClient has full exponential backoff |
| Heartbeat timeout detection | Custom timer logic | `HeartbeatMonitor.james_down` event | Already implemented with 45s timeout |
| HTTP request library | npm install axios/node-fetch | `node:https` built-in | Single POST call doesn't justify a dependency |
| WhatsApp message formatting | Template engine | String interpolation | Messages are one-liners -- no template needed |

**Key insight:** The only genuinely new code is AlertManager (Bono-side, ~80 lines), the Evolution API HTTP helper (~30 lines), alert cooldown (~20 lines), and wiring in watchdog-runner.js (~30 lines) and bono/index.js (~15 lines). Total new code is under 200 lines.

## Common Pitfalls

### Pitfall 1: Evolution API Environment Variables Not Set
**What goes wrong:** AlertManager tries to call Evolution API but URL/key/instance/number are undefined. Silent failure means no alerts ever reach Uday.
**Why it happens:** Evolution API details are not yet available (per CONTEXT.md). Variables will be coordinated with Bono.
**How to avoid:** AlertManager constructor validates that all four env vars are present. If any are missing, log a clear warning and disable WhatsApp alerting (graceful degradation). Do NOT crash the server.
**Warning signs:** `[ALERT] WhatsApp alerting disabled -- missing EVOLUTION_URL` in Bono's logs.

### Pitfall 2: Phone Number Format for Evolution API
**What goes wrong:** Evolution API rejects the number because it has a `+` prefix or spaces.
**Why it happens:** Indian numbers could be stored as +91XXXXXXXXXX, 91XXXXXXXXXX, or 0XXXXXXXXXX.
**How to avoid:** Store Uday's number as digits-only with country code (e.g., `919876543210`). Strip any `+` or spaces in the AlertManager before sending.
**Warning signs:** Evolution API returns 400 or the message is delivered to the wrong number.

### Pitfall 3: Email Sent on Every Crash Instead of Only at Cooldown Cap
**What goes wrong:** Uday gets 5 emails in rapid succession during a crash loop before the 5-minute cooldown kicks in.
**Why it happens:** Checking `crashDetected` instead of checking if cooldown is at the final step.
**How to avoid:** The email fallback condition must check `watchdog.cooldown.attemptCount >= steps.length` (i.e., at or past the 5-minute cap). The CONTEXT.md explicitly says "only after hitting 5min cooldown cap."
**Warning signs:** Multiple `[ALERT] James DOWN` emails in quick succession.

### Pitfall 4: Alert Cooldown Not Reset on Recovery
**What goes wrong:** After James recovers, the next down-alert is suppressed because the alert cooldown window hasn't expired yet.
**Why it happens:** Alert cooldown was set when the first down-alert was sent, and recovery didn't reset it.
**How to avoid:** Reset the alert cooldown when James comes back up. A genuine new down event after recovery should always trigger an alert.
**Warning signs:** James goes down, Uday gets alerted, James recovers, James goes down again 2 minutes later, Uday gets no alert.

### Pitfall 5: Recovery Signal Lost if WebSocket Reconnects After Crash
**What goes wrong:** James crashes, WebSocket drops, James restarts, WebSocket reconnects, but the recovery signal is never sent because it was only triggered during the brief disconnected window.
**Why it happens:** The `self_test_passed` handler fires before WebSocket is fully reconnected.
**How to avoid:** The recovery signal should be sent AFTER the WebSocket is confirmed connected. Two approaches: (a) queue the recovery message via `client.send()` which already queues when disconnected and replays on reconnect, or (b) listen for the `open` event after reconnect and send recovery then. Option (a) is simpler since CommsClient already has message queuing (Phase 2, WS-05).
**Warning signs:** Bono sees james_up from heartbeat resumption but never receives the detailed recovery message with crash count/downtime.

### Pitfall 6: Testing HTTP Calls to Evolution API
**What goes wrong:** Tests make real HTTP calls to Evolution API, or tests are brittle because they mock at the wrong layer.
**Why it happens:** Not injecting the HTTP transport function.
**How to avoid:** AlertManager accepts an injectable `sendFn` (like the `collectFn` pattern in HeartbeatSender). Tests inject a mock that captures calls. Production injects the real `sendEvolutionText` function.
**Warning signs:** Tests fail when there's no internet, or tests accidentally send real WhatsApp messages.

## Code Examples

Verified patterns from the existing codebase:

### Existing Protocol Message Creation (shared/protocol.js)
```javascript
// Source: shared/protocol.js lines 23-31
export function createMessage(type, from, payload = {}) {
  return JSON.stringify({
    v: PROTOCOL_VERSION,
    type,
    from,
    ts: Date.now(),
    id: randomUUID(),
    payload,
  });
}
```

### Existing HeartbeatMonitor Events (bono/heartbeat-monitor.js)
```javascript
// Source: bono/heartbeat-monitor.js lines 39-55
receivedHeartbeat(payload) {
  this.#lastHeartbeat = Date.now();
  this.#lastPayload = payload;
  const wasDown = !this.#isUp;
  this.#isUp = true;
  if (wasDown) {
    this.emit('james_up', { timestamp: this.#lastHeartbeat, payload });
  }
  this.#resetTimeout();
}
// And in the timeout:
this.emit('james_down', { timestamp: Date.now(), lastHeartbeat: this.#lastHeartbeat });
```

### Existing DI Pattern for Testability (james/heartbeat-sender.js)
```javascript
// Source: james/heartbeat-sender.js lines 19-21
constructor(client, options = {}) {
  this.#client = client;
  this.#collectFn = options.collectFn || collectMetrics;
}
```

### Existing CommsClient Message Queuing (james/comms-client.js)
```javascript
// Source: james/comms-client.js lines 121-135
send(type, payload) {
  const msg = createMessage(type, 'james', payload);
  if (this.sm.state === State.CONNECTED && this.#ws?.readyState === WebSocket.OPEN) {
    this.#ws.send(msg);
    return true;
  }
  // Queue for later -- drop oldest if full
  if (this.#queue.length >= this.#maxQueueSize) {
    this.#queue.shift();
  }
  this.#queue.push(msg);
  return false;
}
```

### Existing EscalatingCooldown Cap Detection (james/watchdog.js)
```javascript
// Source: james/watchdog.js lines 138-141
get delay() {
  if (this.#attemptCount === 0) return 0;
  return this.#steps[Math.min(this.#attemptCount - 1, this.#steps.length - 1)];
}
// Steps: [5000, 15000, 30000, 60000, 300000]
// At cap when: attemptCount >= steps.length (5) -> delay is 300000 (5min)
```

### Bono index.js WS Message Handling (bono/index.js)
```javascript
// Source: bono/index.js lines 16-19
wss.on('message', (msg, ws) => {
  if (msg.type === 'heartbeat') {
    monitor.receivedHeartbeat(msg.payload);
  }
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Email-only alerting | WhatsApp primary + email fallback | Phase 6 | Uday gets instant phone notification instead of checking email |
| Alert on every crash | Flapping suppression with cooldown | Phase 6 | No alert floods during crash loops |
| Bono detects down only via heartbeat timeout | Heartbeat timeout (down) + explicit recovery signal (up) | Phase 6 | Bono knows exact downtime duration and crash count, not just "heartbeat resumed" |
| Single notification channel | State-based channel selection (WS connected -> WhatsApp, WS down -> email) | Phase 6 | Always-notified regardless of link state |

**Evolution API specifics:**
- Endpoint: `POST /message/sendText/{instance}`
- Auth: `apikey` header (not Bearer token)
- Body: `{ "number": "919876543210", "text": "message" }`
- Response: 201 Created with message key and PENDING status
- Phone number format: digits with country code, no `+` prefix

## Open Questions

1. **Evolution API credentials**
   - What we know: Bono runs Evolution API on VPS. Need instance name, API key, and Uday's WhatsApp number.
   - What's unclear: Whether Evolution API is already deployed on Bono's VPS, what the base URL is.
   - Recommendation: Plan with env var placeholders (`EVOLUTION_URL`, `EVOLUTION_INSTANCE`, `EVOLUTION_API_KEY`, `UDAY_WHATSAPP`). Coordinate with Bono before execution. AlertManager should gracefully degrade if vars are missing.

2. **Alert suppression window duration**
   - What we know: Must suppress during rapid crash/restart cycles. EscalatingCooldown reaches 5min cap after 5 attempts.
   - What's unclear: How long to suppress WhatsApp alerts on Bono's side.
   - Recommendation: Use 5-minute fixed window on Bono (matches the restart cooldown cap). This means: first down-alert goes through immediately, subsequent within 5 minutes are suppressed. Reset on recovery. This is simple and aligns with the restart cooldown semantics.

3. **Crash timestamp tracking for downtime calculation**
   - What we know: Recovery message should include downtime duration. The `crash_detected` event has a timestamp.
   - What's unclear: Where to store the crash start time so the `self_test_passed` handler can compute duration.
   - Recommendation: Track `#lastCrashTimestamp` in watchdog-runner.js (set on `crash_detected`, used in `self_test_passed` to compute `downtimeMs = Date.now() - lastCrashTimestamp`). Simple module-level variable.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node.js v22.14.0) |
| Config file | None -- uses package.json `test` script |
| Quick run command | `node --test test/alerting.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AL-01 | AlertManager.handleJamesDown sends WhatsApp via injected sendFn | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-01 | AlertManager constructs correct down-alert message format | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-02 | AlertManager.handleRecovery sends WhatsApp via injected sendFn | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-02 | AlertManager constructs correct back-online message format | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-02 | Recovery message sent over WebSocket via client.send('recovery', ...) | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-03 | Email sent when CommsClient.state is DISCONNECTED and cooldown at cap | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-03 | Email NOT sent when CommsClient is CONNECTED (uses WS path instead) | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-03 | Email sent to BOTH Uday and Bono with detailed body | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-04 | AlertCooldown.canSend() returns false within suppression window | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-04 | AlertCooldown.canSend() returns true after window expires | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-04 | AlertCooldown resets on recovery (new down event after recovery triggers alert) | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-04 | James email suppressed -- only one per cooldown cap cycle | unit | `node --test test/alerting.test.js` | Wave 0 |
| AL-01 | sendEvolutionText sends correct HTTP POST to Evolution API endpoint | unit | `node --test test/alerting.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/alerting.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (97 existing + new alerting tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/alerting.test.js` -- new test file covering all AL-01 through AL-04 behaviors
- [ ] `bono/alert-manager.js` -- AlertManager class (tested via DI, no real HTTP calls)
- [ ] `shared/protocol.js` -- extend MessageType with `recovery` (update existing protocol.test.js if needed)

## Sources

### Primary (HIGH confidence)
- Existing codebase: `james/watchdog-runner.js` (176 lines), `james/watchdog.js` (305 lines), `james/comms-client.js` (181 lines), `bono/heartbeat-monitor.js` (84 lines), `bono/index.js` (43 lines), `shared/protocol.js` (52 lines)
- Existing tests: 11 test files, 97 tests passing, `test/watchdog-runner.test.js` (204 lines) demonstrates the DI/mock patterns
- Evolution API official docs: [Send Plain Text](https://doc.evolution-api.com/v2/api-reference/message-controller/send-text) -- POST /message/sendText/{instance}, apikey header, number + text body fields

### Secondary (MEDIUM confidence)
- [Evolution API GitHub](https://github.com/EvolutionAPI/evolution-api) -- open-source WhatsApp integration, Baileys protocol
- Evolution API phone number format: digits with country code, no + prefix (from official docs example)

### Tertiary (LOW confidence)
- None -- all findings verified against official docs or existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - zero new dependencies, all built-in Node.js modules
- Architecture: HIGH - all integration points verified in existing codebase (HeartbeatMonitor events, CommsClient state, EscalatingCooldown cap, protocol message types)
- Pitfalls: HIGH - identified from direct code analysis (message queuing for recovery, cooldown cap check, env var validation)
- Evolution API: MEDIUM - official docs verified, but instance details not yet available from Bono

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- no fast-moving dependencies)
