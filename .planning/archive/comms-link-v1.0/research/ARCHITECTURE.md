# Architecture Research

**Domain:** AI-to-AI Communication Link with Process Supervision
**Researched:** 2026-03-12
**Confidence:** HIGH

## System Overview

```
JAMES SIDE (Windows 11, 192.168.31.27, behind NAT)
===================================================
                                    ┌─────────────────────┐
                                    │   Claude Code CLI    │
                                    │   (monitored proc)   │
                                    └──────────┬──────────┘
                                               │ spawn/kill
                                    ┌──────────┴──────────┐
                                    │     Watchdog         │
                                    │  (top-level daemon)  │
                                    │                      │
                                    │  - process monitor   │
                                    │  - crash detection   │
                                    │  - clean restart     │
                                    │  - zombie cleanup    │
                                    └──────────┬──────────┘
                                               │ events
                                    ┌──────────┴──────────┐
                                    │   Comms Client       │
                                    │                      │
                                    │  - WS client (ws)    │
                                    │  - heartbeat sender  │
                                    │  - file watcher      │
                                    │  - reconnect logic   │
                                    │  - email fallback    │
                                    └──────────┬──────────┘
                                               │
                                    WebSocket (wss://)
                                    outbound from James
                                               │
═══════════════════════════════════════════════════════════
                     INTERNET
═══════════════════════════════════════════════════════════
                                               │
BONO SIDE (Linux VPS, 72.60.101.58, public IP)
===================================================
                                    ┌──────────┴──────────┐
                                    │   Comms Server       │
                                    │                      │
                                    │  - WS server (ws)    │
                                    │  - heartbeat monitor │
                                    │  - file receiver     │
                                    │  - alert dispatcher  │
                                    └──┬───────────────┬───┘
                                       │               │
                            ┌──────────┴──┐   ┌───────┴──────────┐
                            │ Evolution   │   │  Gmail API       │
                            │ API         │   │  (email fallback)│
                            │ (WhatsApp)  │   │                  │
                            └─────────────┘   └──────────────────┘
                                       │               │
                                    ┌──┴───────────────┴──┐
                                    │   Uday's Phone      │
                                    │   (WhatsApp + email) │
                                    └─────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | Host | Typical Implementation |
|-----------|----------------|------|------------------------|
| **Watchdog** | Monitor Claude Code process, detect crashes, clean restart, zombie cleanup | James (Win11) | Node.js daemon using `child_process.spawn` + `tasklist`/`taskkill` |
| **Comms Client** | Maintain WebSocket to Bono, send heartbeats, sync files, reconnect on failure | James (Win11) | `ws` WebSocket client with exponential backoff |
| **File Watcher** | Detect LOGBOOK.md changes, trigger sync | James (Win11) | `chokidar` watching single file, debounced |
| **Comms Server** | Accept WebSocket from James, monitor heartbeats, dispatch alerts | Bono (VPS) | `ws` WebSocket server behind PM2 |
| **Alert Dispatcher** | Send WhatsApp/email notifications on state changes | Bono (VPS) | HTTP calls to Evolution API + Gmail API |
| **Email Fallback** | Send status emails when WebSocket is down | James (Win11) | Gmail MCP or direct Google API call |

## Recommended Architecture: Single-Process Daemon on James

The watchdog and comms client MUST be the same process. Rationale:

1. **The watchdog must outlive Claude Code** -- it cannot be inside Claude Code.
2. **The comms client must report Claude Code status** -- it needs direct access to process state.
3. **Two separate daemons** would require inter-process coordination (IPC) adding complexity and a new failure mode.
4. **One daemon** that both monitors Claude Code AND maintains the WebSocket is simpler, more reliable, and has fewer moving parts.

The daemon is a single Node.js process that:
- Spawns and monitors Claude Code as a child process
- Maintains a WebSocket connection to Bono
- Watches LOGBOOK.md for changes
- Sends heartbeats
- Handles reconnection
- Falls back to email when WebSocket is down

## Recommended Project Structure

```
comms-link/
├── james/                    # James-side daemon (runs on Windows)
│   ├── index.js              # Entry point -- starts all subsystems
│   ├── watchdog.js           # Claude Code process monitor
│   │                         #   spawn, kill, detect crash, clean restart
│   ├── comms-client.js       # WebSocket client to Bono
│   │                         #   connect, reconnect, send/receive messages
│   ├── heartbeat.js          # Heartbeat sender (interval timer)
│   ├── file-sync.js          # LOGBOOK.md watcher + sender
│   ├── email-fallback.js     # Gmail-based fallback when WS is down
│   ├── protocol.js           # Shared message format definitions
│   └── config.js             # Configuration (intervals, URLs, secrets)
├── bono/                     # Bono-side server (runs on VPS)
│   ├── index.js              # Entry point -- starts WS server
│   ├── comms-server.js       # WebSocket server
│   │                         #   accept connections, verify auth
│   ├── heartbeat-monitor.js  # Heartbeat timeout detector
│   ├── alert-dispatcher.js   # WhatsApp + email notification sender
│   ├── file-receiver.js      # LOGBOOK.md receiver + writer
│   ├── protocol.js           # Shared message format definitions (same as james/)
│   └── config.js             # Configuration
├── shared/                   # Shared between james/ and bono/
│   └── protocol.js           # Message types, schema, version
├── package.json
└── .env.example              # Template for secrets
```

### Structure Rationale

- **`james/` and `bono/`**: Separate entry points for each side. James deploys `james/`, Bono deploys `bono/`. Shared protocol ensures they speak the same language.
- **`shared/protocol.js`**: Single source of truth for message formats. Copied (not linked) to each side during deploy to avoid cross-repo dependencies.
- **Flat module structure**: Each file owns one concern. No deep nesting -- this is a small, focused system.

## Architectural Patterns

### Pattern 1: Supervisor Tree (Watchdog as Root)

**What:** The watchdog is the root process. Everything else (Claude Code, WebSocket, file watcher) is a child or subsystem. If the watchdog dies, everything dies -- but the watchdog is designed to never die (it has no external dependencies except the OS).

**When to use:** Always. This is the core architectural pattern.

**Trade-offs:** Simple, proven. The risk is that if the watchdog process itself crashes, everything stops. Mitigate with NSSM or a Scheduled Task that restarts the watchdog.

```
                    ┌──────────────┐
                    │   OS / NSSM  │  (restarts watchdog if it dies)
                    └──────┬───────┘
                           │
                    ┌──────┴───────┐
                    │   Watchdog   │  (Node.js -- the ONE process)
                    │              │
                    │  ┌────────┐  │
                    │  │Claude  │  │  child_process.spawn
                    │  │Code    │  │
                    │  └────────┘  │
                    │              │
                    │  ┌────────┐  │
                    │  │WS Conn │  │  ws.WebSocket client
                    │  └────────┘  │
                    │              │
                    │  ┌────────┐  │
                    │  │File    │  │  chokidar watcher
                    │  │Watcher │  │
                    │  └────────┘  │
                    └──────────────┘
