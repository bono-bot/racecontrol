# Phase 187: self_monitor Coordination - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Changes to rc-agent's self_monitor module: check rc-sentry availability before relaunch, write GRACEFUL_RELAUNCH sentinel and exit if sentry alive, keep PowerShell fallback for when sentry is dead. Changes to rc-agent crate (self_monitor.rs or equivalent).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key constraints from ROADMAP success criteria:
- self_monitor checks rc-sentry on TCP :8091 before relaunch
- If sentry reachable: write GRACEFUL_RELAUNCH sentinel, exit cleanly — let sentry handle restart
- If sentry unreachable: fall back to existing PowerShell+DETACHED_PROCESS path
- No orphan powershell.exe after restart when sentry is up
- Three-state verification on Pod 8: (a) sentry up + kill agent, (b) sentry down + kill agent, (c) sentry down + kill agent + restart sentry
- GRACEFUL_RELAUNCH sentinel prevents port :8090 double-bind race condition

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rc-agent/src/self_monitor.rs` or equivalent — existing relaunch_self() using PowerShell+DETACHED_PROCESS
- `rc-common/src/recovery.rs` — GRACEFUL_RELAUNCH sentinel constants
- `tier1_fixes.rs` (rc-sentry) — GRACEFUL_RELAUNCH_SENTINEL constant path

### Established Patterns
- self_monitor detects crash via internal health check or panic hook
- relaunch_self() spawns PowerShell to run start-rcagent.bat
- PowerShell+DETACHED_PROCESS is the only proven working self-restart on Windows (4 alternatives tested, all failed)

### Integration Points
- TCP check to localhost:8091 to detect rc-sentry availability
- GRACEFUL_RELAUNCH file at C:\RacingPoint\GRACEFUL_RELAUNCH
- rc-sentry's watchdog (Phase 184) already checks this sentinel before acting

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
