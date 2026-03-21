# Project Research Summary

**Project:** v18.0 Seamless Execution - Bidirectional AI-to-AI Dynamic Execution Protocol
**Domain:** Extending a static-registry remote execution system to support runtime command registration, shell relay, execution chains, and Claude-to-Claude task delegation over an existing WebSocket transport
**Researched:** 2026-03-22
**Confidence:** HIGH

## Executive Summary

v18.0 extends the existing comms-link system - a mature Node.js v22 ES module stack with a frozen command registry, 3-tier approval gating, and reliable WebSocket transport - to support four new capabilities: dynamic command registration at runtime, shell relay for arbitrary binary execution, multi-step execution chains, and bidirectional Claude-to-Claude task delegation. All four capabilities are buildable without adding any new npm packages; every gap is covered by Node.js v22 stdlib. The existing ExecHandler, AckTracker, MessageQueue WAL, and ConnectionMode infrastructure remain unchanged and serve as the execution backbone - new components are additive layers above them, not replacements.

The recommended implementation approach treats the existing security model (no-shell execution, frozen allowlist, sanitized env, 3-tier approval) as inviolable and extends it: dynamic commands are registered into a parallel mutable Map with a binary allowlist and forced APPROVE tier; shell relay is a separate handler class that never reuses ExecHandler code path; chain orchestration sequences ExecHandler calls through a shared ExecResultBroker; and Claude-to-Claude delegation is a Promise-over-WS pattern already proven by FailoverOrchestrator. A shared ExecResultBroker is the key architectural insight - it prevents the anti-pattern of multiple competing pending Maps listening to the same exec_result messages.

The dominant risks are security erosion (dynamic registration breaking the frozen-allowlist guarantee, shell relay adding shell-mode execution, or buildSafeEnv accumulating secrets) and operational blind spots (execution chains orphaning on step failure, prompt injection via exec stdout, and the daemon-vs-Claude-session distinction). All risks have concrete prevention strategies defined in PITFALLS.md. The build order is dependency-driven: protocol types first, ExecResultBroker and DynamicRegistry second, then ShellRelayHandler and ChainOrchestrator, then TaskDelegator, then index.js wiring on both sides.

## Key Findings

### Recommended Stack

The entire v18.0 milestone is implementable with zero new npm dependencies. Node.js v22.14.0 stdlib covers every requirement: node:fs/promises for JSON registry persistence and JSONL audit logs, node:crypto.randomUUID for chain and execIds (already imported), node:child_process.execFile for execution with shell:false (already used in ExecHandler with array-args, never shell strings), node:events.EventEmitter for ChainOrchestrator state events (already the base class for all major components), and node:timers/promises for async chain step timeouts. The only runtime dep, ws@8.19.0, requires no version change - new message types are payload-level additions to protocol.js.

**Core technologies:**
- node:fs/promises (v22 stdlib): Dynamic registry persistence to data/dynamic-commands.json, chain state to data/chains/, audit log to data/exec-audit.jsonl - proven WAL pattern from MessageQueue
- node:child_process.execFile with shell:false (existing): Shell relay reuses the same secure execFile path already in ExecHandler - no new execution primitive needed
- node:events.EventEmitter (existing): ChainOrchestrator extends EventEmitter for step/chain lifecycle events - consistent with every other major comms-link component
- ws@8.19.0 (existing): All new message types route over the existing WS connection - no transport changes, only shared/protocol.js MessageType additions

### Expected Features

All 9 table-stakes features (TS-1 through TS-9) must ship for v18.0 to be complete. DynamicRegistry (TS-1) is the foundation for all non-shell-relay features; ChainOrchestrator (TS-3) is the most complex single component and gates delegation (TS-6); shell relay (TS-2) is architecturally simple but operationally the highest-risk feature and requires TS-9 (full command text in Uday approval notification) to ship alongside it.

