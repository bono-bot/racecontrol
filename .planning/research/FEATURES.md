# Feature Research: v18.0 Seamless Execution — Bidirectional AI-to-AI Dynamic Execution Protocol

**Domain:** Bidirectional AI-to-AI remote execution over persistent WebSocket — dynamic command registration, shell relay, execution chains, and Claude-to-Claude task delegation between James (Windows 11, .27) and Bono (Linux VPS).
**Researched:** 2026-03-22
**Confidence:** HIGH (codebase audit of exec-protocol.js, exec-handler.js, protocol.js, james/index.js, bono/comms-server.js; established patterns from LSP protocol, Ansible, supervisor patterns, and task queue systems)

---

## Context: What Already Exists (Do Not Re-Build)

Before mapping features, it is essential to separate "already shipped" from "net new."

| Already in exec-protocol.js + exec-handler.js | Net new for v18.0 |
|------------------------------------------------|-------------------|
| 20 static commands in frozen COMMAND_REGISTRY | Dynamic runtime registration of commands on either side |
| 3-tier approval (AUTO / NOTIFY / APPROVE) | Shell relay for arbitrary commands (maps to APPROVE tier) |
| ExecHandler with dedup (completedExecs Set) | Execution chains: step N+1 receives step N output |
| exec_request / exec_result / exec_approval message types | Chain orchestration protocol (chain_request, chain_result) |
| Bidirectional: both James and Bono have ExecHandler | Claude-to-Claude seamless delegation (auto-invoke, response integration) |
| sanitized env (buildSafeEnv), no-shell execution, array args | Per-command env injection for dynamic commands |
| approvalTimeoutMs (default 10 min), default-deny | Approval UI / callback for Claude-side approval |
| Structured exec_result (exitCode, stdout, stderr, durationMs, truncated) | Chain-level result aggregation |
| AckTracker reliable delivery (3 retries, 10s timeout) | Audit trail persistence (beyond in-memory completedExecs) |
| ConnectionMode graceful degradation (WS to email fallback) | Chain pause/resume across reconnects |
| sendTaskRequest() with timeout tracking in james/index.js | Chain step timeouts individual vs chain-total |

The security model (no-shell execution, sanitized env, frozen registry) must be PRESERVED and EXTENDED — not replaced — for dynamic commands.

---

## Feature Landscape

### Table Stakes (Must Have for Milestone Goal to Be Met)

These are the features that define what "Seamless Execution" actually means. Missing any means the milestone is not complete.

