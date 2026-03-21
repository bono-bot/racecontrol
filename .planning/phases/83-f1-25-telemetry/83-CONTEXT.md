# Phase 83: F1 25 Telemetry - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

F1 25 lap times and sector splits captured from UDP telemetry and emitted as LapCompleted events. The F1 25 SimAdapter already exists and appears complete — this phase verifies it meets requirements, adds unit tests if missing, and closes.

</domain>

<decisions>
## Implementation Decisions

### Scope
- The F1 25 adapter (`sims/f1_25.rs`) already exists with full UDP parsing (packets 1, 2, 4, 6, 7)
- `poll_lap_completed()` already returns `LapData` with `sim_type: F125`, sector splits, track, car
- `event_loop.rs` already polls the adapter and emits `AgentMessage::LapCompleted`
- Phase 83 = verify existing code satisfies TEL-F1-01/02/03, add tests if missing, mark complete
- No new features or adapter code needed — this is a verification + test gap closure phase

### Claude's Discretion
- Whether to add unit tests for packet parsing
- Whether to add integration test for LapCompleted emission
- How to handle the PlayableSignal integration (Phase 82 already wired F1 25 via UdpActive)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### F1 25 Adapter
- `crates/rc-agent/src/sims/f1_25.rs` — Complete F1 25 UDP adapter: packet parsing, LapData construction, poll_lap_completed()
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait definition
- `crates/rc-agent/src/event_loop.rs` — poll_lap_completed() call site (line 174), AgentMessage::LapCompleted emission

### Shared Types
- `crates/rc-common/src/types.rs` — LapData struct, SimType::F125, SessionInfo, SessionType

</canonical_refs>

<code_context>
## Existing Code Insights

### Already Complete
- F125Adapter: UDP socket bind, 5 packet types parsed, LapData with sectors, track name mapping, car team mapping
- event_loop.rs: polls adapter every telemetry_interval, emits LapCompleted
- PlayableSignal: Phase 82 wired F1 25 billing via UdpActive from DrivingDetector

### Integration Points
- LapCompleted flows: adapter → event_loop → AgentMessage → WebSocket → racecontrol → lap_tracker → DB

</code_context>

<specifics>
## Specific Ideas

- This is essentially a "validate prior work" phase — the adapter was built before v13.0 was scoped
- Focus on confirming the adapter works correctly, not building new functionality

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 83-f1-25-telemetry*
*Context gathered: 2026-03-21*
