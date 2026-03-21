# Phase 132: Chain Orchestration - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Build ChainOrchestrator — a class that accepts an array of command steps, executes them sequentially via exec_request/exec_result over the existing WebSocket, passes each step's stdout to the next, supports abort-on-failure (default) with per-step continue_on_error override, enforces chain-level timeout, and returns a single structured chain_result message with all step outputs.

Also build ExecResultBroker — a shared pending-promise pattern (extracted from FailoverOrchestrator) that routes exec_result messages to the correct awaiting handler (ExecHandler, ShellRelayHandler, or ChainOrchestrator) without them competing for the same message.

All implementation in C:/Users/bono/racingpoint/comms-link (separate repo from racecontrol).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance from research:
- ExecResultBroker extracts the pending-promise pattern from FailoverOrchestrator's #pending Map
- ChainOrchestrator is a NEW class, NOT an extension of ExecHandler
- Chain steps use existing exec_request/exec_result message types (no new transport)
- chain_request and chain_result MessageTypes already defined in Phase 130
- Each step sends an exec_request with a unique execId, awaits the matching exec_result
- Default abort_on_failure: true. Per-step continue_on_error: true overrides
- chain_result: { chainId, steps: [{command, exitCode, stdout, stderr, durationMs}], totalDurationMs, aborted, abortReason? }
- Chain-level timeout (chainTimeoutMs) caps entire chain duration
- Both James and Bono sides need chain orchestrator wiring

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- james/failover-orchestrator.js — #pending Map + handleExecResult() + timeout-rejection pattern (extract into ExecResultBroker)
- shared/protocol.js — MessageType.chain_request, chain_step_ack, chain_result already defined (Phase 130)
- james/exec-handler.js — ExecHandler with handleExecRequest() for executing individual steps
- shared/exec-protocol.js — COMMAND_REGISTRY, validateExecRequest()

### Established Patterns
- Promise-based exec result awaiting (FailoverOrchestrator)
- createMessage(type, target, payload) for all WS messages
- Structured exec_result: { command, exitCode, stdout, stderr, durationMs, truncated, tier }

### Integration Points
- james/index.js — WS message routing for chain_request, chain_result
- bono/index.js — same WS routing
- ExecHandler — receives individual exec_request steps from ChainOrchestrator
- james/failover-orchestrator.js — may need refactoring to use shared ExecResultBroker

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