| # | Feature | Why Expected | Complexity | Dependency on Existing Code |
|---|---------|--------------|------------|------------------------------|
| TS-1 | **Dynamic command registration: either side can add a command at runtime** | The 20-entry static COMMAND_REGISTRY requires code deploy to change. Dynamic registration means either AI can teach the other a new operation (e.g., "add a command to query fleet health") without a redeploy. Core milestone requirement. | MEDIUM | COMMAND_REGISTRY is a frozen object — the dynamic registry must be a separate mutable store (Map) that merges with static registry on lookup. ExecHandler already accepts `commandRegistry` injection; dynamic version passes a live Map. |
| TS-2 | **Shell relay with APPROVE tier: send arbitrary shell commands between machines** | James frequently needs to run commands on Bono's Linux VPS and vice versa. Today this requires constructing a new static command and deploying. Shell relay wraps arbitrary binary+args in an exec_request with tier=APPROVE, which requires explicit approval before execution. | MEDIUM | ExecHandler's execute() already supports arbitrary binary+args from the registry spec. Shell relay is a new command type that passes binary+args as payload fields rather than looking them up by name. Must use same sanitized-env + no-shell execution model. |
| TS-3 | **Execution chain: multi-step task where step N+1 receives step N output** | A chain like "git pull then npm install then restart daemon" fails silently if step 2 runs regardless of step 1 exit code. Chain orchestration feeds step N stdout/exitCode into step N+1 invocation decision and argument construction. | HIGH | exec_result payloads already carry exitCode + stdout. Chain orchestrator is a new component that sequences exec_requests and accumulates results. Uses existing AckTracker for each step reliable delivery. |
| TS-4 | **Chain abort on step failure with configurable continue-on-error flag** | A chain that continues past a failed step produces unpredictable state (e.g., deploying after a failed build). Default: abort chain when any step exitCode != 0. Optional `continue_on_error: true` per step for non-critical steps. | LOW | Builds on TS-3. Chain orchestrator checks exitCode after each step result. |
| TS-5 | **Structured chain result: all step outputs returned as single response** | The requesting Claude (James or Bono) needs to see every step output in a single structured result, not one-at-a-time exec_result messages. Chain result aggregates steps into `{ chainId, steps: [{command, exitCode, stdout, stderr, durationMs}], totalDurationMs, aborted, abortReason }`. | LOW | New message type `chain_result` in protocol.js. Aggregation logic in chain orchestrator. |
| TS-6 | **Seamless Claude-to-Claude delegation: James auto-delegates to Bono and integrates response** | When a user asks James something that requires action on Bono machine (e.g., "check if cloud racecontrol is healthy and restart it if not"), James should automatically send the task, wait for Bono response, and return an integrated answer — not expose the delegation to the user. | HIGH | sendTaskRequest() + pendingTasks Map already exists in james/index.js for task tracking. Delegation means James sends a chain_request, Bono executes and replies, James awaits and integrates. TASK_TIMEOUT_MS (default 5 min) already configurable. |
| TS-7 | **Audit trail for all cross-machine execution persisted to disk** | INBOX.md captures messages but not exec outcomes. Every exec_request (who requested it, what command, what args if shell relay, exitCode, durationMs) must be appended to a structured execution log file. This is the compliance record for all remote execution. | LOW | appendAuditLog() in james/index.js already writes to INBOX.md. Execution log is a separate append-only file with structured entries (not free-text). Same pattern, different target file. |
| TS-8 | **Backward compatibility: all 20 existing static commands continue to work** | Existing COMMAND_REGISTRY entries (git_status, activate_failover, etc.) must work identically after dynamic registration is added. The new dynamic layer must fall through to the static registry for unknown command names. | LOW | Lookup order: dynamic registry first, static COMMAND_REGISTRY second. ExecHandler commandRegistry injection already supports custom registries. |
| TS-9 | **Approval gate for shell relay: APPROVE tier always, human-readable prompt to Uday** | Shell relay with arbitrary binary+args is the highest-risk operation in the system. It must always route through the APPROVE tier with a notification that includes the full command being requested (not just the command name). The notification must be Uday-readable: "Bono is requesting to run: pm2 restart racecontrol on Bono VPS." | LOW | ExecHandler's queueForApproval() already calls notifyFn with command name. Shell relay version must pass the full binary+args in the notification text. Uses existing WhatsApp notification path. |

### Differentiators (Beyond the Mandatory Floor)

