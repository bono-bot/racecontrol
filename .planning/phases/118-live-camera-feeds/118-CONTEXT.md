# Phase 118: Live Camera Feeds - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add MJPEG proxy endpoint to rc-sentry-ai that serves live camera frames from go2rtc relay to browser clients. Dashboard UI component for viewing feeds.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase.

Key constraints:
- go2rtc at :1984 already provides MJPEG snapshots via /api/frame.jpeg?src={stream}
- rc-sentry-ai FrameBuffer already has decoded frames
- Axum server at :8096 — add MJPEG streaming endpoint
- MJPEG renders in browser <img> tag natively — no video player library needed
- Must not degrade face detection performance — frame serving independent of AI pipeline
- Under 2-second latency requirement

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- go2rtc HTTP API at :1984 — can proxy /api/frame.jpeg snapshots
- frame.rs: FrameBuffer with latest decoded frames per camera
- health.rs: Existing Axum routes to extend
- config.rs: Camera names and stream configuration

### Integration Points
- New /api/v1/cameras/:name/stream MJPEG endpoint on :8096
- Or proxy directly from go2rtc's existing snapshot API

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
