# Phase 1: WebSocket Connection - Research

**Researched:** 2026-03-12
**Domain:** WebSocket client/server with PSK authentication and connection state tracking (Node.js)
**Confidence:** HIGH

## Summary

Phase 1 establishes the foundation: a persistent, authenticated WebSocket connection from James (Windows, behind NAT) to Bono's VPS (Linux, public IP). This is a well-understood problem with mature tooling. The `ws` library (v8.19.0) is the standard choice for Node.js WebSocket -- zero dependencies, RFC 6455 compliant, actively maintained. The entire phase can be implemented with this single npm dependency plus Node.js built-ins.

The three requirements (WS-01, WS-03, WS-04) map cleanly to distinct implementation units: (1) the WebSocket client connecting outbound to the VPS, (2) PSK authentication during the upgrade handshake, and (3) a connection state machine emitting CONNECTED/RECONNECTING/DISCONNECTED states. Phase 1 intentionally excludes auto-reconnect with backoff (WS-02, Phase 2) and message queuing (WS-05, Phase 2), but the state machine design must accommodate those future additions. A minimal echo test (send JSON, receive JSON) validates the end-to-end path.

**Primary recommendation:** Use `ws` v8.19.0 with custom `headers` option for PSK auth (not query parameters), EventEmitter-based state machine, and `node:test` for validation. Build both James-side client and Bono-side server in this phase.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WS-01 | James establishes persistent WebSocket connection to Bono's VPS (outbound, NAT-safe) | `ws` WebSocket client connects outbound -- NAT-safe by design since James initiates. Server uses `noServer` mode with HTTP upgrade handling. |
| WS-03 | Pre-shared key (PSK) authentication during WebSocket handshake | `ws` client supports `headers` option to send PSK in `Authorization` header during upgrade. Server validates in `upgrade` handler before calling `handleUpgrade`. Reject with 401 + `socket.destroy()`. |
| WS-04 | Connection state machine with three states: CONNECTED, RECONNECTING, DISCONNECTED | Extend `EventEmitter` with explicit state property. Transitions triggered by `ws` events (`open`, `close`, `error`). RECONNECTING state is a placeholder for Phase 2 -- start in DISCONNECTED, transition to CONNECTED on `open`, back to DISCONNECTED on `close`/`error`. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ws` | ^8.19.0 | WebSocket client (James) + server (Bono) | 22.7k GitHub stars, zero dependencies, RFC 6455 compliant, actively maintained (latest release 2026). The undisputed standard for Node.js WebSocket. |
| Node.js | 22.14.0 LTS | Runtime | Already installed on James. LTS = stable. Built-in `EventEmitter`, `crypto`, `http` cover all needs. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `node:events` | built-in | EventEmitter for state machine | Connection state changes emit events for consumers |
| `node:http` | built-in | HTTP server for WebSocket upgrade | Bono-side: create HTTP server, handle `upgrade` event |
| `node:crypto` | built-in | Generate message IDs, PSK validation | `randomUUID()` for message IDs, `timingSafeEqual` for PSK comparison |
| `node:test` | built-in | Test runner | Stable in Node.js 22. Zero dependencies. Sufficient for unit + integration tests. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ws` | Socket.IO | ~100KB overhead, rooms/namespaces/polling fallback unused. 1:1 connection does not need Socket.IO. |
| `ws` | `uWebSockets.js` | C++ compilation, overkill for single connection at ~0.07 msg/sec. |
| Custom headers auth | Query parameter auth (`?token=...`) | Query params appear in server logs and URL history. Since both sides are Node.js (not browser), custom headers work and are more secure. |
| `node:test` | Jest/Vitest | External dependency for ~10 tests. `node:test` is built-in and stable on Node.js 22. |

**Installation:**
```bash
npm init -y
# Set type to module in package.json
npm install ws
```

## Architecture Patterns

### Recommended Project Structure (Phase 1 scope)
```
comms-link/
├── james/                    # James-side (runs on Windows)
│   ├── index.js              # Entry point -- creates CommsClient, connects
│   └── comms-client.js       # WebSocket client + connection state machine
├── bono/                     # Bono-side (runs on VPS)
│   ├── index.js              # Entry point -- starts server
│   └── comms-server.js       # WebSocket server + PSK auth
├── shared/                   # Shared between both sides
│   └── protocol.js           # Message envelope, types, version constant
├── test/                     # Tests
│   ├── protocol.test.js      # Message format validation
│   ├── auth.test.js          # PSK auth accept/reject
│   └── connection.test.js    # State machine transitions
├── package.json
├── .gitignore
└── .env.example              # Template: COMMS_PSK=<64-char-hex>
```