Features that make the execution protocol significantly more capable without being required for the core mission.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Per-command env injection for dynamic commands** | Dynamic commands registered at runtime may need specific environment variables (e.g., API keys, paths) not in the static buildSafeEnv(). Per-command env injection allows the registrant to specify additional env vars merged with safeEnv at execution time. | LOW | ExecHandler execute() already merges EXEC_REASON into safeEnv. Same pattern: merge per-command env dict into safeEnv at execution time. Allowlist-based to prevent leaking arbitrary secrets. |
| D-2 | **Chain step output templating: inject previous step stdout into next step args** | A chain like "get pod IP then connect to pod IP" requires step 2 args to contain step 1 stdout. Output templating lets args contain {{prev_stdout}} which is substituted before execution. | MEDIUM | Requires template substitution in chain orchestrator before sending each step exec_request. Sanitization required: templated value must be validated as safe input (no special characters even with no-shell execution, since array args are passed directly to the process). |
| D-3 | **Chain pause and resume across WS reconnect** | If the WS drops mid-chain, the chain state must be preserved so execution resumes when connection is restored, not silently abandoned. Uses the existing WAL (message-queue.wal) pattern. | HIGH | The existing MessageQueue WAL is for message delivery. Chain state is higher-level: serialized step index + accumulated results. New chain-state.json file or in-memory state with reconnect handler in client. Depends on ConnectionMode RECONNECTING state transition events. |
| D-4 | **Registry introspection endpoint: list all registered commands with descriptions** | Either AI can query the other current command registry (static + dynamic) to discover what is available before sending an exec_request. Returns a list of `{ name, description, tier, timeoutMs }` — never binary/args (security). | LOW | New relay HTTP endpoint GET /relay/commands on the existing HTTP relay server (port 8766). Reads from merged static + dynamic registry. Binary and args fields are omitted from the response — only name, description, tier, timeoutMs are exposed. |
| D-5 | **Chain templates: named reusable chains stored in config** | Common chains (e.g., "deploy-comms-link": git_pull then npm_install then restart_daemon) are defined once in a config section and invoked by name. Eliminates the need to re-specify the steps on every invocation. | MEDIUM | Chain templates in a comms-link chains.json. Loaded at startup. Invocable via chain_request with `template: "deploy-comms-link"`. |
| D-6 | **Execution timeout at chain level (not just per-step)** | Per-step timeouts exist in the registry. A chain of 5 steps each with 60s timeout could run for 5 minutes. Chain-level timeout caps the entire chain regardless of step count. | LOW | Chain orchestrator tracks chain start time. If chain-total elapsed >= chain timeout, abort remaining steps and return partial result. |
| D-7 | **Retry policy per chain step** | Some steps (network calls, git operations) are transient-failure-prone. Per-step retry count + backoff lets the chain handle transient errors without requiring the requesting AI to re-issue the full chain. | MEDIUM | Chain orchestrator: if step fails and `retries > 0`, wait for backoff duration and re-send exec_request for that step. Retry must use a new execId (existing dedup is execId-keyed). |

### Anti-Features (Do Not Build)

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Shell-mode execution for shell relay** | "Easier" to pass command strings including pipes, redirects, operators | Breaks the entire security model. Shell-mode expands the attack surface to shell injection. A compromised message payload can execute anything on the machine. The no-shell + array-args model is the entire foundation of the exec protocol security claim. | Require binary + args array even for shell relay. If pipes are needed, wrap in a script file and register the script as a command, or use the existing pattern of inline node -e scripts (as seen in notify_failover / notify_failback in COMMAND_REGISTRY). |
| **Arbitrary env passthrough for dynamic commands** | Dynamic commands need env vars | Arbitrary env passthrough from message payload to child process leaks whatever is in the message — including anything a compromised peer might inject. | Explicit allowlist: the dynamic command registration includes a named list of env var keys that are permitted. Values come from the registering machine environment, not from the exec_request payload. |
| **Bidirectional shell access without approval** | "It is just between two trusted AIs" | James and Bono are Claude Code instances. Either can be given instructions by a user or a compromised context. AUTO-tier shell relay means a malicious message triggers arbitrary code execution with no human in the loop. The APPROVE tier and Uday notification are non-negotiable for shell relay. | Shell relay always APPROVE tier. AUTO tier only for pre-approved registry entries with known binary+args. |
| **Storing exec_result stdout in WAL** | Audit trail needs output | Stdout from commands can be large (50KB per STDOUT_LIMIT). Storing it in the WAL bloats the WAL and risks leaking sensitive output (API responses, config file contents) if the WAL file is accessed. | Audit log stores command name, exitCode, durationMs, execId, and a truncated summary. Full stdout is returned in the exec_result message to the requesting side and is their responsibility to store if needed. |
| **Command registry merge across machines (shared registry state)** | "Both sides should know about all commands" | Registry replication creates consistency problems. If James registers a Windows-only command (wmic, tasklist) and it replicates to Bono Linux VPS, Bono will try to execute it and fail. Commands are inherently machine-specific. | Keep registries local. Use D-4 (introspection endpoint) to let either AI discover what the other side supports. Commands are registered on the machine that will execute them. |
| **Chain step conditional logic (if/else branching)** | Complex automation needs branching | Conditional chains are a workflow engine. This is scope creep toward building a general DAG executor. Adds significant complexity to the orchestrator for marginal use cases. The two AIs can handle conditional logic in their own reasoning — the execution protocol sends the decisions, not makes them. | The requesting AI evaluates step N result and decides whether to send step N+1 or a different follow-up. Conditional logic stays in the Claude layer, execution protocol stays linear. |
| **Cross-session chain state across process restarts** | "What if comms-link restarts mid-chain?" | Persisting chain state across process restarts requires a durable store with atomic updates and recovery logic. Chains are already bounded by TASK_TIMEOUT_MS (5 min default). A restart within a chain is a failure scenario, not a success path to optimize for. | On reconnect, the requesting AI receives a task timeout notification (existing behavior). The AI can re-issue the chain from the beginning or from the last known step using its own knowledge of what succeeded. |
| **Real-time chain progress streaming** | "Show each step result as it runs" | step_result streaming adds a new message type and requires the client to maintain chain state on the receiving side to assemble partial results. The overall chain_result already returns all steps. Streaming is complexity for marginal UX benefit (chains are typically seconds, not minutes). | The chain_result message includes all step outputs with timestamps. The requesting AI can display them sequentially. If a step has a very long timeout, use D-3 (chain pause/resume) rather than streaming. |

