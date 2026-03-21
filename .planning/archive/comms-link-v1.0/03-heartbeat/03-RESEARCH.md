# Phase 3: Heartbeat - Research

**Researched:** 2026-03-12
**Domain:** Application-level heartbeat with process status and system metrics (Node.js, ws library, Windows process detection)
**Confidence:** HIGH

## Summary

Phase 3 adds a periodic application-level heartbeat from James (client) to Bono (server). Every 15 seconds, James sends a `heartbeat` message containing Claude Code process status (running/stopped) and system metrics (CPU usage, memory usage, uptime). Bono tracks the last received heartbeat timestamp and marks James as DOWN if no heartbeat arrives within 45 seconds (3 missed beats). This is distinct from the WebSocket-level ping/pong already implemented in `comms-server.js` (line 88-100) -- that detects dead TCP connections at the transport layer, while this heartbeat carries application-level health data.

The implementation touches three areas: (1) a new `HeartbeatSender` module on James's side that collects metrics and sends heartbeat messages via `CommsClient.send()`, (2) a new `HeartbeatMonitor` module on Bono's side that tracks received heartbeats and emits a `james_down`/`james_up` event after the 45-second timeout, and (3) a `SystemMetrics` collector module that gathers CPU usage (via `os.cpus()` delta sampling), memory usage (`os.freemem()`/`os.totalmem()`), system uptime (`os.uptime()`), and Claude Code process status (via `tasklist /FI "IMAGENAME eq claude.exe"`).

The protocol already defines `heartbeat` and `heartbeat_ack` message types in `shared/protocol.js`. CPU usage on Windows requires the two-sample delta approach since `os.loadavg()` always returns `[0, 0, 0]` on Windows. Claude Code runs as `claude.exe` on this machine (verified: PID 21840, process name "claude"), installed via the Microsoft Store package (`Claude_pzs8sxrjxfjjc`). Process detection uses `child_process.execFile('tasklist', ...)` which is safe against shell injection.

**Primary recommendation:** Create three small modules: `james/heartbeat-sender.js` (setInterval + send), `james/system-metrics.js` (CPU/mem/uptime/process detection), and `bono/heartbeat-monitor.js` (timeout tracking + up/down events). Keep them decoupled from the WebSocket layer -- they consume/produce via `CommsClient.send()` and `wss.on('message')`. Do NOT modify existing files except to wire up the new modules in `james/index.js` and `bono/index.js`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HB-01 | Application-level heartbeat ping every 15 seconds from James | `setInterval(sendHeartbeat, 15000)` using existing `CommsClient.send('heartbeat', payload)`. Heartbeat type already defined in `shared/protocol.js`. Only send when CONNECTED (send() already handles this -- returns false and queues when disconnected). Do NOT queue heartbeats -- stale heartbeats are worthless. |
| HB-02 | Bono detects missing heartbeat within 45 seconds and marks James as DOWN | Server-side `HeartbeatMonitor` with `setTimeout` reset on each received heartbeat. 45s = 3 missed beats. Emit `james_down` event. Reset timer and emit `james_up` when heartbeat resumes. |
| HB-03 | Heartbeat payload includes Claude Code process status (running/stopped) | `child_process.execFile('tasklist', ['/NH', '/FI', 'IMAGENAME eq claude.exe'])` -- check if output contains "claude.exe". Returns boolean. Async, non-blocking, ~50ms on Windows. |
| HB-04 | Heartbeat payload includes system metrics (CPU usage, memory, uptime) | `os.cpus()` delta for CPU% (Windows-specific: loadavg is always [0,0,0]), `os.freemem()/os.totalmem()` for memory%, `os.uptime()` for system uptime. All Node.js built-ins, zero dependencies. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ws` | ^8.19.0 | WebSocket transport (already installed) | No additional dependency. Heartbeat messages use existing `send()` method. |
| Node.js `os` | built-in (v22.14.0) | System metrics: CPU, memory, uptime | `os.cpus()` for CPU sampling, `os.freemem()`/`os.totalmem()` for memory, `os.uptime()` for uptime. All cross-platform but CPU calculation needs Windows-specific handling. |
| Node.js `child_process` | built-in (v22.14.0) | Claude Code process detection | `execFile('tasklist', ...)` to check for `claude.exe`. Safer than shell-based alternatives (no shell injection). |
| Node.js `node:test` | built-in (v22.14.0) | Test runner | Same as Phase 1 and 2. Zero external test deps. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `node:timers` | built-in | `setInterval` / `setTimeout` | Heartbeat interval (15s) and timeout detection (45s) |
| `node:events` | built-in | EventEmitter | HeartbeatMonitor emits `james_down` / `james_up` events |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tasklist` via `execFile` | `wmic process` | `wmic` is deprecated on Windows 11. `tasklist` is the supported tool. |
| `os.cpus()` delta sampling | `systeminformation` npm package | 9MB dependency for one metric. `os.cpus()` is built-in and gives exact same data after delta calculation. Overkill. |
| Application-level heartbeat | WebSocket ping/pong only | ping/pong cannot carry payload (Claude status, CPU, memory). Also, ping/pong is already implemented for transport-level keepalive -- the two serve different purposes. |
| `setInterval` for heartbeat | `setTimeout` chain | `setInterval` is correct here -- fixed 15s interval, no variable delays. `setTimeout` chains are for variable timing (like backoff). |

