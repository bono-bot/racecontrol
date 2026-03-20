# Stack Research: comms-link v2.0

**Domain:** AI-to-AI communication infrastructure (process supervision, message queuing, remote execution, observability)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Current Baseline

v1.0 ships with exactly one dependency: `ws@^8.19.0`. Everything else uses Node.js stdlib (`node:crypto`, `node:events`, `node:child_process`, `node:os`, `node:http`, `node:fs`, `node:test`). Node.js version is 22.14.0 on James, PM2 on Bono's VPS. ESM throughout.

This minimal footprint is a strength. The v2.0 stack should add only what is genuinely needed.

## Recommended Stack Additions

### New Runtime Dependencies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| better-sqlite3 | ^12.8.0 | Transactional message queue + metrics store | Synchronous API eliminates callback complexity. WAL mode gives concurrent read/write. `.transaction()` wrapper auto-commits/rollbacks. 4,700+ dependents, actively maintained (12.8.0 released March 2026). Native binding compiles on both Windows and Linux. Replaces both INBOX.md file races and in-memory `#queue` array. |

**That is the only new runtime dependency.** Everything else is achievable with Node.js stdlib already in use.

### What Does NOT Need a Library

| Capability | Why No Library Needed | Implementation Approach |
|------------|----------------------|------------------------|
| Message ACK + sequence numbers | Pure protocol logic | Extend `shared/protocol.js`: add `seq` (monotonic counter) and `ack_seq` (last received from peer) to envelope. ACK messages reference `id` of original. All in-memory state. |
| Process supervision (watchdog-of-watchdog) | Windows Task Scheduler + `node:child_process` | `schtasks` via `execFile` creates a periodic task (every 5 min) that checks watchdog PID file staleness. If stale, kills zombie and restarts. Already proven pattern from rc-agent. |
| Remote command execution | `node:child_process.execFile` already in use | Add approval flow as protocol messages (`cmd_request` -> `cmd_approve`/`cmd_deny` -> `cmd_result`). Strict allowlist of commands. `execFile` only -- never shell-based execution. |
| Health snapshots | `node:os` + existing `system-metrics.js` | Extend `collectMetrics()` with pod status (HTTP to racecontrol :8080), disk space (`node:fs.statfsSync`), process list. |
| Metrics export | `node:http` server + JSON | Expose `/metrics` on port 8766 returning JSON counters. Bono scrapes periodically. One consumer does not justify Prometheus. |
| File locking (replace INBOX.md races) | SQLite replaces file I/O | INBOX.md append races vanish because SQLite handles concurrency via WAL mode journal. |
| `wmic` replacement | Already done in v1.0 | Codebase already uses `tasklist /FI` via `execFile`. Confirmed by code inspection -- no `wmic` calls present. |

### Dev Dependencies

No new dev dependencies. The existing `node --test test/*.test.js` pipeline is unchanged.

## Installation

```bash
# Single new dependency (both James and Bono sides)
npm install better-sqlite3@^12.8.0
```

### Build Requirements for better-sqlite3

- **Windows (James):** Needs C++ build tools for native addon. If Visual Studio Build Tools 2022 are installed, `npm install` just works. Verify with: `npm install better-sqlite3 --build-from-source`.
- **Linux (Bono VPS):** Needs `build-essential` and `python3`. Likely already present if PM2 and other native addons compile.

## Why better-sqlite3 for the Message Queue

### The Problem

v1.0 has two message storage mechanisms, both flawed:

1. **INBOX.md** -- `appendFileSync` races with git operations. No ACK. No ordering guarantees. No query capability.
2. **In-memory `#queue` array** in `CommsClient` -- Lost on process crash. No persistence across restarts.

### Why SQLite

- Handles tens of messages per minute, not thousands per second -- SQLite is ideal at this scale
- Both James and Bono are single-process Node.js -- no need for inter-process queue broker
- ACID transactions give crash recovery for free -- pending messages survive restart and replay
- WAL mode allows daemon writes while metrics endpoint reads concurrently
- Queryable history enables debugging ("show me all failed messages in last hour")
- Single `.db` file, no server process, zero configuration

### Message Queue Schema