**Why this structure:**
- `james/` and `bono/` are separate entry points. James deploys `james/` + `shared/`, Bono deploys `bono/` + `shared/`.
- `shared/protocol.js` is the single source of truth for message format. Both sides import it.
- Flat modules: each file owns one concern. No deep nesting for ~200 lines of code.

### Pattern 1: EventEmitter-Based Connection State Machine
**What:** The `CommsClient` class extends `EventEmitter`. It tracks connection state as a property and emits state-change events. Consumers (future watchdog, heartbeat) subscribe to events.
**When to use:** Always -- this is the core pattern for WS-04.
**Example:**
```javascript
// Source: ws library events + Node.js EventEmitter pattern
import { EventEmitter } from 'node:events';
import WebSocket from 'ws';

const State = { DISCONNECTED: 'DISCONNECTED', RECONNECTING: 'RECONNECTING', CONNECTED: 'CONNECTED' };

class CommsClient extends EventEmitter {
  #state = State.DISCONNECTED;
  #ws = null;

  get state() { return this.#state; }

  #setState(newState) {
    if (this.#state === newState) return;
    const prev = this.#state;
    this.#state = newState;
    this.emit('state', newState, prev);
  }

  connect(url, psk) {
    this.#ws = new WebSocket(url, {
      headers: { 'Authorization': `Bearer ${psk}` }
    });
    this.#ws.on('open', () => this.#setState(State.CONNECTED));
    this.#ws.on('close', () => this.#setState(State.DISCONNECTED));
    this.#ws.on('error', () => {}); // 'close' always follows 'error'
  }

  send(type, payload) {
    if (this.#state !== State.CONNECTED) return false;
    this.#ws.send(JSON.stringify({ v: 1, type, from: 'james', ts: Date.now(), payload }));
    return true;
  }
}
```

### Pattern 2: PSK Authentication During HTTP Upgrade
**What:** Bono's server creates an HTTP server with `noServer` mode for the WebSocketServer. The `upgrade` event is intercepted to validate the PSK before accepting the connection. Invalid PSK gets a 401 response and the socket is destroyed.
**When to use:** Always -- this is the core pattern for WS-03.
**Example:**
```javascript
// Source: ws GitHub README authentication example
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { timingSafeEqual } from 'node:crypto';

const server = createServer();
const wss = new WebSocketServer({ noServer: true });

server.on('upgrade', (request, socket, head) => {
  socket.on('error', (err) => console.error('Socket error:', err));

  const authHeader = request.headers['authorization'];
  const token = authHeader?.startsWith('Bearer ') ? authHeader.slice(7) : null;

  if (!token || !isValidPSK(token)) {
    socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
    socket.destroy();
    return;
  }

  socket.removeListener('error', console.error);
  wss.handleUpgrade(request, socket, head, (ws) => {
    wss.emit('connection', ws, request);
  });
});

function isValidPSK(token) {
  const expected = Buffer.from(process.env.COMMS_PSK, 'utf8');
  const received = Buffer.from(token, 'utf8');
  if (expected.length !== received.length) return false;
  return timingSafeEqual(expected, received);
}
```

### Pattern 3: JSON Message Envelope
**What:** Every message follows a standard envelope format with version, type, sender, timestamp, and payload. This allows future phases to add new message types without protocol changes.
**When to use:** Every message sent over the WebSocket.
**Example:**
```javascript
// shared/protocol.js
export const PROTOCOL_VERSION = 1;
export const MessageType = {
  ECHO: 'echo',           // Phase 1: simple round-trip test
  ECHO_REPLY: 'echo_reply',
  HEARTBEAT: 'heartbeat', // Phase 3 (reserved)
  STATUS: 'status',       // Phase 4 (reserved)
  FILE_SYNC: 'file_sync', // Phase 7 (reserved)
};

export function createMessage(type, from, payload = {}) {
  return JSON.stringify({
    v: PROTOCOL_VERSION,
    type,
    from,
    ts: Date.now(),
    id: crypto.randomUUID(),
    payload,
  });
}

export function parseMessage(raw) {
  const msg = JSON.parse(raw);
  if (msg.v !== PROTOCOL_VERSION) throw new Error(`Unknown protocol version: ${msg.v}`);
  if (!msg.type || !msg.from) throw new Error('Missing required fields: type, from');
  return msg;
}
```