**Installation:**
```bash
# No new packages needed. All features use Node.js built-ins.
```

## Architecture Patterns

### Modified Project Structure (Phase 3 additions)
```
comms-link/
  james/
    comms-client.js       # NO CHANGES
    heartbeat-sender.js   # NEW: starts/stops heartbeat interval, collects metrics, sends
    system-metrics.js     # NEW: CPU sampling, memory, uptime, claude process detection
    index.js              # MODIFIED: wire up HeartbeatSender after client connects
  bono/
    comms-server.js       # NO CHANGES
    heartbeat-monitor.js  # NEW: tracks heartbeat timestamps, emits james_down/james_up
    index.js              # MODIFIED: wire up HeartbeatMonitor to wss message events
  shared/
    protocol.js           # NO CHANGES (heartbeat type already defined)
    state.js              # NO CHANGES
  test/
    heartbeat.test.js     # NEW: heartbeat send/receive, timeout detection
    system-metrics.test.js # NEW: CPU, memory, uptime, process detection
    (existing tests)      # UNCHANGED
```

**Key design:** HeartbeatSender and HeartbeatMonitor are standalone modules that take a connection/server as a constructor argument. They are NOT mixed into CommsClient or CommsServer. This keeps the WebSocket layer clean and makes heartbeat independently testable.

### Pattern 1: Heartbeat Sender (James side)
**What:** A class that starts a `setInterval` to send heartbeat messages every 15 seconds. Collects system metrics before each send.
**When to use:** After `CommsClient` emits `open` (connected). Stopped on `close`.
**Example:**
```javascript
// james/heartbeat-sender.js
import { collectMetrics } from './system-metrics.js';

const HEARTBEAT_INTERVAL = 15_000; // 15 seconds

export class HeartbeatSender {
  #client;
  #interval = null;

  constructor(client) {
    this.#client = client;
  }

  start() {
    this.stop(); // prevent duplicate intervals
    // Send immediately, then every 15s
    this.#sendHeartbeat();
    this.#interval = setInterval(() => this.#sendHeartbeat(), HEARTBEAT_INTERVAL);
  }

  stop() {
    if (this.#interval) {
      clearInterval(this.#interval);
      this.#interval = null;
    }
  }

  async #sendHeartbeat() {
    const metrics = await collectMetrics();
    // send() handles CONNECTED check internally
    this.#client.send('heartbeat', metrics);
  }
}
```