**Must have (table stakes - v18.0 core):**
- TS-1 Dynamic command registration - runtime Map-backed registry with binary allowlist; ExecHandler commandRegistry injection already exists
- TS-2 Shell relay with APPROVE tier - separate handler, binary allowlist, always-APPROVE, no escalation possible
- TS-3 Chain orchestration - sequential exec sequencing, step N output feeds step N+1, new ChainOrchestrator class
- TS-4 Abort on chain step failure - default abort-on-failure; per-step continue_on_error opt-in
- TS-5 Structured chain_result message - aggregate all steps into single response to requester
- TS-6 Claude-to-Claude delegation - James sends chain_request to Bono, awaits chain_result via Promise
- TS-7 Execution audit log - append-only data/exec-audit.jsonl with execId, command, tier, exitCode, durationMs
- TS-8 Backward compatibility - all existing static commands unchanged; dynamic registry falls through to static on lookup miss
- TS-9 Shell relay approval notification - full binary+args in Uday WhatsApp message, not just command name

**Should have (v18.1 after core validation):**
- D-4 Registry introspection (GET /relay/commands) - returns name/description/tier only, never binary/args
- D-1 Per-command env injection - allowedEnvKeys list in dynamic command spec, values from local env not payload
- D-6 Chain-level timeout - chainTimeoutMs cap on total chain duration regardless of step count
- D-5 Chain templates - named reusable chains loaded from chains.json at startup

**Defer (v18.2+):**
- D-2 Output templating in step args - high sanitization complexity, defer until core chains validated in production
- D-7 Per-step retry - significant ChainOrchestrator complexity for low-frequency transient failures
- D-3 Chain pause/resume across WS reconnect - new serialization layer; rare given 5-minute TASK_TIMEOUT_MS

**Anti-features (never build):**
- Shell-mode execution for shell relay - breaks the entire security model; the relay is of binary+args, not a shell string
- Arbitrary env passthrough from message payload - use allowedEnvKeys allowlist only
- AUTO or NOTIFY tier for any dynamic or shell relay command - all must be APPROVE without exception
- Cross-machine registry replication - commands are machine-specific (Windows on James, Linux on Bono)
- Chain step conditional branching - AI handles conditional logic in its reasoning; execution protocol stays linear

### Architecture Approach

Five new shared modules are added alongside unchanged existing ones. The transport (ws), approval flow (ExecHandler), reliability (AckTracker + WAL), and connection management (ConnectionMode) are all unchanged. New components are: DynamicCommandRegistry (Map-backed, falls through to static COMMAND_REGISTRY), ShellRelayHandler (separate APPROVE-only handler), ChainOrchestrator (sequences ExecHandler calls, abort-on-failure default), TaskDelegator (Promise-over-WS using the FailoverOrchestrator pending-Map pattern), and ExecResultBroker (single shared pending Map preventing multiple orchestrators competing for the same exec_result). The routing layer in james/index.js and bono/index.js is extended with new message type handlers; ExecHandler itself requires zero changes.

**Major components:**
1. shared/dynamic-registry.js - DynamicCommandRegistry: Map + JSON persistence + binary allowlist + falls through to static COMMAND_REGISTRY on lookup miss
2. shared/shell-relay.js - ShellRelayHandler: separate APPROVE-tier execution path for arbitrary binary+args; own pendingApprovals Map; hardcoded APPROVE tier - escalation from payload is rejected
3. shared/exec-result-broker.js - ExecResultBroker: single shared Map keyed by execId serving ChainOrchestrator and TaskDelegator; called from exec_result handler in index.js - prevents duplicate pending Map anti-pattern
4. shared/chain-orchestrator.js - ChainOrchestrator: sequential step execution, condition checking, abort-on-failure, chain-level audit entry, chain_result aggregation
5. shared/task-delegator.js - TaskDelegator: Promise-returning delegate and handleDelegateResult methods; replaces fire-and-forget sendTaskRequest pattern with a result-carrying request-response pair
6. shared/protocol.js (modified) - 8 new MessageType entries: cmd_register, cmd_deregister, shell_request, shell_result, chain_request, chain_result, delegate_request, delegate_result

