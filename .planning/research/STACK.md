# Stack Research

**Domain:** Bidirectional AI-to-AI dynamic execution protocol (v18.0 Seamless Execution)
**Researched:** 2026-03-22
**Confidence:** HIGH — grounded in direct codebase analysis plus Node.js v22 stdlib capabilities

---

## Context: What Already Exists (Do Not Re-Research)

The comms-link is a mature Node.js v22.14.0 ES module system. Before any decisions, here is what is already in place:

| Component | File | What It Does |
|-----------|------|--------------|
| WebSocket transport | `james/comms-client.js`, `bono/comms-server.js` | Persistent WS, PSK auth, reconnect with backoff |
| Message protocol | `shared/protocol.js` | Envelope with v/type/from/ts/id, MessageType enum |
| ACK + WAL queue | `shared/ack-tracker.js`, `shared/message-queue.js` | Sequence tracking, JSON Lines WAL, crash-safe |
| Static command registry | `shared/exec-protocol.js` | 13 frozen commands, ApprovalTier enum, buildSafeEnv(), validateExecRequest() |
| Execution handler | `james/exec-handler.js` | ExecHandler class, 3-tier approval, execFile (shell:false), dedup, pending approval Map |
| Connection mode | `shared/connection-mode.js` | REALTIME/EMAIL_FALLBACK/OFFLINE_QUEUE graceful degradation |
| Task delegation | `james/index.js` | sendTaskRequest() + pendingTasks Map, existing task_request/task_response MessageTypes |

**npm deps:** only `ws@8.19.0` and `toml@3.0.0`. **Runtime:** Node.js v22.14.0.

**Central finding:** All four new v18.0 features are implementable with zero new npm packages. The Node.js v22 stdlib covers every gap.

---

## Recommended Stack for New Features

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Node.js stdlib `node:fs/promises` | v22.14.0 (current) | Dynamic registry persistence (JSON file), chain state persistence (JSONL), audit log | Already used throughout `james/index.js` and `shared/message-queue.js`. Proven pattern. |
| Node.js stdlib `node:crypto` | v22.14.0 (current) | randomUUID() for chain IDs, exec IDs, delegation correlation IDs | Already imported in `james/index.js`. |
| Node.js stdlib `node:child_process` | v22.14.0 (current) | execFile for shell relay (array-args, shell:false always) | ExecHandler already uses this correctly. Shell relay reuses the same path — no new execution primitive needed. |
| Node.js stdlib `node:events` | v22.14.0 (current) | EventEmitter base for ChainOrchestrator and DynamicRegistry state events | ExecHandler, MessageQueue, CommsClient all extend EventEmitter. Established pattern. |
| Node.js stdlib `node:timers/promises` | v22.14.0 (current) | setTimeout as a Promise for async chain step timeouts without callback nesting | Available since Node.js v16, stable in v22. |
| `ws@8.19.0` (existing) | 8.19.0 | All message transport — new message types route over existing WS connection | No version change needed. New MessageType entries added to `protocol.js` only. |

### Supporting Libraries

None needed. The table below documents what was evaluated and rejected:

| Feature | Tempting External Library | Reason to Reject | Node.js Stdlib Alternative |
|---------|--------------------------|------------------|---------------------------|
| Dynamic command registry persistence | conf, lowdb, keyv | All solve a non-problem: a JSON file with fewer than 100 entries needs only readFile/writeFile. Each adds transitive deps. | node:fs/promises read/write to data/dynamic-commands.json |
| Shell relay execution | execa npm package | ExecHandler already implements the secure pattern: execFile with shell:false, array-args, sanitized env, timeout, maxBuffer. execa adds zero security value here and would be a third npm package. | node:child_process execFile (already used in ExecHandler) |
| Execution chain orchestration | bull, bullmq, p-queue | All require Redis or add significant infrastructure. 2-machine linear chains with 2-10 steps do not need a queue broker. The WAL already provides crash-safe durability. | In-memory Map plus WAL file per chain in data/chains/ |
| Claude-to-Claude delegation | Any orchestration library | The WS channel is the transport. Delegation is a structured task_request with a resolve/reject Promise keyed by taskId. This is 30 lines of code, not a library. | Extend sendTaskRequest() in james/index.js with a Promise-returning variant |
| Audit trail | winston, pino, SQLite | Structured JSON append to a .jsonl file is sufficient for 2-machine audit. Log databases add schema migration complexity. | node:fs/promises appendFile to data/exec-audit.jsonl |
| Schema validation for dynamic registry | zod, ajv | Dynamic command spec validation is 5 checks: binary is string, args is array, tier is valid enum value, timeoutMs is positive integer, no shell metacharacters in binary. Not worth a dep. | Inline validation function in DynamicRegistry.register() |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `node --test` (Node.js built-in) | Unit tests for new modules: DynamicRegistry, ChainOrchestrator, DelegationHandler, ExecAudit | Already used: `"test": "node --test test/*.test.js"`. No Jest or Vitest needed. |

