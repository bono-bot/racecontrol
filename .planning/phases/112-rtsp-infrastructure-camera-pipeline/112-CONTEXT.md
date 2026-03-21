# Phase 112: RTSP Infrastructure & Camera Pipeline - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Set up RTSP relay infrastructure on James (.27) to proxy Dahua camera streams, create the rc-sentry-ai crate for frame extraction, implement stream health monitoring with auto-reconnect, and migrate the existing people tracker to use the relay.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase. Key constraints from project context:
- Dahua cameras at entrance (.8) and reception (.15, .154) with RTSP subtype=1, auth admin/Admin@123
- NVR at .18 -- do not disrupt its recording
- Existing people tracker at :8095 (YOLOv8 + FastAPI) must continue working
- New service on :8096 on James (.27)
- Research recommends go2rtc or mediamtx for RTSP relay, retina crate for Rust RTSP
- RTX 4070 available but not needed for this phase (frame extraction only)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol monorepo with Cargo workspace (crates/racecontrol, crates/rc-agent, crates/rc-common)
- Existing rc-common for shared types
- People tracker at :8095 already consumes RTSP streams directly

### Established Patterns
- Axum-based HTTP services (racecontrol :8080, rc-agent :8090)
- Tokio async runtime throughout
- SQLite for persistent storage
- TOML config files (racecontrol.toml, rc-agent.toml)

### Integration Points
- New crate: crates/rc-sentry-ai/ in Cargo workspace
- Health endpoint at :8096
- People tracker at :8095 needs to switch RTSP source to relay

</code_context>

<specifics>
## Specific Ideas

No specific requirements -- infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>