### Critical Pitfalls

1. **Dynamic registration breaks the frozen allowlist** - Maintain a binary allowlist (ALLOWED_BINARIES) that all registration requests must reference. Tier for dynamically registered commands is always forced to APPROVE server-side regardless of what the requester specifies. The mutable dynamic Map is a separate layer from the frozen static COMMAND_REGISTRY, never a replacement for it.

2. **Shell relay enables unguarded execution** - Shell relay MUST use execFile with shell:false and array-args (same as ExecHandler). Enforce with validateShellRequest that rejects any tier other than APPROVE. Add HMAC approval token (execId + timestamp + PSK, 60s TTL) to prevent replay attacks on captured approval messages.

3. **AI session smuggling via exec stdout** - Raw stdout from exec results must never be fed directly into an AI reasoning context as instructions. Wrap all delegation results in a structural envelope tagged as [REMOTE DATA]. Validate that result payloads contain only expected fields (exitCode, stdout, stderr, durationMs) and reject extra protocol-level keys.

4. **Execution chain orphaning on step failure** - Every chain step MUST check the previous step exitCode before proceeding. Default is abort-on-failure. Chain ID must be recorded for every step in the audit log so failures surface as one coherent FAILED chain, not a partial apparent success.

5. **buildSafeEnv secret accumulation** - Refactor buildSafeEnv to buildCommandEnv(commandName, safeEnv) before adding any dynamic commands. Per-command env isolation ensures notify_failover credentials never reach unrelated commands. Add a test that fails if any KEY, SECRET, or TOKEN variable appears in the general safeEnv passed to other commands.

## Implications for Roadmap

Based on the ARCHITECTURE.md build order and PITFALLS.md phase-to-pitfall mapping, 5 phases are recommended.

### Phase 1: Foundation - Protocol Types, Dynamic Registry, Env Isolation

**Rationale:** Everything imports shared/protocol.js - it must be first. DynamicRegistry and the buildCommandEnv refactor are prerequisites for all other phases. The binary allowlist and platform field must be in the registry spec schema from day one (retrofitting is costlier than building it right initially). PITFALLS.md explicitly maps Pitfalls 1, 5, and 8 to this phase.
**Delivers:** 8 new MessageType constants; DynamicCommandRegistry with JSON persistence, binary allowlist, and platform field; buildCommandEnv replacing buildSafeEnv for per-command env isolation; backward compat tests confirming all existing static commands pass unchanged
**Addresses:** TS-1 (dynamic registration), TS-8 (backward compatibility), D-1 groundwork (per-command env injection)
**Avoids:** Pitfall 1 (frozen allowlist broken by mutable registry), Pitfall 5 (cross-platform path divergence), Pitfall 8 (buildSafeEnv secret accumulation)

### Phase 2: Shell Relay with Hardened Approval Gate

**Rationale:** Shell relay is the highest-risk feature and must be isolated before any chain or delegation work begins. Its security model (APPROVE-only, binary allowlist, HMAC approval token) must be proven independently before chains can call it as a step type. PITFALLS.md maps Pitfalls 2 and 7 specifically to this phase.
**Delivers:** ShellRelayHandler (separate from ExecHandler); validateShellRequest in exec-protocol.js; HMAC approval token (execId + timestamp + PSK, 60s TTL); full binary+args in Uday WhatsApp notification; /relay/shell HTTP routes; integration tests confirming shell relay cannot be triggered via exec_request pathway
**Addresses:** TS-2 (shell relay), TS-9 (approval notification with full command text)
**Avoids:** Pitfall 2 (unguarded execution via shell relay), Pitfall 7 (approval gate replay and stale approvals)

### Phase 3: ExecResultBroker and Chain Orchestration