---

## Installation

```bash
# No new packages needed.
# npm install is not required for v18.0.
```

---

## Integration Points with Existing Code

### 1. Dynamic Command Registration

**Integration target:** `shared/exec-protocol.js` (read-only augmentation, not modification)

The existing COMMAND_REGISTRY is Object.freeze()-d by design — it is the static trusted baseline. Dynamic registration adds a parallel mutable layer:

**New file: `shared/dynamic-registry.js`**

```
class DynamicRegistry
  #commands: Map<name, spec>     -- in-memory, mutable
  #persistPath: string           -- data/dynamic-commands.json

  async load()                   -- reads JSON on startup, validates each entry
  async register(name, spec)     -- validate spec, add to Map, persist, emit 'registered'
  async deregister(name)         -- remove from Map, persist, emit 'deregistered'
  lookup(name): spec | null      -- caller checks static registry first, then this
  list(): Array<{name, spec}>    -- for audit/display

  #validate(spec): void
    - binary: non-empty string, no shell metacharacters (|;&`$(){}), no path traversal
    - args: array of strings
    - tier: must be ApprovalTier.APPROVE (all dynamic commands are APPROVE-only)
    - timeoutMs: integer 1000-300000
```

**ExecHandler change (minimal):** The constructor already accepts commandRegistry as a DI param. Change: pass a lookup function that checks static COMMAND_REGISTRY first (the trusted baseline), falls back to DynamicRegistry.lookup(). One-line change to how spec is resolved in handleExecRequest.

**Protocol additions to `shared/protocol.js`:**
```javascript
register_command: 'register_command',    // Request to register a new dynamic command
register_result: 'register_result',      // Confirmation or rejection of registration
deregister_command: 'deregister_command', // Remove a dynamic command
```

Registration itself is always APPROVE tier. The approval flow runs through the existing queueForApproval path in ExecHandler — no new approval mechanism needed.

### 2. Shell Relay with Approval Gate

**Integration target:** `shared/dynamic-registry.js` (special built-in entry) + ExecHandler (no changes)

Shell relay is not a separate execution path. It is a pre-registered dynamic command where the binary is resolved from the first element of a validated args array:

```
Command name: "shell_relay"
Tier: ApprovalTier.APPROVE (hardcoded, cannot be overridden by caller)
Invocation: execFile(cmd_args[0], cmd_args.slice(1), { shell: false })
```

**Security gate (before ExecHandler sees it):**
```javascript
validateShellRelayArgs(cmd_args) {
  // Reject if any arg contains shell injection chars: |;&`$(){}><
  // Reject if cmd_args[0] is not on the approved-binaries allowlist
  // Reject if cmd_args is empty or cmd_args[0] is a relative path
}
```

**ExecHandler changes:** None. Shell relay arrives as a normal exec_request with command: "shell_relay" and payload.cmd_args: [...]. The APPROVE tier flow handles queueing, Uday notification, and timeout default-deny exactly as today.

**Protocol additions (exec_request payload shape for shell relay):**
```javascript
{ command: "shell_relay", cmd_args: ["git", "status"], reason: "...", requestedBy: "bono" }
```

### 3. Execution Chain Orchestration

**New files:**

```
shared/exec-chain.js
  class ExecChain
    chainId: string              -- randomUUID()
    steps: ChainStep[]           -- [{ command, argsOverride?, captureOutput: bool }]
    state: ChainState            -- 'pending'|'running'|'completed'|'failed'|'cancelled'
    currentStep: number          -- index into steps[]
    results: ExecStepResult[]    -- accumulated per-step result objects
    startedAt: number

    toJSON(): object             -- serialized for WAL persistence
    static fromJSON(obj): ExecChain

  class ChainOrchestrator extends EventEmitter
    #chains: Map<chainId, ExecChain>
    #execHandler: ExecHandler
    #persistDir: string          -- data/chains/

    async start(chainDef): chainId    -- validate, persist, begin step 0
    async onStepResult(execId, result) -- advance chain or fail it
    async cancel(chainId): bool

    Events emitted:
      'step_complete': { chainId, stepIndex, result }
      'chain_complete': { chainId, results[] }
      'chain_failed':   { chainId, failedStep, result }
