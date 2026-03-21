# Phase 130: Protocol Foundation + Dynamic Registry - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add new message types to protocol.js (chain_request, chain_step_ack, chain_result, registry_register, registry_ack) and build a DynamicCommandRegistry class that allows runtime command registration with binary allowlist enforcement, per-command env injection, and fallback to the existing frozen COMMAND_REGISTRY.

All implementation in C:/Users/bono/racingpoint/comms-link (separate repo from racecontrol).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance from research:
- DynamicCommandRegistry should be a Map-backed class in shared/dynamic-registry.js
- ExecHandler already accepts commandRegistry via DI — extend lookup to check dynamic first, static second
- Binary allowlist: node, git, pm2, cargo, systemctl, curl, sqlite3, taskkill, shutdown, net, wmic
- Per-command env injection uses allowedEnvKeys field — values come from local process.env, not from payload
- New message types go into shared/protocol.js alongside existing MessageType constants
- Registry persistence via JSON file in data/ directory
- Fix unbounded completedExecs Set (add LRU eviction or max-size cap) as tech debt cleanup

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- shared/exec-protocol.js — COMMAND_REGISTRY (frozen), ApprovalTier, buildSafeEnv(), validateExecRequest()
- shared/protocol.js — MessageType enum, CONTROL_TYPES, createMessage(), parseMessage()
- james/exec-handler.js — ExecHandler class with commandRegistry DI injection point
- shared/ack-tracker.js — reliable delivery for new message types
- shared/message-queue.js — WAL-backed durable queue

### Established Patterns
- Frozen object registries with DI injection (ExecHandler constructor)
- Message types as string constants in protocol.js
- createMessage(type, target, payload) factory function
- execFile with shell:false + sanitized env for all command execution

### Integration Points
- james/index.js — ExecHandler instantiation (inject merged registry)
- bono/index.js — ExecHandler instantiation (inject merged registry)
- shared/protocol.js — add new MessageType constants
- james/index.js WS message handler — route new message types

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
