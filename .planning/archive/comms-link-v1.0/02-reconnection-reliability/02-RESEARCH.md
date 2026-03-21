# Phase 2: Reconnection & Reliability - Research

**Researched:** 2026-03-12
**Domain:** WebSocket auto-reconnect with exponential backoff + offline message queuing (Node.js, ws library)
**Confidence:** HIGH

## Summary

Phase 2 adds two capabilities to the Phase 1 WebSocket client: (1) automatic reconnection with exponential backoff when the connection drops, and (2) a bounded message queue that buffers outgoing messages while disconnected and replays them in order upon reconnection. Both features are implemented entirely in `james/comms-client.js` -- the server side (`bono/comms-server.js`) requires zero changes because it already accepts new connections with PSK auth and has no concept of "sessions."

The implementation is straightforward because Phase 1 already laid the groundwork: the `ConnectionStateMachine` already supports the `RECONNECTING` state with valid transitions from both `DISCONNECTED` and `CONNECTED`, the `connect()` method already cleans up old WebSocket instances before creating new ones, and the `send()` method already returns `false` when not connected. Phase 2 modifies `CommsClient` to: (a) enter `RECONNECTING` on close (instead of `DISCONNECTED`), (b) schedule reconnect attempts with exponential backoff + jitter, (c) queue messages when `send()` would return `false`, and (d) flush the queue in order on successful reconnect.

The "no duplicates" requirement (success criterion 3) is satisfied for free because this is a client-side queue of outgoing messages -- each message is sent exactly once after reconnection. The existing message `id` field (UUID) in `shared/protocol.js` provides deduplication if needed in the future, but for this phase it is unnecessary since we are queuing unsent messages, not re-sending already-sent ones.

**Primary recommendation:** Add reconnection logic and a bounded message queue to `CommsClient`. Do NOT create separate classes or modules -- this is 60-80 lines of additions to the existing client. Server needs no changes. Use `setTimeout` for backoff scheduling (not `setInterval`). Add jitter to prevent thundering herd (irrelevant for a 2-node system today, but costs nothing and is correct practice).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WS-02 | Auto-reconnect with exponential backoff (1s start, 30s cap) on connection loss | Exponential backoff with jitter is the standard pattern. Formula: `min(baseDelay * 2^attempt + jitter, maxDelay)`. Reset attempt counter on successful reconnect. Use `RECONNECTING` state already in state machine. Skip reconnect on intentional close (code 1000). |
| WS-05 | Message queuing during disconnection with replay on reconnect | Bounded FIFO queue (max 100 messages). `send()` pushes to queue when not CONNECTED. `open` handler flushes queue in order via `shift()`. Serialized JSON strings stored (not objects) so they are ready to send. No dedup needed -- these are unsent messages, not retries. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ws` | ^8.19.0 | WebSocket client (already installed) | No additional dependency needed. Phase 2 uses the same `ws` WebSocket constructor. |
| Node.js | 22.14.0 LTS | Runtime (timers, events) | `setTimeout` for backoff scheduling. `Math.random()` for jitter. No new built-ins needed. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `node:test` | built-in | Test runner | Tests for reconnect behavior, queue behavior, backoff timing |
| `node:events` | built-in | EventEmitter | Already used by CommsClient and ConnectionStateMachine |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled reconnect in CommsClient | `reconnecting-websocket` npm | Abandoned since 2020 (v4.4.0). Reconnect logic is ~40 lines. Adding a dead dependency for trivial logic is wrong. |
| In-memory array queue | External message broker (Redis, AMQP) | Absurd overkill for a 2-node system with ~1 msg/min throughput. Array with max-size cap is correct. |
| `setTimeout` per attempt | `setInterval` with conditional | `setInterval` is harder to reason about with variable delays. `setTimeout` with dynamic delay is cleaner and self-terminating. |

**Installation:**
```bash
# No new packages needed. Phase 1 already has ws installed.
```

## Architecture Patterns

### Modified Project Structure (Phase 2 additions)
```
comms-link/
  james/
    comms-client.js       # MODIFIED: add reconnect logic + message queue
    index.js              # MODIFIED: wire up reconnect (remove manual connect, let auto-reconnect handle it)
  bono/
    comms-server.js       # NO CHANGES
    index.js              # NO CHANGES
  shared/
    protocol.js           # NO CHANGES
    state.js              # NO CHANGES
  test/
    reconnect.test.js     # NEW: reconnect with backoff tests
    queue.test.js         # NEW: message queue + replay tests
    (existing tests)      # UNCHANGED