```

**Protocol additions to `shared/protocol.js`:**
```javascript
chain_request: 'chain_request',         // Initiate a multi-step chain on remote machine
chain_step_result: 'chain_step_result', // Intermediate step result (informational)
chain_complete: 'chain_complete',       // Final bundle of all step results
chain_cancelled: 'chain_cancelled',     // Chain was cancelled (timeout or explicit)
```

**Integration in `james/index.js`:** exec_result messages carry an optional chainId field. The message handler checks: if chainId is present, route to ChainOrchestrator.onStepResult(); otherwise route to existing standalone result handler. This is additive — existing flows are unaffected.

**Persistence:** Each chain writes `data/chains/<chainId>.json` after every step. On startup, ChainOrchestrator scans `data/chains/` for in-progress chains (state not 'completed' or 'failed') and resumes them. This covers the WS-drop-mid-chain case.

### 4. Claude-to-Claude Task Delegation

**Integration target:** `james/index.js` — sendTaskRequest() and pendingTasks Map

The task_request / task_response MessageTypes already exist in protocol.js. The existing sendTaskRequest() sends and forgets (fire-and-forget with a timeout warning). The v18.0 addition is Promise-based result resolution:

**New file: `james/delegation-handler.js`**

```
class DelegationHandler
  #pending: Map<taskId, { resolve, reject, timer }>

  async delegate(task): Promise<DelegationResult>
    -- task: { taskType: 'exec_single'|'exec_chain', target: 'bono', command?, chainDef?, reason }
    -- sends task_request via connectionMode.sendCritical()
    -- returns Promise that resolves on matching task_response
    -- rejects on timeout (default: TASK_TIMEOUT_MS = 300s)

  onTaskResponse(msg)
    -- called from message handler when task_response arrives
    -- resolves matching Promise in #pending Map
    -- no-ops unknown taskIds (dedup safe)
```

**Bono side (`bono/index.js`):** Receives task_request with taskType 'exec_chain' or 'exec_single'. Routes to its own ExecHandler or ChainOrchestrator. Returns task_response with:
```javascript
{ taskId, success: bool, exitCode, stdout?, stderr?, results?: ExecStepResult[], durationMs }
```

**No new transport.** Everything routes over the existing WS connection through connectionMode.sendCritical() — which already handles REALTIME/EMAIL_FALLBACK/OFFLINE_QUEUE degradation.

### 5. Audit Trail (Mandatory for All Cross-Machine Execution)

**New file: `shared/exec-audit.js`**

```
class ExecAudit
  #path: string              -- data/exec-audit.jsonl
  #appendFn: Function        -- node:fs/promises appendFile (injectable for tests)
  #maxSizeBytes: number      -- default 10MB, then rotate

  async log(entry): void
    entry: {
      ts: number, requestedBy: string, machine: 'james'|'bono',
      command: string, chainId?: string, stepIndex?: number,
      execId: string, exitCode: number, durationMs: number,
      tier: ApprovalTier, truncated: bool, registrationType: 'static'|'dynamic'|'shell_relay'
    }

  async rotate(): void       -- rename to exec-audit.YYYY-MM-DD.jsonl if >maxSizeBytes