---

## Feature Dependencies

```
TS-1 (dynamic registry: mutable Map of commands)
    └──used by──> TS-8 (backward compat: lookup dynamic first, static second)
    └──used by──> D-4 (introspection endpoint: returns merged registry)
    └──required by──> D-1 (per-command env injection: registered with the command)
    └──required by──> D-5 (chain templates: steps reference registry entries)

TS-2 (shell relay: arbitrary binary+args with APPROVE tier)
    └──requires──> TS-9 (APPROVE tier notification includes full command)
    └──uses──> ExecHandler execute() (existing, no changes needed to ExecHandler itself)

TS-3 (chain orchestration: sequential exec_requests, step N output feeds step N+1)
    └──requires──> TS-4 (abort on failure: orchestrator checks exitCode after each step)
    └──requires──> TS-5 (chain_result: aggregate all steps into one response)
    └──enables──> TS-6 (Claude-to-Claude delegation: James sends chain, awaits chain_result)
    └──enables──> D-2 (output templating: orchestrator substitutes prev_stdout into next args)
    └──enables──> D-6 (chain-level timeout: orchestrator tracks total elapsed)
    └──enables──> D-7 (per-step retry: orchestrator retries step before moving forward)
    └──enables──> D-5 (templates: pre-defined step lists loaded and sent as chains)

TS-6 (Claude-to-Claude delegation)
    └──requires──> TS-3 (chains: delegation is typically a chain not a single command)
    └──requires──> TS-5 (chain_result: integration requires structured response)
    └──uses──> sendTaskRequest() + pendingTasks (existing in james/index.js)
    └──uses──> TASK_TIMEOUT_MS (existing configurable timeout)

TS-7 (audit trail: persisted execution log)
    └──independent: can be built standalone, enhanced by TS-3 (chain steps logged)
    └──uses──> appendAuditLog() pattern (existing in james/index.js)

D-3 (chain pause/resume across WS reconnect)
    └──requires──> TS-3 (chain state exists to be paused)
    └──requires──> ConnectionMode reconnect events (existing, connection-mode.js)
    └──requires──> New chain-state serialization (not in existing WAL)

D-2 (output templating)
    └──requires──> TS-3 (chain orchestrator is the substitution point)
    └──adds──> sanitization step before each exec_request (must strip metacharacters)
```

### Dependency Notes

- **TS-1 is the foundation for everything except TS-2**: Dynamic registry enables all non-shell-relay features. It must be the first component built.
- **TS-3 is the most complex single feature**: Chain orchestration introduces state management (which step are we on, what was the output, did it abort). The chain orchestrator is a new class, not an extension of ExecHandler. ExecHandler handles individual command execution; chain orchestrator sequences multiple ExecHandler calls.
- **TS-6 (delegation) requires zero new protocol work on Bono side**: Bono already has an ExecHandler. Delegation is James sending chain_requests that Bono handler processes. The only new code is the chain orchestrator and the chain_result message type.
- **D-3 (chain pause/resume) has the highest risk-to-value ratio**: It requires new serialization infrastructure. Given that TASK_TIMEOUT_MS defaults to 5 minutes and chains are typically 10-60 seconds, WS drops during chains are rare. Defer to post-MVP.
- **TS-2 (shell relay) is architecturally simple but operationally significant**: The code change is minimal (new dynamic command type with binary/args from payload). The operational significance is large — it permanently expands what can be executed. Must be tested under approval gate before shipping.

