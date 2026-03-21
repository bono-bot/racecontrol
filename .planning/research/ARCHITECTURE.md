# Architecture Research

**Domain:** Bidirectional AI-to-AI dynamic execution protocol — v18.0 Seamless Execution
**Researched:** 2026-03-22
**Confidence:** HIGH (direct codebase inspection of comms-link source, no speculation)

---

## Existing Architecture Baseline

Before describing integration points, here is the current comms-link structure as read from source.

### Transport and Connection Layer

```
James (Windows 11, .27)                         Bono (VPS, srv1422716.hstgr.cloud)
  james/comms-client.js                           bono/comms-server.js
    CommsClient (EventEmitter)                      createCommsServer({ port, psk })
    - WS outbound to Bono :8765                     - WS server on :8765
    - PSK Bearer auth                               - PSK timing-safe validation
    - exponential backoff reconnect                 - 45s ping keepalive
    - offline send queue (100 cap)                  - HTTP relay routes (/relay/sync, /relay/action, /relay/health)
    - sendRaw() for AckTracker retry
```

James connects outbound — this makes the architecture NAT-safe. All WS flows originate from James.

### Protocol Layer (shared/protocol.js)

Every message follows a standard envelope:

```
{ v: 1, type, from, ts, id (UUID), payload: {} }
```

Current registered message types: `echo`, `echo_reply`, `heartbeat`, `heartbeat_ack`, `msg_ack`,
`status`, `recovery`, `file_sync`, `file_ack`, `message`, `task_request`, `task_response`,
`status_query`, `status_response`, `daily_report`, `sync_push`, `sync_pull`, `sync_action`,
`sync_action_ack`, `exec_request`, `exec_result`, `exec_approval`.

Control messages (heartbeat, echo, msg_ack) skip ACK tracking. All others go through AckTracker.

### Exec Protocol Layer (shared/exec-protocol.js)

The static command registry (`COMMAND_REGISTRY`) contains 13 named commands with:
- `binary` + `args[]` — no shell strings ever
- `tier: AUTO | NOTIFY | APPROVE`
- `timeoutMs`, `cwd`

`ExecHandler` (james/exec-handler.js, also instantiated in bono/index.js as `bonoExecHandler`):
- Routes `exec_request` messages by tier
- `#pendingApprovals` Map with 10-min default-deny timeout
- Emits events: `exec_started`, `exec_completed`, `pending_approval`, `approval_timeout`
- Injectable: `execFileFn`, `sendResultFn`, `notifyFn`, `commandRegistry`

The `commandRegistry` parameter is already injectable in `ExecHandler` constructor — this is the primary extension point for dynamic registration.

### Reliability Layer

```
shared/ack-tracker.js   -- AckTracker: tracks in-flight messages, 3 retries × 10s timeout
                           DeduplicatorCache: 1000-entry LRU dedup on receiver side
shared/message-queue.js -- MessageQueue: WAL-backed durable queue, survives crash/restart
shared/connection-mode.js -- ConnectionMode: WS-primary / email-fallback degradation
```

### Current Message Routing (James side, james/index.js)

The central `client.on('message', handler)` dispatches by `msg.type`:

| Incoming type | Handler |
|---------------|---------|
| `msg_ack` | AckTracker.acknowledge |
| `sync_push` | forward to rcCoreUrl/sync/push |
| `sync_action` | forward to rcCoreUrl/sync/receive-action |
| `sync_action_ack` | forward to rcCoreUrl/actions/{id}/ack |
| `exec_request` | ExecHandler.handleExecRequest |
| `exec_approval` | ExecHandler.approve/rejectCommand |
| `task_request` | send task_response accepted + audit log |
| `task_response` | clear pendingTasks timer |
| `status_query` | send status_response (uptime) |
| `exec_result` | log + failoverOrchestrator.handleExecResult |
| `message` | audit log (INBOX.md) |

### Current Message Routing (Bono side, bono/index.js wireBono)

| Incoming type | Handler |
|---------------|---------|
| `msg_ack` | AckTracker.acknowledge |
| `heartbeat` | HeartbeatMonitor.receivedHeartbeat |
| `recovery` | AlertManager.handleRecovery |
| `exec_result` | log + wss.emit('exec_result', payload) |
| `exec_request` | bonoExecHandler.handleExecRequest |
| `task_request` | send task_response + persist to comms.db |
| `task_response` | clear pendingTasks timer |
| `status_query` | send status_response |
| `daily_report` | DailySummaryScheduler.receivePodReport |
| `sync_push` | forward to rcCoreUrl/sync/push |
| `sync_action_ack` | forward to rcCoreUrl/actions/{id}/ack |
| `message` | relay to other clients + persist to comms.db |

