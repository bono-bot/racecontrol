# Phase 160: RC-Sentry AI Migration - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace rc-sentry's blind 5s health poll + restart with AI-driven recovery: pattern memory checks before restarting, Ollama query for unknown patterns, every decision logged, graceful restart detection via sentinel file.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- rc-sentry already has: RestartTracker (tier1_fixes.rs:26-90), debug_memory.rs (DebugMemory), tier1_fixes.rs (handle_crash)
- Phase 159 delivered: RecoveryAuthority, ProcessOwnership, RecoveryLogger, CascadeGuard — rc-sentry must use these
- rc-sentry's Ollama integration already exists (ollama.rs) — enhance, don't replace
- RCAGENT_SELF_RESTART writes sentinel file `C:\RacingPoint\rcagent-restart-sentinel.txt` (added in Phase 140)
- rc-sentry must check for sentinel file before escalating — graceful restart ≠ crash
- Pattern memory: debug-memory-sentry.json already exists on pods
- Must not slow down real crash recovery — AI escalation is async, restart proceeds if Ollama is slow

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-sentry/src/tier1_fixes.rs: RestartTracker, handle_crash(), restart_service()
- rc-sentry/src/debug_memory.rs: DebugMemory (pattern → fix_summary → hit_count)
- rc-sentry/src/ollama.rs: query_async() — fire-and-forget Ollama query
- rc-sentry/src/watchdog.rs: WatchdogState FSM (Healthy → Suspect → Crashed)
- rc-common/src/recovery.rs: RecoveryAuthority, RecoveryLogger, RecoveryDecision (Phase 159)

### Integration Points
- rc-sentry/src/main.rs: crash handler (lines 119-198) — wire pattern memory check + recovery logging
- rc-sentry/src/tier1_fixes.rs: handle_crash() — add pattern check before restart
- rc-sentry/src/watchdog.rs: crash detection — add sentinel file check

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
