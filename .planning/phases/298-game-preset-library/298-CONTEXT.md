# Phase 298: Game Preset Library - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Game presets are server-managed, pushed to pods, and flagged when their launch reliability is poor. Server stores car/track/session presets in SQLite, pushes them to pods via the config channel, and tracks reliability scores.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase.

Key observations:
- Existing game catalog in crates/racecontrol/src/catalog.rs
- Games config already in AgentConfig via GamesConfig struct
- Presets are car/track/session combos with a name and game type
- Reliability scores derive from game launch success/failure in billing/session data
- Push via FullConfigPush channel (Phase 296) or separate WS message
- Kiosk already shows game selection — presets could feed into that

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/catalog.rs` — game catalog
- `crates/rc-common/src/types.rs` — SimType, game-related types
- `crates/racecontrol/src/config_push.rs` — push infrastructure (Phase 296)
- `crates/racecontrol/src/db/mod.rs` — SQLite table creation pattern

### Integration Points
- New SQLite table: game_presets
- Push presets alongside config on WS connect
- Kiosk game selection could reference presets

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