### Existing exec_result Promise Resolution (FailoverOrchestrator pattern)

`james/failover-orchestrator.js` already demonstrates the exec-result-as-promise pattern:
- `#pending` Map: `execId -> { resolve, reject, timer }`
- `handleExecResult(payload)` called from `james/index.js` on `exec_result`
- Resolves the promise for the matching execId
- 30s timeout per pending exec

This pattern is the foundation for execution chain orchestration.

---

## System Overview: v18.0 Target State

```
James (Windows 11, .27)                              Bono (VPS, Linux)
+-------------------------------------------------+  +-------------------------------------------------+
| james/index.js (message router)                 |  | bono/index.js / wireBono (message router)       |
|                                                 |  |                                                 |
| [NEW] DynamicCommandRegistry                    |  | [NEW] DynamicCommandRegistry (Bono-side)        |
|   - runtime register/deregister                 |  |   - runtime register/deregister                 |
|   - merges with static COMMAND_REGISTRY         |  |   - merges with static COMMAND_REGISTRY         |
|                                                 |  |                                                 |
| [EXTENDED] ExecHandler                          |  | [EXTENDED] bonoExecHandler                      |
|   - injected registry = DynamicCommandRegistry  |  |   - injected registry = DynamicCommandRegistry  |
|   - adds exec_result promise-tracking           |  |   - adds exec_result promise-tracking           |
|                                                 |  |                                                 |
| [NEW] ShellRelayHandler                         |  | [NEW] ShellRelayHandler                         |
|   - APPROVE-tier shell execution                |  |   - APPROVE-tier shell execution (Linux)        |
|   - sanitized environment                       |  |   - sanitized environment                       |
|                                                 |  |                                                 |
| [NEW] ChainOrchestrator                         |  | [NEW] ChainOrchestrator                         |
|   - multi-step chain definition + execution     |  |   - multi-step chain definition + execution     |
|   - step N+1 receives step N output             |  |   - step N+1 receives step N output             |
|   - chain-level audit entry                     |  |   - chain-level audit entry                     |
|                                                 |  |                                                 |
| [NEW] TaskDelegator                             |  | [NEW] TaskDelegator                             |
|   - Claude-to-Claude delegation API             |  |   - Claude-to-Claude delegation API             |
|   - awaitable remote task results               |  |   - awaitable remote task results               |
|                                                 |  |                                                 |
| [EXTENDED] james/index.js                       |  | [EXTENDED] bono/index.js                        |
|   - new message types routed                    |  |   - new message types routed                    |
|   - exec_result forwarded to ChainOrchestrator  |  |   - exec_result forwarded to ChainOrchestrator  |
+-------------------------------------------------+  +-------------------------------------------------+
           |  WebSocket :8765  |
           | (unchanged transport) |
```

### New Protocol Message Types

Six new types need registration in `shared/protocol.js`:

| Type | Direction | Purpose |
|------|-----------|---------|
| `cmd_register` | Bono -> James or James -> Bono | Register a new runtime command |
| `cmd_deregister` | Bono -> James or James -> Bono | Remove a runtime command |
| `shell_request` | Either direction | Arbitrary shell command (APPROVE tier only) |
| `shell_result` | Either direction | Output of shell_request |
| `chain_request` | Either direction | Multi-step execution chain definition |
| `chain_result` | Either direction | Final aggregated chain result |
| `delegate_request` | Either direction | Claude-to-Claude task delegation |
| `delegate_result` | Either direction | Result of delegated Claude task |

`chain_request` and `chain_result` both flow through the existing `exec_request`/`exec_result` mechanism internally per step — they are a layer above it.

---

## Component Map: New vs Modified

### New Components

#### shared/dynamic-registry.js

```
DynamicCommandRegistry
  #static: COMMAND_REGISTRY (frozen, read-only source)
  #dynamic: Map<string, CommandSpec>

  register(name, spec)        -- validates spec shape, rejects CONTROL_TYPES conflicts
  deregister(name)            -- only dynamic entries removable, static are permanent
  get(name)                   -- checks dynamic first, falls back to static
  list()                      -- merged view for introspection
  serialize() / hydrate()     -- JSON round-trip for persistence across restarts
```