```

### Pattern 2: Event-Driven State Machine

**What:** The watchdog tracks system state as a finite state machine. State transitions trigger actions (send alert, reconnect, restart).

**When to use:** For the connection lifecycle and Claude Code lifecycle.

**Trade-offs:** Clear reasoning about what state the system is in. Prevents impossible transitions (e.g., sending "James is back" before connection is established).

**States for Claude Code:**
```
                ┌──────────┐
       ┌────────│ STARTING │──── spawn fails ────┐
       │        └────┬─────┘                      │
       │             │ spawn success               │
       │        ┌────┴─────┐                      │
       │        │ RUNNING  │◄──── restart ────────┤
       │        └────┬─────┘                      │
       │             │ process exit                │
       │        ┌────┴─────┐                      │
       │        │ CRASHED  │──── clean + restart ─┘
       │        └────┬─────┘
       │             │ max retries exceeded
       │        ┌────┴─────┐
       └───────►│ DEAD     │──── alert Uday, wait for manual intervention
                └──────────┘
```

**States for WebSocket Connection:**
```
    ┌──────────────┐
    │ DISCONNECTED │◄──────── connection lost / error
    └──────┬───────┘                │
           │ connect()              │
    ┌──────┴───────┐                │
    │ CONNECTING   │────────────────┘ (on failure)
    └──────┬───────┘
           │ 'open' event
    ┌──────┴───────┐
    │ CONNECTED    │──── heartbeat timeout ──► DISCONNECTED
    └──────────────┘