```

Every exec_result, chain_step_result, and chain_complete flows through ExecAudit.log() before acknowledgment. This is the source of truth for "who requested, what ran, exit code, duration."

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| In-memory Map + JSON file for dynamic registry | SQLite | SQLite is a native module; complex on Windows without prebuilt binaries. Fewer than 100 dynamic commands fit in a 10KB JSON file. |
| node:child_process execFile (existing) | execa npm package | ExecHandler's usage is already correct: array-args, shell:false, sanitized env, timeout. execa adds zero security value and would be the third npm package. |
| Custom ChainOrchestrator (Map + EventEmitter) | bull or bullmq | Require Redis. 2-machine linear chains with up to 20 steps don't need a queue broker or persistent job store beyond the existing WAL pattern. |
| Existing WAL pattern for chain persistence | New SQLite store | WAL is proven crash-safe in MessageQueue. Reuse the pattern (one JSON file per chain) rather than adding a new persistence layer. |
| node --test for new module tests | Jest or Vitest | Already established in the project. Consistent tooling matters more than test runner features for this scope. |
| Structured JSONL audit file | Grafana/Loki/ELK | No infrastructure change is justified for a 2-machine protocol. JSONL is readable, grepable, and rotatable with 20 lines of code. |
| Promise-based DelegationHandler | RxJS observables | Overkill. Each delegation is a single request-response pair, not a stream. Promise is the right primitive. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Any job queue library (bull, bullmq, p-queue, bee-queue) | All require external infrastructure (Redis) or add multi-hundred-KB deps. 2-machine linear chains do not need a broker. | ChainOrchestrator with in-memory Map + per-chain JSON persistence |
| zod or ajv for spec validation | Dynamic registry validation is 5 checks. A validation library would add more code than it replaces. | Inline validate() method in DynamicRegistry |
| execa or shelljs | ExecHandler already implements the secure pattern. shelljs enables shell:true by default (injection risk). execa is well-designed but redundant here. | node:child_process execFile with shell:false (existing) |
| New WebSocket library or HTTP transport layer | ws@8.19.0 handles all message routing. New message types are protocol additions, not transport changes. | Add new entries to MessageType in shared/protocol.js |
| TypeScript migration for this milestone | Would require adding a build step, tsconfig, and type-checking to a working ES module project. No return on investment for this scope. | Continue ES modules with JSDoc type annotations |
| Storing execution results in SQLite | Schema migration complexity, native binary dep on Windows. JSONL audit is sufficient. | data/exec-audit.jsonl with rotation |
| Shell relay with shell:true | Enables shell string interpolation, opening command injection. The entire security model of ExecHandler is built on shell:false with array-args. | shell:false always, validate cmd_args as an array before dispatch |
| Auto-approve tier for any dynamic command | Dynamic commands are unknown at deploy time. Auto-approve lets Bono run arbitrary code on James's machine without human confirmation. All dynamic commands must be APPROVE tier — no exceptions. | Force APPROVE tier in DynamicRegistry.register() validation |

---

## Stack Patterns by Variant

**If a chain step fails:**
- Fail the entire chain (no partial-commit semantics)
- Persist state 'failed' to data/chains/<chainId>.json
- Emit chain_failed with { chainId, failedStep: stepIndex, result }
- Do NOT auto-retry — re-run requires a new chain_request with explicit approval

**If the WS drops mid-chain:**
- Chain state is persisted to disk after each completed step
- On reconnect, ChainOrchestrator scans data/chains/ for in-progress chains
- Caller (or DelegationHandler) emits chain_resume_needed event
- Human decides: resume from last completed step or cancel

**If dynamic registration is requested:**
- Always APPROVE tier — validate binary path before queuing for approval
- Log the registration attempt to exec-audit.jsonl regardless of approval outcome
- On approval: persist to data/dynamic-commands.json, emit 'registered' event
- On rejection or timeout: remove from pending, log rejection

**If Claude-to-Claude delegation times out:**
- DelegationHandler.delegate() rejects the Promise with { timedOut: true, taskId }
- Caller (James's Claude session) handles: log to audit, notify Uday via existing WhatsApp channel
- No auto-retry — human decides next action

**If shell relay args fail validation:**
- Reject synchronously in DynamicRegistry before the request reaches ExecHandler
- Return exec_result with exitCode: -1, stderr: "Shell relay args failed validation: <reason>"
- Log attempt to audit trail with registrationType: 'shell_relay'

---

## Version Compatibility

| Package / API | Version | Notes |
|---------------|---------|-------|
| ws | 8.19.0 | No change. New MessageType entries are payload-level, not library-level. |
| toml | 3.0.0 | No change. Used only for racecontrol.toml config reading. |
| node:timers/promises | stable since Node.js v16 | setTimeout as Promise. Fully stable in v22.14.0, no polyfill needed. |
| node:fs/promises | stable since Node.js v14 | appendFile, readFile, writeFile, readdir, rename — all used. |
| node:crypto randomUUID | stable since Node.js v15.6.0 | Already imported in james/index.js. |
| node:events EventEmitter | stable | Already the base class for ExecHandler, MessageQueue, CommsClient, ConnectionStateMachine. |

---

## Sources

- Direct codebase analysis: `shared/exec-protocol.js`, `james/exec-handler.js`, `shared/protocol.js`, `shared/message-queue.js`, `shared/state.js`, `james/index.js` — HIGH confidence (code read 2026-03-22)
- Node.js v22 LTS stdlib documentation: node:child_process, node:fs/promises, node:events, node:timers/promises, node:crypto — all stable APIs. HIGH confidence.
- ws@8.19.0 — no breaking changes to message routing in ws@8.x series. HIGH confidence.
- PROJECT.md v18.0 feature requirements — HIGH confidence (authoritative spec, read 2026-03-22)

---

*Stack research for: v18.0 Seamless Execution — dynamic execution protocol additions to comms-link*
*Researched: 2026-03-22 IST*
