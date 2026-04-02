# Phase 295: Config Schema & Validation - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

A typed, versioned AgentConfig struct is the single source of truth for all pod-level configuration. The struct lives in rc-common so both rc-agent and racecontrol can use it. Invalid fields fall back to defaults with warnings. Schema version enables forward compatibility.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key observations from codebase scout:
- Existing AgentConfig in rc-agent/src/config.rs uses `#[serde(deny_unknown_fields)]` — must change to allow unknown fields with warnings
- AgentConfig currently only derives Debug + Deserialize — needs Serialize for WS push (Phase 296)
- Sub-configs (PodConfig, CoreConfig, etc.) need Clone + Serialize too
- validate_config() already exists — extend with schema_version validation
- config_search_paths() handles TOML loading — keep in rc-agent (agent-specific logic)
- rc-common already has types.rs, mesh_types.rs — config_schema.rs fits the pattern

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/config.rs` — full AgentConfig with 12 sub-structs, validation, TOML loading
- `crates/rc-common/src/types.rs` — shared types (SimType, etc.)
- `crates/rc-common/src/verification.rs` — ColdVerificationChain for config loading

### Established Patterns
- serde defaults via `#[serde(default)]` and `#[serde(default = "fn_name")]`
- Sub-config structs implement Default manually
- Validation collects all errors then returns (not fail-fast)
- TOML config files at `C:\RacingPoint\rc-agent.toml`

### Integration Points
- rc-agent main.rs loads config via config_search_paths()
- racecontrol will need AgentConfig for Phase 296 (server-pushed config)
- WS handler will send config as JSON

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