**Integration:** Replaces the direct `COMMAND_REGISTRY` import in `ExecHandler` constructor.
`ExecHandler` already accepts `commandRegistry` as a constructor parameter — inject
`DynamicCommandRegistry` instance. Zero changes to ExecHandler internals.

**Persistence:** On registration, serialize to `./data/dynamic-registry.json`. Load at startup
before wiring `ExecHandler`. Registration survives process restart.

#### shared/shell-relay.js

```
ShellRelayHandler
  #execFileFn                 -- injectable (same as ExecHandler)
  #sendResultFn               -- (shellId, result) => void
  #notifyFn                   -- APPROVE tier notification
  #approvalTimeoutMs          -- default 600000ms (same as ExecHandler)
  #pendingApprovals: Map

  handleShellRequest(msg)     -- msg.payload: { shellId, command, args[], cwd, tier, reason }
  approveShell(shellId)
  rejectShell(shellId, reason)
  get pendingApprovals()
```

**Key constraint:** `tier` in the shell_request payload MUST be `APPROVE` — any attempt to
send a shell_request with AUTO or NOTIFY tier is rejected at the handler level with an error
result. Shell relay is an escape hatch for one-off operations not in the static registry, and
it is always gated by operator approval.

**Integration:** Wired alongside ExecHandler in both `james/index.js` and `bono/index.js`.
New HTTP relay routes added for shell approval: `/relay/shell/pending`,
`/relay/shell/approve/:shellId`, `/relay/shell/reject/:shellId`.

#### shared/chain-orchestrator.js

```
ChainOrchestrator
  #execRequestFn              -- (command, args, reason) => Promise<ExecResult>
  #shellRequestFn             -- (command, args, cwd, reason) => Promise<ShellResult>
  #nowFn

  executeChain(chain)         -- runs steps sequentially, passes output forward
  #resolveStep(step, ctx)     -- resolves template vars in args from prior step output
  #auditChain(chain, results) -- writes chain-level audit entry
```

**Chain definition structure:**

```json
{
  "chainId": "uuid",
  "name": "deploy-and-verify",
  "steps": [
    {
      "stepId": "pull",
      "type": "exec",
      "command": "git_pull",
      "reason": "deploy step 1"
    },
    {
      "stepId": "verify",
      "type": "exec",
      "command": "health_check",
      "reason": "deploy step 2",
      "dependsOn": "pull",
      "condition": { "exitCode": 0 }
    }
  ],
  "onFailure": "abort"
}
```

`condition` allows step N+1 to be skipped or the chain to abort if step N's exit code or
stdout does not match expectations. This prevents half-completed deploy chains from silently
proceeding.

**Integration:** ChainOrchestrator wraps ExecHandler's exec-result-as-promise pattern
(already demonstrated in FailoverOrchestrator). The `#execRequestFn` sends an `exec_request`
and returns a promise resolved by `handleExecResult`. For cross-machine chains, the exec
travels via WS; for local chains, it calls ExecHandler directly.

#### shared/task-delegator.js

```
TaskDelegator
  #sendFn                     -- client.send or ws.send
  #pendingDelegations: Map    -- delegationId -> { resolve, reject, timer }
  #delegationTimeoutMs        -- default 300000ms (5 min)

  delegate(payload)           -- sends delegate_request, returns Promise<DelegateResult>
  handleDelegateResult(msg)   -- resolves pending promise by delegationId
  handleDelegateRequest(msg)  -- executes local Claude response, sends delegate_result
```

**Claude-to-Claude flow:**

```
James receives user question requiring Bono data
  -> TaskDelegator.delegate({ question, context })
  -> sends delegate_request via WS
  -> Bono receives delegate_request
  -> Bono runs query (exec, HTTP, DB read)
  -> Bono sends delegate_result with { answer, data, exitCode }
  -> James receives delegate_result
  -> TaskDelegator resolves promise
  -> James integrates answer into response
```

**Audit:** Every delegate_request and delegate_result is appended to INBOX.md on both sides.
The `delegationId` links request to result in the audit log.

