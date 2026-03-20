# Architecture Patterns

**Domain:** AI-to-AI communication system (comms-link v2.0)
**Researched:** 2026-03-20
**Confidence:** HIGH (based on direct codebase analysis of all v1.0 source files)

## Current Architecture (v1.0 Baseline)

```
                        INTERNET
                           |
    James (192.168.31.27)  |  Bono (VPS 72.60.101.58)
    =====================  |  ==========================
                           |
    start-comms-link.bat   |  ecosystem.config.cjs (pm2)
      |                    |    |
      +-- james/index.js   |    +-- bono/index.js
      |     CommsClient ---|-->|     CommsServer (:8765)
      |     HeartbeatSender |  |     HeartbeatMonitor
      |     HTTP relay :8766|  |     AlertManager
      |     INBOX.md writer |  |     HealthAccumulator
      |                    |  |     DailySummaryScheduler
      +-- ping-heartbeat.js|  |     LogbookWatcher
            5-min ping     |  |
            daemon watchdog|  |
                           |
    shared/                |
      protocol.js (25 types, envelope {v,type,from,ts,id,payload})
      state.js (DISCONNECTED/RECONNECTING/CONNECTED FSM)
      logbook-merge.js (3-way merge)
      http-post.js (HTTP client helper)
```

### Key Properties of v1.0
- **Single persistent WebSocket** from James to Bono (NAT traversal via outbound)
- **PSK auth** via Bearer header (timing-safe comparison)
- **Protocol v1 envelope:** `{v:1, type, from, ts, id, payload}` -- `id` is UUID per message
- **Offline queue** in CommsClient (100 messages max, drops oldest, replays on reconnect)
- **No delivery confirmation** -- fire-and-forget except for file_sync/file_ack (logbook only)
- **INBOX.md** as message sink -- appendFileSync, no locking, races with git
- **Ping heartbeat** is a SEPARATE process that opens ephemeral WS connections (not the daemon's)
- **3 processes on James:** daemon (james/index.js), ping-heartbeat.js, Claude Code watchdog (started via watchdog-runner.js)
- **1 process on Bono:** bono/index.js (managed by pm2)
- **22 test files** with 222 tests using node:test

### Existing Message Types (25)
| Type | Direction | Has Response? |
|------|-----------|--------------|
| echo / echo_reply | bidir | yes (echo_reply) |
| heartbeat / heartbeat_ack | J->B | no (monitor tracks timeout) |
| status / recovery | J->B | no |
| file_sync / file_ack | bidir | yes (file_ack) |
| message | bidir | no |
| task_request / task_response | bidir | yes |
| status_query / status_response | bidir | yes |
| daily_report | J->B | no |
| sync_push | bidir | no |
| sync_pull | future | n/a |
| sync_action / sync_action_ack | bidir | yes |

### Existing Patterns Worth Preserving
1. **DI everywhere** -- ClaudeWatchdog, HeartbeatSender, HealthAccumulator all accept injectable functions for testability
2. **EventEmitter** for cross-component communication -- ConnectionStateMachine, HeartbeatMonitor, ClaudeWatchdog, AlertManager
3. **Object.freeze enums** -- MessageType, State, ExecState
4. **wireBono() wiring function** -- testable composition of components
5. **HTTP relay** for local service integration (rc-core to WS bridge)

---

## Recommended Architecture (v2.0)

### Component Map -- New vs Modified

```
    James (192.168.31.27)           Bono (VPS)
    =====================           ==========

    NEW: process-supervisor.js      (no new files needed)
      replaces ping-heartbeat.js
      manages daemon lifecycle
      mid-session crash recovery

    MODIFIED: james/index.js        MODIFIED: bono/index.js
      CommsClient (unchanged)         wireBono (add ACK + exec + metrics handlers)
      HeartbeatSender (unchanged)
      HTTP relay (add queue + exec routes)
      + wire AckTracker              MODIFIED: bono/comms-server.js
      + wire ExecHandler               (add /metrics endpoint)
      + wire MetricsCollector
      + wire MessageQueue

    NEW shared modules:             Consumed by both sides:
      shared/ack-tracker.js           ACK tracking + retry
      shared/message-queue.js         Durable file-based queue
      shared/exec-protocol.js         Command types + approval logic
      shared/metrics-schema.js        Metric types + export format

    NEW james modules:
      james/exec-handler.js           Command execution + approval

    MODIFIED: shared/protocol.js      (add ~8 new message types)
    UNCHANGED: shared/state.js        (FSM is fine as-is)
    UNCHANGED: shared/logbook-merge.js
    UNCHANGED: shared/http-post.js
```

### New Components (6 files)

#### 1. `shared/ack-tracker.js` -- Message ACK Protocol

**Responsibility:** Track sent messages awaiting ACK, handle timeouts, retries.

```javascript
// Core API shape
export class AckTracker extends EventEmitter {
  constructor({ timeoutMs = 10000, maxRetries = 3, sendFn, nowFn })
  track(messageId, rawMessage)       // Register message for ACK tracking
  acknowledge(messageId)             // Mark as delivered, clear timers
  isAcked(messageId)                 // Check delivery status
  get pendingCount()                 // How many awaiting ACK
  // Events: 'retry' (messageId, rawMessage, attempt), 'timeout' (messageId)
  // Internal: per-message timer fires retry via sendFn, then emits 'timeout'
}
```

**Integration points:**
- James: `james/index.js` wraps `client.send()` to auto-track ACK-requiring message types
- Bono: `wireBono()` auto-sends `msg_ack` for tracked types on receipt
- Both sides instantiate their own AckTracker for messages they originate

**Design decision:** ACK at the application layer, NOT the WebSocket layer. WebSocket has TCP guarantees, but we need to confirm the *receiving application* processed the message (not just that the OS TCP stack accepted it). This matters because the daemon could crash between TCP receipt and queue write.

**Design decision:** Only certain message types require ACK (exec_request, task_request, exec_result). Heartbeats and metrics do NOT need ACK -- they are ephemeral and replaced by the next one. This keeps traffic low.

#### 2. `shared/message-queue.js` -- Transactional Message Queue

**Responsibility:** Replace INBOX.md append-only with durable, ACK-gated queue.

```javascript
export class MessageQueue {
  constructor({ storePath, maxSize = 1000 })
  enqueue(message)                   // Add to queue, persist to disk
  dequeue()                          // Get next unprocessed, mark in-flight
  acknowledge(messageId)             // Remove from queue (processed)
  nack(messageId)                    // Return to queue head (retry)
  peek(n)                            // View next N without removing
  get size()                         // Queue depth
  get inFlightCount()                // Messages dequeued but not ACK'd
  // Persistence: JSON file with atomic write (write tmp, rename)
}
```

**Integration points:**
- Replaces `appendFileSync(inboxPath, ...)` in james/index.js (lines 131-136)
- Replaces `appendFile(inboxPath, ...)` in bono/index.js (lines 52-55)
- HTTP relay gains `/relay/queue/peek`, `/relay/queue/ack` endpoints (so Claude Code sessions can consume)
- INBOX.md remains as human-readable audit log (write-only, never read programmatically)

**Design decision:** File-based persistence (JSON), not SQLite. Rationale: zero native dependencies (no compilation needed), the queue is small (hundreds not millions), and the existing codebase is pure JS + ws. Atomic write pattern: write to `.queue.tmp`, then rename to `.queue.json` -- avoids corruption on crash.

#### 3. `shared/exec-protocol.js` -- Remote Command Execution Protocol

**Responsibility:** Define command request/response/approval lifecycle.

```javascript
export const ExecState = Object.freeze({
  PENDING_APPROVAL: 'pending_approval',
  APPROVED: 'approved',
  REJECTED: 'rejected',
  RUNNING: 'running',
  COMPLETED: 'completed',
  FAILED: 'failed',
  TIMED_OUT: 'timed_out',
});

export const ApprovalMode = Object.freeze({
  AUTO: 'auto',                // Pre-approved read-only commands
  REQUIRE_ACK: 'require_ack', // Needs explicit approval from Claude Code session
});

// Allowlist -- auto-approved safe patterns
export const AUTO_APPROVE_PATTERNS = [
  /^git\s+(status|log|diff|show|branch)/,
  /^tasklist/,
  /^systeminfo/,
  /^node\s+--version/,
  /^type\s+/,         // read file contents (Windows)
  /^dir\s+/,          // list directory (Windows)
];

export function isAutoApproved(command) { ... }
export function createExecRequest(command, options = {}) { ... }
```

**Design decision:** Allowlist-based auto-approval, not blocklist. Unknown commands default to `REQUIRE_ACK`. This is safer -- a new dangerous command cannot slip through an incomplete blocklist.

**Security note:** Command execution MUST use `execFile` (array-based arguments), NEVER shell-based execution. This prevents command injection. The existing codebase already follows this pattern in watchdog.js and system-metrics.js.

#### 4. `shared/metrics-schema.js` -- Metrics Export Schema

**Responsibility:** Define metric types, collection intervals, export format.

```javascript
export function createMetricsSnapshot({
  uptimeSeconds,        // process uptime
  reconnectCount,       // WS reconnections since start
  messagesSent,         // total messages sent
  messagesReceived,     // total messages received
  ackPendingCount,      // messages awaiting ACK
  queueDepth,           // message queue size
  avgLatencyMs,         // average ACK round-trip
  wsState,              // CONNECTED/DISCONNECTED/RECONNECTING
  podSnapshots,         // optional: pod health from rc-core
}) { ... }

export function formatPrometheus(snapshot, prefix = 'comms_link') { ... }
```

**Integration points:**
- James: collect and send via `metrics_push` on 60s interval
- Bono: HealthAccumulator already tracks reconnects/uptime -- extend it to consume the new metrics
- Bono: new `GET /metrics` endpoint on comms-server for Prometheus scraping

#### 5. `james/exec-handler.js` -- Command Executor

**Responsibility:** Receive exec requests, check approval, run commands, return results.

```javascript
export class ExecHandler extends EventEmitter {
  constructor({ client, timeoutMs = 30000, execFileFn })
  handleExecRequest(msg)             // Entry from message router
  approveCommand(execId)             // Claude Code approves pending command
  rejectCommand(execId, reason)      // Claude Code rejects
  get pendingApprovals()             // Commands awaiting approval
  // Internal: uses execFile (NOT shell) with timeout, captures stdout/stderr,
  //           sends exec_result via client.send()
  // Events: 'pending_approval' (execId, command)
  //         'exec_started' (execId, command)
  //         'exec_completed' (execId, result)
}
```

**Integration points:**
- Wired into james/index.js message router for `exec_request` messages
- HTTP relay gains `/relay/exec/pending` and `/relay/exec/approve/:id` endpoints
- Claude Code sessions poll `/relay/exec/pending` or get notified via queue

#### 6. `james/process-supervisor.js` -- Process Supervisor

**Responsibility:** Manage daemon lifecycle with mid-session crash recovery. Replaces ping-heartbeat.js entirely.

```
// Supervises: james/index.js (daemon)
// The Claude Code watchdog remains as a subsystem within the daemon
//
// Lifecycle:
//   1. Supervisor starts (via HKCU Run key)
//   2. Spawns daemon as child process
//   3. Monitors via HTTP /relay/health (every 15s)
//   4. On failure: kill stale processes, respawn (EscalatingCooldown)
//   5. On success: reset cooldown
//   6. Writes supervisor.pid for external monitoring
//
// Key improvements over ping-heartbeat.js:
//   - No wmic usage (replaced with tasklist /FO CSV)
//   - No ephemeral WS connections (monitors via HTTP only)
//   - Reuses EscalatingCooldown from watchdog.js
//   - Handles mid-session daemon crashes (not just boot-time startup)
```

**Integration points:**
- Replaces `ping-heartbeat.js` entirely
- Imports and reuses `EscalatingCooldown` from `james/watchdog.js`
- start-comms-link.bat simplified: spawns supervisor only (supervisor spawns daemon)
- Supervisor does NOT spawn Claude Code watchdog separately -- the watchdog runs inside the daemon process

### Modified Components (4 files)

#### 1. `shared/protocol.js` -- Add ~8 New Message Types

```javascript
// Additions to MessageType:
msg_ack: 'msg_ack',                 // Generic delivery acknowledgment
exec_request: 'exec_request',       // Command execution request
exec_approval: 'exec_approval',     // Approval/rejection of pending command
exec_result: 'exec_result',         // Command output (stdout, stderr, exitCode)
health_snapshot: 'health_snapshot', // Rich pod/deployment state snapshot
metrics_push: 'metrics_push',       // Periodic metrics bundle
process_status: 'process_status',   // Supervisor reports process states
supervisor_cmd: 'supervisor_cmd',   // Remote restart/stop commands to supervisor
```

**Protocol version:** Keep `v: 1`. The envelope format is unchanged -- only new type strings are added. Both sides already ignore unknown message types (silently dropped in parseMessage consumers). Bumping to v2 would break during rolling deploy for no benefit. Only bump version if the envelope structure changes.

#### 2. `james/index.js` -- Add Message Routing for New Types

**Modifications (additive, not breaking):**
- Replace `appendFileSync(inboxPath, ...)` with `messageQueue.enqueue(msg)` (lines 131-136, 146-151)
- Add `msg_ack` handler: pass to `ackTracker.acknowledge(msg.payload.ackId)`
- Add `exec_request` handler: delegate to `execHandler.handleExecRequest(msg)`
- Add `metrics_push` sending on 60s setInterval (collect from system-metrics.js + queue + ack stats)
- Add `health_snapshot` sending: extend heartbeat payload OR separate 5-min interval with pod data from rc-core
- Add HTTP relay routes:
  - `GET /relay/queue/peek?n=10` -- peek at queue for Claude Code
  - `POST /relay/queue/ack` -- acknowledge processed message
  - `GET /relay/exec/pending` -- list commands awaiting approval
  - `POST /relay/exec/approve/:id` -- approve pending command
  - `POST /relay/exec/reject/:id` -- reject pending command

#### 3. `bono/index.js` -- Add ACK + Exec + Metrics Handlers to wireBono

**Modifications to `wireBono()` function:**
- Add `msg_ack` handling: pass to bono-side `ackTracker.acknowledge()`
- Auto-send `msg_ack` for received exec_request, task_request, exec_result
- Add `exec_request` handler (bono-side execution for James-initiated commands)
- Add `exec_result` handler (log/store results from James-executed commands)
- Add `health_snapshot` handler (store latest snapshot for status_response enrichment)
- Add `metrics_push` handler (feed into HealthAccumulator or new MetricsStore)

#### 4. `bono/comms-server.js` -- Add Metrics HTTP Endpoint

**Modifications:**
- Add `GET /metrics` endpoint returning Prometheus text format
- Enhance `GET /relay/health` to include queue depth, last ACK time, metrics summary

---

## Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| shared/protocol.js | Message types, envelope create/parse | All components |
| shared/ack-tracker.js | Track ACKs, timeouts, retries | CommsClient, wireBono |
| shared/message-queue.js | Durable message storage with enqueue/dequeue/ACK | james/index.js, bono/index.js |
| shared/exec-protocol.js | Exec types, approval allowlist | ExecHandler (both sides) |
| shared/metrics-schema.js | Metric types, Prometheus export | MetricsCollector, HealthAccumulator |
| james/process-supervisor.js | Daemon lifecycle, mid-session recovery | Spawns james/index.js |
| james/exec-handler.js | Run commands safely with approval gate | james/index.js message router |
| james/index.js | James daemon (WS client + HTTP relay) | Bono via WS, rc-core via HTTP |
| bono/index.js | Bono server wiring + message routing | James via WS, AlertManager, HealthAccumulator |
| bono/comms-server.js | WS server + HTTP endpoints | Bono wiring, external Prometheus scrapers |

---

## Data Flows -- New Features

### ACK Protocol Flow

```
James                              Bono
  |-- exec_request {id:abc} -------->|
  |                                  |-- msg_ack {ackId:abc}
  |<-- msg_ack {ackId:abc} ---------|
  |   AckTracker.acknowledge(abc)    |
  |                                  |
  |   [if no ACK within 10s]        |
  |-- exec_request {id:abc} -------->|  (AckTracker retry #1, same id)
  |                                  |  (Bono deduplicates by id)
  |   [if 3 retries fail]           |
  |   AckTracker emits 'timeout'     |
  |   -> enqueue for later delivery  |
```

**Key detail:** The msg_ack payload includes `{ackId: original_message_id}`. This reuses the existing UUID `id` field from the envelope. No new ID system needed.

### Remote Execution Flow

```
Bono                               James
  |-- exec_request {               -->|
  |     cmd: "git status",            |
  |     args: ["status"],             |
  |     mode: "auto",                 |
  |     execId: "ex_123"              |
  |   }                               |
  |                                    |
  |<-- msg_ack {ackId: msg_id} --------|  (delivery confirmed)
  |                                    |
  |                                    |-- isAutoApproved("git status")?
  |                                    |   YES: run via execFile (no shell)
  |                                    |
  |<-- exec_result {                ---|
  |     execId: "ex_123",              |
  |     exitCode: 0,                   |
  |     stdout: "On branch main...",   |
  |     stderr: "",                    |
  |     durationMs: 145                |
  |   }                                |

--- OR if command needs approval ---

Bono                               James
  |-- exec_request {               -->|
  |     cmd: "npm",                    |
  |     args: ["install", "foo"],      |
  |     mode: "require_ack",          |
  |     execId: "ex_456"              |
  |   }                               |
  |                                    |
  |<-- msg_ack {ackId: msg_id} --------|
  |                                    |
  |                                    |-- NOT auto-approved
  |                                    |-- queue to pendingApprovals
  |                                    |-- emit 'pending_approval'
  |                                    |
  |   [Claude Code polls /relay/exec/pending]
  |   [Claude Code calls POST /relay/exec/approve/ex_456]
  |                                    |
  |                                    |-- run via execFile (no shell)
  |<-- exec_result {exitCode, ...} ----|
```

### Metrics Push Flow

```
James (every 60s)                  Bono
  |                                  |
  | collectMetrics() + {             |
  |   uptimeSeconds,                 |
  |   reconnectCount,                |
  |   messagesSent,                  |
  |   messagesReceived,              |
  |   ackPendingCount,               |
  |   queueDepth,                    |
  |   wsState,                       |
  |   podSnapshots (optional)        |
  | }                                |
  |                                  |
  |-- metrics_push {bundle} -------->|
  |                                  |-- merge into HealthAccumulator
  |                                  |-- expose via GET /metrics
  |                                  |   (Prometheus text format)
```

**Note:** metrics_push does NOT require ACK. If one metrics push is lost, the next one (60s later) replaces it. Ephemeral data does not need delivery guarantees.

### Health Snapshot Flow

```
James (every 5 min or on-demand)   Bono
  |                                  |
  | Gather from rc-core :8080:       |
  |   /api/v1/pods -> pod states     |
  |   /api/v1/billing -> active      |
  |                                  |
  |-- health_snapshot {              |
  |     pods: [{id,state,game,...}], |
  |     billing: {active_sessions},  |
  |     deployment: {version,uptime},|
  |     timestamp                    |
  |   }                              |
  |                                  |
  |                            ----->|-- store as lastSnapshot
  |                                  |-- enrich status_response
  |                                  |-- include in daily_report
```

### Process Supervisor Flow

```
process-supervisor.js (started by HKCU Run key)
  |
  +-- write supervisor.pid
  |
  +-- spawn james/index.js (daemon)
  |     |-- monitor via HTTP GET http://127.0.0.1:8766/relay/health
  |     |   every 15s
  |     |
  |     |-- health check OK?
  |     |   YES: reset EscalatingCooldown
  |     |   NO:  cooldown.ready()?
  |     |        YES: kill stale node processes via tasklist, respawn daemon
  |     |        NO:  wait for cooldown delay
  |     |
  |     |-- supervisor does NOT maintain its own WS connection
  |         monitors daemon via HTTP only
  |
  +-- self: HKCU Run key ensures restart on reboot
      supervisor itself has no external supervisor
      (acceptable: supervisor is tiny, unlikely to crash)
```

**Design decision:** The supervisor does NOT open its own WebSocket connection. It monitors the daemon via HTTP only. Rationale: if the supervisor had a WS connection, it would need its own auth, reconnect logic, and message handling -- essentially duplicating the daemon. The supervisor's only job is to keep the daemon alive.

---

## Patterns to Follow

### Pattern 1: Dependency Injection (Existing Pattern -- Mandatory)
**What:** All new classes accept injectable functions for external dependencies.
**When:** Every new class (AckTracker, MessageQueue, ExecHandler).
**Example from codebase:** `ClaudeWatchdog` accepts `detectFn`, `killFn`, `spawnFn`, `findExeFn`, `cooldown`. `HeartbeatSender` accepts `collectFn`. `HealthAccumulator` accepts `nowFn`. Follow this pattern exactly.
**Why:** The existing test suite has 222 tests that work because of DI. Breaking this pattern means untestable code.

### Pattern 2: EventEmitter for Cross-Component Communication (Existing Pattern)
**What:** Components emit named events, wiring functions connect them.
**When:** Any component that needs to notify others of state changes.
**Example from codebase:** `ConnectionStateMachine` emits `'state'`, `HeartbeatMonitor` emits `'james_down'`/`'james_up'`, `ClaudeWatchdog` emits `'crash_detected'`/`'restart_success'`.
**Why:** Consistent, testable, decoupled. wireBono() and wireLogbook() demonstrate the wiring pattern.

### Pattern 3: Protocol Envelope Extension (Not Replacement)
**What:** New message types use the same `{v, type, from, ts, id, payload}` envelope.
**When:** Adding any new feature that communicates over WebSocket.
**Why:** All parsing, validation, and routing code works unchanged. No version bump needed. Unknown types are silently ignored by existing handlers.

### Pattern 4: HTTP Relay for Local Integration (Existing Pattern)
**What:** Local HTTP server bridges WebSocket to venue services (rc-core, Claude Code sessions).
**When:** Any feature that needs local service integration.
**Example from codebase:** james/index.js :8766 has `/relay/sync`, `/relay/action`, `/relay/health`. Extend with new routes for queue and exec.

### Pattern 5: Atomic File Operations (Existing Pattern)
**What:** Write to temp file, rename to target -- never write directly to the target.
**Example from codebase:** `atomicWrite()` in `james/logbook-watcher.js` uses write + rename pattern for LOGBOOK.md.
**Apply to:** MessageQueue persistence. Write `.queue.tmp`, rename to `.queue.json`.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Separate WebSocket Connections per Feature
**What:** Opening new WS connections for exec or metrics (like ping-heartbeat.js does for pings).
**Why bad:** Multiplies auth, reconnect, and state management complexity. ping-heartbeat.js opening ephemeral WS per 5-min cycle is a v1.0 design smell that the supervisor must not repeat.
**Instead:** All features multiplex over the single persistent daemon WebSocket. The supervisor monitors via HTTP, not WS.

### Anti-Pattern 2: File-Based IPC for Structured Data
**What:** Using INBOX.md-style file appends for message passing.
**Why bad:** No atomicity, races with git, no ACK, no structure, hard to parse back. Current codebase has `appendFileSync(inboxPath, entry)` which is fragile.
**Instead:** MessageQueue with proper enqueue/dequeue/ACK semantics. Keep INBOX.md only as a human-readable audit log (append-only, never read programmatically by the system).

### Anti-Pattern 3: wmic for Process Detection
**What:** Using `wmic process where "name='node.exe'"` for process discovery.
**Where in codebase:** ping-heartbeat.js line 70-72.
**Why bad:** Deprecated in Windows 11, produces deprecation warnings, may be removed.
**Instead:** Use `tasklist /FI "IMAGENAME eq node.exe" /FO CSV` (already used elsewhere in the same file, line 61). The supervisor must use tasklist exclusively.

### Anti-Pattern 4: Blocking execFileSync in Event Loop
**What:** Using synchronous child process execution in the main daemon process.
**Where in codebase:** ping-heartbeat.js lines 61, 70-72 use synchronous variants.
**Why bad:** Blocks all WebSocket message handling during execution. A 5-second tasklist timeout means 5 seconds of message processing freeze.
**Instead:** Use async child process execution with Promise wrapper + timeout. The exec-handler MUST be fully async. The supervisor (which has no WS event loop) can use sync calls since it has nothing to block.

### Anti-Pattern 5: ACK Everything
**What:** Requiring delivery confirmation for every message type.
**Why bad:** Doubles message traffic. Heartbeats, metrics, and status updates are ephemeral -- the next one supersedes the last. ACK-ing them adds latency and complexity for zero benefit.
**Instead:** Only ACK messages that have lasting side effects: exec_request, exec_result, task_request. Heartbeats and metrics are fire-and-forget.

### Anti-Pattern 6: Shell-Based Command Execution
**What:** Using shell-based process execution for remote commands.
**Why bad:** Enables command injection if input is not perfectly sanitized.
**Instead:** Always use `execFile` with explicit argument arrays. The exec-protocol should pass `{cmd: "git", args: ["status"]}`, never `{cmd: "git status"}` as a single string. This matches the existing pattern in watchdog.js and system-metrics.js.

---

## Suggested Build Order

Build order is driven by dependency chains and the principle that each phase must be independently testable and deployable.

### Phase 1: Protocol Foundation (ACK + Queue)
**Build:** `shared/ack-tracker.js`, `shared/message-queue.js`, protocol.js additions (new type strings)
**Why first:** These are shared infrastructure that every subsequent feature depends on. The ACK protocol is the backbone of reliable delivery. The message queue replaces the fragile INBOX.md pattern. Both are pure library code with no side effects -- easy to test in isolation.
**Test plan:** Unit tests for AckTracker (track/acknowledge/retry/timeout lifecycle) and MessageQueue (enqueue/dequeue/persistence/crash recovery/atomic writes). Target: 30+ tests.
**Deploy:** No behavioral change yet -- just new modules available for import.

### Phase 2: Process Supervisor
**Build:** `james/process-supervisor.js`, update start-comms-link.bat
**Why second:** Mid-session recovery is the highest-priority production gap (the 15-hour blind outage of Mar 17-18 was caused by lack of this). Independent of ACK/queue -- can ship standalone. The supervisor only needs HTTP health check, not the new protocol types.
**Test plan:** Unit tests with DI mocks for spawn/kill/detect. Integration test: start supervisor, kill daemon, verify restart within 15s. Verify EscalatingCooldown prevents restart storm.
**Deploy:** Replace ping-heartbeat.js in start-comms-link.bat. James-only change, no Bono coordination needed.

### Phase 3: Wire ACK + Queue into Daemon
**Build:** Modify james/index.js and bono/index.js to use AckTracker and MessageQueue. Add msg_ack auto-sending on Bono side.
**Why third:** Now that foundation (Phase 1) and supervision (Phase 2) exist, wire them in. This is the riskiest change -- touches both sides of the link simultaneously. Having the supervisor in place means we can recover from deploy mistakes.
**Test plan:** Integration tests: send message, verify ACK received, verify queue persistence across daemon restart. Test deduplication (same message ID sent twice). Target: 20+ tests.
**Deploy:** Requires coordinated deploy -- Bono must be updated to send msg_ack before James starts expecting them. Deploy Bono first (backward compatible: sending ACKs for messages that don't expect them is harmless). Then deploy James.

### Phase 4: Remote Execution
**Build:** `shared/exec-protocol.js`, `james/exec-handler.js`, bono-side exec routing in wireBono()
**Why fourth:** Depends on ACK protocol (exec_request must be reliably delivered). Approval flow depends on queue (pending approvals survive daemon restart).
**Test plan:** Unit tests for allowlist matching (isAutoApproved). Unit tests for ExecHandler lifecycle (pending -> approved -> running -> completed). Integration test: Bono sends safe command, James executes, result arrives. Test: dangerous command triggers approval gate. Test: timeout kills long-running command.
**Deploy:** James-side first (exec handler), then Bono-side (exec request sending). Safe: unknown message types are already ignored.

### Phase 5: Health Snapshots + Metrics
**Build:** `shared/metrics-schema.js`, james MetricsCollector (extend system-metrics.js), bono /metrics endpoint, health_snapshot sending
**Why last:** Observability is important but not critical path. Depends on reliable delivery (ACK) to ensure metrics actually arrive. No other feature depends on metrics.
**Test plan:** Verify metrics snapshot format. Verify Prometheus text output. Verify /metrics endpoint returns valid Prometheus format. Verify health_snapshot includes pod data from rc-core.
**Deploy:** James sends metrics (harmless if Bono doesn't handle yet), then Bono adds handler + endpoint.

### Phase Dependency Graph

```
Phase 1: shared/ack-tracker + shared/message-queue + protocol additions
    |                    |
    v                    v
Phase 2: supervisor   Phase 3: wire ACK + queue into daemon
    (independent)           |
                            v
                      Phase 4: remote execution
                            |
                            v
                      Phase 5: metrics + health snapshots
```

---

## Scalability Considerations

| Concern | Current (2 nodes) | At 5 nodes | Notes |
|---------|-------------------|------------|-------|
| WS connections | 1 persistent | 4 persistent | CommsServer already iterates wss.clients -- handles N clients |
| Message throughput | ~100/min | ~500/min | JSON parse is not the bottleneck at this scale |
| Queue depth | <100 messages | <500 | File-based JSON queue is fine. Consider SQLite at 10K+ |
| ACK tracking | <10 concurrent | <50 concurrent | In-memory Map with timers. No persistence needed |
| Metrics storage | In-memory | In-memory | HealthAccumulator resets daily. Prometheus handles long-term |
| Exec concurrency | 1 at a time | Serial per node | Sufficient -- operational commands, not batch processing |

---

## Sources

- Direct codebase analysis of all comms-link v1.0 source files (james/, bono/, shared/, ping-heartbeat.js)
- PROJECT.md v2.0 requirements and constraints
- v1.0 production incident: 15-hour blind outage (Mar 17-18, 2026)
- Existing test suite: 22 test files, 222 tests using node:test
- v1.0 architecture research from 2026-03-12 (previous ARCHITECTURE.md)

---
*Architecture research for comms-link v2.0 integration*
*Researched: 2026-03-20*
