# Phase 85: LMU Telemetry - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Le Mans Ultimate shared memory reader using rFactor 2 shared memory plugin. Captures lap times and sector splits, emits LapCompleted events with sim_type=LMU.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion (all areas)
- rFactor 2 shared memory mapped files: `$rFactor2SMMP_Telemetry$`, `$rFactor2SMMP_Scoring$`
- Use same `winapi::OpenFileMappingW` + `MapViewOfFile` pattern as iRacing and AC adapters
- rF2 Scoring struct has lap times and sector splits (unlike iRacing which lacks sectors)
- Session transition handling: detect session change via scoring data, reset lap tracking
- PlayableSignal: use rF2 driving flag from telemetry (replaces 90s process fallback)
- `poll_lap_completed()` returns `LapData` with `sim_type: SimType::LeMansUltimate`
- Follow iRacing adapter structure closely — same shared memory pattern, similar wiring
- `read_is_on_track()` trait override inside `impl SimAdapter` (not inherent method — learned from Phase 84 checker)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Adapters (patterns to follow)
- `crates/rc-agent/src/sims/iracing.rs` — Closest pattern: shared memory, variable lookup, session transitions, trait dispatch
- `crates/rc-agent/src/sims/assetto_corsa.rs` — Original shared memory pattern
- `crates/rc-agent/src/sims/f1_25.rs` — LapData construction pattern
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait with read_is_on_track() default

### Integration Points
- `crates/rc-agent/src/event_loop.rs` — PlayableSignal dispatch, poll_lap_completed()
- `crates/rc-agent/src/main.rs` — Adapter creation match arm
- `crates/rc-common/src/types.rs` — SimType::LeMansUltimate, LapData

### Research
- `../game-launcher/.planning/research/STACK.md` — rFactor 2 shared memory plugin details

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- iRacing adapter's `ShmHandle` pattern for shared memory lifecycle
- iRacing adapter's variable lookup approach (adapted for rF2 fixed struct layout)
- SimAdapter trait with `read_is_on_track()` already has default `None`

### Integration Points
- `sims/mod.rs` needs `pub mod lmu;` added
- `main.rs` needs `SimType::LeMansUltimate` match arm
- `event_loop.rs` needs LMU PlayableSignal arm (replace process fallback)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow iRacing adapter pattern. rF2 has sector data unlike iRacing, so sector1/2/3_ms should be populated.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 85-lmu-telemetry*
*Context gathered: 2026-03-21*