**Integration:** TaskDelegator replaces the current `sendTaskRequest` + `pendingTasks` Map
pattern in both `james/index.js` and `bono/index.js wireBono`. The existing `task_request` /
`task_response` flow currently only ACKs receipt (no actual result payload returned).
TaskDelegator extends this with a proper request-response pair that carries data.

### Modified Components (Existing Files Extended)

#### shared/protocol.js — ADD new message types

Add to `MessageType` object:
- `cmd_register`, `cmd_deregister` — dynamic registry sync
- `shell_request`, `shell_result` — shell relay
- `chain_request`, `chain_result` — chain orchestration
- `delegate_request`, `delegate_result` — Claude-to-Claude delegation

These are all data messages (not control) — they participate in ACK tracking automatically.

**Backward compatibility:** All existing types unchanged. New types are additive.

#### shared/exec-protocol.js — ADD shell_request validation

Add `validateShellRequest(payload)` alongside existing `validateExecRequest`. Validates:
- `command` is a non-empty string
- `args` is an array
- `tier` must be `APPROVE` (reject anything else)
- `cwd` if present is an absolute path

The static `COMMAND_REGISTRY` and `buildSafeEnv` are unchanged.

#### james/exec-handler.js — NO changes needed

ExecHandler already accepts `commandRegistry` as a constructor injection point. The only
change is at instantiation time in `james/index.js` — pass `DynamicCommandRegistry` instead
of the static `COMMAND_REGISTRY`. ExecHandler's internal `#commandRegistry` lookup already
reads from whatever was injected.

#### james/index.js — ADD routing for new message types

The existing `client.on('message', handler)` block is extended with:

```
cmd_register    -> dynamicRegistry.register(payload.name, payload.spec)
                   + persist + log
cmd_deregister  -> dynamicRegistry.deregister(payload.name) + persist + log
shell_request   -> shellRelayHandler.handleShellRequest(msg)
shell_result    -> chainOrchestrator.handleShellResult(msg.payload)
chain_request   -> chainOrchestrator.executeChain(msg.payload)
chain_result    -> chainOrchestrator.handleChainResult(msg.payload) (for remote chains)
delegate_request -> taskDelegator.handleDelegateRequest(msg)
delegate_result  -> taskDelegator.handleDelegateResult(msg)
```

Existing `exec_result` routing is extended: in addition to `failoverOrchestrator.handleExecResult`,
also call `chainOrchestrator.handleExecResult` so chain steps waiting on exec results are resolved.

New HTTP relay routes added to `relayServer`:
```
POST /relay/cmd/register       -- register a new command (JSON body: name, spec)
POST /relay/cmd/deregister     -- remove a command (JSON body: name)
GET  /relay/cmd/list           -- list all registered commands (static + dynamic)
POST /relay/shell/send         -- trigger shell_request to Bono
GET  /relay/shell/pending      -- list pending shell approvals
POST /relay/shell/approve/:id  -- approve a pending shell
POST /relay/shell/reject/:id   -- reject a pending shell
POST /relay/chain/send         -- trigger chain_request to Bono
POST /relay/delegate           -- send delegate_request to Bono
```

#### bono/index.js (wireBono) — ADD routing for new message types

Mirrors james/index.js additions, symmetric:

```
cmd_register    -> bonoRegistry.register(...) + persist
cmd_deregister  -> bonoRegistry.deregister(...)
shell_request   -> bonoShellHandler.handleShellRequest(msg)
shell_result    -> bonoChainOrchestrator.handleShellResult(msg.payload)
chain_request   -> bonoChainOrchestrator.executeChain(msg.payload)
chain_result    -> bonoChainOrchestrator.handleChainResult(msg.payload)
delegate_request -> bonoTaskDelegator.handleDelegateRequest(msg)
delegate_result  -> bonoTaskDelegator.handleDelegateResult(msg)
```

Existing `exec_result` routing (currently only logs + wss.emit) extended to also call
`bonoChainOrchestrator.handleExecResult`.

---

## Data Flow

### Dynamic Command Registration Flow

```
Claude Code (James session)
  -> POST /relay/cmd/register { name: "build_racecontrol", spec: { binary, args, tier, timeoutMs } }
  -> james/index.js relayServer
  -> dynamicRegistry.register("build_racecontrol", spec)
  -> persist to ./data/dynamic-registry.json
  -> optionally: client.send('cmd_register', { name, spec })
     -> Bono receives cmd_register -> bonoRegistry.register(name, spec)
  Response: { ok: true, name: "build_racecontrol" }
```

