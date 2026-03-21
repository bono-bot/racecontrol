# Phase 134: Advanced Chain Features + Integration Hardening - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add advanced chain features: named chain templates from config file (chains.json), output templating ({{prev_stdout}} substitution in step args), per-step retry with configurable count and backoff, chain state persistence for pause/resume across WS disconnects, and registry introspection endpoint (GET /relay/commands).

All implementation in C:/Users/bono/racingpoint/comms-link (separate repo from racecontrol).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance from research:
- Chain templates: comms-link/chains.json loaded at startup. Invocable by template name in chain_request payload.
- Output templating: {{prev_stdout}} in step args substituted by ChainOrchestrator before sending exec_request. Strip metacharacters from substituted values (sanitization).
- Per-step retry: { retries: N, retryBackoffMs: M } per step. Retry uses new execId (existing dedup is execId-keyed). ChainOrchestrator handles retry logic internally.
- Chain pause/resume: serialize chain state (step index + accumulated results) to chain-state.json on WS disconnect. Resume from interrupted step on reconnect. HIGH complexity — keep implementation simple.
- Registry introspection: GET /relay/commands returns { commands: [{name, description, tier, timeoutMs}] } from merged static + dynamic registry. Binary and args never exposed. Both James and Bono sides.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- shared/chain-orchestrator.js — ChainOrchestrator.execute() (extend with templates, templating, retry)
- shared/dynamic-registry.js — DynamicCommandRegistry (for introspection endpoint)
- shared/exec-protocol.js — COMMAND_REGISTRY (for introspection)
- james/index.js — HTTP relay server (add /relay/commands endpoint)
- shared/connection-mode.js — ConnectionMode states for pause/resume trigger

### Integration Points
- ChainOrchestrator — extend execute() for templates, templating, retry
- james/index.js — add GET /relay/commands, chain state persistence hooks
- bono/index.js — same introspection endpoint + chain state hooks

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