### Pattern 2: System Metrics Collection (James side)
**What:** Collects CPU usage, memory usage, system uptime, and Claude Code process status. CPU requires two-sample delta approach on Windows.
**When to use:** Called before each heartbeat send.
**Example:**
```javascript
// james/system-metrics.js
import os from 'node:os';
import { execFile } from 'node:child_process';

let prevCpuTimes = null;

function getCpuTimes() {
  const cpus = os.cpus();
  let user = 0, nice = 0, sys = 0, idle = 0, irq = 0;
  for (const cpu of cpus) {
    user += cpu.times.user;
    nice += cpu.times.nice;
    sys += cpu.times.sys;
    idle += cpu.times.idle;
    irq += cpu.times.irq;
  }
  return { user, nice, sys, idle, irq };
}

function calcCpuPercent() {
  const curr = getCpuTimes();
  if (!prevCpuTimes) {
    prevCpuTimes = curr;
    return 0; // first call -- no delta yet
  }
  const totalDelta = (curr.user - prevCpuTimes.user)
    + (curr.nice - prevCpuTimes.nice)
    + (curr.sys - prevCpuTimes.sys)
    + (curr.idle - prevCpuTimes.idle)
    + (curr.irq - prevCpuTimes.irq);
  const idleDelta = curr.idle - prevCpuTimes.idle;
  prevCpuTimes = curr;
  return totalDelta > 0
    ? Math.round((totalDelta - idleDelta) / totalDelta * 1000) / 10
    : 0;
}

function isClaudeRunning() {
  return new Promise((resolve) => {
    execFile('tasklist', ['/NH', '/FI', 'IMAGENAME eq claude.exe'],
      { encoding: 'utf8', timeout: 5000 },
      (err, stdout) => {
        if (err) { resolve(false); return; }
        resolve(!stdout.includes('No tasks are running'));
      }
    );
  });
}

export async function collectMetrics() {
  const [claudeRunning] = await Promise.all([isClaudeRunning()]);
  return {
    cpu: calcCpuPercent(),
    memoryUsed: Math.round((1 - os.freemem() / os.totalmem()) * 1000) / 10,
    memoryTotal: os.totalmem(),
    uptime: Math.floor(os.uptime()),
    claudeRunning,
  };
}
```

### Pattern 3: Heartbeat Monitor (Bono side)
**What:** Tracks the last heartbeat timestamp. If no heartbeat arrives within 45 seconds, emits `james_down`. When heartbeat resumes, emits `james_up`.
**When to use:** Wired to `wss.on('message')` in the server.
**Example:**
```javascript
// bono/heartbeat-monitor.js
import { EventEmitter } from 'node:events';

const HEARTBEAT_TIMEOUT = 45_000; // 45 seconds (3 missed beats)

export class HeartbeatMonitor extends EventEmitter {
  #timeout = null;
  #isUp = false;
  #lastHeartbeat = null;
  #lastPayload = null;

  get isUp() { return this.#isUp; }
  get lastHeartbeat() { return this.#lastHeartbeat; }
  get lastPayload() { return this.#lastPayload; }

  receivedHeartbeat(payload) {
    this.#lastHeartbeat = Date.now();
    this.#lastPayload = payload;

    if (!this.#isUp) {
      this.#isUp = true;
      this.emit('james_up', { timestamp: this.#lastHeartbeat, payload });
    }

    this.#resetTimeout();
  }

  #resetTimeout() {
    clearTimeout(this.#timeout);
    this.#timeout = setTimeout(() => {
      this.#isUp = false;
      this.emit('james_down', {
        timestamp: Date.now(),
        lastHeartbeat: this.#lastHeartbeat,
      });
    }, HEARTBEAT_TIMEOUT);
  }

  stop() {
    clearTimeout(this.#timeout);
    this.#timeout = null;
  }
}
```

### Pattern 4: Heartbeat Should NOT Be Queued
**What:** Heartbeat messages should be discarded (not queued) when disconnected. Stale heartbeats are worthless -- they carry outdated metrics and would create a false "alive" signal on reconnect.
**When to use:** Design decision for HeartbeatSender.
**Implementation:** The HeartbeatSender should stop its interval when the WebSocket disconnects and restart when it reconnects.

```javascript
// In james/index.js
client.on('open', () => {
  heartbeatSender.start();
});

client.on('close', () => {
  heartbeatSender.stop();
});
```

