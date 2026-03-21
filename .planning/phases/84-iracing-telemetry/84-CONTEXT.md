# Phase 84: iRacing Telemetry - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

iRacing shared memory reader that captures lap times and sector splits, handles session transitions cleanly, and includes a pre-flight check for irsdkEnableMem=1. Emits LapCompleted events with sim_type=IRacing.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion (all areas)
- iRacing shared memory mapped file name: `Local\\IRSDKMemMapFileName`
- Use same `winapi::OpenFileMappingW` + `MapViewOfFile` pattern as AC adapter in `sims/assetto_corsa.rs`
- Double-buffer tick synchronization for reading shared memory safely
- YAML session info string parsing for track name, car, session type
- Session transition handling: re-open shared memory handle when session UID changes
- Pre-flight check: read `app.ini` for `irsdkEnableMem=1`, warn staff via tracing if missing (don't block launch)
- PlayableSignal integration: once this adapter exists, it replaces the 90s process fallback from Phase 82
- `poll_lap_completed()` returns `LapData` with `sim_type: SimType::IRacing`
- Follow F1 25 adapter structure: struct fields for tracking state, packet parsing methods, SimAdapter trait impl

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Adapters (patterns to follow)
- `crates/rc-agent/src/sims/assetto_corsa.rs` — Shared memory pattern: OpenFileMappingW, MapViewOfFile, struct layout, connect/disconnect lifecycle
- `crates/rc-agent/src/sims/f1_25.rs` — UDP adapter pattern: LapData construction, poll_lap_completed(), session_info(), sector tracking
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait definition

### Integration Points
- `crates/rc-agent/src/event_loop.rs` — poll_lap_completed() call site (line 174), adapter selection logic
- `crates/rc-agent/src/driving_detector.rs` — DetectorSignal for PlayableSignal integration
- `crates/rc-common/src/types.rs` — LapData, SimType::IRacing, SessionInfo, DrivingState

### Research
- `../game-launcher/.planning/research/STACK.md` — iRacing shared memory protocol details
- `../game-launcher/.planning/research/PITFALLS.md` — iRacing handle re-open on session transitions, irsdkEnableMem requirement

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- AC adapter's shared memory read pattern (OpenFileMappingW + MapViewOfFile + struct casting)
- F1 25 adapter's LapData construction and poll_lap_completed() take semantics
- SimAdapter trait with connect/disconnect/read_telemetry/poll_lap_completed/session_info

### Established Patterns
- Adapters live in `crates/rc-agent/src/sims/{game}.rs`
- Registered in `sims/mod.rs`
- event_loop.rs selects adapter based on current sim_type
- LapCompleted flows: adapter → event_loop → AgentMessage → WebSocket → racecontrol → lap_tracker

### Integration Points
- `sims/mod.rs` needs `pub mod iracing;` added
- event_loop.rs adapter selection needs IRacing arm
- Phase 82's process fallback should be replaced by iRacing's shared memory IsOnTrack signal for PlayableSignal

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow AC adapter pattern for shared memory, F1 25 pattern for LapData construction.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 84-iracing-telemetry*
*Context gathered: 2026-03-21*