```sql
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,              -- UUID from protocol envelope
  seq INTEGER NOT NULL,             -- Monotonic sequence number
  type TEXT NOT NULL,               -- Message type (from MessageType enum)
  direction TEXT NOT NULL,          -- 'outbound' | 'inbound'
  payload TEXT NOT NULL,            -- JSON serialized payload
  status TEXT NOT NULL DEFAULT 'pending',  -- pending|sent|acked|failed|expired
  created_at INTEGER NOT NULL,      -- Unix ms timestamp
  sent_at INTEGER,                  -- When WebSocket.send() completed
  acked_at INTEGER,                 -- When ACK received from peer
  retry_count INTEGER DEFAULT 0,
  expires_at INTEGER                -- TTL for automatic cleanup
);

CREATE TABLE IF NOT EXISTS metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,               -- e.g. 'ws_reconnect', 'msg_latency'
  value REAL NOT NULL,
  labels TEXT,                      -- JSON string for dimensions
  recorded_at INTEGER NOT NULL      -- Unix ms timestamp
);

CREATE INDEX idx_msg_status ON messages(status);
CREATE INDEX idx_msg_dir_status ON messages(direction, status);
CREATE INDEX idx_metrics_name_time ON metrics(name, recorded_at);
```

### Why better-sqlite3, Not node:sqlite

- `node:sqlite` is still experimental in Node 22.14.0 (confirmed: prints `ExperimentalWarning`, API may change)
- better-sqlite3 is battle-tested (12+ years, 4,700+ dependents, v12.8.0)
- Synchronous API matches single-threaded message processing -- no async ceremony for simple inserts
- `.transaction()` wrapper handles commit/rollback automatically
- When `node:sqlite` stabilizes (likely Node 24+), migration is trivial -- same SQL, same concepts

## Protocol Version Bump

v2.0 envelope extends the current format:

```javascript
{
  v: 2,                    // Bumped from 1
  type: 'task_request',
  from: 'james',
  ts: 1711000000000,
  id: 'uuid-v4',
  seq: 42,                 // NEW: per-session monotonic counter
  ack_seq: 41,             // NEW: last received seq from peer (piggybacked)
  payload: { ... }
}
```

`parseMessage()` in `shared/protocol.js` already validates `v` field -- bump to 2, add `seq`/`ack_seq` validation. Both sides must upgrade simultaneously (coordinate via existing comms-link before cutover, or accept brief downtime).

New message types to add to `MessageType`:

```javascript
// v2.0 additions
msg_ack: 'msg_ack',              // Explicit ACK for important messages
cmd_request: 'cmd_request',      // Remote command request
cmd_approve: 'cmd_approve',      // Approval for pending command
cmd_deny: 'cmd_deny',            // Denial for pending command
cmd_result: 'cmd_result',        // Command execution result
health_snapshot: 'health_snapshot', // Extended health data
```

## Metrics Export Design

JSON endpoint, not Prometheus. One consumer (Bono) does not justify prom-client overhead.

```
GET http://localhost:8766/metrics
```

```json
{
  "uptime_seconds": 86400,
  "ws_state": "CONNECTED",
  "ws_reconnects_total": 3,
  "ws_last_reconnect_at": 1711000000000,
  "messages_sent_total": 142,
  "messages_acked_total": 140,
  "messages_failed_total": 2,
  "messages_pending": 0,
  "avg_ack_latency_ms": 45,
  "heartbeat_rtt_ms": 23,
  "claude_running": true,
  "cpu_percent": 12.5,
  "memory_percent": 45.2,
  "queue_depth": 0,
  "last_heartbeat_at": 1711000000000
}
```

Implemented as ~15 lines of `node:http.createServer` -- no Express, no Fastify, no framework.

If Prometheus/Grafana integration is ever needed later, wrap the same counters with prom-client gauges. The data model is identical.

## Process Supervision Strategy

### Watchdog-of-Watchdog (No New Dependencies)

The v1.0 gap: if the watchdog process itself dies mid-session, nothing restarts it until next reboot.

**Solution:** Windows Task Scheduler periodic task, created via `schtasks` through `execFile`:

```
schtasks /Create /TN "CommsLinkSupervisor" /TR "node watchdog-runner.js" /SC MINUTE /MO 5 /F
```

- Runs every 5 minutes
- Checks PID file freshness (watchdog writes heartbeat timestamp to `watchdog.pid`)
- If PID file is stale (>10 min old) or process not running, kills zombie and restarts
- Task Scheduler is the OS-level supervisor -- it cannot crash