### Anti-Patterns to Avoid
- **Mixing heartbeat logic into CommsClient:** The heartbeat is an application concern, not a transport concern. Keep it in a separate module that consumes CommsClient via composition.
- **Using WebSocket ping/pong for heartbeat:** ping/pong frames cannot carry application payload. The existing ping/pong in comms-server.js (25s interval) detects dead TCP connections. The heartbeat (15s interval) carries Claude Code status and system metrics. Both serve different purposes and should coexist.
- **Queuing heartbeats during disconnection:** Stale heartbeats are actively harmful -- they carry old timestamps and outdated metrics. Stop the heartbeat interval on disconnect, restart on reconnect.
- **Blocking the event loop with synchronous process detection:** Synchronous process detection blocks for ~50ms. Use `execFile` (async) instead. The heartbeat interval is 15 seconds -- plenty of time for an async call.
- **Using shell-based process execution:** Always use `execFile()` which bypasses the shell, preventing command injection. Never use `exec()` for fixed commands.
- **Sending heartbeat on first tick without initial metrics:** The first CPU reading always returns 0% because there is no previous sample. The HeartbeatSender should send an initial heartbeat immediately (with cpu=0 for the first one) so Bono knows James is alive right away. The second heartbeat at 15s will have accurate CPU data.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CPU usage percentage | Custom perf counters or WMI calls | `os.cpus()` delta sampling | Built-in, cross-platform API. The two-sample delta approach is ~15 lines and gives per-interval CPU%. No native modules needed. |
| Process detection | Custom Win32 API bindings / `node-ffi` | `child_process.execFile('tasklist', ...)` | `tasklist` is built into every Windows installation. ~50ms async call. No native dependencies. |
| System metrics collection | `systeminformation` npm package | Node.js `os` module | `systeminformation` is 9MB with native bindings. We need exactly 3 metrics that `os` provides natively. |
| Timeout-based failure detection | Custom heartbeat protocol | Simple `setTimeout` reset pattern | The "reset timer on each heartbeat, fire callback on expiry" pattern is 10 lines. No framework needed. |

**Key insight:** Every metric we need (CPU, memory, uptime, process status) is available through Node.js built-ins or Windows shell commands. Zero external dependencies for this entire phase.

## Common Pitfalls

### Pitfall 1: setInterval Drift and Accumulation
**What goes wrong:** `setInterval` callbacks can fire later than scheduled due to event loop load. Over time, the interval drifts. Additionally, if the callback takes longer than the interval (unlikely at 15s but possible with process detection), callbacks accumulate.
**Why it happens:** `setInterval` guarantees minimum delay, not exact timing. If the event loop is busy, the callback is delayed.
**How to avoid:** For a 15s heartbeat interval, drift is irrelevant (sub-millisecond). The 45s timeout on Bono's side absorbs up to 3 missed beats. No special handling needed. If the `collectMetrics()` async call takes >15s (extremely unlikely -- tasklist is ~50ms), the next interval will simply overlap. This is harmless because each heartbeat is independent.
**Warning signs:** Two heartbeat messages with timestamps <1s apart (accumulated callbacks). Monitor the `ts` field in heartbeat payloads.

### Pitfall 2: False DOWN Detection on Reconnection
**What goes wrong:** James disconnects, reconnects after 10 seconds, but Bono's HeartbeatMonitor fires `james_down` at 45 seconds because no heartbeat arrived during the reconnection window.
**Why it happens:** The WebSocket-level disconnect and the heartbeat timeout are on different clocks. WebSocket reconnection happens, but the first heartbeat has not arrived yet.
**How to avoid:** When Bono detects a new WebSocket connection (wss `connection` event), do NOT reset the heartbeat monitor. Let the first heartbeat message reset it naturally. The 45s timeout is generous enough (3 beats) that a brief reconnection (James sends heartbeat immediately on connect) will not trigger false DOWN. HeartbeatSender sends immediately on start(), and start() is called on 'open', so the first heartbeat arrives within milliseconds of reconnection.
**Warning signs:** `james_down` followed immediately by `james_up` during reconnection.