---

## MVP Definition

### Launch With (v18.0 core — Phases to build)

The minimum that achieves "Seamless Execution" as stated in the milestone goal.

- [ ] **TS-1** — Dynamic command registration: `DynamicRegistry` class (Map-backed). `register(name, spec)` and `lookup(name)` methods. ExecHandler modified to accept a DynamicRegistry that takes precedence over COMMAND_REGISTRY.
- [ ] **TS-8** — Backward compat: static COMMAND_REGISTRY remains untouched. DynamicRegistry.lookup() falls through to static. All 20 existing commands pass existing tests unchanged.
- [ ] **TS-3** — Chain orchestrator: new `ChainOrchestrator` class that accepts an array of `{command, reason}` steps, sends them sequentially as exec_requests via the existing ExecHandler, awaits each exec_result, and proceeds or aborts.
- [ ] **TS-4** — Abort on failure: built into ChainOrchestrator. Default `abort_on_failure: true`. Per-step `continue_on_error: true` opt-in.
- [ ] **TS-5** — `chain_result` message type added to protocol.js. ChainOrchestrator emits one chain_result with all step outputs after the chain completes or aborts.
- [ ] **TS-6** — Claude-to-Claude delegation: James sends `chain_request` message to Bono; Bono ChainOrchestrator runs the steps and sends back `chain_result`. James awaits using pendingTasks Map pattern (already exists). Result integrated into James response to user.
- [ ] **TS-7** — Execution audit log: `/data/exec-audit.log` on both James and Bono. Append-only structured entries: `[ISO_TIMESTAMP] execId=X command=Y from=Z exitCode=N durationMs=N tier=auto`. Chain entries include chainId and stepIndex.
- [ ] **TS-2** — Shell relay: `__shell_relay` as a special command type (not a registry entry). Payload: `{ binary, args, cwd, reason }`. Always APPROVE tier. Uday notification includes full `binary args` string. Executed via existing ExecHandler after approval. Binary must be in allowlist (node, git, pm2, cargo, systemctl, curl, sqlite3, taskkill, shutdown, net, wmic) — matching binaries already in static COMMAND_REGISTRY.
- [ ] **TS-9** — Shell relay notification: full command text in Uday WhatsApp message. Uses existing notifyFn path.

### Add After Validation (v18.1)

- [ ] **D-4** — Registry introspection: `GET /relay/commands` endpoint on relay HTTP server (port 8766). Returns `{ commands: [{name, description, tier, timeoutMs}] }`. Binary/args never exposed.
- [ ] **D-1** — Per-command env injection: dynamic commands can specify `allowedEnvKeys: ['MY_VAR']`. At execution, those keys are read from the registering machine env and merged with safeEnv.
- [ ] **D-6** — Chain-level timeout: `chainTimeoutMs` field on chain_request. ChainOrchestrator tracks total elapsed and aborts with `chain_timed_out` reason if exceeded.
- [ ] **D-5** — Chain templates: load named chains from `comms-link/chains.json` at startup. Invocable by template name.

### Future Consideration (v18.2+)

