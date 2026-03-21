# Phase 8: Coordination & Daily Ops - Research

**Researched:** 2026-03-12
**Domain:** Real-time AI-to-AI coordination protocol, scheduled health summaries, legacy system retirement
**Confidence:** HIGH

## Summary

Phase 8 is the final phase of the comms-link project. It adds three capabilities on top of the solid foundation built in Phases 1-7: (1) structured coordination messages so James and Bono can delegate tasks, query status, and send notifications -- not just heartbeats; (2) twice-daily health summaries to Uday via WhatsApp and email; (3) retirement of Bono's legacy `[FAILSAFE]` heartbeat to eliminate duplicate monitoring.

The codebase is well-structured for extension. The `shared/protocol.js` MessageType enum is the single source of truth for message types, and `createMessage()`/`parseMessage()` handle all serialization. The `wireBono()` and `wireRunner()` functions are the established pattern for routing new message types -- adding coordination messages follows the exact same `wss.on('message', ...)` / `client.on('message', ...)` routing pattern used for heartbeat, recovery, and file_sync. The project has 178 passing tests using `node:test` with zero external test dependencies, and the DI-via-constructor pattern makes all new classes testable.

**Primary recommendation:** Extend protocol.js with new coordination message types (task_request, task_response, status_query, status_response, daily_report), add a DailySummaryScheduler class on Bono's side with a pure-setTimeout scheduler (no new npm deps), and create a PROTOCOL.md with Mermaid diagrams documenting all message flows.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Full coordination suite: task delegation, status queries, and one-way notifications
- Fully bidirectional -- both James and Bono can initiate any message type
- Hybrid message format: typed commands for common operations (deploy, check-status, restart) + generic `message` type for freeform coordination
- The existing `message` type in protocol.js is available for freeform; add new typed commands for structured operations
- Twice daily health summary: morning (9:00 AM IST) + evening (11:00 PM IST)
- Metrics: uptime percentage, restart count, connection stability (reconnection count, longest disconnect, latency), pod/venue status
- Both channels: WhatsApp (minimal one-liner) + email (detailed with tables)
- Both AIs contribute: Bono aggregates connection/uptime from heartbeat monitoring, James adds pod/venue status. Bono computes and sends combined summary
- James sends a `daily_report` coordination message to Bono with pod/venue data before each summary window
- Must ensure no gap in monitoring coverage during transition -- new system must fully replace [FAILSAFE] before removed
- Must include Mermaid sequence diagrams showing message flows
- Documentation must be agreed/usable by both AIs as a reference

### Claude's Discretion
- Task request flow pattern (immediate execute vs ack-then-execute) -- pick based on existing ack patterns in codebase
- Specific typed command names for protocol.js
- [FAILSAFE] retirement approach (clean replacement vs gradual integration)
- [FAILSAFE] scope (comms-link only vs both-sides coordination)
- Protocol doc format and location
- Health summary scheduling mechanism (cron-like timer, setInterval, etc.)
- How pod/venue status data is collected and transmitted from James to Bono

### Deferred Ideas (OUT OF SCOPE)
- Web-based status dashboard for Uday (EM-01, v2)
- Historical uptime tracking and graphs (EM-02, v2)
- Connection latency monitoring (EM-03, v2)
- Sync additional shared files beyond LOGBOOK.md (AS-01, v2)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CO-01 | Bidirectional real-time messaging for AI-to-AI coordination (not just heartbeat) | New MessageType entries in protocol.js, wireCoordination() on both sides, ack-then-execute pattern matching existing file_ack flow |
| CO-02 | Coordinate with Bono to implement WebSocket server on VPS | Email Bono with PROTOCOL.md specifying new message types + expected routing in wireBono(); Bono-side changes are documented but implemented by Bono |
| CO-03 | Coordinate with Bono to retire/integrate existing [FAILSAFE] heartbeat mechanism | Recommend clean replacement: email Bono transition instructions, [FAILSAFE] disabled once comms-link heartbeat is verified stable |
| AL-05 | Daily health summary -- uptime percentage, restart count, connection stability | DailySummaryScheduler class on Bono side, HealthAccumulator for metrics collection, WhatsApp one-liner + email detailed table |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node:test | built-in (Node 22) | Test runner | Already in use across 178 tests, 16 files, zero deps |
| node:events | built-in | EventEmitter for lifecycle events | Already the backbone of every class in the project |
| node:timers | built-in (setTimeout) | Daily summary scheduling | Zero-dep approach; project policy is one npm dep (`ws`) only |
| ws | ^8.19.0 | WebSocket transport | Already installed, sole npm dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| send_email.js (racecontrol) | N/A | Gmail API email delivery | For detailed health summary emails (via execFile, fire-and-forget) |
| sendEvolutionText (alert-manager.js) | N/A | WhatsApp delivery | For one-liner health summary WhatsApp messages |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Pure setTimeout scheduler | node-cron | Adds a dependency for 15 lines of code; project strictly avoids new deps |
| Custom accumulator | No alternative | Simple counter/max tracker, no library needed |

