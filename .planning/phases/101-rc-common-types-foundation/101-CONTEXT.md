# Phase 101: rc-common Types Foundation - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add SentryCrashReport and CrashDiagResult structs to rc-common so both rc-sentry and racecontrol can import them. Library-only change — no binary deploy needed.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-common/src/types.rs — all shared types live here (PodFailureReason, AgentMessage, etc.)
- rc-common/src/protocol.rs — protocol types with serde Serialize/Deserialize

### Established Patterns
- All shared types derive Debug, Clone, Serialize, Deserialize
- Optional fields use `#[serde(default)]` for backward compat
- Types have unit tests in the same file

### Integration Points
- rc-sentry imports rc-common for shared types
- racecontrol imports rc-common for protocol + types
- rc-agent imports rc-common for protocol + types + exec

</code_context>

<specifics>
## Specific Ideas

From research: SentryCrashReport should contain pod_id, crash_timestamp, crash_context (log excerpt), fix_applied, fix_result, restart_count, escalated. CrashDiagResult should contain fix_type, detail, success (same pattern as AutoFixResult in ai_debugger.rs).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