### Pitfall 3: CPU Always Reports 0% on First Heartbeat
**What goes wrong:** The first CPU reading is always 0% because there is no previous sample to compute a delta against.
**Why it happens:** CPU percentage is calculated from the difference between two `os.cpus()` snapshots. On the first call, there is no "previous" snapshot.
**How to avoid:** Accept 0% on the first heartbeat. It is correct -- we have no data yet. The second heartbeat at t=15s will have a valid 15-second CPU average. Document this in the payload spec so Bono does not alarm on the first 0%. Alternatively, take a "warm-up" snapshot at module load time so the first heartbeat at t=0 has a short baseline.
**Warning signs:** CPU always 0% (forgot to store previous sample) or CPU wildly high (comparing against a very old baseline).

### Pitfall 4: tasklist Fails or Hangs
**What goes wrong:** Process detection fails (access denied, command not found) or hangs (Windows is under heavy load, the process table is huge).
**Why it happens:** `tasklist` is a Windows command that queries the process table. Under extreme load or with permissions issues, it can be slow or fail.
**How to avoid:** Set a 5-second timeout on the execFile call. On error or timeout, report `claudeRunning: null` (unknown) rather than `false` (stopped). This lets Bono distinguish "Claude is definitely stopped" from "we could not check."
**Warning signs:** Heartbeat payloads with `claudeRunning: null` appearing regularly.

### Pitfall 5: Timer Leak When HeartbeatMonitor/Sender is Not Properly Stopped
**What goes wrong:** If `stop()` is not called on shutdown, the `setInterval` (sender) or `setTimeout` (monitor) keeps the Node.js process alive, preventing clean exit.
**Why it happens:** Active timers prevent the event loop from draining. `setInterval` never stops on its own.
**How to avoid:** Both HeartbeatSender and HeartbeatMonitor must have `stop()` methods. Call `stop()` in the shutdown handler (SIGTERM/SIGINT) of both `james/index.js` and `bono/index.js`. Use `unref()` on non-critical timers as a safety net -- but explicit cleanup is the correct approach.
**Warning signs:** Process hangs on shutdown. `process.exit()` needed to force quit.

### Pitfall 6: Heartbeat and WebSocket Ping/Pong Confusion
**What goes wrong:** Someone modifies the existing WebSocket ping/pong interval (25s in comms-server.js) thinking it IS the heartbeat, breaking transport-level keepalive.
**Why it happens:** Both "heartbeat" and "ping/pong" sound like the same thing. They are not.
**How to avoid:** Clear code comments distinguishing the two. The ping/pong (transport-level, 25s interval, comms-server.js lines 88-100) detects dead TCP connections -- it has no payload and is handled by the `ws` library automatically. The heartbeat (application-level, 15s interval, heartbeat-sender.js) carries Claude Code status and system metrics as a JSON message. Both must coexist. Do NOT remove the ping/pong.
**Warning signs:** James stops being detected as "alive" at the transport level after removing ping/pong.

## Code Examples

Verified patterns from official sources and the existing codebase:

### Heartbeat Payload Schema
```javascript
// The heartbeat message uses the existing createMessage() envelope.
// Payload structure:
{
  cpu: 4.3,            // CPU usage percentage (0-100, one decimal)
  memoryUsed: 15.3,    // Memory usage percentage (0-100, one decimal)
  memoryTotal: 68641587200, // Total RAM in bytes
  uptime: 123456,      // System uptime in seconds (integer)
  claudeRunning: true, // Claude Code process status (true/false/null)
}

// Full message on the wire (via createMessage):
{
  v: 1,
  type: 'heartbeat',
  from: 'james',
  ts: 1710234567890,
  id: 'uuid-here',
  payload: {
    cpu: 4.3,
    memoryUsed: 15.3,
    memoryTotal: 68641587200,
    uptime: 123456,
    claudeRunning: true,
  }
}
```

### Wiring HeartbeatSender in james/index.js
```javascript
import { HeartbeatSender } from './heartbeat-sender.js';

const heartbeat = new HeartbeatSender(client);

client.on('open', () => {
  heartbeat.start();
});

client.on('close', () => {
  heartbeat.stop();
});

function shutdown() {
  heartbeat.stop();
  client.disconnect();
  process.exit(0);
}
```