```

### Pattern 3: Heartbeat with Monotonic Sequence Numbers

**What:** Each heartbeat includes a monotonically increasing sequence number and a timestamp. The receiver tracks the last received sequence. If sequence jumps, missed heartbeats are quantified. If sequence resets to 1, James restarted.

**When to use:** Always for the heartbeat protocol.

**Trade-offs:** Slightly more data per heartbeat (adds ~20 bytes). But enables precise downtime calculation and restart detection without separate "I restarted" messages.

```javascript
// Heartbeat message
{
  type: "heartbeat",
  seq: 42,              // monotonic, resets on process restart
  ts: 1710230400000,    // Unix ms timestamp
  uptime: 3600,         // seconds since watchdog started
  claude: "running",    // claude code status
  memory: 450           // MB, optional health metric
}
```

## Data Flow

### Heartbeat Flow (James --> Bono)

```
James Watchdog                          Bono Server
     │                                       │
     │──── heartbeat {seq:1, claude:"running"} ──►│
     │                                       │ reset timeout timer
     │                                       │
     │  (15 seconds later)                   │
     │                                       │
     │──── heartbeat {seq:2, claude:"running"} ──►│
     │                                       │ reset timeout timer
     │                                       │
     │  (James loses internet)               │
     │                                       │
     │  X  heartbeat {seq:3} never arrives   │
     │                                       │ timeout fires (45s)
     │                                       │──► WhatsApp: "James is DOWN"
     │                                       │──► Email: "James is DOWN"
     │                                       │
     │  (Internet restored)                  │
     │                                       │
     │──── heartbeat {seq:3, claude:"running"} ──►│
     │                                       │──► WhatsApp: "James is BACK"
```

### LOGBOOK.md Sync Flow

```
James commits to LOGBOOK.md
     │
     │ chokidar detects change
     │
     ├─── WS connected?
     │    YES: Send full file content over WebSocket
     │         { type: "file_sync", path: "LOGBOOK.md", content: "...", hash: "sha256..." }
     │         Bono receives, verifies hash, writes file
     │
     │    NO:  Queue for retry when WS reconnects
     │         Also send via email as fallback
```

**Why full file, not diff:** LOGBOOK.md is small (typically <50KB). Delta sync adds complexity (version tracking, merge conflicts, corruption risk) for negligible bandwidth savings. Full file replace is idempotent -- if a message is delivered twice, the result is the same. This eliminates an entire class of sync bugs.

### Claude Code Crash Flow

```
Claude Code exits unexpectedly
     │
     ├─ Watchdog detects 'exit' event on child process
     │
     ├─ Send over WS: { type: "status", claude: "crashed", exit_code: 1 }
     │
     ├─ Clean up zombie processes:
     │   tasklist | findstr claude → taskkill /PID <pid> /F
     │   (kill any orphaned node processes from Claude Code)
     │
     ├─ Wait cooldown (5 seconds)
     │
     ├─ Increment restart counter
     │
     ├─ restart_count < MAX_RESTARTS?
     │   YES: Spawn Claude Code again
     │        Send: { type: "status", claude: "restarting" }
     │        On success: { type: "status", claude: "running" }
     │
     │   NO:  Send: { type: "status", claude: "dead", restarts_exhausted: true }
     │        Alert Uday: "James Claude Code won't stay up -- needs manual attention"
     │        Enter DEAD state (no more auto-restarts)
