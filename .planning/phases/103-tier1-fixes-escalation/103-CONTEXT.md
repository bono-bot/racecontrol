# Phase 103: Tier 1 Fixes + Escalation FSM - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add deterministic fix functions to rc-sentry (zombie kill, port wait, socket clean, config repair, shader cache clear) + EscalatingBackoff for restart cooldown + maintenance mode after 3 restarts in 10 minutes. Wire into the crash handler from Phase 102.

Requirements: FIX-01 through FIX-06, ESC-01, ESC-02

</domain>

<decisions>
## Implementation Decisions

### Fix Sequence
- On crash: kill zombies → wait for port 8090 free → apply context-specific fixes → restart rc-agent
- Port wait: poll netstat for 8090 TIME_WAIT, up to 10s
- Restart: cmd /C start "" "C:\RacingPoint\start-rcagent.bat"

### Escalation
- Reuse rc_common::backoff::EscalatingBackoff (already exists)
- 3+ restarts within 10 minutes → write MAINTENANCE_MODE flag file → no more restarts
- MAINTENANCE_MODE file: C:\RacingPoint\MAINTENANCE_MODE

### Test Guards
- ALL fix functions MUST have #[cfg(test)] guards returning mock results
- Standing rule from today's debugging session — never execute real system commands during cargo test

### Claude's Discretion
- Module structure (single tier1_fixes.rs or split)
- Exact netstat parsing for port wait
- MAINTENANCE_MODE file format

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-sentry watchdog.rs (Phase 102): CrashContext, mpsc channel
- rc-common backoff: EscalatingBackoff with configurable steps
- ai_debugger.rs fix patterns: hidden_cmd(), fix_stale_sockets(), fix_kill_error_dialogs() — reference patterns

### Integration Points
- Crash handler thread in main.rs receives CrashContext via channel
- Fix functions consume CrashContext, produce CrashDiagResult (rc-common Phase 101)
- EscalatingBackoff tracks restart timing

</code_context>

<specifics>
## Specific Ideas

No specific requirements — clear design from research.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