**Installation:**
```bash
# No new packages needed -- everything built on existing stack
```

## Architecture Patterns

### Recommended Project Structure
```
shared/
  protocol.js           # ADD: new MessageType entries (task_request, task_response, etc.)
  protocol.js           # EXISTING: createMessage(), parseMessage() unchanged
bono/
  index.js              # EXTEND: wireBono() to route coordination messages
  index.js              # ADD: wireCoordination() or inline in wireBono()
  health-accumulator.js # NEW: tracks uptime, restarts, reconnections over time
  daily-summary.js      # NEW: DailySummaryScheduler -- schedule + format + send
james/
  watchdog-runner.js    # EXTEND: wireRunner() to handle coordination messages
  watchdog-runner.js    # ADD: pod-status collection + daily_report sending
test/
  coordination.test.js  # NEW: coordination message routing tests
  daily-summary.test.js # NEW: DailySummaryScheduler + HealthAccumulator tests
docs/
  PROTOCOL.md           # NEW: Full protocol reference with Mermaid diagrams
```

### Pattern 1: Protocol Extension (Adding New Message Types)
**What:** Add new entries to the frozen MessageType enum in protocol.js
**When to use:** Every time a new message flow is needed
**Example:**
```javascript
// Source: shared/protocol.js (existing pattern)
export const MessageType = Object.freeze({
  // Existing
  echo: 'echo',
  echo_reply: 'echo_reply',
  heartbeat: 'heartbeat',
  heartbeat_ack: 'heartbeat_ack',
  status: 'status',
  recovery: 'recovery',
  file_sync: 'file_sync',
  file_ack: 'file_ack',
  message: 'message',
  // NEW: Coordination types
  task_request: 'task_request',
  task_response: 'task_response',
  status_query: 'status_query',
  status_response: 'status_response',
  daily_report: 'daily_report',
});
```

### Pattern 2: Ack-Then-Execute (Task Request Flow)
**What:** Sender sends task_request, receiver sends immediate ack (task_response with status: 'accepted'), then executes, then sends final task_response with result.
**When to use:** For operations that take time (deploy, restart). Mirrors the file_sync -> file_ack flow already established.
**Why this pattern:** The codebase already uses ack-then-proceed for file_sync. The `pendingAck` gate and `ackTimeout` pattern in logbook sync is the template. Task requests follow the same discipline: ack receipt, then do work, then report result.
**Example:**
```javascript
// James sends task request
client.send('task_request', {
  taskId: randomUUID(),
  command: 'deploy',
  target: 'rc-agent',
  args: { pod: 8, version: 'latest' },
});

// Bono acks immediately
ws.send(createMessage('task_response', 'bono', {
  taskId: msg.payload.taskId,
  status: 'accepted',
}));

// ...later, Bono sends result
ws.send(createMessage('task_response', 'bono', {
  taskId: msg.payload.taskId,
  status: 'completed',  // or 'failed'
  result: { ... },
}));
```