### Wiring HeartbeatMonitor in bono/index.js
```javascript
import { HeartbeatMonitor } from './heartbeat-monitor.js';

const monitor = new HeartbeatMonitor();

wss.on('message', (msg, ws) => {
  if (msg.type === 'heartbeat') {
    monitor.receivedHeartbeat(msg.payload);
  }
});

monitor.on('james_down', (evt) => {
  console.log(`James is DOWN. Last heartbeat: ${new Date(evt.lastHeartbeat).toISOString()}`);
});

monitor.on('james_up', (evt) => {
  console.log(`James is UP. Claude running: ${evt.payload.claudeRunning}`);
});

function shutdown() {
  monitor.stop();
  stop().then(() => process.exit(0));
}
```

### Testing Heartbeat Timeout (mock timers)
```javascript
// node:test supports mock timers via test context
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';

describe('HeartbeatMonitor', () => {
  it('emits james_down after 45s without heartbeat', (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const monitor = new HeartbeatMonitor();
    const events = [];
    monitor.on('james_down', (e) => events.push(e));

    monitor.receivedHeartbeat({ cpu: 1, claudeRunning: true });
    assert.equal(monitor.isUp, true);

    t.mock.timers.tick(44_999);
    assert.equal(events.length, 0, 'should not fire before 45s');

    t.mock.timers.tick(1);
    assert.equal(events.length, 1, 'should fire at 45s');
    assert.equal(monitor.isUp, false);

    monitor.stop();
  });
});
```

### CPU Delta Sampling (tested on this machine)
```javascript
// Verified output on James's machine (Windows 11, 16-core, Node.js v22.14.0):
// First call returns 0 (no baseline)
// Subsequent calls return accurate CPU% averaged over the interval since last call
//
// os.loadavg() returns [0, 0, 0] on Windows -- NEVER use it.
// os.cpus() returns per-core times object with { user, nice, sys, idle, irq } in ms.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WebSocket ping/pong only | Application-level heartbeat + WS ping/pong | Always separate concerns | ping/pong detects dead TCP. Heartbeat carries app-level status. Both needed. |
| `os.loadavg()` for CPU on Windows | `os.cpus()` delta sampling | N/A (loadavg never worked on Windows) | loadavg returns `[0,0,0]` on Windows. Delta sampling is the correct approach. |
| `wmic process` for process detection | `tasklist /FI` | wmic deprecated Win11 | wmic still works but Microsoft recommends PowerShell or tasklist. tasklist is simpler from Node.js. |
| Shell-based process execution | `child_process.execFile()` | Security best practice | execFile bypasses shell, preventing injection. Always prefer for fixed commands. |

**Deprecated/outdated:**
- `wmic`: Deprecated on Windows 11. Still functional but not recommended. Use `tasklist` or PowerShell `Get-Process`.
- `os.loadavg()` on Windows: Returns `[0, 0, 0]`. Never use for Windows CPU monitoring.

## Open Questions

1. **Heartbeat acknowledgment (heartbeat_ack)**
   - What we know: The protocol defines `heartbeat_ack` as a message type. The requirements do not mention Bono sending an acknowledgment back.
   - What's unclear: Should Bono reply with a `heartbeat_ack` containing Bono's own status?
   - Recommendation: For Phase 3, do NOT implement `heartbeat_ack`. The requirements only specify one-way heartbeat (James -> Bono) with server-side timeout detection. `heartbeat_ack` can be added later if bidirectional health is needed. The message type is already reserved in the protocol.

2. **Multiple WebSocket connections to Bono**
   - What we know: The server currently accepts any number of authenticated connections. HeartbeatMonitor tracks "James" as a single entity.
   - What's unclear: What happens if James reconnects while the old connection is still in the server's client set?
   - Recommendation: The existing ping/pong (25s) will eventually terminate stale connections. HeartbeatMonitor should track by latest heartbeat regardless of which WebSocket it arrives on. For Phase 3, there is only one James -- no multi-client routing needed.

3. **Graceful shutdown heartbeat**
   - What we know: When James calls `disconnect()` intentionally, the heartbeat stops.
   - What's unclear: Should James send a "goodbye" heartbeat before disconnecting so Bono knows it is an intentional shutdown (not a crash)?
   - Recommendation: Not for Phase 3. Phase 5 (WD-07) will handle restart notifications via email. For now, an intentional shutdown looks like a crash to Bono (45s timeout fires). This is acceptable for v1.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Node.js built-in test runner (`node:test`) v22.14.0 |
| Config file | None needed -- zero config |
| Quick run command | `node --test test/heartbeat.test.js test/system-metrics.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HB-01 | HeartbeatSender sends heartbeat message every 15s | unit (mock timers) | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-01 | HeartbeatSender sends immediately on start() | unit | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-01 | HeartbeatSender stops on stop(), no more heartbeats sent | unit | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-01 | HeartbeatSender does not send when not connected (no stale queuing) | integration | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-02 | HeartbeatMonitor emits james_down after 45s without heartbeat | unit (mock timers) | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-02 | HeartbeatMonitor resets timeout on each received heartbeat | unit (mock timers) | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-02 | HeartbeatMonitor emits james_up when heartbeat resumes after DOWN | unit (mock timers) | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-03 | collectMetrics() returns claudeRunning boolean | unit | `node --test test/system-metrics.test.js` | Wave 0 |
| HB-03 | Heartbeat payload contains claudeRunning field | integration | `node --test test/heartbeat.test.js` | Wave 0 |
| HB-04 | collectMetrics() returns cpu, memoryUsed, memoryTotal, uptime | unit | `node --test test/system-metrics.test.js` | Wave 0 |
| HB-04 | CPU first reading returns 0 (no baseline) | unit | `node --test test/system-metrics.test.js` | Wave 0 |
| HB-04 | Heartbeat payload contains all system metric fields | integration | `node --test test/heartbeat.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/*.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (all 38 existing + new Phase 3 tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/heartbeat.test.js` -- covers HB-01, HB-02, HB-03 (payload), HB-04 (payload)
- [ ] `test/system-metrics.test.js` -- covers HB-03 (process detection), HB-04 (metrics collection)