```

### Key Data Flows Summary

1. **Heartbeat (every 15s):** James --> Bono. Proves liveness. Carries Claude Code status.
2. **File sync (on change):** Bidirectional over WS. Full file replace.
3. **Status events (on change):** James --> Bono. Claude Code lifecycle transitions.
4. **Alerts (on state change):** Bono --> Uday (WhatsApp + email). Triggered by heartbeat timeout or status events.
5. **Email fallback (when WS down):** James --> Bono email. Carries same information as WS messages.

## Protocol Design

### Message Format: JSON over WebSocket

Use JSON, not protobuf. Rationale:
- Only 2 endpoints (James + Bono), not thousands
- Messages are small (<50KB even with LOGBOOK.md)
- Human-readable for debugging
- No build step required
- Protobuf's advantages (compact binary, schema evolution) are irrelevant at this scale

### Message Envelope

```javascript
{
  "v": 1,                    // protocol version
  "type": "heartbeat",       // message type
  "from": "james",           // sender identity
  "ts": 1710230400000,       // Unix ms
  "id": "msg_abc123",        // unique message ID (for ack/dedup)
  "payload": { ... }         // type-specific data
}
```

### Message Types

| Type | Direction | Payload | Purpose |
|------|-----------|---------|---------|
| `heartbeat` | James --> Bono | `{seq, uptime, claude_status, memory_mb}` | Liveness proof |
| `heartbeat_ack` | Bono --> James | `{seq}` | Confirms receipt (optional, for RTT measurement) |
| `status` | James --> Bono | `{claude, detail, restart_count}` | Claude Code state change |
| `file_sync` | Either --> Either | `{path, content, hash}` | Full file replace |
| `file_ack` | Either --> Either | `{path, hash, ok}` | Confirms file received |
| `message` | Either --> Either | `{text}` | Free-form coordination message |
| `alert_sent` | Bono --> James | `{channel, recipient, text}` | Confirms alert was dispatched |
| `ping` | Either | (empty) | WebSocket-level keepalive |
| `pong` | Either | (empty) | WebSocket-level keepalive response |

### Authentication: Pre-Shared Key (PSK)

Use a pre-shared secret, not JWT. Rationale:
- Only 2 parties. No token issuer/verifier separation needed.
- No token expiry management.
- Simpler to implement and debug.

**Handshake:**
1. James connects: `wss://72.60.101.58:PORT/comms?token=<PSK>`
2. Bono's server validates the token in the `upgrade` handler before accepting the WebSocket.
3. If invalid, respond 401 and destroy the socket.
4. PSK is a 64-char hex string stored in `.env` on both sides.

```javascript
// Bono server -- authentication during upgrade
server.on('upgrade', (request, socket, head) => {
  const url = new URL(request.url, 'wss://localhost');
  const token = url.searchParams.get('token');

  if (token !== process.env.COMMS_PSK) {
    socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
    socket.destroy();
    return;
  }

  wss.handleUpgrade(request, socket, head, (ws) => {
    wss.emit('connection', ws, request);
  });
});
```

### Heartbeat Interval: 15 seconds

- **Send interval:** 15 seconds (James sends heartbeat)
- **Timeout:** 45 seconds (Bono declares James dead after 3 missed heartbeats)
- **Rationale:** 15s is responsive enough for operational awareness (Uday knows within ~1 minute) without generating excessive traffic. 45s timeout tolerates transient network hiccups (one or two missed heartbeats are absorbed).

### Reconnection: Exponential Backoff with Jitter

```
Attempt 1: wait 1s + random(0, 500ms)
Attempt 2: wait 2s + random(0, 500ms)
Attempt 3: wait 4s + random(0, 500ms)
Attempt 4: wait 8s + random(0, 500ms)
Attempt 5: wait 16s + random(0, 500ms)
Attempt 6+: wait 30s + random(0, 500ms)  (cap at 30s)
```

**Reset:** Backoff resets to 1s after a successful connection that stays up for >60 seconds.