### Anti-Patterns to Avoid
- **PSK in query parameter:** `wss://host?token=SECRET` leaks the token into server access logs, proxy logs, and URL history. Since both sides are Node.js (not browser-limited), use the `headers` option instead: `new WebSocket(url, { headers: { Authorization: 'Bearer SECRET' } })`.
- **PSK in code or config.json:** Gets committed to git. Use `.env` file (gitignored) with `process.env.COMMS_PSK`.
- **Ignoring `error` event:** In `ws`, the `error` event is always followed by a `close` event. Register an empty `error` handler to prevent unhandled error crashes, but do state transitions in the `close` handler only.
- **Checking readyState instead of state machine:** `ws.readyState` has 4 states (CONNECTING/OPEN/CLOSING/CLOSED) which do not map to our 3-state model. Use the custom state machine that wraps `ws` events, not raw readyState checks.
- **Timing-unsafe PSK comparison:** Using `===` for string comparison is vulnerable to timing attacks. Use `crypto.timingSafeEqual()` for PSK validation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WebSocket protocol | Raw TCP/HTTP upgrade | `ws` library | RFC 6455 compliance, frame parsing, masking, ping/pong -- hundreds of edge cases |
| UUID generation | Custom ID generator | `crypto.randomUUID()` | Built-in, cryptographically secure, RFC 4122 v4 compliant |
| Timing-safe comparison | `===` string comparison | `crypto.timingSafeEqual()` | Prevents timing side-channel attacks on PSK comparison |
| JSON parse error handling | Try-catch everywhere | Centralized `parseMessage()` in protocol.js | Single validation point, consistent error handling |

**Key insight:** The `ws` library handles the entire WebSocket protocol (framing, masking, close handshake, ping/pong, permessage-deflate). Writing this from scratch would be thousands of lines with subtle security implications. The protocol envelope (`shared/protocol.js`) is the only custom format code needed.

## Common Pitfalls