**Why not NSSM (from v1.0 research):** NSSM runs services in Session 0, which cannot show GUI. The v1.0 post-mortem (15-hour outage) specifically identified Session 0 as the problem. Task Scheduler with current user runs in the user's session. This is the correct approach for a process that may need to interact with GUI applications like Claude Code.

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| better-sqlite3 | node:sqlite (built-in) | Experimental in Node 22, API unstable, prints warnings. Revisit at Node 24+. |
| better-sqlite3 | LevelDB / RocksDB | Key-value stores. SQLite gives SQL queries for debugging/metrics/reporting. |
| better-sqlite3 | Redis | Requires separate server process. Overkill for 2-node system. |
| better-sqlite3 | JSON files | No transactions, no concurrent access, same INBOX.md problems. |
| JSON metrics endpoint | prom-client (Prometheus) | Excellent library (300k+ weekly downloads), but one consumer (Bono) does not justify it. Add when/if Grafana dashboard is needed. |
| Task Scheduler | NSSM service | Session 0 problem -- services cannot interact with GUI in user session. Learned from v1.0 15-hour outage. |
| Task Scheduler | PM2 on James | PM2 has poor Windows support (file watcher issues, service registration hacks). Already proven unreliable for Windows services. |
| Allowlisted execFile | node-pty / shell-based execution | Remote commands must NEVER use shell. `execFile` with allowlist prevents injection. node-pty is for interactive terminals. |

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Any message broker (RabbitMQ, NATS, BullMQ) | Massive overkill for 2-node, tens-of-messages-per-minute system | SQLite-backed queue |
| `appendFileSync` for INBOX.md | Race conditions with git, no ACK, no ordering | SQLite message table |
| Shell-based command execution | Command injection risk | `execFile` with allowlist and array args |
| Express/Fastify for metrics endpoint | One endpoint, one port, one consumer | `node:http.createServer` (~15 lines) |
| `node-cron` for supervisor scheduling | Unnecessary dependency | `schtasks` via `execFile` |
| NSSM for watchdog service | Session 0 cannot interact with user GUI | Task Scheduler with user session |
| `wmic` | Deprecated in Windows 11 | `tasklist /FI` via `execFile` (already done) |
| TypeScript | Build step overhead for infrastructure glue code | JavaScript + JSDoc (unchanged from v1.0) |
| dotenv | One config pattern, no benefit over JSON/env vars | `process.env` + ecosystem.config.cjs (unchanged) |

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| better-sqlite3@12.8.0 | Node.js >= 20 | James: 22.14.0, Bono VPS: verify >= 20 |
| better-sqlite3@12.8.0 | Windows 11 + Linux | Native addon via node-gyp, compiles on both |
| ws@8.19.0 | Node.js >= 18 | No change needed |
| better-sqlite3@12.8.0 | ws@8.19.0 | No interaction -- independent concerns |

## Integration Points

### Where better-sqlite3 Touches Existing Code

1. **`james/comms-client.js`** -- Replace `#queue` array with SQLite insert/select. `#flushQueue()` becomes `SELECT * FROM messages WHERE status='pending' AND direction='outbound' ORDER BY seq`.
2. **`bono/comms-server.js`** -- Mirror queue on Bono side for inbound tracking and ACK generation.
3. **`shared/protocol.js`** -- Add `seq`, `ack_seq` to `createMessage()`. Add new MessageType entries. Bump `PROTOCOL_VERSION` to 2.
4. **`james/heartbeat-sender.js`** -- Write heartbeat metrics to SQLite instead of (or in addition to) sending over wire.
5. **New: `shared/queue.js`** -- Shared SQLite queue abstraction used by both sides.
6. **New: `james/metrics-server.js`** -- HTTP endpoint reading from SQLite metrics table.
7. **New: `james/supervisor.js`** -- Watchdog-of-watchdog using Task Scheduler.

### What Does NOT Change

- `shared/state.js` -- ConnectionStateMachine is unchanged
- `james/system-metrics.js` -- Extended but not replaced
- `james/watchdog.js` -- Still the primary watchdog; supervisor watches it
- `bono/alert-manager.js` -- Unchanged, still sends WhatsApp/email
- `ecosystem.config.cjs` -- Bono PM2 config unchanged (add env vars for SQLite path)
- Test runner -- Still `node --test test/*.test.js`

## Sources

- [better-sqlite3 npm](https://www.npmjs.com/package/better-sqlite3) -- v12.8.0, published March 2026 (HIGH confidence)
- [better-sqlite3 GitHub](https://github.com/WiseLibs/better-sqlite3) -- API docs, .transaction() wrapper (HIGH confidence)
- [Node.js SQLite docs](https://nodejs.org/api/sqlite.html) -- confirmed experimental in Node 22 (HIGH confidence)
- [prom-client npm](https://www.npmjs.com/package/prom-client) -- evaluated, deferred (MEDIUM confidence)
- [Node.js child_process docs](https://nodejs.org/api/child_process.html) -- execFile security model (HIGH confidence)
- Local codebase inspection -- confirmed ws is sole dependency, wmic absent, tasklist in use (HIGH confidence)
- `node --version` on James: 22.14.0 (HIGH confidence)
- `node:sqlite` test on James: available but prints ExperimentalWarning (HIGH confidence)
- v1.0 NSSM/Session 0 learnings from PROJECT.md post-mortem (HIGH confidence)

---
*Stack research for: comms-link v2.0 -- process supervision, message queuing, remote execution, observability*
*Researched: 2026-03-20*
