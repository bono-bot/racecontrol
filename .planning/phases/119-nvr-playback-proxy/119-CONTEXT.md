# Phase 119: NVR Playback Proxy - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Proxy stored footage from the Dahua NVR at .18 through rc-sentry-ai for dashboard playback. Time-range selector in dashboard, attendance event markers on playback timeline, no interference with NVR recording.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase.

Key constraints:
- Dahua NVR at 192.168.31.18, auth admin/Admin@123
- Dahua HTTP/CGI API for playback search and retrieval (mediaFileFind, loadFile)
- RTSP playback also available (rtsp://admin:Admin%40123@192.168.31.18:554/cam/playback?channel=1&starttime=...)
- Attendance events in SQLite (attendance/db.rs) for timeline markers
- Dashboard at :3200 — new playback page or tab on cameras page
- Must not interfere with NVR's ongoing recording
- rc-sentry-ai at :8096 serves as the proxy

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- mjpeg.rs: MJPEG streaming pattern (can adapt for playback)
- attendance/db.rs: query_attendance_log for event markers
- health.rs / main.rs: Axum Router patterns
- config.rs: camera name → NVR channel mapping

### Integration Points
- New /api/v1/playback/* endpoints on :8096
- Dashboard playback UI component
- Attendance events overlaid on timeline

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