**Jitter:** Random 0-500ms prevents thundering herd (not relevant with 1 client, but good practice that costs nothing).

### File Sync Protocol: Full Replace with Hash Verification

1. Sender reads full file content
2. Computes SHA-256 hash
3. Sends `file_sync` message with content + hash
4. Receiver writes file, computes hash of written content
5. Sends `file_ack` with hash match status
6. If hash mismatch, sender retries once

**Debounce:** File changes are debounced by 2 seconds (LOGBOOK.md may be written to multiple times in quick succession during a commit).

## Failure Mode Analysis

### F1: WebSocket Connection Drops

**Detection:** `ws` 'close' or 'error' event on James side.
**Response:**
- James: Enter DISCONNECTED state. Start exponential backoff reconnection. Queue any outgoing messages.
- Bono: Heartbeat timeout fires after 45s. Send WhatsApp alert to Uday.
- James: Continue monitoring Claude Code locally. Use email fallback for critical status changes.
**Recovery:** When WS reconnects, James sends current state snapshot. Bono sends `alert_sent` confirmation. Queued messages are flushed.

### F2: Claude Code Crashes

**Detection:** `child_process` 'exit' event with non-zero code (or signal).
**Response:**
- Watchdog enters CRASHED state.
- Kill any zombie `claude` or `node` processes left behind.
- Wait 5s cooldown.
- Restart Claude Code (up to 5 attempts in 5 minutes).
- Report each state transition over WS (or email if WS is down).
**Recovery:** Claude Code starts, watchdog enters RUNNING state, Bono notified.

### F3: VPS is Down (Bono's Server Unreachable)

**Detection:** WebSocket connection fails. Reconnection attempts keep failing.
**Response:**
- James: Continue all local operations (Claude Code monitoring, LOGBOOK watching).
- James: Use email fallback for critical alerts (email goes through Google, not the VPS).
- James: Keep reconnecting with exponential backoff (capped at 30s).
**Recovery:** When VPS comes back, WS reconnects. James sends full state snapshot. Bono reconciles.

### F4: Both Sides Down (Power Outage, etc.)

**Detection:** N/A -- nothing is running.
**Response:**
- When James's machine boots, NSSM/Scheduled Task starts the watchdog.
- Watchdog starts Claude Code.
- Watchdog attempts WS connection to Bono.
- If Bono is also starting up, connection will fail but backoff retries will eventually succeed.
- Each side is independently self-starting. No circular dependency.
**Recovery:** Eventually both are up, WS connects, LOGBOOK syncs, normal operation resumes. Uday gets "James is BACK" when Bono's alerting comes online.

### F5: James's Internet is Down but Machine is Running