```

**Key insight:** Phase 2 is entirely a client-side change. The server already accepts new connections transparently. No protocol changes. No new message types. No shared module changes.

### Pattern 1: Exponential Backoff with Jitter
**What:** On connection loss, schedule reconnect attempts with increasing delays. Each attempt doubles the delay up to a cap. Add random jitter to prevent synchronized reconnect storms.
**When to use:** Every unintentional disconnection (close code != 1000).
**Example:**
```javascript
// Backoff calculation
const BACKOFF_BASE = 1000;    // 1 second (requirement: "1s start")
const BACKOFF_MAX = 30000;    // 30 seconds (requirement: "30s cap")
const JITTER_MAX = 500;       // 0-500ms random jitter

#reconnectAttempt = 0;

#scheduleReconnect() {
  const delay = Math.min(
    BACKOFF_BASE * Math.pow(2, this.#reconnectAttempt),
    BACKOFF_MAX
  );
  const jitter = Math.random() * JITTER_MAX;

  this.#reconnectTimer = setTimeout(() => {
    this.#reconnectAttempt++;
    this.connect();  // existing method already cleans up old ws
  }, delay + jitter);
}
```

**Delay sequence (without jitter):** 1s, 2s, 4s, 8s, 16s, 30s, 30s, 30s, ...

### Pattern 2: Intentional vs Unintentional Close Detection
**What:** Distinguish between intentional disconnects (user calls `disconnect()`) and unintentional ones (server crash, network loss). Only auto-reconnect on unintentional close.
**When to use:** In the WebSocket `close` event handler.
**Example:**
```javascript
// Set a flag before intentional close
#intentionalClose = false;

disconnect() {
  this.#intentionalClose = true;
  if (this.#ws) {
    this.#ws.close(1000, 'client disconnect');
    this.#ws = null;
  }
}

// In the close handler:
this.#ws.on('close', (code) => {
  if (this.#intentionalClose) {
    this.sm.transition(State.DISCONNECTED);
    this.emit('close');
    return;  // do NOT reconnect
  }

  this.sm.transition(State.RECONNECTING);
  this.emit('close');
  this.#scheduleReconnect();
});
```

**Why a flag and not close code:** The `ws` library emits code 1006 for both network drops and `terminate()`. A clean close (code 1000) only happens when the server explicitly sends a close frame. Using an internal flag (`#intentionalClose`) is more reliable than parsing close codes. But we also check: if code is 1000 AND the flag is set, it is intentional.

### Pattern 3: Bounded Message Queue with Replay
**What:** When `send()` is called while not CONNECTED, serialize the message and push it onto a bounded FIFO queue. On reconnection, flush the queue in order.
**When to use:** Every `send()` call while disconnected or reconnecting.
**Example:**
```javascript
#queue = [];
#maxQueueSize = 100;

send(type, payload) {
  const msg = createMessage(type, 'james', payload);

  if (this.sm.state === State.CONNECTED && this.#ws?.readyState === WebSocket.OPEN) {
    this.#ws.send(msg);
    return true;
  }

  // Queue when not connected
  if (this.#queue.length >= this.#maxQueueSize) {
    this.#queue.shift();  // drop oldest
    this.emit('queue_overflow');
  }
  this.#queue.push(msg);
  return false;  // caller knows it was queued, not sent
}

#flushQueue() {
  while (this.#queue.length > 0 && this.#ws?.readyState === WebSocket.OPEN) {
    this.#ws.send(this.#queue.shift());
  }
}

// Call #flushQueue() in the open handler, after state transition
```