### Pitfall 1: NAT/Firewall Silently Kills Idle WebSocket
**What goes wrong:** Intermediate network equipment (James's router, ISP NAT) silently drops idle TCP connections after 30-300 seconds. Neither side receives a close frame. The connection appears alive but messages vanish.
**Why it happens:** NAT tables evict idle entries. Windows TCP keepalive defaults to 2 hours -- too long to prevent NAT timeout.
**How to avoid:** Phase 1 does not implement heartbeat (that is Phase 3), but the connection should use WebSocket-level ping/pong to keep NAT entries alive. The `ws` server can send `ws.ping()` every 25 seconds. The `ws` client automatically responds with pong (RFC 6455 requirement).
**Warning signs:** Messages sent but never received. No `close` event fires. Connection appears stuck in CONNECTED state.
**Phase 1 mitigation:** Add a simple ping interval on the server side to keep the connection alive. This is NOT the application-level heartbeat (Phase 3) -- it is a transport-level keepalive.

### Pitfall 2: Error Event Without Close Handler Crashes Process
**What goes wrong:** An unhandled `error` event on the WebSocket instance throws and crashes the Node.js process. In `ws`, the `error` event fires before `close` on connection failures.
**Why it happens:** Node.js EventEmitter throws on unhandled `error` events by default. If you only listen for `close` but not `error`, the process crashes.
**How to avoid:** Always register an `error` handler: `ws.on('error', () => {})`. Do NOT perform state transitions or reconnection in the `error` handler -- the `close` event that follows is the definitive signal.
**Warning signs:** Unhandled rejection or "Error event not caught" in logs.

### Pitfall 3: PSK Committed to Git
**What goes wrong:** The pre-shared key is placed in `config.json` or hardcoded, then accidentally committed to the repository. Anyone with repo access can impersonate James or Bono.
**Why it happens:** Convenience -- putting secrets in config files is easy. `.gitignore` not set up early enough.
**How to avoid:** Store PSK in `.env` (gitignored). Create `.env.example` with placeholder. Validate `process.env.COMMS_PSK` exists on startup, exit with clear error if missing.
**Warning signs:** `git diff` shows the PSK value.

### Pitfall 4: Timing Attack on PSK Comparison
**What goes wrong:** Using `===` for PSK comparison leaks information about which characters match through response timing differences. An attacker can brute-force the PSK character by character.
**Why it happens:** JavaScript `===` short-circuits on first mismatch. Comparing a correct prefix takes longer than a completely wrong string.
**How to avoid:** Use `crypto.timingSafeEqual(Buffer.from(received), Buffer.from(expected))`. Check lengths match first (different-length buffers throw).
**Warning signs:** None visible -- this is a silent vulnerability.

### Pitfall 5: Multiple WebSocket Instances on Reconnect
**What goes wrong:** On connection failure, a new `WebSocket` is created without properly cleaning up the old one. Event listeners accumulate. Multiple `open` callbacks fire. State machine receives contradictory events.
**Why it happens:** The old `ws` instance is not explicitly closed/terminated before creating a new one. Old listeners are not removed.
**How to avoid:** Before creating a new WebSocket: (1) call `ws.removeAllListeners()`, (2) call `ws.terminate()` (not `ws.close()` -- terminate is immediate), (3) then create the new instance. In Phase 1 we don't implement auto-reconnect, but the `connect()` method must clean up any existing connection.
**Warning signs:** Memory leak, duplicate event handlers, state machine logging contradictory transitions.

## Code Examples

Verified patterns from official sources:

### WebSocket Client Connection with Custom Headers (James side)
```javascript
// Source: ws GitHub README + issue #467
import WebSocket from 'ws';

const ws = new WebSocket('wss://72.60.101.58:PORT/comms', {
  headers: {
    'Authorization': `Bearer ${process.env.COMMS_PSK}`
  }
});

ws.on('open', () => {
  console.log('Connected to Bono');
  ws.send(JSON.stringify({ v: 1, type: 'echo', from: 'james', ts: Date.now(), payload: { text: 'hello' } }));
});

ws.on('message', (data) => {
  const msg = JSON.parse(data.toString());
  console.log('Received:', msg);
});

ws.on('close', (code, reason) => {
  console.log(`Disconnected: ${code} ${reason}`);
});

ws.on('error', () => {}); // close always follows
```

### WebSocket Server with PSK Auth (Bono side)
```javascript
// Source: ws GitHub README authentication example
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { timingSafeEqual } from 'node:crypto';

const PSK = process.env.COMMS_PSK;
const server = createServer();
const wss = new WebSocketServer({ noServer: true });

server.on('upgrade', (request, socket, head) => {
  socket.on('error', (err) => console.error(err));

  const auth = request.headers['authorization'];
  const token = auth?.startsWith('Bearer ') ? auth.slice(7) : null;

  if (!token || !safeCompare(token, PSK)) {
    socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
    socket.destroy();
    return;
  }

  socket.removeListener('error', console.error);
  wss.handleUpgrade(request, socket, head, (ws) => {
    wss.emit('connection', ws, request);
  });
});

function safeCompare(a, b) {
  const bufA = Buffer.from(a, 'utf8');
  const bufB = Buffer.from(b, 'utf8');
  if (bufA.length !== bufB.length) return false;
  return timingSafeEqual(bufA, bufB);
}

wss.on('connection', (ws) => {
  console.log('James connected');
  ws.on('message', (data) => {
    const msg = JSON.parse(data.toString());
    if (msg.type === 'echo') {
      ws.send(JSON.stringify({ v: 1, type: 'echo_reply', from: 'bono', ts: Date.now(), payload: msg.payload }));
    }
  });
});

server.listen(PORT);
```

### Transport-Level Keepalive (Server-Side Ping)
```javascript
// Source: ws GitHub README heartbeat/ping-pong example
// Keep NAT entries alive with WebSocket-level pings (not application heartbeat)
const PING_INTERVAL = 25_000; // 25 seconds -- under typical NAT timeout

const interval = setInterval(() => {
  for (const ws of wss.clients) {
    if (ws.isAlive === false) return ws.terminate();
    ws.isAlive = false;
    ws.ping();
  }
}, PING_INTERVAL);

wss.on('connection', (ws) => {
  ws.isAlive = true;
  ws.on('pong', () => { ws.isAlive = true; });
});

wss.on('close', () => clearInterval(interval));
```

### Connection State Machine
```javascript
// Source: Custom pattern based on ws events + EventEmitter
import { EventEmitter } from 'node:events';

export const State = Object.freeze({
  DISCONNECTED: 'DISCONNECTED',
  RECONNECTING: 'RECONNECTING',  // Phase 2 will use this
  CONNECTED: 'CONNECTED',
});

export class ConnectionStateMachine extends EventEmitter {
  #state = State.DISCONNECTED;

  get state() { return this.#state; }

  transition(newState) {
    const valid = {
      [State.DISCONNECTED]: [State.CONNECTED, State.RECONNECTING],
      [State.RECONNECTING]: [State.CONNECTED, State.DISCONNECTED],
      [State.CONNECTED]: [State.DISCONNECTED, State.RECONNECTING],
    };
    if (!valid[this.#state]?.includes(newState)) {
      throw new Error(`Invalid transition: ${this.#state} -> ${newState}`);
    }
    const prev = this.#state;
    this.#state = newState;
    this.emit('state', { state: newState, previous: prev, timestamp: Date.now() });
  }
}
```

### Test Example with node:test
```javascript
// Source: Node.js 22 built-in test runner
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { ConnectionStateMachine, State } from '../james/comms-client.js';