- [ ] **D-2** — Output templating: `{{prev_stdout}}` substitution in step args. Requires robust sanitization — defer until core chains are validated.
- [ ] **D-7** — Per-step retry: `retries: N, retryBackoffMs: M` per step in chain_request. Adds significant complexity to ChainOrchestrator.
- [ ] **D-3** — Chain pause/resume across reconnect: new chain-state serialization. High complexity, low frequency of need given 5-minute TASK_TIMEOUT_MS.

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|-------------------|---------------------|----------|
| TS-1 Dynamic registry | HIGH — foundation for all new features | MEDIUM — new Map-backed class, ExecHandler injection | P1 |
| TS-8 Backward compat | HIGH — existing 20 commands must not regress | LOW — lookup order change only | P1 |
| TS-3 Chain orchestrator | HIGH — multi-step automation is the core value | HIGH — new stateful class, message sequencing | P1 |
| TS-4 Abort on failure | HIGH — prevents bad state from partial chains | LOW — exitCode check in orchestrator | P1 |
| TS-5 chain_result message | HIGH — structured response for delegation | LOW — new message type + aggregation | P1 |
| TS-6 Claude-to-Claude delegation | HIGH — core milestone requirement | MEDIUM — chain_request/chain_result wiring, pendingTasks integration | P1 |
| TS-7 Audit log | HIGH — compliance, post-mortem | LOW — append pattern, new file | P1 |
| TS-2 Shell relay | MEDIUM-HIGH — escape hatch for ad-hoc operations | MEDIUM — new command type, allowlist validation | P1 |
| TS-9 Shell relay notification | HIGH (security gate — required with TS-2) | LOW — notification text update | P1 |
| D-4 Registry introspection | MEDIUM — AI discovery of capabilities | LOW — new HTTP endpoint | P2 |
| D-1 Per-command env injection | MEDIUM — needed for dynamic API-calling commands | LOW — env merge at execute time | P2 |
| D-6 Chain-level timeout | MEDIUM — prevents runaway chains | LOW — elapsed time tracking | P2 |
| D-5 Chain templates | MEDIUM — operational efficiency | MEDIUM — config loading, template lookup | P2 |
| D-2 Output templating | MEDIUM — enables data-flow chains | HIGH — sanitization complexity | P3 |
| D-7 Per-step retry | LOW-MEDIUM — nice for flaky operations | HIGH — retry state in orchestrator | P3 |
| D-3 Chain pause/resume | LOW — WS drops during 10-60s chains are rare | HIGH — new serialization layer | P3 |

---

## Protocol Changes Required

The existing protocol.js needs new message types for this milestone:

| New Type | Direction | Purpose | Payload Fields |
|----------|-----------|---------|----------------|
| `chain_request` | James to Bono or Bono to James | Initiate a multi-step chain | `chainId, steps: [{command, args?, reason, continue_on_error?}], chainTimeoutMs?` |
| `chain_step_ack` | Receiver to Sender | Confirm chain received and step N started | `chainId, stepIndex, execId` |
| `chain_result` | Executor to Requester | Final chain outcome with all steps | `chainId, steps: [{command, exitCode, stdout, stderr, durationMs}], totalDurationMs, aborted, abortReason?` |
| `registry_register` | Either direction | Register a new command in remote dynamic registry | `name, spec: {binary, args, tier, timeoutMs, description, allowedEnvKeys?}` |
| `registry_ack` | Either direction | Confirm command registration or error | `name, success, error?` |

Shell relay does NOT need a new message type — it uses the existing `exec_request` with a special command name (`__shell_relay`) and binary/args in the payload.

The existing `exec_request`, `exec_result`, `exec_approval` message types are UNCHANGED.

---

## Security Model for New Features

The existing security model (no-shell execution, sanitized env, array args, frozen registry) is the foundation. New features must not erode it:

| New Feature | Security Mechanism | Risk Level |
|-------------|-------------------|------------|
| Dynamic registry | Registration requires a valid spec with binary in allowlist | MEDIUM — binary allowlist is the gate |
| Shell relay | Always APPROVE tier + Uday notification + binary allowlist | HIGH — mitigated by mandatory human approval |
| Chain orchestration | Each step validated against registry before execution | LOW — same validation as single exec |
| Per-command env injection | Only keys listed in `allowedEnvKeys` are passed; values come from local env, not from payload | LOW — allowlist prevents payload injection |
| Registry introspection | Returns name/description/tier only, never binary/args | LOW — read-only, no sensitive data |
| Output templating (D-2, future) | Strip metacharacters from substituted values even with no-shell execution | HIGH — template injection possible if not sanitized |