**Design decisions:**
- Store serialized JSON strings (not objects) so flush is a simple `ws.send()` loop with no re-serialization.
- Drop oldest on overflow (FIFO eviction). For this system, newer messages are more valuable than older ones.
- `send()` still returns `false` when queuing -- callers can distinguish "sent immediately" (true) from "queued for later" (false). This matches the Phase 1 API contract.
- Queue is flushed synchronously in the `open` handler before any new messages are sent, preserving order.

### Pattern 4: Reset Backoff on Success
**What:** Reset the reconnect attempt counter to 0 when a connection succeeds.
**When to use:** In the WebSocket `open` handler.
**Example:**
```javascript
this.#ws.on('open', () => {
  this.#reconnectAttempt = 0;       // reset backoff
  this.#intentionalClose = false;   // clear flag
  this.sm.transition(State.CONNECTED);
  this.#flushQueue();               // replay queued messages
  this.emit('open');
});
```

### Anti-Patterns to Avoid
- **Reconnecting on intentional close:** If the user calls `disconnect()`, the close handler must NOT schedule a reconnect. Use an `#intentionalClose` flag.
- **Creating a new WebSocket without cleaning up the old one:** Phase 1's `connect()` already handles this (`removeAllListeners` + `terminate`), so Phase 2 reuses `connect()` as-is.
- **Unbounded message queue:** Without a max size, a long disconnection with active `send()` calls will consume unbounded memory. Cap at 100 messages with FIFO eviction.
- **Using `setInterval` for reconnection:** Variable delays (exponential backoff) do not work with `setInterval`. Use `setTimeout` for each individual attempt.
- **Flushing queue before state transition:** The state machine must be in CONNECTED before flushing. Otherwise, if `send()` is called during flush by an event listener, the state check fails.
- **Reconnecting during reconnection:** If a reconnect attempt fails quickly and fires `close` again, a new `#scheduleReconnect()` is triggered. Clear any existing `#reconnectTimer` before scheduling a new one to prevent timer accumulation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WebSocket reconnection | `reconnecting-websocket` npm package | 40 lines in CommsClient | Package is abandoned (2020). Our needs are specific (PSK headers, state machine integration, custom queue). Rolling our own is simpler and more maintainable. |
| Message serialization | Custom binary protocol | JSON via `createMessage()` | Already built in Phase 1. JSON is human-debuggable and sufficient for ~1 msg/min throughput. |
| Unique message IDs | Custom sequence counter | `crypto.randomUUID()` | Already in `createMessage()` from Phase 1. UUIDs are globally unique without coordination. |
| Timer management | Custom scheduler | `setTimeout` / `clearTimeout` | Built-in, zero overhead, perfect for single-delay-then-execute pattern. |

**Key insight:** Phase 2 is a small, focused enhancement to an existing class. There is no need for new modules, new dependencies, or architectural changes. The Phase 1 design anticipated this (state machine already has RECONNECTING, connect() already cleans up).

## Common Pitfalls

### Pitfall 1: Timer Leak on Rapid Reconnect Failures
**What goes wrong:** A reconnect attempt fails immediately (e.g., DNS resolution error). The `close` handler fires and schedules another `setTimeout`. But the previous timer might still be pending if the failure was faster than the delay.
**Why it happens:** `setTimeout` returns a timer ID that must be explicitly cleared. If you schedule a new timer without clearing the old one, multiple timers run concurrently, each calling `connect()`.
**How to avoid:** Store the timer ID in `#reconnectTimer`. In `#scheduleReconnect()`, call `clearTimeout(this.#reconnectTimer)` before setting a new one. Also clear it in `disconnect()` and `connect()` (on success).
**Warning signs:** Multiple "attempting reconnect" log lines firing simultaneously. CPU spike from parallel connection attempts.