describe('ConnectionStateMachine', () => {
  it('starts in DISCONNECTED state', () => {
    const sm = new ConnectionStateMachine();
    assert.equal(sm.state, State.DISCONNECTED);
  });

  it('transitions DISCONNECTED -> CONNECTED', () => {
    const sm = new ConnectionStateMachine();
    sm.transition(State.CONNECTED);
    assert.equal(sm.state, State.CONNECTED);
  });

  it('rejects invalid transitions', () => {
    const sm = new ConnectionStateMachine();
    // Cannot go from DISCONNECTED directly to RECONNECTING... wait, actually we CAN
    // The DISCONNECTED->RECONNECTING transition is valid (Phase 2 reconnect attempt)
    // But CONNECTED->CONNECTED is not
    sm.transition(State.CONNECTED);
    assert.throws(() => sm.transition(State.CONNECTED), /Invalid transition/);
  });

  it('emits state event on transition', () => {
    const sm = new ConnectionStateMachine();
    let received = null;
    sm.on('state', (evt) => { received = evt; });
    sm.transition(State.CONNECTED);
    assert.equal(received.state, State.CONNECTED);
    assert.equal(received.previous, State.DISCONNECTED);
  });
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reconnecting-websocket` npm | Hand-rolled reconnect on `ws` | 2020+ (last release) | `reconnecting-websocket` abandoned 6 years. Reconnect logic is ~30 lines with `ws`. |
| Socket.IO for everything | Raw `ws` for known-endpoint connections | Always (for 1:1 connections) | Socket.IO overhead unjustified for 2-node systems. |
| Query param auth (`?token=`) | Custom `Authorization` header | N/A (Node.js always supported headers) | Headers don't leak into logs. Only matters for Node.js clients (browsers cannot set WS headers). |
| `===` for secret comparison | `crypto.timingSafeEqual()` | Best practice since Node.js 6 | Prevents timing attacks on PSK. |
| CommonJS (`require`) | ESM (`import`) | Node.js 16+ | `"type": "module"` in package.json. Matches racingpoint-mcp-gmail pattern. |

**Deprecated/outdated:**
- `reconnecting-websocket` (v4.4.0, 2020): Abandoned. Trivial to implement.
- `extraHeaders` option in some ws examples: The correct option name is `headers` (not `extraHeaders`). Some old blog posts use `extraHeaders` which is not a ws option.

## Open Questions

1. **VPS port and TLS**
   - What we know: Bono's VPS is at 72.60.101.58. Port 443 is ideal (avoids ISP blocking).
   - What's unclear: Is port 443 already used by another service (Nginx, etc.)? Does Bono already have a reverse proxy that could terminate TLS for the WebSocket path?
   - Recommendation: Start with a non-standard port (e.g., 8765) for development/testing. Coordinate with Bono on production port + TLS setup. If Bono has Nginx, add a `location /comms` proxy_pass to the ws server.

2. **PSK generation and distribution**
   - What we know: PSK should be a 64-char hex string stored in `.env` on both sides.
   - What's unclear: How to securely share the initial PSK between James and Bono.
   - Recommendation: Generate with `node -e "console.log(require('crypto').randomBytes(32).toString('hex'))"`. Share via existing secure channel (email between james@ and bono@racingpoint.in, or direct file placement).

3. **Bono-side deployment coordination**
   - What we know: Bono runs PM2 on the VPS. The server-side code must be deployed there.
   - What's unclear: Exact deployment workflow. Does Bono's Claude Code auto-pull from a shared repo?
   - Recommendation: Push server code to the `comms-link` repo. Email Bono with deployment instructions. This is requirement CO-02 which is Phase 8, but the server must exist for Phase 1 testing.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Node.js built-in test runner (`node:test`) v22.14.0 |
| Config file | None needed -- Node.js test runner works with zero config |
| Quick run command | `node --test test/*.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WS-01 | James connects to Bono via WebSocket and connection stays open | integration | `node --test test/connection.test.js` | Wave 0 |
| WS-03 | Connection rejected without valid PSK | integration | `node --test test/auth.test.js` | Wave 0 |
| WS-03 | Connection accepted with valid PSK | integration | `node --test test/auth.test.js` | Wave 0 |
| WS-04 | State is DISCONNECTED before connect | unit | `node --test test/state-machine.test.js` | Wave 0 |
| WS-04 | State transitions to CONNECTED on open | unit | `node --test test/state-machine.test.js` | Wave 0 |
| WS-04 | State transitions to DISCONNECTED on close | unit | `node --test test/state-machine.test.js` | Wave 0 |
| WS-04 | State is observable at any time | unit | `node --test test/state-machine.test.js` | Wave 0 |
| E2E | JSON message from James arrives at Bono and vice versa | integration | `node --test test/echo.test.js` | Wave 0 |

### Sampling Rate
- **Per task commit:** `node --test test/*.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test/state-machine.test.js` -- covers WS-04 state machine transitions
- [ ] `test/auth.test.js` -- covers WS-03 PSK accept/reject
- [ ] `test/connection.test.js` -- covers WS-01 persistent connection
- [ ] `test/echo.test.js` -- covers success criterion 4 (JSON message round-trip)
- [ ] `test/protocol.test.js` -- covers message envelope format validation
- [ ] `package.json` -- project initialization with `ws` dependency and `"type": "module"`

## Sources

### Primary (HIGH confidence)
- [ws GitHub repository](https://github.com/websockets/ws) - Authentication during upgrade example, ping/pong, event model
- [ws issue #467](https://github.com/websockets/ws/issues/467) - Confirmed `headers` option (not `extraHeaders`) for client-side custom headers
- [ws npm page](https://www.npmjs.com/package/ws) - Version 8.19.0 confirmed as latest (published ~2 months ago)
- [Node.js test runner docs](https://nodejs.org/api/test.html) - Stable in Node.js 22, built-in assertions
- [MDN WebSocket readyState](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/readyState) - 4 states: CONNECTING(0), OPEN(1), CLOSING(2), CLOSED(3)
- [Node.js crypto.timingSafeEqual](https://nodejs.org/api/crypto.html#cryptotimingsafeequala-b) - Prevents timing attacks

### Secondary (MEDIUM confidence)
- [Ably WebSocket authentication guide](https://ably.com/blog/websocket-authentication) - Headers vs query params security comparison
- [Ably FAQ on access_token in query params](https://faqs.ably.com/is-it-secure-to-send-the-access_token-as-part-of-the-websocket-url-query-params) - Confirmed query params leak into logs
- [WebSocket heartbeat patterns](https://oneuptime.com/blog/post/2026-01-24-websocket-heartbeat-ping-pong/view) - Jan 2026 guide on ping/pong intervals
- [websockets library keepalive docs](https://websockets.readthedocs.io/en/stable/topics/keepalive.html) - NAT timeout warning

### Tertiary (LOW confidence)
- [ws releases page](https://github.com/websockets/ws/releases) - Recent changelog items (fetched summary only)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - `ws` is verified as actively maintained, v8.19.0 confirmed, zero-dependency. Node.js 22 built-ins confirmed stable.
- Architecture: HIGH - `noServer` + `upgrade` handler auth pattern is from the official ws README. EventEmitter state machine is standard Node.js pattern.
- Pitfalls: HIGH - NAT timeout is well-documented. PSK security practices are standard. The `headers` vs `extraHeaders` confusion is verified via ws issue #467.
- Testing: HIGH - `node:test` is stable in Node.js 22 (confirmed by official docs and multiple independent sources).

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (30 days -- stable domain, unlikely to change)