The registration is local-first: the registering side can use it immediately. Syncing to the
other side is optional and triggered by an explicit sync flag in the POST body.

### Single Exec Request Flow (existing, unchanged)

```
Bono wants to run git_status on James:
  wireBono.sendExecRequest(ws, { command: 'git_status', reason: '...' })
  -> createMessage('exec_request', 'bono', { execId, command, reason, requestedBy: 'bono' })
  -> WS to James
  -> james/index.js: execHandler.handleExecRequest(msg)
  -> ExecHandler: lookup in DynamicCommandRegistry (dynamic first, then static)
  -> execFile('git', ['status'], safeEnv)
  -> sendResultFn -> connectionMode.sendCritical('exec_result', { execId, ...result })
  -> WS back to Bono
  -> wireBono: log + wss.emit('exec_result', payload)
```

### Execution Chain Flow (new)

```
Bono wants to deploy + verify on James:
  POST /relay/chain/send {
    name: "deploy-and-verify",
    steps: [
      { stepId: "pull", type: "exec", command: "git_pull" },
      { stepId: "install", type: "exec", command: "npm_install", dependsOn: "pull", condition: { exitCode: 0 } }
    ]
  }
  -> james/index.js: ChainOrchestrator.executeChain(chain)
  -> Step 1: sendExecRequest('git_pull') -> await execResult promise
  -> Step 1 result: { exitCode: 0, stdout: "..." }
  -> condition check: exitCode === 0, proceed
  -> Step 2: sendExecRequest('npm_install') -> await execResult promise
  -> Step 2 result: { exitCode: 0 }
  -> ChainOrchestrator: aggregate results, write audit entry
  -> optionally: send chain_result back to requester
```

For remote chains (Bono sends chain_request to James), the chain executes on James and the
aggregated result returns as a `chain_result` message.

### Shell Relay Flow (new)

```
Bono wants to run an arbitrary command not in the registry:
  POST /relay/shell/send {
    command: "powershell",
    args: ["-Command", "Get-NetAdapter"],
    reason: "diagnose network adapter state",
    tier: "approve"
  }
  -> james/index.js: client.send('shell_request', { shellId, command, args, cwd, tier, reason })
  -> Bono receives shell_request
  -> bonoShellHandler.handleShellRequest(msg)
  -> tier === 'approve': queue, send WhatsApp to Uday: "Approval required: powershell Get-NetAdapter"
  -> Uday approves via /relay/shell/approve/:shellId on Bono's relay
  -> execFile('powershell', ['-Command', 'Get-NetAdapter'], safeEnv)
  -> send shell_result back to James
```

### Claude-to-Claude Delegation Flow (new)

```
User asks James: "What are the latest 5 sessions on the cloud DB?"
  James (Claude) knows this requires Bono's SQLite:
  -> TaskDelegator.delegate({
       question: "SELECT top 5 billing_sessions from cloud DB",
       context: { dbPath: "/root/racecontrol/racecontrol.db" },
       type: "db_query"
     })
  -> sends delegate_request via WS
  -> Bono receives delegate_request
  -> TaskDelegator.handleDelegateRequest: runs export_failover_sessions exec (or dynamic cmd)
  -> sends delegate_result { delegationId, data: [...sessions], exitCode: 0 }
  -> James TaskDelegator resolves promise
  -> James integrates data into user response
```

---

## Component Boundaries and Communication

| Component | File | Communicates With | Data Contract |
|-----------|------|-------------------|---------------|
| DynamicCommandRegistry | shared/dynamic-registry.js | ExecHandler (injected), index.js (persist) | CommandSpec: { binary, args, tier, timeoutMs, cwd, description } |
| ShellRelayHandler | shared/shell-relay.js | index.js (routing), notifyFn, execFileFn | ShellRequest: { shellId, command, args, cwd, tier, reason } |
| ChainOrchestrator | shared/chain-orchestrator.js | ExecHandler (exec-result promises), index.js | ChainDef: { chainId, name, steps[], onFailure } |
| TaskDelegator | shared/task-delegator.js | sendFn (WS), index.js (routing) | DelegationPayload: { delegationId, question, context, type } |
| ExecHandler | james/exec-handler.js | DynamicCommandRegistry (injected), sendResultFn | Unchanged from v10.0 |