*(Existing test infrastructure covers all other needs -- no framework install or fixture files needed)*

## Sources

### Primary (HIGH confidence)
- Node.js v22.14.0 `os` module -- verified on this machine: `os.cpus()` returns per-core times, `os.loadavg()` returns `[0,0,0]` on Windows, `os.freemem()`/`os.totalmem()` return bytes, `os.uptime()` returns seconds
- Node.js v22.14.0 `child_process.execFile` -- verified: `execFile('tasklist', ['/NH', '/FI', 'IMAGENAME eq claude.exe'])` returns "claude.exe PID ... K" when running, "No tasks are running" when not
- Existing codebase: `shared/protocol.js` -- `heartbeat` and `heartbeat_ack` message types already defined
- Existing codebase: `bono/comms-server.js` -- WebSocket-level ping/pong already at 25s interval (lines 88-100)
- Existing codebase: `james/comms-client.js` -- `send()` method handles connected/queued logic, returns boolean
- Windows process detection: `claude.exe` confirmed running as PID 21840, installed via `Claude_pzs8sxrjxfjjc` MSIX package
- Existing watchdog: `C:\Users\bono\.claude\claude_watchdog.ps1` -- uses `Get-Process -Name "claude"` which confirms process name is "claude"

### Secondary (MEDIUM confidence)
- Node.js `node:test` mock timers API (`t.mock.timers.enable()`) -- documented in Node.js v22 test runner docs for timer mocking in unit tests

### Tertiary (LOW confidence)
- None. All findings verified against primary sources on this machine.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All features use Node.js built-ins already available. No new dependencies. Verified on this machine.
- Architecture: HIGH - Clean module separation follows established Phase 1/2 patterns. Protocol types already defined. HeartbeatSender/Monitor are simple, independently testable classes.
- Pitfalls: HIGH - All pitfalls verified against actual behavior on this Windows 11 machine (loadavg returns zeros, tasklist works with execFile, CPU delta sampling produces correct results).

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (30 days -- stable domain, all Node.js built-ins, no external dependencies)
