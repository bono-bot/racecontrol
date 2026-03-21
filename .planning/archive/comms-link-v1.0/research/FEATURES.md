# Feature Research

**Domain:** AI-to-AI Communication Link & Process Watchdog
**Researched:** 2026-03-12
**Confidence:** HIGH (well-understood domain patterns from process supervision, WebSocket, and alerting ecosystems)

## Feature Landscape

### Table Stakes (System Fails Without These)

Features that are non-negotiable. Without any one of these, the system does not fulfill its core purpose: "James and Bono are always connected and always in sync."

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Heartbeat ping/pong** | Without heartbeat, neither side knows if the other is alive. Silent TCP death (NAT timeout, firewall drop) is the norm, not the exception. | LOW | 10-15s interval, 3 missed = dead. Application-level, not TCP keep-alive (TCP defaults to 2hr which is useless). |
| **WebSocket client (James -> Bono)** | James is behind NAT. Only outbound connections work. This is the entire transport layer. | MEDIUM | ws/wss library, must handle TLS. James initiates, Bono listens. Single persistent connection. |
| **Auto-reconnect with exponential backoff** | Connections WILL drop (ISP hiccups, VPS restarts, router reboots). Without auto-reconnect, a single drop kills the system permanently until manual intervention. | LOW | Start at 1s, cap at 30s, reset on successful connection. Standard pattern, well-documented. |
| **Watchdog process (standalone)** | Claude Code crashes. The thing watching it cannot BE it. Must be a separate process that survives Claude Code death. | MEDIUM | Windows service or startup script. Must monitor Claude Code's Node.js process. PID tracking + process polling. |
| **Zombie process cleanup before restart** | Claude Code leaves orphan node.exe processes. Starting a new instance without killing old ones causes EBUSY file lock errors and port conflicts. Known Windows issue (GitHub #11707). | MEDIUM | tasklist + taskkill for node.exe/claude processes. Must identify the RIGHT processes (not all node.exe). Process tree enumeration. |
| **WhatsApp alert: James down** | Uday needs to know immediately when James goes offline. He checks WhatsApp, not email. This is the whole point of the alerting system. | LOW | Bono detects missing heartbeat, calls Evolution API POST /message/sendText/{instance}. Bono already has this infrastructure (WhatsApp bot on PM2). |
| **WhatsApp alert: James back online** | Equally critical. "James is back" clears the mental alarm. Without recovery notification, Uday has to manually check. | LOW | James reconnects WebSocket, Bono sends "recovered" message. Pair with the down alert. |
| **Connection state tracking** | Both sides need to know: CONNECTED, DISCONNECTED, RECONNECTING. Without explicit state, every feature that depends on connection status is guessing. | LOW | Simple state machine. 3 states, clear transitions. Foundation for everything else. |
| **LOGBOOK.md push on change** | Both AIs must have current state. Stale LOGBOOK = wrong decisions. Current method (git pull) has minutes of delay. | MEDIUM | On commit hook or file watcher, serialize content over WebSocket. Receiver writes to disk. Last-writer-wins is fine for this use case (see anti-features). |
| **Email fallback for alerts** | When WebSocket is down, alerts still need to reach Uday. Email is slower but works independently of the comms link. Already implemented (james@/bono@ via Google Workspace). | LOW | Already have racingpoint-mcp-gmail. Fire-and-forget email when WebSocket is unavailable. |

### Differentiators (Valuable But Not Required for v1)

Features that improve reliability, observability, or operational quality. System works without them, but works better with them.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Message queuing during disconnect** | Messages sent while disconnected are lost without a queue. Short disconnects (< 5min) should not lose coordination messages. | MEDIUM | In-memory queue with max size (100 msgs) and max age (5min). Drain on reconnect. Don't persist to disk -- if the process dies, queued messages are stale anyway. |
| **Structured message protocol** | Without message types, everything is string parsing. Typed messages (heartbeat, logbook_sync, alert, command) enable clean routing and future extensibility. | LOW | JSON envelope: `{type, payload, timestamp, sender}`. Simple and sufficient. Avoid protobuf/msgpack complexity for this scale. |
| **Watchdog health self-reporting** | The watchdog itself could crash. If the watchdog is silent, something is very wrong. Having it report its own health gives a second layer of confidence. | LOW | Watchdog sends its own heartbeat to Bono at lower frequency (every 60s). Separate from Claude Code's heartbeat. |
| **Graceful shutdown signaling** | When James is intentionally stopped (maintenance, reboot), Uday should NOT get an alarm. Distinguishing planned vs. unplanned downtime prevents alert fatigue. | LOW | Send "shutting_down" message over WebSocket before exit. Bono suppresses alert for N minutes. |
| **Connection quality metrics** | Latency spikes and packet loss precede full disconnects. Tracking round-trip time on heartbeats gives early warning. | LOW | Measure heartbeat round-trip time. Log it. Alert if >2s sustained. No complex dashboard needed -- just a number. |
| **Startup self-test** | After restart, verify Claude Code is actually functional (not a zombie process that started but hung). Run a health check command and confirm output. | MEDIUM | Watchdog runs `claude --version` or similar and checks for expected output within timeout. Harder than it sounds -- Claude Code CLI startup can be slow. |
| **Alert deduplication / rate limiting** | If connection is flapping (up/down/up/down), Uday should not get 50 WhatsApp messages. Debounce alerts with cooldown period. | LOW | 5-minute cooldown after each alert type. Simple timestamp tracking. |
| **Reconnection status to Uday** | "James is trying to reconnect (attempt 3/10)" gives Uday visibility into whether the system is self-healing or actually dead. | LOW | Send progress update to Uday after N failed reconnect attempts (e.g., at attempt 5, 10, 20). |

### Anti-Features (Deliberately NOT Building)

Features that seem appealing but create disproportionate complexity or solve problems that don't actually exist in this system.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **CRDT / OT for LOGBOOK.md** | "What if both AIs edit LOGBOOK.md simultaneously?" | Two AI assistants don't type simultaneously. They take turns responding to human prompts. Concurrent edits are astronomically unlikely. CRDTs add massive complexity (Yjs, Automerge) for a problem that won't occur. | Last-writer-wins with timestamp. If a conflict somehow happens, the receiving side saves a `.conflict` backup and accepts the newer version. |
| **Bidirectional file sync engine** | "Sync all files, not just LOGBOOK.md" | This is not Dropbox. Syncing arbitrary files between Windows and Linux introduces path normalization, permission mismatches, binary file handling, and .gitignore-aware filtering. Massive scope creep. | Sync exactly one file (LOGBOOK.md). If more files are needed later, sync them explicitly by name. |
| **Web-based health dashboard** | "Build a status page showing system health" | Two users (Uday + the AIs). A web dashboard is over-engineering for an audience of one human. Uday checks WhatsApp, not a browser. | WhatsApp status messages. Bono can respond to "status?" with a text summary. |
| **Message persistence / database** | "Store all messages in SQLite for history" | The messages are coordination ephemera ("I'm restarting pod 3", "logbook updated"). They don't have long-term value. Adding a database adds migration, backup, and storage concerns for data nobody will query. | Log messages to a rotating text file. Grep when needed. |
| **End-to-end encryption** | "Encrypt the WebSocket traffic" | WSS (WebSocket over TLS) already encrypts in transit. The two endpoints are trusted machines we control. E2E encryption on top of TLS adds key management complexity for zero security benefit in this trust model. | Use WSS (TLS). Done. |
| **Multi-node / mesh topology** | "What if we add a third AI?" | There are exactly two AIs. Building for N nodes adds routing, topology discovery, consensus, and partition handling. If a third node is ever needed, refactor then. | Point-to-point WebSocket. One client, one server. |
| **Voice/video/screen sharing** | "It would be cool if the AIs could share screens" | Out of scope per PROJECT.md. Text coordination is the entire use case. Media streaming is a different product. | Text messages over WebSocket. |
| **Process auto-restart without kill** | "Just start a new Claude Code alongside the old one" | On Windows, Claude Code's file locks (EBUSY errors) and port bindings mean two instances will fight. The old one MUST die first. | Always kill-then-restart. Never start alongside. |
| **Complex alert routing rules** | "Route alerts differently based on time of day, severity, etc." | One recipient (Uday). Two alert types (down/up). Routing rules are overhead for a system this simple. | Send every alert to Uday via WhatsApp. Send every alert via email as backup. That's it. |

## Feature Dependencies

```
[WebSocket Client]
    |
    |--requires--> [Connection State Tracking]
    |                   |
    |                   |--enables--> [Auto-reconnect with Backoff]
    |                   |--enables--> [Heartbeat Ping/Pong]
    |                   |--enables--> [Message Queuing (v1.x)]
    |                   |--enables--> [Graceful Shutdown Signaling (v1.x)]
    |
    |--enables--> [LOGBOOK.md Sync]
    |--enables--> [Structured Message Protocol (v1.x)]

[Heartbeat Ping/Pong]
    |--enables--> [WhatsApp Alert: Down]
    |--enables--> [WhatsApp Alert: Back Online]
    |--enables--> [Connection Quality Metrics (v1.x)]

[WhatsApp Alert: Down]
    |--requires--> [Evolution API on Bono's VPS] (already exists)
    |--enhanced-by--> [Alert Deduplication (v1.x)]
    |--fallback--> [Email Fallback]

[Watchdog Process]
    |--requires--> [Zombie Process Cleanup]
    |--enables--> [Auto-restart Claude Code]
    |--enables--> [Startup Self-Test (v1.x)]
    |--enhanced-by--> [Watchdog Health Self-Reporting (v1.x)]

[Auto-restart Claude Code]
    |--triggers--> [WebSocket Client] (re-establish connection)
    |--triggers--> [WhatsApp Alert: Back Online]
```

### Dependency Notes

- **WebSocket Client requires Connection State Tracking:** Every other feature queries "are we connected?" -- state tracking is foundational.
- **Heartbeat enables WhatsApp Alerts:** Bono detects James is down via missed heartbeats, then fires alerts. Without heartbeat, there is no detection.
- **Watchdog requires Zombie Cleanup:** Starting Claude Code without killing the old one causes file lock errors on Windows. Kill-first is mandatory.
- **Auto-restart triggers WebSocket reconnection:** After Claude Code restarts, the comms link must re-establish. The watchdog should invoke the WebSocket client as part of the restart sequence.
- **Email Fallback is independent:** Email works even when WebSocket is completely dead. It depends only on the MCP Gmail server, which already exists.

## MVP Definition

### Launch With (v1)

The minimum set that delivers the core value: "James and Bono are always connected; if the link drops, Uday knows immediately."

- [ ] **WebSocket client (James -> Bono)** -- The transport. Everything rides on this.
- [ ] **Connection state tracking** -- CONNECTED / DISCONNECTED / RECONNECTING state machine.
- [ ] **Heartbeat ping/pong** -- 10s interval, 3 missed = dead declaration. Application-level.
- [ ] **Auto-reconnect with exponential backoff** -- 1s initial, 30s cap, infinite retries.
- [ ] **Watchdog process** -- Standalone Windows process that polls for Claude Code's node.exe PID.
- [ ] **Zombie process cleanup** -- Kill stale node.exe/claude processes before restart.
- [ ] **WhatsApp alert: James down** -- Bono calls Evolution API when heartbeat stops.
- [ ] **WhatsApp alert: James back online** -- Bono sends recovery message on reconnect.
- [ ] **LOGBOOK.md push on change** -- Send file content over WebSocket on commit/change.
- [ ] **Email fallback** -- When WebSocket is down, send alert via email as backup.

### Add After Validation (v1.x)

Features to add once v1 is running stable for a week.

- [ ] **Message queuing** -- Buffer messages during short disconnects (trigger: first time a coordination message is lost during a reconnect).
- [ ] **Structured message protocol** -- Move from string messages to typed JSON envelopes (trigger: when adding a third message type beyond heartbeat and logbook).
- [ ] **Alert deduplication** -- Cooldown after alerts (trigger: first time connection flapping spams Uday).
- [ ] **Graceful shutdown signaling** -- Suppress alerts during planned maintenance (trigger: first time Uday gets alarmed by an intentional restart).
- [ ] **Watchdog health self-reporting** -- Watchdog heartbeat to Bono (trigger: first time the watchdog itself dies silently).
- [ ] **Connection quality metrics** -- RTT tracking on heartbeats (trigger: when investigating intermittent disconnects).

### Future Consideration (v2+)

Features to defer until the system has proven its value.

- [ ] **Startup self-test** -- Verify Claude Code is functional post-restart, not just running (defer: need to understand Claude CLI startup behavior better first).
- [ ] **Reconnection progress to Uday** -- "Attempt 5/10" messages (defer: only useful if auto-recovery regularly fails, which we hope it won't).
- [ ] **Multi-file sync** -- Sync additional files beyond LOGBOOK.md (defer: no current need, add files by name if needed).

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| WebSocket client | HIGH | MEDIUM | P1 |
| Connection state tracking | HIGH | LOW | P1 |
| Heartbeat ping/pong | HIGH | LOW | P1 |
| Auto-reconnect w/ backoff | HIGH | LOW | P1 |
| Watchdog process | HIGH | MEDIUM | P1 |
| Zombie cleanup | HIGH | MEDIUM | P1 |
| WhatsApp alert: down | HIGH | LOW | P1 |
| WhatsApp alert: back | HIGH | LOW | P1 |
| LOGBOOK.md sync | HIGH | MEDIUM | P1 |
| Email fallback | MEDIUM | LOW | P1 |
| Message queuing | MEDIUM | MEDIUM | P2 |
| Structured protocol | MEDIUM | LOW | P2 |
| Alert deduplication | MEDIUM | LOW | P2 |
| Graceful shutdown | MEDIUM | LOW | P2 |
| Watchdog self-health | LOW | LOW | P2 |
| Connection metrics | LOW | LOW | P2 |
| Startup self-test | LOW | MEDIUM | P3 |
| Reconnect progress | LOW | LOW | P3 |

**Priority key:**
- P1: Must have for launch (10 features)
- P2: Should have, add when triggered by real need (6 features)
- P3: Nice to have, future consideration (2 features)

## Comparable Systems Analysis

| Feature | PM2 (Node.js) | systemd (Linux) | Supervisor (Python) | Our Approach |
|---------|---------------|-----------------|---------------------|--------------|
| Process monitoring | PID file + polling | cgroups + kernel notifications | PID file + polling | PID polling (Windows compatibility) |
| Restart strategy | Exponential backoff, memory-based, exit-code-based | on-failure, on-watchdog, always | autorestart with backoff | Unconditional restart with backoff (simple, our process should always run) |
| Zombie cleanup | Built-in (managed processes) | PID 1 reaps orphans | Built-in | Manual taskkill (Windows has no PID 1 reaping) |
| Health check | HTTP endpoint, custom script | WatchdogSec + sd_notify | HTTP/TCP/custom | Custom (claude --version or process check) |
| Logging | Built-in log rotation | journald | Built-in log rotation | Log to rotating file |
| Startup on boot | pm2 startup (Linux only) | systemd enable | supervisord service | HKLM Run key or Task Scheduler |
| Heartbeat | N/A (local only) | sd_notify WATCHDOG=1 | N/A (local only) | Custom WebSocket heartbeat (remote) |
| Alert notification | PM2+ (paid) | journal + alertmanager | supervisorctl events | Direct WhatsApp via Evolution API |

**Key insight from comparable systems:** Process supervision is a well-solved problem on Linux (systemd) but poorly served on Windows. PM2 works on Windows but is designed for Node.js apps, not arbitrary CLI tools. Our watchdog needs to be purpose-built for Claude Code on Windows -- there is no off-the-shelf solution that fits.

## Sources

- [PM2 Restart Strategies](https://pm2.keymetrics.io/docs/usage/restart-strategies/) -- exponential backoff, exit code handling
- [systemd Watchdog](http://0pointer.de/blog/projects/watchdog.html) -- WatchdogSec, sd_notify patterns
- [WebSocket Heartbeat Best Practices](https://oneuptime.com/blog/post/2026-01-27-websocket-heartbeat/view) -- application-level vs protocol-level
- [WebSocket Reconnection Logic](https://oneuptime.com/blog/post/2026-01-27-websocket-reconnection-logic/view) -- exponential backoff, state management
- [Socket.IO Offline Behavior](https://socket.io/docs/v3/client-offline-behavior/) -- message buffering during disconnect
- [Evolution API Documentation](https://doc.evolution-api.com/v2/api-reference/message-controller/send-text) -- WhatsApp message sending
- [Claude Code Windows Issues (GitHub #11707)](https://github.com/anthropics/claude-code/issues/11707) -- EBUSY, zombie node.exe processes
- [Ably WebSocket Architecture](https://ably.com/topic/websocket-architecture-best-practices) -- connection state, heartbeat, reconnection
- [CRDT File Sync (Tonsky)](https://tonsky.me/blog/crdt-filesync/) -- why CRDTs are overkill for simple file sync
- [Microsoft node-native-watchdog](https://github.com/microsoft/node-native-watchdog) -- event loop unresponsiveness detection

---
*Feature research for: AI-to-AI Communication Link & Process Watchdog*
*Researched: 2026-03-12*