### Pattern 3: DI Constructor for Scheduling (DailySummaryScheduler)
**What:** All time-dependent behavior injected via constructor (nowFn, scheduleFn/setTimeoutFn) for testability
**When to use:** For the daily summary scheduler -- tests must not wait for real clock
**Example:**
```javascript
// Source: follows AlertCooldown DI pattern from bono/alert-manager.js
export class DailySummaryScheduler extends EventEmitter {
  #nowFn;
  #setTimeoutFn;
  #timer = null;

  constructor({ nowFn = Date.now, setTimeoutFn = setTimeout, ...opts } = {}) {
    super();
    this.#nowFn = nowFn;
    this.#setTimeoutFn = setTimeoutFn;
    // ...
  }

  start() {
    this.#scheduleNext();
  }

  #scheduleNext() {
    const msUntilNext = this.#msUntilNextWindow();
    this.#timer = this.#setTimeoutFn(() => {
      this.emit('summary_due', { window: this.#currentWindow() });
      this.#scheduleNext(); // re-arm for next window
    }, msUntilNext);
  }
}
```

### Pattern 4: Health Accumulator (Metrics Collection)
**What:** A stateful class that listens to HeartbeatMonitor and ConnectionStateMachine events, accumulating uptime/restart/reconnection metrics over a rolling period.
**When to use:** For building the daily health summary data that Bono aggregates.
**Example:**
```javascript
export class HealthAccumulator {
  #restartCount = 0;
  #reconnectionCount = 0;
  #longestDisconnectMs = 0;
  #lastDisconnectTime = null;
  #uptimeStart;

  constructor({ nowFn = Date.now } = {}) {
    this.#uptimeStart = nowFn();
  }

  recordRestart() { this.#restartCount++; }
  recordDisconnect(ts) { this.#lastDisconnectTime = ts; }
  recordReconnect(ts) {
    this.#reconnectionCount++;
    if (this.#lastDisconnectTime) {
      const gap = ts - this.#lastDisconnectTime;
      if (gap > this.#longestDisconnectMs) this.#longestDisconnectMs = gap;
      this.#lastDisconnectTime = null;
    }
  }

  snapshot(nowTs) {
    const totalMs = nowTs - this.#uptimeStart;
    // uptime% = (total - total disconnect time) / total * 100
    return {
      uptimePercent: /* computed */,
      restartCount: this.#restartCount,
      reconnectionCount: this.#reconnectionCount,
      longestDisconnectMs: this.#longestDisconnectMs,
    };
  }

  reset(nowTs) { /* reset counters for new period */ }
}
```

### Pattern 5: Pure-setTimeout Daily Scheduler
**What:** Calculate milliseconds until the next target time (9:00 AM IST or 11:00 PM IST), use a single setTimeout, re-arm on fire.
**When to use:** Instead of node-cron or setInterval for twice-daily tasks.
**Why:** The project uses zero external deps beyond `ws`. A cron library would add a dependency for 15 lines of code. setTimeout is more testable with injected `setTimeoutFn`.
**Example:**
```javascript
function msUntilTime(hour, minute, nowFn = Date.now) {
  const now = new Date(nowFn());
  // Target time in IST (UTC+5:30)
  const target = new Date(now.toLocaleString('en-US', { timeZone: 'Asia/Kolkata' }));
  target.setHours(hour, minute, 0, 0);
  let ms = target.getTime() - now.getTime();
  if (ms <= 0) ms += 24 * 60 * 60 * 1000; // next day
  return ms;
}
```
**Caveat:** IST conversion uses `toLocaleString` with timezone, which is reliable in Node.js 22 (full ICU data included by default). The re-arm pattern (setTimeout -> callback -> setTimeout) avoids setInterval drift over long periods.

### Anti-Patterns to Avoid
- **Adding message types without updating protocol.js MessageType:** Every new type MUST be in the frozen enum. The existing test `protocol.test.js` verifies all types exist.
- **Routing coordination messages in comms-server.js:** The server is a dumb relay. All message routing goes in wireBono() and wireRunner(). comms-server.js only handles echo and emits `wss.emit('message', msg, ws)` for everything else.
- **Using setInterval for daily scheduling:** setInterval(86400000) drifts due to event loop delays and DST changes. Use chained setTimeout with recalculated delay.
- **Blocking on task execution:** Task responses should be fire-and-forget. If Bono sends a deploy task, James should ack immediately and execute asynchronously. Never block the message handler.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WhatsApp delivery | HTTP client for Evolution API | `sendEvolutionText()` from alert-manager.js | Already handles timeouts, error recovery, DI for testing |
| Email delivery | SMTP client or Gmail API wrapper | `send_email.js` via execFile (racecontrol) | Already proven, handles OAuth refresh, fire-and-forget via child process |
| Message serialization | Custom wire format | `createMessage()` / `parseMessage()` | Standard envelope with version, type, from, ts, id, payload |
| WebSocket routing | New routing layer | Extend `wireBono()` / `wireRunner()` with `if (msg.type === 'task_request')` | Established pattern across 3 phases of wiring code |
| Connection state tracking | New state machine | Existing `HeartbeatMonitor.isUp` + `ConnectionStateMachine` | Already tracks up/down transitions with events |