**Rationale:** ExecResultBroker must be built before ChainOrchestrator because both ChainOrchestrator and TaskDelegator depend on it. ChainOrchestrator is the most complex single component; its abort-on-failure logic, chain ID audit trail, and structured chain_result are all correctness-critical before delegation can use chains as its execution primitive. PITFALLS.md maps Pitfall 4 (chain orphaning) to this phase.
**Delivers:** ExecResultBroker (single shared pending Map, replaces duplicated Maps in FailoverOrchestrator); ChainOrchestrator (sequential steps, condition checking, abort-on-failure default, chain-level audit entry, chain_result message); chain_request and chain_result routing in index.js; test confirming step 1 fails with non-zero exit code and step 2 does NOT execute
**Addresses:** TS-3 (chain orchestration), TS-4 (abort on failure), TS-5 (structured chain_result)
**Avoids:** Pitfall 4 (execution chain orphaning on step failure)

### Phase 4: Task Delegation and Audit Trail

**Rationale:** TaskDelegator requires chains (Phase 3) and ExecResultBroker (Phase 3) before it can function. The audit trail is independent but logically belongs here because delegation adds the most complex cross-machine audit requirements. Pitfall 3 (AI session smuggling) is addressed via structural result envelopes. Pitfall 6 (daemon assumption) requires a task queue design decision before the delegation protocol is finalized.
**Delivers:** TaskDelegator (Promise-returning delegate and handleDelegateResult methods); delegate_request and delegate_result routing on both sides; structural [REMOTE DATA] envelope preventing prompt injection; data/exec-audit.jsonl with rotation at 10MB; chain-level audit entries with chainId, stepIndex, per-step exitCode and durationMs; bounded completedExecs (max 10,000 entries with eviction)
**Addresses:** TS-6 (Claude-to-Claude delegation), TS-7 (execution audit log)
**Avoids:** Pitfall 3 (AI session smuggling via exec stdout), Pitfall 6 (daemon assumption - task queue for async delegation), completedExecs unbounded memory growth

### Phase 5: HTTP Relay Routes, Introspection, and Integration Hardening

**Rationale:** All core features are working after Phase 4. This phase wires the full relay HTTP interface, adds registry introspection, and runs cross-platform validation. It is last because it depends on all other phases being stable and exercised.
**Delivers:** Full relay HTTP routes for all new features (/relay/cmd, /relay/chain, /relay/delegate); GET /relay/commands introspection endpoint (name/description/tier/timeoutMs only - never binary/args); cross-platform test suite verifying Windows commands are rejected on Bono Linux handler; chain-level timeout (D-6) added here as a low-complexity addition; end-to-end chain test across a real WS connection
**Addresses:** D-4 (registry introspection), D-6 (chain-level timeout)
**Avoids:** Pitfall 5 (cross-platform command sent to wrong machine)

### Phase Ordering Rationale

- Protocol types before everything because all modules import shared/protocol.js; additive MessageType change is the lowest-risk first commit
- DynamicRegistry and buildCommandEnv refactor in Phase 1 because both are imported by Phases 2, 3, and 4 - retrofitting after the fact would require touching all later phases
- Shell relay in Phase 2 before chains because its security model must be proven in isolation before being embedded in a more complex flow
- ExecResultBroker before ChainOrchestrator because ChainOrchestrator takes a broker reference in its constructor; building them in the wrong order creates circular dependency risk during development
- TaskDelegator after ChainOrchestrator because delegation sends chain_requests and awaits chain_results - it cannot be integration-tested without working chains
- HTTP routes and introspection last because they are integration surface, not core correctness - all security properties are validated in earlier phases

### Research Flags

Phases that may benefit from deeper review before implementation:
- **Phase 2 (Shell relay HMAC approval):** The one-time HMAC approval token is a new security primitive not currently in the codebase. Implementation details of the out-of-band approval confirmation flow (WhatsApp message triggers daemon approval with a verifiable token) warrant careful design review before coding begins.
- **Phase 4 (Delegation task queue):** Pitfall 6 (daemon assumption - AI sessions are not always-on) requires a persistent task queue for incoming delegate_requests when no Claude Code session is active. The queue design (JSONL persistence + session-start re-presentation) needs architectural review before implementation to avoid building a queue that does not solve the session-lifecycle problem.

