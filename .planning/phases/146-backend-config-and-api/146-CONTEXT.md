# Phase 146: Backend Config and API - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend rc-sentry-ai config and API to serve complete camera metadata (display_name, display_order, nvr_channel, zone) and persist layout preferences via a server-side camera-layout.json file with GET/PUT endpoints.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key decisions to make during execution:
- CameraConfig struct extension (add display_name, display_order, zone fields)
- /api/v1/cameras response shape extension (include new fields)
- camera-layout.json file location and atomic write strategy
- PUT /api/v1/cameras/layout request/response format
- Whether to use serde_json for layout persistence or a dedicated approach

### Locked Constraints
- rc-sentry-ai.toml is NEVER written at runtime — layout state goes to camera-layout.json only
- Camera names come from TOML config (venue constants), not user input
- Build with dynamic CRT: RUSTFLAGS="-C target-feature=-crt-static"
- No .unwrap() in production Rust — use ?, .ok(), or match

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- CameraConfig in config.rs — already has name, stream_name, role, fps, nvr_channel
- MjpegState in mjpeg.rs — shared state for camera endpoints
- cameras_list_handler in mjpeg.rs — existing /api/v1/cameras endpoint
- nvr_snapshot_handler in mjpeg.rs — existing snapshot proxy

### Established Patterns
- Axum Router with Arc<State> pattern (health, enrollment, attendance, alerts, mjpeg, playback)
- TOML config loaded at startup via Config::load()
- tower-http CORS layer on each router
- serde Serialize/Deserialize for all request/response types

### Integration Points
- config.rs: CameraConfig struct needs display_name, display_order, zone fields
- mjpeg.rs: cameras_list_handler needs to return extended fields
- mjpeg.rs: new layout GET/PUT routes
- main.rs: pass layout state to MjpegState
- rc-sentry-ai.toml: add display_name, display_order, zone to each [[cameras]] entry

</code_context>

<specifics>
## Specific Ideas

- Camera names should be descriptive venue locations: "Entrance", "Reception", "Pod Area", "Cashier", etc.
- Zone grouping: entrance, reception, pods, other
- camera-layout.json stores: { grid_mode: "3x3", camera_order: [1,3,4,...], zone_filter: null }
- Layout endpoint must survive rc-sentry-ai restart (file-based, not in-memory)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