### Pitfall 2: State Machine Invalid Transition on Double Close
**What goes wrong:** A WebSocket can fire `close` after `error`. If the state machine is already in RECONNECTING (from a previous close), a second `close` event tries to transition RECONNECTING -> RECONNECTING, which throws.
**Why it happens:** The `ws` library fires `error` then `close` on connection failure. If the `error` handler also triggers state changes, the `close` handler encounters an unexpected state.
**How to avoid:** Only do state transitions in the `close` handler, never in `error`. Guard the `close` handler: if already in RECONNECTING, do not transition again (just re-schedule). The Phase 1 code already guards against DISCONNECTED -> DISCONNECTED; Phase 2 must similarly guard RECONNECTING -> RECONNECTING.
**Warning signs:** `Error: Invalid transition: RECONNECTING -> RECONNECTING` in logs.

### Pitfall 3: Queue Replay Interleaving with New Messages
**What goes wrong:** The queue is flushed asynchronously or after event listeners fire. A listener on `open` calls `send()` with a new message, which arrives at the server before the queued messages.
**Why it happens:** If `#flushQueue()` runs after `this.emit('open')`, any `send()` calls inside `open` event handlers bypass the queue and go directly to the WebSocket.
**How to avoid:** Flush the queue BEFORE emitting the `open` event. Order of operations in the `open` handler: (1) reset backoff, (2) transition to CONNECTED, (3) flush queue, (4) emit `open`.
**Warning signs:** Messages arrive at the server out of order (new messages before queued ones).

### Pitfall 4: Message Queue Grows Unbounded During Extended Outage
**What goes wrong:** The connection is down for hours. Code keeps calling `send()` (e.g., future heartbeat attempts). The queue grows without limit, consuming memory.
**Why it happens:** No max-size cap on the queue. No TTL on queued messages.
**How to avoid:** Cap the queue at 100 messages (configurable). When the cap is hit, drop the oldest message (shift). Optionally emit a `queue_overflow` event so the application can log/alert. For this 2-node system with low message rates, 100 is generous.
**Warning signs:** Memory usage climbing during extended disconnection.

### Pitfall 5: Reconnect Loop After Auth Failure (Wrong PSK)
**What goes wrong:** The PSK is wrong or expired. Every reconnect attempt is immediately rejected with 401. The client enters an infinite reconnect loop, generating server logs and wasting resources.
**Why it happens:** The backoff caps at 30s, which is still frequent for a known-bad credential. The `close` handler does not distinguish "auth rejected" from "network error."
**How to avoid:** When the server rejects auth, it destroys the socket before the upgrade completes. In `ws`, this manifests as a close with code 1006 and an `error` event with `Unexpected server response: 401`. Track consecutive failures; if they exceed a threshold (e.g., 10), transition to DISCONNECTED and stop reconnecting. Emit a `max_retries` event. The application can then alert or take corrective action.
**Warning signs:** Rapid-fire reconnect attempts with immediate close events. Server logs showing repeated 401s.

## Code Examples

Verified patterns from official sources and the existing codebase:

### Complete Modified CommsClient (conceptual)
```javascript
// Source: Existing james/comms-client.js + standard reconnect patterns
import { EventEmitter } from 'node:events';
import WebSocket from 'ws';
import { createMessage, parseMessage } from '../shared/protocol.js';
import { ConnectionStateMachine, State } from '../shared/state.js';

const BACKOFF_BASE = 1000;
const BACKOFF_MAX = 30_000;
const JITTER_MAX = 500;
const MAX_QUEUE_SIZE = 100;
const MAX_RECONNECT_FAILURES = 0;  // 0 = unlimited retries

export class CommsClient extends EventEmitter {
  #ws = null;
  #reconnectTimer = null;
  #reconnectAttempt = 0;
  #intentionalClose = false;
  #queue = [];

  constructor({ url, psk, maxQueueSize = MAX_QUEUE_SIZE }) {
    super();
    this.url = url;
    this.psk = psk;
    this.#maxQueueSize = maxQueueSize;
    this.sm = new ConnectionStateMachine();
    this.sm.on('state', (evt) => this.emit('state', evt));
  }

  #maxQueueSize;

  get state() { return this.sm.state; }
  get queueSize() { return this.#queue.length; }

  connect() {
    // Clear any pending reconnect timer
    clearTimeout(this.#reconnectTimer);
    this.#reconnectTimer = null;

    // Clean up old WebSocket
    if (this.#ws) {
      this.#ws.removeAllListeners();
      this.#ws.terminate();
      this.#ws = null;
    }

    this.#ws = new WebSocket(this.url, {
      headers: { 'Authorization': 'Bearer ' + this.psk },
    });

    this.#ws.on('open', () => {
      this.#reconnectAttempt = 0;
      this.#intentionalClose = false;
      this.sm.transition(State.CONNECTED);
      this.#flushQueue();
      this.emit('open');
    });

    this.#ws.on('close', () => {
      if (this.#intentionalClose) {
        if (this.sm.state !== State.DISCONNECTED) {
          this.sm.transition(State.DISCONNECTED);
        }
        this.emit('close');
        return;
      }

      // Unintentional close -- reconnect
      if (this.sm.state !== State.RECONNECTING) {
        this.sm.transition(State.RECONNECTING);
      }
      this.emit('close');
      this.#scheduleReconnect();
    });

    this.#ws.on('message', (data) => {
      try {
        const msg = parseMessage(data.toString());
        this.emit('message', msg);
      } catch (err) {
        // Invalid messages silently dropped
      }
    });

    this.#ws.on('error', (err) => {
      this.emit('error', err);
    });
  }

  send(type, payload) {
    const msg = createMessage(type, 'james', payload);

    if (this.sm.state === State.CONNECTED && this.#ws?.readyState === WebSocket.OPEN) {
      this.#ws.send(msg);
      return true;
    }

    // Queue the message
    if (this.#queue.length >= this.#maxQueueSize) {
      this.#queue.shift();
    }
    this.#queue.push(msg);
    return false;
  }

  disconnect() {
    this.#intentionalClose = true;
    clearTimeout(this.#reconnectTimer);
    this.#reconnectTimer = null;
    if (this.#ws) {
      this.#ws.close(1000, 'client disconnect');
    }
  }

  #scheduleReconnect() {
    clearTimeout(this.#reconnectTimer);
    const delay = Math.min(
      BACKOFF_BASE * Math.pow(2, this.#reconnectAttempt),
      BACKOFF_MAX
    );
    const jitter = Math.random() * JITTER_MAX;

    this.#reconnectTimer = setTimeout(() => {
      this.#reconnectAttempt++;
      this.connect();
    }, delay + jitter);
  }

  #flushQueue() {
    while (this.#queue.length > 0 && this.#ws?.readyState === WebSocket.OPEN) {
      this.#ws.send(this.#queue.shift());
    }
  }
}
```

