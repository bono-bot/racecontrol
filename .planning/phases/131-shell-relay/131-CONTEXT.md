# Phase 131: Shell Relay - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a separate ShellRelayHandler that accepts arbitrary binary+args from the remote side, enforces APPROVE-only tier with WhatsApp notification to Uday showing the full command, validates binary against the same ALLOWED_BINARIES list from DynamicCommandRegistry, and executes via execFile with shell:false + sanitized env.

All implementation in C:/Users/bono/racingpoint/comms-link (separate repo from racecontrol).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance from research:
- Shell relay MUST be a SEPARATE handler from ExecHandler — never share a code path with AUTO/NOTIFY tier
- Special command name `__shell_relay` in exec_request payload, with binary/args/cwd in payload fields
- Binary must be in ALLOWED_BINARIES (imported from shared/dynamic-registry.js)
- Tier is HARDCODED APPROVE — payload tier value is ignored
- Notification to Uday must include full "binary args" string, not just command name
- Uses same sanitized env (buildSafeEnv) + execFile(shell:false) as static commands
- Both James and Bono sides need the shell relay handler
- Approval flow reuses existing ExecHandler.queueForApproval() or similar APPROVE pattern

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- shared/dynamic-registry.js — ALLOWED_BINARIES set (frozen)
- shared/exec-protocol.js — buildSafeEnv(), ApprovalTier
- james/exec-handler.js — ExecHandler with queueForApproval(), approveCommand(), rejectCommand() pattern
- james/index.js — WS message routing, HTTP relay endpoints, notifyFn wiring

### Established Patterns
- ExecHandler 3-tier routing: auto → execute, notify → execute + notify, approve → queue + notify + await
- execFile with shell:false + sanitized env for all command execution
- Structured exec_result: { command, exitCode, stdout, stderr, durationMs, truncated, tier }

### Integration Points
- james/index.js WS message handler — route __shell_relay exec_request to ShellRelayHandler
- bono/index.js WS message handler — same routing
- Existing notifyFn (WhatsApp) — pass full command text

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following research recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