**Detection:** WS drops. Reconnection attempts fail.
**Response:**
- Same as F3 from James's perspective.
- Bono sees heartbeat timeout, alerts Uday: "James went offline" (could be internet or crash -- Bono can't distinguish).
- James queues file sync changes locally.
**Recovery:** Internet restored, WS reconnects, queued changes flush, Bono sends "James is BACK".

### F6: Watchdog Process Itself Crashes

**Detection:** OS-level -- NSSM or Scheduled Task detects the process is gone.
**Response:** NSSM restarts the watchdog. Watchdog on startup checks if Claude Code is running (via `tasklist`). If running, attach to monitoring. If not, start it.
**Key design:** Watchdog startup is idempotent. It never assumes clean state.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Evolution API (WhatsApp) | HTTP POST from Bono server | `POST /message/sendText/<instance>` with API key header. Already running on VPS via PM2. |
| Gmail API | Existing `@racingpoint/google` package | James uses MCP Gmail for fallback. Bono uses same package for alert emails. |
| Claude Code CLI | `child_process.spawn('claude', [...args])` | Watchdog spawns with `--continue` flag to resume last session. `stdio: 'inherit'` to preserve terminal. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Watchdog <--> Comms Client | In-process function calls | Same Node.js process. Watchdog owns the event loop, comms client is a module. |
| Comms Client <--> Comms Server | WebSocket (wss://) | JSON messages over TLS. Single persistent connection. |
| Comms Server <--> Alert Dispatcher | In-process function calls | Same Node.js process on Bono's VPS. |
| File Watcher <--> Comms Client | EventEmitter | Watcher emits 'change', comms client sends `file_sync`. |

## Build Order (Dependencies Between Components)

The build order is driven by testability -- each phase should be independently verifiable.

```
Phase 1: Protocol + Comms (foundation)
  ├── shared/protocol.js      (message format, types, validation)
  ├── bono/comms-server.js     (WS server with auth)
  └── james/comms-client.js    (WS client with reconnect)
  TEST: James connects to Bono, sends message, Bono receives.

Phase 2: Heartbeat (liveness detection)
  ├── james/heartbeat.js       (send heartbeats on interval)
  └── bono/heartbeat-monitor.js (timeout detection)
  TEST: James sends heartbeats. Stop James. Bono detects timeout.

Phase 3: Alerts (notification delivery)
  └── bono/alert-dispatcher.js (WhatsApp + email)
  TEST: Heartbeat timeout triggers WhatsApp message to Uday.

Phase 4: Watchdog (process supervision)
  └── james/watchdog.js        (spawn, monitor, restart Claude Code)
  TEST: Watchdog starts Claude Code. Kill Claude Code. Watchdog restarts it.

Phase 5: File Sync (LOGBOOK.md)
  ├── james/file-sync.js       (watch + send)
  └── bono/file-receiver.js    (receive + write)
  TEST: Edit LOGBOOK.md on James side. Bono side gets update.

Phase 6: Email Fallback + Hardening
  ├── james/email-fallback.js  (send alerts via email when WS is down)
  └── NSSM/Scheduled Task setup (watchdog auto-start on boot)
  TEST: Kill WS server. James sends status email. Boot machine. Watchdog starts automatically.
```

**Why this order:**
- Protocol first because everything else depends on message format.
- Heartbeat before alerts because alerts are triggered BY heartbeat timeout.
- Watchdog after comms because we want to report watchdog events over the comms link.
- File sync last because it is the most independent feature -- it just uses the existing WS connection.
- Email fallback last because it is the safety net for everything else.

## Scaling Considerations

This system has exactly 2 endpoints. It will never scale beyond that. Scaling considerations are therefore about **reliability**, not throughput.

| Concern | Current (2 nodes) | Notes |
|---------|-------------------|-------|
| WebSocket connections | 1 persistent | No load balancer needed |
| Messages/second | ~0.07 (one heartbeat/15s) | Negligible |
| LOGBOOK.md size | <50KB | Full replace is fine up to ~1MB |
| Alert frequency | ~0/day (ideally) | Rate-limit to max 1 alert per 5 minutes to avoid spam |

### Reliability Priority

1. **Watchdog must never die.** NSSM as outer supervisor. Minimal dependencies. No network calls in the critical path.
2. **Reconnection must be automatic.** Exponential backoff handles all transient failures.
3. **Email fallback must work independently.** James can send email without the VPS.

## Anti-Patterns

### Anti-Pattern 1: Watchdog Inside Claude Code

**What people do:** Run the monitoring logic as part of Claude Code itself (e.g., a hook or plugin).
**Why it's wrong:** When Claude Code crashes, the monitor dies too. The whole point of a watchdog is to outlive the thing it watches.
**Do this instead:** Standalone Node.js process that spawns Claude Code as a child.

### Anti-Pattern 2: Polling Process Status via tasklist

**What people do:** Run `tasklist | findstr claude` on a timer to check if Claude Code is alive.
**Why it's wrong:** Race conditions (process could die between polls), wasteful CPU, delayed detection (up to one poll interval).
**Do this instead:** Use `child_process.spawn` and listen for the `'exit'` event. Instant detection, zero polling overhead. Only use `tasklist` for zombie cleanup on startup.

### Anti-Pattern 3: Bidirectional Heartbeat

**What people do:** Both sides send heartbeats to each other.
**Why it's wrong:** Adds complexity for no benefit. If James can't reach Bono, Bono's heartbeat won't arrive either. The WebSocket connection state already tells James whether Bono is reachable.
**Do this instead:** One-way heartbeat: James sends, Bono monitors. James detects Bono-down via WebSocket close/error events.

### Anti-Pattern 4: Delta Sync for Small Files

**What people do:** Implement rsync-style delta sync for LOGBOOK.md.
**Why it's wrong:** LOGBOOK.md is <50KB. Delta sync adds version tracking, merge conflict handling, corruption detection -- enormous complexity for saving ~40KB per sync (~20 syncs/day = 800KB saved, meaningless).
**Do this instead:** Full file replace. Idempotent. No merge conflicts. No version tracking. No corruption risk.

### Anti-Pattern 5: Socket.IO Instead of Raw ws

**What people do:** Use Socket.IO for "easy" reconnection and room management.
**Why it's wrong:** Socket.IO adds ~100KB of client/server code, has its own protocol on top of WebSocket, requires both sides to use Socket.IO, and the reconnection logic it provides is trivially implementable with raw `ws` in ~20 lines. The room/namespace features are useless for a 2-node system.
**Do this instead:** Use the `ws` library directly. It is 0 dependencies, battle-tested, and gives full control over the protocol.

### Anti-Pattern 6: Storing PSK in Code or Config Files

**What people do:** Hardcode the pre-shared key or put it in `config.json`.
**Why it's wrong:** Gets committed to git, visible in logs, shared with anyone who has repo access.
**Do this instead:** Environment variable (`COMMS_PSK`) loaded from `.env` file (which is in `.gitignore`).

## Technology Choices

| Component | Technology | Version | Rationale |
|-----------|-----------|---------|-----------|
| WebSocket | `ws` | 8.x | Zero dependencies, fastest Node.js WS implementation, supports ping/pong natively |
| File watching | `chokidar` | 4.x | Reliable cross-platform file watching, fs.watch-based (no polling) |
| Process spawn | Node.js `child_process` | built-in | No external dependency needed for process management |
| Process supervision | NSSM | 2.24+ | Runs the watchdog as a Windows service, auto-restarts on crash |
| VPS process mgmt | PM2 | 5.x | Already used on Bono's VPS for WhatsApp bot, proven |
| Hashing | Node.js `crypto` | built-in | SHA-256 for file integrity verification |
| Runtime | Node.js | 20+ LTS | Both sides already have Node.js. No additional runtime needed. |

## Sources

- [ws library (npm)](https://www.npmjs.com/package/ws) -- WebSocket client/server for Node.js
- [ws GitHub](https://github.com/websockets/ws) -- API docs, authentication during upgrade example
- [respawn (npm)](https://www.npmjs.com/package/respawn) -- Process monitor reference (pattern only, not using directly)
- [chokidar (npm)](https://www.npmjs.com/package/chokidar) -- File watching library
- [WebSocket Reconnection Strategies](https://dev.to/hexshift/robust-websocket-reconnection-strategies-in-javascript-with-exponential-backoff-40n1) -- Exponential backoff patterns
- [WebSocket Heartbeat Ping-Pong](https://oneuptime.com/blog/post/2026-01-27-websocket-heartbeat-ping-pong/view) -- Heartbeat interval best practices
- [Claude Code CLI Reference](https://code.claude.com/docs/en/cli-reference) -- CLI flags for session management
- [PM2 Documentation](https://pm2.keymetrics.io/docs/usage/quick-start/) -- VPS process management
- [Evolution API](https://github.com/EvolutionAPI/evolution-api) -- WhatsApp integration API
- [NSSM](https://nssm.cc/) -- Windows service wrapper for the watchdog process
- [Servy vs NSSM vs WinSW](https://dev.to/aelassas/servy-vs-nssm-vs-winsw-2k46) -- Windows service manager comparison

---
*Architecture research for: AI-to-AI Communication Link with Process Supervision*
*Researched: 2026-03-12*