### Test: Reconnect After Server Restart
```javascript
// Source: Pattern from existing test/connection.test.js adapted for reconnect
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { createCommsServer } from '../bono/comms-server.js';
import { CommsClient } from '../james/comms-client.js';
import { State } from '../shared/state.js';

describe('Auto-Reconnect', () => {
  it('reconnects after server restart', async () => {
    const PSK = 'test-key';
    let server = createCommsServer({ port: 0, psk: PSK });
    await server.start();
    const port = server.server.address().port;

    const client = new CommsClient({ url: `ws://localhost:${port}`, psk: PSK });

    // Wait for initial connect
    const connected1 = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.CONNECTED) r(); });
    });
    client.connect();
    await connected1;
    assert.equal(client.state, State.CONNECTED);

    // Kill server -- client should enter RECONNECTING
    const reconnecting = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.RECONNECTING) r(); });
    });
    await server.stop();
    await reconnecting;
    assert.equal(client.state, State.RECONNECTING);

    // Start a new server on the same port
    server = createCommsServer({ port, psk: PSK });
    await server.start();

    // Client should reconnect
    const connected2 = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.CONNECTED) r(); });
    });
    await connected2;
    assert.equal(client.state, State.CONNECTED);

    client.disconnect();
    await server.stop();
  });
});
```

### Test: Message Queue Replay
```javascript
// Source: Pattern based on existing echo.test.js
describe('Message Queue', () => {
  it('queues messages while disconnected and replays on reconnect', async () => {
    const PSK = 'test-key';
    let server = createCommsServer({ port: 0, psk: PSK });
    await server.start();
    const port = server.server.address().port;

    const received = [];
    server.wss.on('message', (msg) => received.push(msg));

    const client = new CommsClient({ url: `ws://localhost:${port}`, psk: PSK });

    // Connect, then kill server
    const connected = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.CONNECTED) r(); });
    });
    client.connect();
    await connected;

    const reconnecting = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.RECONNECTING) r(); });
    });
    await server.stop();
    await reconnecting;

    // Send messages while disconnected -- they should be queued
    assert.equal(client.send('echo', { seq: 1 }), false);
    assert.equal(client.send('echo', { seq: 2 }), false);
    assert.equal(client.send('echo', { seq: 3 }), false);
    assert.equal(client.queueSize, 3);

    // Restart server -- client reconnects, queue flushes
    server = createCommsServer({ port, psk: PSK });
    const received2 = [];
    server.wss.on('message', (msg) => received2.push(msg));
    await server.start();

    // Wait for reconnect + flush
    const connected2 = new Promise(r => {
      client.on('state', (e) => { if (e.state === State.CONNECTED) r(); });
    });
    await connected2;

    // Give a tick for queue flush
    await new Promise(r => setTimeout(r, 50));

    assert.equal(received2.length, 3);
    assert.equal(received2[0].payload.seq, 1);  // order preserved
    assert.equal(received2[1].payload.seq, 2);
    assert.equal(received2[2].payload.seq, 3);
    assert.equal(client.queueSize, 0);

    client.disconnect();
    await server.stop();
  });
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reconnecting-websocket` npm | Hand-rolled reconnect in client class | Package abandoned 2020 | 40 lines of code vs dead dependency. Custom is better. |
| Fixed retry interval | Exponential backoff + jitter | Standard since 2015+ | Prevents server overload during outages. |
| Unbounded queue | Bounded FIFO with configurable max | Always best practice | Prevents OOM during extended outages. |
| Re-create client on reconnect | Reuse client, create new WebSocket internally | N/A (our design from Phase 1) | Single `CommsClient` instance across reconnects. Clean API. |

**Deprecated/outdated:**
- `reconnecting-websocket` (v4.4.0, 2020): Last release 6 years ago. Do not use.
- Browser `navigator.onLine` for reconnect triggering: Not applicable -- this is a Node.js CLI process, not a browser.

## Open Questions

1. **Max reconnect attempts vs infinite retry**
   - What we know: The requirement says "auto-reconnect with exponential backoff." It does not specify a max retry count.
   - What's unclear: Should the client give up after N failures and transition to DISCONNECTED, or retry forever?
   - Recommendation: Retry forever (no max). The comms-link is a persistent infrastructure service. If Bono's VPS reboots, James should keep trying until it comes back. Add an event (`max_retries`) that fires at a configurable threshold (e.g., 10 attempts) for logging/alerting, but do NOT stop reconnecting. The watchdog (Phase 4+) will handle true failure scenarios. Set `MAX_RECONNECT_FAILURES = 0` to mean "unlimited."

2. **Queue TTL (time-to-live) for messages**
   - What we know: The requirement says "no messages are lost." The queue has a max size of 100.
   - What's unclear: Should very old messages (e.g., queued 2 hours ago) be dropped?
   - Recommendation: For v1, do NOT implement TTL. The message rate is ~1 msg/min. With a 100-message cap, the queue cannot hold more than ~100 minutes of messages. Stale messages are still informative (timestamps are in the envelope). If needed in the future, add TTL as a simple filter during flush.

3. **Server-side awareness of reconnection**
   - What we know: Bono's server treats each WebSocket connection independently. It has no concept of "James reconnected."
   - What's unclear: Does Bono need to know that this is a reconnection (vs first connection)?
   - Recommendation: Not for Phase 2. Phase 3 (heartbeat) will add application-level connection awareness. For now, a reconnect looks like a new connection to the server, which is fine.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Node.js built-in test runner (`node:test`) v22.14.0 |
| Config file | None needed -- zero config |
| Quick run command | `node --test test/*.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WS-02 | Client enters RECONNECTING state on unintentional close | integration | `node --test test/reconnect.test.js` | Wave 0 |
| WS-02 | Client reconnects after server restart | integration | `node --test test/reconnect.test.js` | Wave 0 |
| WS-02 | Backoff delay increases exponentially (1s, 2s, 4s...) capped at 30s | unit | `node --test test/reconnect.test.js` | Wave 0 |
| WS-02 | Backoff resets to 1s after successful reconnect | integration | `node --test test/reconnect.test.js` | Wave 0 |
| WS-02 | No reconnect after intentional disconnect() | integration | `node --test test/reconnect.test.js` | Wave 0 |
| WS-05 | send() queues messages when not connected (returns false) | unit | `node --test test/queue.test.js` | Wave 0 |
| WS-05 | Queued messages are replayed in order on reconnect | integration | `node --test test/queue.test.js` | Wave 0 |
| WS-05 | Queue is bounded (oldest dropped when full) | unit | `node --test test/queue.test.js` | Wave 0 |
| WS-05 | Queue is empty after successful flush | integration | `node --test test/queue.test.js` | Wave 0 |
| WS-05 | No duplicate messages received by server after reconnect | integration | `node --test test/queue.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/*.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green (all 27 existing + new Phase 2 tests) before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/reconnect.test.js` -- covers WS-02 (auto-reconnect, backoff, intentional close)
- [ ] `test/queue.test.js` -- covers WS-05 (queue, replay, bounded size, no duplicates)

*(Existing test infrastructure covers all other needs -- no framework install or fixture files needed)*

## Sources

### Primary (HIGH confidence)
- [ws GitHub repository](https://github.com/websockets/ws) - `close` event, `terminate()` vs `close()`, ping/pong, event model
- [ws issue #1142](https://github.com/websockets/ws/issues/1142) - Confirmed `terminate()` is immediate, `close()` does handshake; both emit `close` event
- [WebSocket close codes reference](https://websocket.org/reference/close-codes/) - Code 1000 = normal, 1006 = abnormal (set by runtime, not app). Codes 1000/1008/1003 should NOT trigger reconnect.
- Existing codebase: `james/comms-client.js`, `shared/state.js` - Phase 1 state machine already has RECONNECTING with all valid transitions

### Secondary (MEDIUM confidence)
- [OneUptime WebSocket reconnection guide (Jan 2026)](https://oneuptime.com/blog/post/2026-01-24-websocket-reconnection-logic/view) - Backoff formula, message queue pattern, state management. Verified against ws library behavior.
- [DEV.to exponential backoff article](https://dev.to/hexshift/robust-websocket-reconnection-strategies-in-javascript-with-exponential-backoff-40n1) - Backoff with jitter pattern, thundering herd prevention
- [Codegenes.net ws reconnect guide](https://www.codegenes.net/blog/nodejs-websocket-how-to-reconnect-when-server-restarts/) - Complete reconnect + queue + cleanup pattern for ws library
- [OneUptime abnormal closure guide (Jan 2026)](https://oneuptime.com/blog/post/2026-01-24-websocket-connection-closed-abnormally/view) - Code 1006 handling

### Tertiary (LOW confidence)
- None. All findings verified against primary or secondary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies. All patterns use ws ^8.19.0 + Node.js built-ins already installed and tested in Phase 1.
- Architecture: HIGH - All changes are confined to `CommsClient`. The state machine, connect/cleanup, and `send()` return-value API were designed in Phase 1 to accommodate this exact enhancement.
- Pitfalls: HIGH - Timer leaks, double-close state errors, queue interleaving, and auth-failure loops are all well-documented patterns verified across multiple sources and the ws issue tracker.

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (30 days -- stable domain, ws library is mature, patterns are well-established)
