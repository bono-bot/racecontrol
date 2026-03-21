# Phase 117: Alerts & Notifications - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add real-time alert system: WebSocket notifications to racecontrol dashboard for attendance events, Windows toast notifications with sound on James machine for entrance detections, and unknown person alert pipeline with face crop saving and rate limiting.

</domain>

<decisions>
## Implementation Decisions

### Alert Channels
- Dashboard notifications via WebSocket event (JSON with type, person, camera, timestamp) — connect to racecontrol's existing WS infrastructure
- Desktop popup on James via `winrt-toast` crate — native Windows 11 toast notifications
- Sound: Windows system sound (SystemDefault) on entrance camera detection

### Unknown Person Handling
- "Unknown" triggered when face detected but no gallery match above 0.45 threshold
- Rate-limit unknown alerts to once per 5 minutes per camera (avoid spam)
- Save face crop as JPEG to disk (C:\RacingPoint\face-crops\), include path in alert event

### Claude's Discretion
- WebSocket server implementation details (port, auth)
- Toast notification content layout
- Face crop JPEG quality and naming convention
- Alert event JSON schema
- How alerts subscribe to recognition broadcast

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- detection/pipeline.rs: broadcasts RecognitionResult via tokio::broadcast
- attendance/engine.rs: already subscribes to broadcast — pattern to follow
- recognition/types.rs: RecognitionResult with person_id, confidence, camera
- Axum server at :8096 — can add WebSocket upgrade endpoint

### Established Patterns
- tokio::broadcast for event fan-out
- Axum Router with Arc state
- face crop images available from detection pipeline (decoded RGB frames)

### Integration Points
- Subscribe to recognition broadcast (same as attendance engine)
- WebSocket endpoint on :8096 for dashboard clients
- Toast notifications via winrt-toast (Windows-only)
- Face crop storage at C:\RacingPoint\face-crops\

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond decisions above.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
