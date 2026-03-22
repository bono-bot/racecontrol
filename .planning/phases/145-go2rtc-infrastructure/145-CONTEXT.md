# Phase 145: go2rtc Infrastructure - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Configure go2rtc with all 13 NVR cameras via RTSP sub-streams, enable CORS for cross-port WebRTC access, and verify that snapshot polling and WebRTC can coexist on the same NVR channel.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key decisions to make during execution:
- Whether to use NVR RTSP URLs (rtsp://192.168.31.18/cam/realmonitor?channel=N&subtype=1) or direct camera IPs
- go2rtc CORS config: `origin: "*"` under `[api]` section
- Stream naming convention: ch1-ch13 mapping to NVR channels 1-13
- Whether H.264 transcoding is needed (via ffmpeg: prefix) or native H.265 relay works

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- go2rtc.yaml at C:\RacingPoint\go2rtc\go2rtc.yaml — currently has 3 direct-IP cameras + 3 H.264 transcoded streams
- NVR at 192.168.31.18 with admin/Admin@123 auth
- SnapshotCache in mjpeg.rs — background fetcher for all 13 channels

### Established Patterns
- go2rtc RTSP URL format: `rtsp://admin:Admin%40123@{IP}/cam/realmonitor?channel=1&subtype=1`
- H.264 transcoded streams use `ffmpeg:` prefix for openh264 compatibility
- go2rtc config uses YAML format with `streams:` top-level key

### Integration Points
- go2rtc WebRTC API at ws://192.168.31.27:1984/api/ws?src={stream_name}
- go2rtc web UI at http://192.168.31.27:1984 for testing
- SnapshotCache fetches from NVR directly (not through go2rtc)

</code_context>

<specifics>
## Specific Ideas

- Use NVR RTSP URLs (through NVR, not direct camera IPs) for consistency — NVR handles failover
- Stream names should match the camera naming convention used in the dashboard (ch1-ch13)
- CORS must allow both :8096 (rc-sentry-ai) and :3200 (web dashboard)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