Phases with well-established patterns (standard implementation, research-phase not needed):
- **Phase 1 (Protocol + DynamicRegistry):** Additive MessageType constants and Map-backed registry are straightforward; FailoverOrchestrator already demonstrates the persistence pattern
- **Phase 3 (ExecResultBroker):** Directly mirrors FailoverOrchestrator pending Map pattern - low novelty, proven approach in the same codebase
- **Phase 5 (HTTP routes):** James relay HTTP server on port 8766 already has existing routes as reference; all new routes are additive

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Direct codebase analysis of comms-link source; Node.js v22 stdlib APIs are stable; zero new deps confirmed by exhaustive evaluation of alternatives |
| Features | HIGH | Codebase audit of exec-protocol.js, exec-handler.js, protocol.js, james/index.js; feature boundaries derived from existing code structure, not speculation |
| Architecture | HIGH | Direct inspection of all 11 comms-link source files; component boundaries grounded in existing FailoverOrchestrator, ExecHandler, and MessageQueue patterns |
| Pitfalls | HIGH (security), MEDIUM (delegation quirks) | Security pitfalls backed by CVE evidence and source analysis; Claude-to-Claude delegation quirks (session lifecycle, prompt injection) have limited published evidence and rely on pattern inference |

**Overall confidence:** HIGH

### Gaps to Address

- **HMAC approval token implementation:** The approval hardening (execId + timestamp + PSK HMAC, 60s TTL) is specified but not yet prototyped. The exact mechanism for Uday WhatsApp-triggered approval to carry a verifiable token back to the daemon needs design confirmation before Phase 2 coding begins.
- **Task queue session handoff:** How pending delegate_requests are re-presented when a Claude Code session starts is described conceptually (JSONL file scanned at startup) but the exact trigger and UI mechanism are not specified. Resolve at the start of Phase 4.
- **completedExecs persistence across restarts:** PITFALLS.md recommends persisting recent completedExecs (last 1000) to disk with TTL to prevent replay after daemon restart. The existing code uses an in-memory Set. This is a replay-protection gap for APPROVE-tier commands if the daemon restarts between request and approval. Flag for Phase 2 hardening.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis (2026-03-22): comms-link/shared/exec-protocol.js, comms-link/james/exec-handler.js, comms-link/shared/protocol.js, comms-link/shared/message-queue.js, comms-link/james/index.js, comms-link/bono/index.js, comms-link/james/failover-orchestrator.js, comms-link/shared/ack-tracker.js, comms-link/shared/connection-mode.js
- Node.js v22 LTS stdlib documentation - node:child_process, node:fs/promises, node:events, node:timers/promises, node:crypto - all stable APIs
- .planning/PROJECT.md - v18.0 Seamless Execution feature targets and constraints

### Secondary (MEDIUM confidence)
- CVE-2025-52882: WebSocket authentication bypass in Claude Code extensions - WebSocket auth bypass pattern
- CVE-2025-59536 / CVE-2026-21852: RCE and API Token Exfiltration via Claude Code Project Files - env var leakage and code injection via config files
- CVE-2025-0110: PAN-OS OpenConfig Plugin Command Injection - input not neutralized before shell insertion; direct prior art for Pitfall 2
- Unit42: Agent Session Smuggling in A2A Systems - prompt injection via AI-to-AI result forwarding; direct prior art for Pitfall 3
- Ansible playbook model - task sequencing, abort-on-failure default, per-task ignore_errors opt-in (direct prior art for ChainOrchestrator design)
- LSP dynamic capability registration - runtime protocol extension without reconnection (direct prior art for DynamicRegistry)
- Temporal.io durable execution patterns - chain orchestration with abort-on-failure and saga compensation

### Tertiary (LOW confidence)
- OWASP BLA1:2025 - Lifecycle and Orphaned Transitions Flaws - orphan task and stale state patterns (pattern inference, not direct API evidence)

---
*Research completed: 2026-03-22 IST*
*Ready for roadmap: yes*