The binary allowlist for shell relay should initially include: `node`, `git`, `pm2`, `cargo`, `systemctl`, `curl`, `sqlite3`, `taskkill`, `shutdown`, `net`, `wmic`. This matches the binaries already used in the static COMMAND_REGISTRY.

---

## Relationship to Existing Modules

| Feature | Extends | New Code | File Location |
|---------|---------|----------|---------------|
| TS-1 Dynamic registry | ExecHandler (commandRegistry injection already exists) | `DynamicRegistry` class | `shared/dynamic-registry.js` |
| TS-2 Shell relay | ExecHandler execute() (no changes needed to ExecHandler itself) | Shell relay validation + allowlist check + notification text | `shared/exec-protocol.js` or `shared/shell-relay.js` |
| TS-3 Chain orchestrator | ExecHandler (calls it per step), AckTracker (per-step reliable delivery) | `ChainOrchestrator` class | `shared/chain-orchestrator.js` |
| TS-5 chain_result | protocol.js MessageType | New message type constants | `shared/protocol.js` |
| TS-6 Delegation wiring | pendingTasks + sendTaskRequest in james/index.js | chain_request send + chain_result receive handler | `james/index.js`, `bono/comms-server.js` |
| TS-7 Audit log | appendAuditLog() pattern | New exec-audit.log target + structured formatter | `james/index.js`, `bono/comms-server.js` |
| D-4 Introspection | Relay HTTP server (port 8766, already in james/index.js) | New `/relay/commands` route | `james/index.js` |

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `comms-link/shared/exec-protocol.js` — 20-entry frozen COMMAND_REGISTRY, buildSafeEnv(), ApprovalTier, validateExecRequest()
  - `comms-link/james/exec-handler.js` — ExecHandler class: dedup, 3-tier routing, execute(), queueForApproval(), approveCommand(), rejectCommand(); commandRegistry injection via constructor
  - `comms-link/shared/protocol.js` — MessageType enum, CONTROL_TYPES, createMessage(), parseMessage(); confirmed exec_request/exec_result/exec_approval exist
  - `comms-link/james/index.js` — ExecHandler wiring, sendTaskRequest(), pendingTasks Map, TASK_TIMEOUT_MS, appendAuditLog(), relay HTTP server on 8766, ConnectionMode
  - `.planning/PROJECT.md` — v18.0 target features, constraints (no new transport, retain approval tiers, backward compat, Tailscale + WS paths)

- **Execution chain patterns (HIGH confidence — established systems):**
  - Ansible playbook model: tasks run sequentially, each task result available to subsequent tasks via register variables. Abort on failure is default; ignore_errors is per-task opt-in. Direct prior art for TS-3/TS-4.
  - LSP (Language Server Protocol) JSON-RPC: bidirectional request/response over a persistent channel with message IDs for correlation. The exec_request/exec_result pattern mirrors LSP request/response pairing exactly. Direct prior art for exec_request correlation.
  - Supervisor pattern (Erlang/Akka): supervisor monitors child workers and decides restart vs escalation. The chain orchestrator plays the supervisor role — it decides whether to continue, abort, or retry based on step outcomes.

- **Dynamic plugin/registry patterns (HIGH confidence):**
  - LSP dynamic capability registration (client/registerCapability) — runtime registration of new protocol capabilities without reconnection. Same pattern as TS-1: static capabilities at startup, dynamic ones added via message exchange.
  - Redis COMMAND INFO — read-only introspection of the command set without exposing implementation. Same pattern as D-4: expose metadata, not internals.

- **Security model (HIGH confidence — Node.js official docs + execFile design):**
  - The existing exec-protocol.js security model (no-shell execution, array args, sanitized env, frozen registry) eliminates shell injection by never invoking a shell. This is the Node.js child_process.execFile vs exec distinction — execFile passes arguments directly to the OS without shell interpretation.
  - Binary allowlisting is the industry standard for restricting what a relay can run. Same approach used by Ansible execution modules, Salt execution modules, and SSH forced commands.

---

*Feature research for: v18.0 Seamless Execution — Bidirectional AI-to-AI Dynamic Execution Protocol*
*Researched: 2026-03-22*
