# Phase 133: Task Delegation + Audit Trail - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Build transparent Claude-to-Claude delegation: when James needs something done on Bono's machine (or vice versa), it sends a delegate_request (which is a chain_request wrapped with delegation metadata), the remote side executes via ChainOrchestrator, and returns a delegate_result that the requesting side integrates transparently. Also build an append-only JSONL audit log that captures every remote execution on both machines.

All implementation in C:/Users/bono/racingpoint/comms-link (separate repo from racecontrol).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance from research:
- Delegation uses existing chain_request/chain_result flow — the delegate_request wraps a chain with delegation metadata (requestedBy, reason, transparent flag)
- task_request/task_response already exist but carry no result data — extend or add new delegate_request/delegate_result types
- Result must include [REMOTE DATA] envelope to structurally separate exec output from AI instructions (prompt injection prevention)
- Delegation works symmetrically: James→Bono and Bono→James
- "Transparent" means the requesting AI integrates the response without exposing relay scaffolding to the user
- Audit log: data/exec-audit.jsonl — append-only, one JSON line per execution
- Audit entries: { ts, execId, command, from, to, exitCode, durationMs, tier, chainId?, stepIndex? }
- Both machines write their own audit entries (both requester and executor log)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- shared/chain-orchestrator.js — ChainOrchestrator.execute() (Phase 132)
- shared/exec-result-broker.js — ExecResultBroker.waitFor() (Phase 132)
- shared/protocol.js — MessageType.chain_request, chain_result already defined
- james/index.js — sendTaskRequest(), pendingTasks Map (existing task tracking)
- james/failover-orchestrator.js — proves the broker.waitFor() pattern works

### Established Patterns
- Promise-based exec result awaiting via ExecResultBroker
- WS message routing in james/index.js and bono/index.js
- createMessage(type, target, payload) for all WS messages

### Integration Points
- james/index.js — add delegation handler, audit log calls
- bono/index.js — same delegation handler + audit logging
- Existing exec_request/exec_result/chain_request/chain_result handlers — add audit log calls

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