**Key insight:** Phase 8 is almost entirely about wiring new message types into existing infrastructure. The transport, auth, reconnection, alerting, and file sync are all done. The work is: define types, route them, accumulate health data, schedule summaries.

## Common Pitfalls

### Pitfall 1: Duplicate Monitoring (FAILSAFE + Comms-Link)
**What goes wrong:** Bono's WhatsApp bot has a `[FAILSAFE]` heartbeat that monitors James via email. The comms-link HeartbeatMonitor + AlertManager already monitors James via WebSocket. Both fire alerts to Uday simultaneously, causing alert fatigue.
**Why it happens:** Two independent systems monitoring the same thing. The [FAILSAFE] uses slow email; comms-link uses fast WebSocket. They fire at different times with different wording.
**How to avoid:** The comms-link REPLACES [FAILSAFE] for James-health monitoring. Email Bono specific instructions: once comms-link heartbeat is confirmed stable for 24h, disable [FAILSAFE] James-monitoring in the WhatsApp bot. Keep [FAILSAFE] as a dormant fallback that only activates if the comms-link WebSocket itself goes down for >1h.
**Warning signs:** Uday receives two alerts for the same event. Ask him.

### Pitfall 2: Timezone Calculation Errors for IST Scheduling
**What goes wrong:** Daily summary fires at the wrong time because IST offset is hardcoded as UTC+5:30 but the system clock uses a different timezone. Or DST-like edge cases in the conversion (India does not observe DST, but the Node.js `toLocaleString` with `Asia/Kolkata` handles this correctly).
**Why it happens:** Mixing UTC arithmetic with local time, or using `new Date().setHours()` which sets hours in the local timezone of the machine (which is IST on James but may be UTC on Bono's VPS).
**How to avoid:** Always calculate IST target time using `toLocaleString('en-US', { timeZone: 'Asia/Kolkata' })` and then compute the delta from `Date.now()`. This works regardless of the machine's local timezone.
**Warning signs:** Summary arrives at unexpected times. Log the computed delay in minutes when scheduling.

### Pitfall 3: Timer Accumulation on Reconnect
**What goes wrong:** Every time the WebSocket reconnects, coordination wiring re-registers message handlers. If handlers are registered in `client.on('open', ...)`, they accumulate with each reconnect, leading to duplicate message processing.
**Why it happens:** EventEmitter `.on()` adds listeners, it does not replace them. The existing `wireRunner()` avoids this by registering handlers once at wiring time, not per-connection.
**How to avoid:** Follow the established pattern: register all message handlers once in the wire function, outside any `open`/`close` listeners. The CommsClient already re-emits `message` events after reconnect -- handlers wired once will still receive them.
**Warning signs:** `console.log` lines appear multiple times for a single message. Use `EventEmitter.listenerCount()` assertions in tests.

### Pitfall 4: Pod Status Collection Blocking James
**What goes wrong:** James collects pod/venue status by hitting rc-core's HTTP API or querying all 8 pods. If a pod is unresponsive, the HTTP request hangs, blocking the daily_report message from being sent before the summary window.
**Why it happens:** Synchronous or poorly-timed HTTP requests to pod-agents that may be offline.
**How to avoid:** Pod status collection should have aggressive timeouts (2s per pod) and use `Promise.allSettled()` to get whatever data is available. Missing pod data should be reported as "unknown" rather than blocking the report.
**Warning signs:** daily_report arrives late or not at all before the summary window.

### Pitfall 5: Health Accumulator Not Resetting Between Periods
**What goes wrong:** The HealthAccumulator tracks restarts and reconnections since the process started, but the daily summary should show metrics for the reporting period (overnight or daytime). Without a reset, the numbers grow monotonically and the summary shows cumulative data rather than period-specific data.
**Why it happens:** No clear "period boundary" logic.
**How to avoid:** The DailySummaryScheduler should call `accumulator.snapshot()` (get current values) then `accumulator.reset()` after sending the summary. The snapshot captures the period's data; the reset starts a fresh accumulation for the next period.
**Warning signs:** "5 restarts" in the morning summary, then "5 restarts" again in the evening even though none occurred during the day.

## Code Examples

Verified patterns from this codebase (not external sources):

### Adding a New Message Type and Routing It (Bono Side)
```javascript
// In shared/protocol.js -- add to MessageType
task_request: 'task_request',
task_response: 'task_response',

// In bono/index.js wireBono() -- add routing
wss.on('message', (msg, ws) => {
  // ... existing heartbeat, recovery routing ...

  if (msg.type === 'task_request') {
    // Ack immediately
    ws.send(createMessage('task_response', 'bono', {
      taskId: msg.payload.taskId,
      status: 'accepted',
    }));
    // Execute asynchronously -- fire and forget
    handleTaskRequest(msg.payload).catch((err) => {
      ws.send(createMessage('task_response', 'bono', {
        taskId: msg.payload.taskId,
        status: 'failed',
        error: err.message,
      }));
    });
  }

  if (msg.type === 'daily_report') {
    // James sends pod/venue status before summary window
    handleDailyReport(msg.payload);
  }
});
```

### Sending a Coordination Message (James Side)
```javascript
// In james/watchdog-runner.js -- add to wireRunner or new wireCoordination
client.on('message', (msg) => {
  if (msg.type === 'task_request') {
    // Bono asked James to do something
    const { taskId, command, args } = msg.payload;
    client.send('task_response', { taskId, status: 'accepted' });
    executeTask(command, args)
      .then((result) => client.send('task_response', { taskId, status: 'completed', result }))
      .catch((err) => client.send('task_response', { taskId, status: 'failed', error: err.message }));
  }

  if (msg.type === 'status_query') {
    // Bono (or James) queries the other's status
    const status = collectCurrentStatus();
    client.send('status_response', {
      queryId: msg.payload.queryId,
      ...status,
    });
  }
});
```

### WhatsApp One-Liner Health Summary (Follows Phase 6 Style)
```javascript
// Follows the emoji-prefix pattern from AlertManager
const morningText = `\u{1F4CA} Daily Report 09:00\n` +
  `Uptime: ${uptimePercent}% | Restarts: ${restartCount}\n` +
  `Reconnects: ${reconnectCount} | Max gap: ${maxGapMin}min\n` +
  `Pods: ${podSummary}`;

// Example output:
// \u{1F4CA} Daily Report 09:00
// Uptime: 99.2% | Restarts: 1
// Reconnects: 3 | Max gap: 2min
// Pods: 8/8 online
```

### Email Detailed Health Summary
```javascript
// Uses send_email.js via execFile (same as watchdog-runner.js)
const emailBody = [
  'James-Bono Comms Link Health Summary',
  `Period: ${periodStart} to ${periodEnd}`,
  '',
  'CONNECTION',
  `  Uptime: ${uptimePercent}%`,
  `  Restarts: ${restartCount}`,
  `  Reconnections: ${reconnectCount}`,
  `  Longest disconnect: ${longestDisconnectMin}min`,
  '',
  'POD STATUS',
  ...podLines.map(p => `  Pod ${p.id}: ${p.status}`),
  '',
  'Generated by Bono at ' + new Date().toISOString(),
].join('\n');

execFileFn('node', [sendEmailPath, 'usingh@racingpoint.in', subject, emailBody], { timeout: 30000 }, ...);
```

## Coordination Protocol Design Decisions

### Task Request Flow: Ack-Then-Execute
**Recommendation:** Use the ack-then-execute pattern, consistent with file_sync -> file_ack.

The flow:
1. Sender sends `task_request` with `{ taskId, command, target, args }`
2. Receiver sends `task_response` with `{ taskId, status: 'accepted' }` immediately
3. Receiver executes the command asynchronously
4. Receiver sends `task_response` with `{ taskId, status: 'completed', result }` or `{ taskId, status: 'failed', error }`

This matches the existing ack gate pattern from logbook sync and avoids blocking the message handler.

### Typed Command Names
**Recommendation:** Use these specific typed commands:

| MessageType | Direction | Purpose |
|-------------|-----------|---------|
| `task_request` | Bidirectional | Request the other AI to perform an operation |
| `task_response` | Bidirectional | Ack + result for a task_request |
| `status_query` | Bidirectional | Ask for current operational status |
| `status_response` | Bidirectional | Reply with current status snapshot |
| `daily_report` | James -> Bono | Pod/venue status data for health summary |
| `message` | Bidirectional | Freeform text coordination (EXISTING) |

The `command` field inside `task_request.payload` provides the typed operations: `deploy`, `check-status`, `restart`, `check-pod`, etc. This avoids creating a new MessageType for every operation while keeping the protocol extensible.

### [FAILSAFE] Retirement: Clean Replacement
**Recommendation:** Clean replacement, not gradual integration.

**Rationale:**
1. The comms-link HeartbeatMonitor + AlertManager already does everything [FAILSAFE] does, but faster (15s heartbeat vs email-based detection).
2. Gradual integration means maintaining two codepaths with complex logic ("if comms-link is up use it, else fall back to [FAILSAFE]"). This adds complexity for a transition that takes one day.
3. The comms-link has been stable through Phases 1-7 with 178 tests. It is ready to be the single source of truth.

**Transition plan:**
1. Phase 8 implements coordination messages and daily summary -- proving the comms-link can do everything.
2. Email Bono with explicit instructions: "Disable [FAILSAFE] James-monitoring in the WhatsApp bot config. The comms-link HeartbeatMonitor on port 8765 is now the sole monitoring path."
3. Keep the [FAILSAFE] code dormant (commented out or feature-flagged) for 1 week as insurance.
4. After 1 week with no issues, remove [FAILSAFE] code entirely.

**Scope:** This is a Bono-side change. James's comms-link code does not need to know about [FAILSAFE]. The CO-03 requirement is fulfilled by emailing Bono with the transition instructions and verifying he confirms the change.

### Health Summary Scheduling
**Recommendation:** Pure setTimeout with chained re-arm.

The DailySummaryScheduler computes milliseconds until the next summary window (9:00 AM IST or 11:00 PM IST), sets a single setTimeout, fires the summary, then recomputes and re-arms. This avoids:
- setInterval drift (86400000ms accumulates ~100ms/day drift)
- External dependencies (node-cron, node-schedule)
- DST issues (India does not observe DST, and we use `Asia/Kolkata` timezone explicitly)

The scheduler lives on Bono's side because Bono aggregates both his own connection data and James's pod data. James sends a `daily_report` message ~5 minutes before each window (8:55 AM and 10:55 PM IST) with current pod status. Bono combines this with his HealthAccumulator data and sends the summary.

### Pod/Venue Status Collection (James Side)
**Recommendation:** James collects pod status from rc-core's `/api/pods` endpoint (running on 192.168.31.23:8080) with a 5s timeout. This gives a snapshot of all 8 pods' online/offline status.

If rc-core is unreachable, James sends `{ podsAvailable: false, reason: 'rc-core unreachable' }` in the daily_report. Bono's summary reports "Pod status unavailable" rather than blocking.

The collection should use `http.get()` (built-in) since rc-core is on the LAN. No new dependencies needed.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Email-only communication | WebSocket real-time + email fallback | Phase 1-7 (this project) | Sub-second messaging vs minutes |
| [FAILSAFE] email heartbeat | HeartbeatMonitor WebSocket heartbeat | Phase 3 (HB-01 through HB-04) | 15s detection vs minutes |
| No coordination protocol | Typed messages with ack flow | Phase 8 (this phase) | Structured task delegation |
| No health reporting | Twice-daily WhatsApp + email summary | Phase 8 (this phase) | Proactive monitoring for Uday |

**Deprecated/outdated:**
- `[FAILSAFE]` heartbeat in WhatsApp bot: Replaced by comms-link HeartbeatMonitor. To be retired in Phase 8.
- Email-as-primary-communication: Now secondary/fallback only. WebSocket is primary.

## Open Questions

1. **RC-Core API endpoint for pod status**
   - What we know: rc-core runs on .23:8080 and manages all 8 pods
   - What's unclear: The exact endpoint path and response format for pod status
   - Recommendation: Use a simple HTTP GET with a 5s timeout. If the endpoint does not exist yet, James can report `podsAvailable: false` and this can be wired later. Do NOT block Phase 8 on rc-core API work.

2. **Bono's WhatsApp bot codebase location**
   - What we know: [FAILSAFE] lives in Bono's WhatsApp bot on the VPS, managed by PM2
   - What's unclear: Exact file/config to modify to disable [FAILSAFE]
   - Recommendation: Email Bono with the transition instructions. He knows his codebase. CO-03 is fulfilled by coordination (email), not by modifying Bono's code from James's side.

3. **Daily report timing precision**
   - What we know: James should send pod data ~5 min before summary window
   - What's unclear: What if James's WebSocket is down at that exact time?
   - Recommendation: Bono should proceed with whatever data it has. If no daily_report received, include "Pod status: unavailable (James offline)" in the summary. The summary must never be blocked by missing data.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | node:test (built-in, Node.js 22.14.0) |
| Config file | none -- scripts.test in package.json |
| Quick run command | `node --test test/coordination.test.js test/daily-summary.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CO-01 | Bidirectional coordination messages routed correctly | unit | `node --test test/coordination.test.js` | No -- Wave 0 |
| CO-01 | task_request -> ack -> result flow works end-to-end | unit | `node --test test/coordination.test.js` | No -- Wave 0 |
| CO-02 | Protocol documented with all message types | manual-only | Review PROTOCOL.md | N/A |
| CO-03 | [FAILSAFE] retirement instructions documented | manual-only | Review email/doc to Bono | N/A |
| AL-05 | DailySummaryScheduler fires at correct IST times | unit | `node --test test/daily-summary.test.js` | No -- Wave 0 |
| AL-05 | HealthAccumulator tracks uptime/restarts/reconnections | unit | `node --test test/daily-summary.test.js` | No -- Wave 0 |
| AL-05 | WhatsApp one-liner formatted correctly | unit | `node --test test/daily-summary.test.js` | No -- Wave 0 |
| AL-05 | Email detailed summary formatted correctly | unit | `node --test test/daily-summary.test.js` | No -- Wave 0 |
| AL-05 | James sends daily_report before summary window | unit | `node --test test/coordination.test.js` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/coordination.test.js test/daily-summary.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (currently 178 tests + new tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/coordination.test.js` -- covers CO-01 (message routing, ack flow, bidirectional)
- [ ] `test/daily-summary.test.js` -- covers AL-05 (scheduler timing, accumulator, formatting)
- No framework install needed -- node:test already in use
- No shared fixtures needed -- existing mock patterns (makeMockWss, makeMockMonitor) can be reused

## Sources

### Primary (HIGH confidence)
- Codebase analysis: shared/protocol.js, bono/index.js, james/watchdog-runner.js, bono/alert-manager.js, bono/heartbeat-monitor.js -- direct code review of all 14 source files
- Existing test patterns: 16 test files, 178 tests -- established DI/mock patterns
- Node.js 22 timers documentation: setTimeout/setInterval behavior
- .planning/research/PITFALLS.md -- Pitfall #6 (duplicate alerting) directly informs [FAILSAFE] retirement

### Secondary (MEDIUM confidence)
- [setInterval vs Cron Job in Node.js](https://www.sabbir.co/blogs/68e2852ae6f20e639fc2c9bc) -- confirms setTimeout chaining is preferred for precise daily scheduling
- [Node.js Timers guide](https://nodejs.org/en/docs/guides/timers-in-node) -- official Node.js timer behavior documentation
- [How to run a Javascript function at a specific time of day](https://gist.github.com/farhad-taran/f487a07c16fd53ee08a12a90cdaea082) -- pure setTimeout daily scheduling pattern

### Tertiary (LOW confidence)
- None -- all findings verified against codebase or official docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, everything built on existing proven stack
- Architecture: HIGH -- extending established patterns (wireBono, wireRunner, MessageType, DI constructors)
- Pitfalls: HIGH -- duplicate alerting pitfall directly observed in research; timezone issues well-understood for IST (no DST)
- Coordination protocol: HIGH -- ack-then-execute matches existing file_sync pattern; typed commands extensible

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no fast-moving dependencies)
