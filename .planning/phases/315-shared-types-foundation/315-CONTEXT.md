# Phase 315: Shared Types Foundation - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

All rc-common shared types needed by v41.0 are defined with correct serde compatibility before any crate uses them. This includes new DiagnosticTrigger variants (GameLaunchTimeout, CrashLoop) with #[serde(other)] backward compat, new AgentMessage variants (GameInventoryUpdate, ComboValidationResult, LaunchTimelineReport), and tier_engine match arms for new triggers.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from research:
- Add `#[serde(other)]` Unknown variant to DiagnosticTrigger BEFORE adding new variants (backward compat with old KB entries)
- GameLaunchTimeout routes to Tier 1 (Game Doctor: kill stale process + retry once)
- CrashLoop routes to Tier 0 (hardened rule: disable combo + escalate via EscalationRequest)
- New AgentMessage variants must be backward compat — old server should ignore unknown variants

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/rc-common/src/mesh_types.rs` — DiagnosticTrigger enum, DiagnosticSession, MeshSolution
- `crates/rc-common/src/protocol.rs` — AgentMessage, CoreToAgentMessage enums
- `crates/rc-common/src/types.rs` — PodInfo (installed_games field exists but unpopulated), GamePreset, GamePresetWithReliability
- `crates/rc-agent/src/tier_engine.rs` — tier1_deterministic() match on DiagnosticTrigger
- `crates/rc-agent/src/game_doctor.rs` — GameFailureCause, diagnose_and_fix()
- `crates/rc-agent/src/game_launch_retry.rs` — RetryResult, retry_game_launch()

### Established Patterns
- DiagnosticTrigger variants are matched in tier_engine.rs tier1_deterministic()
- AgentMessage variants are serialized via serde JSON over WebSocket
- Standing rule: no .unwrap() in production, no lock held across .await

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
