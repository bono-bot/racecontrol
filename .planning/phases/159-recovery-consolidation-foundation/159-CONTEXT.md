# Phase 159: Recovery Consolidation Foundation - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish the recovery authority pattern: a registry mapping each process to exactly one recovery owner, a decision log for all restart/kill/wake actions, and an anti-cascade guard that pauses all recovery when 3+ actions fire within 60s.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Standing rule #10: recovery systems must not fight each other
- The registry must be checked by rc-sentry, pod_monitor, pod_healer, and James watchdog before acting
- Decision log must be append-only JSONL for reliability
- Anti-cascade guard must distinguish "server down, all pods restart" (normal) from "3 different systems restarting the same process" (cascade)
- This phase creates the FRAMEWORK — phases 160-162 wire existing systems into it

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-sentry/src/tier1_fixes.rs: RestartTracker (lines 26-90) — tracks restarts in 10min window, already has escalation
- rc-sentry/src/debug_memory.rs: DebugMemory — pattern memory persistence
- racecontrol/src/pod_healer.rs: HealAction enum — existing action types
- racecontrol/src/state.rs: AppState — server-side shared state
- rc-common/src/protocol.rs: AgentMessage variants for pod→server communication

### Integration Points
- rc-common: shared RecoveryAuthority enum + RecoveryDecision struct (used by all crates)
- racecontrol/src/pod_healer.rs: check authority before healing
- rc-sentry/src/tier1_fixes.rs: check authority before restarting
- New: recovery-log.jsonl on each machine

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
