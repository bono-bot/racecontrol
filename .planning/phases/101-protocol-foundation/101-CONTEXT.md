# Phase 101: Protocol Foundation - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add new types and AgentMessage variants to rc-common for the process guard system. This phase delivers compile-time dependencies that rc-agent (Phase 103) and racecontrol (Phase 104) will reference. No enforcement logic, no config, no deployment — just shared types.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints from research:
- Follow existing AgentMessage variant patterns in protocol.rs (15+ existing variants)
- MachineWhitelist, ProcessViolation, ViolationType must derive Serialize, Deserialize, Clone, Debug
- Add CoreToAgentMessage::UpdateProcessWhitelist for server-to-pod whitelist push
- AgentMessage::ProcessGuardStatus for periodic guard health reporting
- Do NOT add windows or winapi crates in this phase — types only
- Severity tiers: Kill, Escalate, Monitor (maps to PROC-05)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-common/src/protocol.rs — AgentMessage enum with 15+ variants, all #[serde(tag/content)]
- Existing patterns: PreFlightResult, PodFleetStatus, ExecResponse — all serializable structs
- rc-common/src/lib.rs — pub mod exports

### Established Patterns
- All protocol types derive Serialize, Deserialize, Clone, Debug
- AgentMessage uses #[serde(tag = "type", content = "payload")]
- CoreToAgentMessage is the server-to-agent direction
- Types are simple structs with pub fields — no builder pattern

### Integration Points
- rc-agent imports from rc-common via workspace dependency
- racecontrol imports from rc-common via workspace dependency
- Both must compile unchanged after adding new variants (backward compat via serde defaults)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