### ExecResult Promise Pattern (shared concern)

Both ChainOrchestrator and TaskDelegator need the exec-result-as-promise pattern. The cleanest
approach is a shared `ExecResultBroker` (or inline Map in each orchestrator):

```
ExecResultBroker
  #pending: Map<execId, { resolve, reject, timer }>

  waitFor(execId, timeoutMs)  -> Promise<ExecResult>
  settle(execId, result)      -> void  (called from index.js exec_result handler)
```

`james/index.js` and `bono/index.js` each instantiate one broker and call `broker.settle()`
in the `exec_result` handler alongside the existing `failoverOrchestrator.handleExecResult`.
ChainOrchestrator and TaskDelegator both reference the same broker instance.

This replaces the duplicated `#pending` Maps in FailoverOrchestrator, ChainOrchestrator,
and TaskDelegator — one broker serves all.

---

## Audit Trail Architecture

All cross-machine execution must produce an audit entry. The current INBOX.md + comms.db
pattern is extended:

```
Audit entry fields:
  timestamp (IST)
  direction (james->bono | bono->james)
  type (exec | shell | chain | delegation)
  requestedBy (james | bono | operator)
  command / chainId / delegationId
  exitCode(s)
  durationMs
  tier (auto | notify | approve)
  approved_by (for APPROVE tier: who triggered /relay/*/approve)
```

James side: append to INBOX.md (existing audit file).
Bono side: persist to comms.db via `persistToCommsDb()` (existing mechanism).

Chain audits write one entry per chain (not per step) to avoid log spam, but include step
summaries in the `body` field.

---

## Recommended Project Structure Changes

```
comms-link/
  shared/
    protocol.js          -- ADD 8 new message types
    exec-protocol.js     -- ADD validateShellRequest(), shell_request schema
    dynamic-registry.js  -- NEW: DynamicCommandRegistry
    shell-relay.js       -- NEW: ShellRelayHandler
    chain-orchestrator.js-- NEW: ChainOrchestrator
    task-delegator.js    -- NEW: TaskDelegator
    exec-result-broker.js-- NEW: ExecResultBroker (shared promise resolver)
    [existing unchanged]
  james/
    index.js             -- EXTENDED: new routing + new relay HTTP routes
    exec-handler.js      -- NO CHANGES (registry injected)
    [existing unchanged]
  bono/
    index.js             -- EXTENDED: wireBono gets new routing
    [existing unchanged]
  data/
    dynamic-registry.json-- NEW: persisted runtime commands (gitignored, machine-local)
```

Total new files: 5 in shared/. Total modified files: 3 (protocol.js, exec-protocol.js, james/index.js, bono/index.js). ExecHandler itself requires zero changes.

---

## Build Order (dependency-driven)

Build order must respect import dependencies. Lower numbers have no dependencies on higher numbers.

