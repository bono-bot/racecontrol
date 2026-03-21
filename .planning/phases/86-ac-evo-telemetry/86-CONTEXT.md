# Phase 86: AC EVO Telemetry - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Best-effort AC EVO shared memory reader using ACC struct layout. Feature-flagged. Graceful degradation — if telemetry fields are unpopulated or the API changes, log warning and continue without crashing. Game launch and billing still work regardless.

</domain>

<decisions>
## Implementation Decisions

### Approach
- **Reuse existing AC adapter's shared memory struct layout** (ACC format: `acpmf_physics`, `acpmf_graphics`, `acpmf_static`)
- AC EVO is built on ACC's engine — likely uses same or similar shared memory layout
- If fields are populated: extract lap times, sector splits, emit LapCompleted
- If fields are empty/zero: log warning, adapter reports no telemetry, billing continues via process fallback
- Feature-flagged: adapter only activates when `SimType::AssettoCorsaEvo` is the current game

### Graceful Degradation (TEL-EVO-02)
- Never panic on unpopulated fields — all reads check for zero/default values
- If shared memory map doesn't exist (EVO doesn't use ACC format): `connect()` returns Ok but `is_connected = false`
- Tracing warns once per session, not per poll cycle
- Game launch and billing work regardless of telemetry success

### Claude's Discretion (all implementation details)
- Whether to create a new EVO-specific adapter file or extend the existing AC adapter with EVO support
- Exact shared memory map names to try (ACC names vs potential EVO-specific names)
- PlayableSignal: use physics data (non-zero speed/RPM) or separate IsOnTrack equivalent
- `read_is_on_track()` trait override for EVO
- How to distinguish AC1 vs EVO if both use similar shared memory names

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing AC Adapter (source of truth for struct layout)
- `crates/rc-agent/src/sims/assetto_corsa.rs` — ACC shared memory: acpmf_physics, acpmf_graphics, acpmf_static. Struct offsets, connect/disconnect, lap detection
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait

### Integration Points
- `crates/rc-agent/src/event_loop.rs` — PlayableSignal dispatch, poll_lap_completed()
- `crates/rc-agent/src/main.rs` — Adapter creation match arm
- `crates/rc-common/src/types.rs` — SimType::AssettoCorsaEvo

### Research
- `../game-launcher/.planning/research/PITFALLS.md` — AC EVO telemetry is incomplete and unstable
- `../game-launcher/.planning/research/STACK.md` — ACC shared memory structs, EVO may differ

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- AC adapter's entire shared memory pattern (struct offsets, ShmHandle, connect/disconnect)
- Same `OpenFileMappingW` + `MapViewOfFile` pattern used by iRacing and LMU adapters
- SimAdapter trait with `read_is_on_track()` default

### Key Risk
- EVO may use different shared memory map names than ACC
- EVO may only populate physics struct (not graphics/static)
- Struct field order/size may differ between ACC and EVO

</code_context>

<specifics>
## Specific Ideas

- This is deliberately best-effort — if it works, bonus. If not, the adapter silently degrades
- Feature-flagged means it can be disabled without code changes if EVO breaks it

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 86-ac-evo-telemetry*
*Context gathered: 2026-03-21*