```
Phase 1: shared/protocol.js — add new types
  Rationale: everything imports protocol.js; must be first.
  Risk: LOW — pure additive to frozen object.

Phase 2: shared/exec-result-broker.js — new standalone module
  Rationale: no dependencies on new components; consumed by phases 4 and 5.
  Risk: LOW — small, well-understood pattern (mirrors FailoverOrchestrator#pending).

Phase 3: shared/dynamic-registry.js — new standalone module
  Rationale: no dependencies on new components; consumed by phases 4 and 6.
  Risk: LOW — straightforward Map + JSON persistence.
  Dependency: exec-protocol.js (COMMAND_REGISTRY as static base).

Phase 4: shared/exec-protocol.js — add validateShellRequest
  Rationale: needed before ShellRelayHandler.
  Risk: LOW — additive function only.

Phase 5: shared/shell-relay.js — new ShellRelayHandler
  Rationale: depends on exec-protocol.js (phase 4) + protocol.js (phase 1).
  Risk: MEDIUM — new APPROVE-tier flow; approval routing new.

Phase 6: shared/chain-orchestrator.js — new ChainOrchestrator
  Rationale: depends on exec-result-broker.js (phase 2) + protocol.js (phase 1).
  Risk: MEDIUM — step sequencing + template var resolution is new logic.

Phase 7: shared/task-delegator.js — new TaskDelegator
  Rationale: depends on exec-result-broker.js (phase 2) + protocol.js (phase 1).
  Risk: LOW — promise-over-WS pattern already proven in FailoverOrchestrator.

Phase 8: james/index.js — wire new components, add relay routes
  Rationale: depends on all phases 1-7.
  Risk: MEDIUM — large file, many new routing cases; existing routes must not regress.
  Mitigation: add new routing blocks AFTER existing ones to avoid accidental shadowing.

Phase 9: bono/index.js (wireBono) — mirror james-side wiring
  Rationale: depends on phases 1-7 plus james/index.js being stable for reference.
  Risk: MEDIUM — same pattern as phase 8 but Linux-side env assumptions differ.
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Registering Commands with Shell Strings

**What people do:** Register a dynamic command with `binary: 'bash'` and `args: ['-c', 'rm -rf /tmp && rsync ...']`.

**Why wrong:** Shell interpretation re-opens all injection vectors the current design closes. The `shell: false` + array-args guarantee is the entire security model.

**Do this instead:** Register granular commands — one command per binary. If a multi-step operation is needed, use a chain rather than a shell string.

### Anti-Pattern 2: Returning exec_result Payloads to Claude Without Sanitization

**What people do:** Pipe raw stdout from an exec_result directly into Claude's context as "trusted data."

**Why wrong:** If a compromised process writes to stdout, it could inject instructions into the AI context (prompt injection via exec output).

**Do this instead:** TaskDelegator always labels delegated data as `[REMOTE DATA]` in the context. Claude Code on the receiving side treats it as untrusted user content, not trusted system prompt content.

### Anti-Pattern 3: Bypassing APPROVE Tier for Shell Relay

**What people do:** Add a `shell_relay_auto` option to skip approval for "trusted" one-liners.

**Why wrong:** Shell relay is the only path to arbitrary execution. A single AUTO-tier shell command breaks the containment model for all dynamic commands.

**Do this instead:** If a shell operation is needed frequently enough to feel tedious to approve, convert it into a named dynamic command (binary + args array) in the registry.

### Anti-Pattern 4: Separate ExecResult Pending Maps per Orchestrator

**What people do:** Give ChainOrchestrator its own `#pending` Map and TaskDelegator its own `#pending` Map, both listening to `exec_result`.

**Why wrong:** The exec_result handler in `james/index.js` can only call one resolver. If two Maps both claim the same `execId`, only one resolves — the other hangs until timeout.

**Do this instead:** One shared `ExecResultBroker` instance, all orchestrators register with it via `broker.waitFor(execId)`. The `exec_result` handler calls `broker.settle(execId, result)` exactly once.

### Anti-Pattern 5: Storing Dynamic Commands Only in Memory

**What people do:** Register commands at startup via startup script, skip persistence.

**Why wrong:** Comms-link restarts on every deploy. Without persistence, dynamic commands disappear. The next deploy to comms-link undoes all runtime registrations silently.

**Do this instead:** `DynamicCommandRegistry` serializes to `./data/dynamic-registry.json` on every mutation. `james/index.js` loads it at startup before wiring ExecHandler.

---

## Scaling Considerations

This system is single-connection (one James, one Bono) by design. Scaling concerns are
reliability and throughput, not concurrency:

| Concern | Current state | With v18.0 |
|---------|---------------|------------|
| Chain step failures | N/A | `onFailure: abort` prevents cascades |
| Shell approval timeout | ExecHandler: 10 min default-deny | ShellRelayHandler: same 10 min default-deny |
| Delegation timeout | task_request: 5 min timer (no result carried) | TaskDelegator: 5 min, rejects promise with error |
| Dynamic registry size | Static: 13 commands | Dynamic: expected <50 commands, no scaling issue |
| Exec concurrency | ExecHandler: unlimited concurrent | ChainOrchestrator: sequential per chain, parallel chains allowed |

---

## Sources

- Direct inspection of `comms-link/` source (2026-03-22): comms-client.js, comms-server.js,
  exec-handler.js, exec-protocol.js, protocol.js, message-queue.js, ack-tracker.js,
  connection-mode.js, james/index.js, bono/index.js, failover-orchestrator.js
- Existing FailoverOrchestrator `#pending` map as proven pattern for exec-result promises
- PROJECT.md v18.0 Seamless Execution feature targets

---
*Architecture research for: v18.0 Seamless Execution — comms-link bidirectional dynamic execution*
*Researched: 2026-03-22 IST*
