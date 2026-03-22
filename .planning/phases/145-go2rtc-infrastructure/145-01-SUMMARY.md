---
phase: 145-go2rtc-infrastructure
plan: 01
subsystem: infra
tags: [go2rtc, rtsp, webrtc, cors, nvr, dahua, cameras]

# Dependency graph
requires:
  - phase: 116-camera-snapshot-cache
    provides: snapshot polling infrastructure that must coexist with WebRTC streams
provides:
  - go2rtc configured with all 13 NVR channels (ch1-ch13) + ch1_h264 transcoded test stream
  - CORS enabled on go2rtc API (origin: "*") for cross-origin browser WebRTC
  - Verified WebRTC session on live hardware via go2rtc web UI
  - Confirmed snapshot + WebRTC coexistence without NVR connection drops
affects:
  - 146-camera-dashboard (frontend WebRTC consumer)
  - any phase adding new camera streams

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "NVR RTSP passthrough via subtype=1 sub-stream — not main stream (subtype=0)"
    - "go2rtc ch1-ch13 naming convention for NVR channels (1-indexed, matching NVR channel numbers)"
    - "H.264 transcoded stream (ch1_h264) via ffmpeg: prefix for Chrome WebRTC compatibility"

key-files:
  created: []
  modified:
    - C:/RacingPoint/go2rtc/go2rtc.yaml

key-decisions:
  - "145-01: Used subtype=1 (sub-stream) for all 13 NVR channels — lower bandwidth, sufficient for dashboard"
  - "145-01: ch1_h264 uses ffmpeg: prefix for H.264 transcoding — Chrome may not support H.265 WebRTC natively"
  - "145-01: CORS origin set to '*' — go2rtc API must be reachable from dashboard on any port"
  - "145-01: All 6 existing AI detection streams preserved (entrance_h264, reception_h264, reception_wide_h264, etc.)"

patterns-established:
  - "NVR channel URL pattern: rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=N&subtype=1"
  - "go2rtc stream naming: ch1-ch13 for NVR passthrough, ch1_h264 for H.264 transcoded variant"

requirements-completed:
  - INFRA-01
  - INFRA-02

# Metrics
duration: checkpoint-approved
completed: 2026-03-22
---

# Phase 145 Plan 01: go2rtc Infrastructure Summary

**go2rtc configured with 13 NVR sub-stream channels (ch1-ch13), CORS enabled, and WebRTC + snapshot coexistence verified on live Dahua NVR hardware**

## Performance

- **Duration:** Checkpoint-based (Task 1 automated, Task 2 human-verified)
- **Started:** 2026-03-22
- **Completed:** 2026-03-22T18:00:00+05:30 (approx, IST)
- **Tasks:** 2 of 2
- **Files modified:** 1

## Accomplishments

- Registered all 13 NVR cameras as go2rtc streams (ch1-ch13) using RTSP sub-stream URLs via NVR at 192.168.31.18
- Added ch1_h264 H.264-transcoded test stream via ffmpeg: prefix for browser WebRTC compatibility
- Added CORS `origin: "*"` to go2rtc API section so browser dashboard can connect cross-origin
- Preserved all 6 existing AI detection streams (entrance, reception, reception_wide, *_h264 variants)
- Human-verified on live hardware: WebRTC video plays in go2rtc web UI, snapshot endpoint returns HTTP 200 concurrently

## Task Commits

Each task was committed atomically:

1. **Task 1: Register 13 NVR channels + CORS in go2rtc.yaml** - `f500c26` (feat)
2. **Task 2: Restart go2rtc and verify all streams + CORS + coexistence** - checkpoint:human-verify, approved by user

## Files Created/Modified

- `C:/RacingPoint/go2rtc/go2rtc.yaml` - Added ch1-ch13 NVR sub-stream channels, ch1_h264 H.264 transcoded stream, CORS origin: "*" to api section; all existing AI streams preserved

## Decisions Made

- Used subtype=1 (sub-stream) for all NVR channels — lower bandwidth suitable for dashboard display, avoids saturating NVR main streams used by recording
- ch1_h264 added as ffmpeg-transcoded H.264 stream because Chrome WebRTC does not reliably support H.265
- CORS set to `"*"` rather than a specific origin — dashboard port may vary during development

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - go2rtc accepted the YAML config, all 13 channels registered, WebRTC and snapshot coexistence confirmed on live hardware.

## User Setup Required

None - go2rtc.yaml is already deployed at `C:/RacingPoint/go2rtc/go2rtc.yaml` on James's machine (.27). go2rtc process was restarted as part of Task 2 verification.

## Next Phase Readiness

- go2rtc is running at http://192.168.31.27:1984 with all 13 NVR channels available as WebRTC streams
- CORS headers present — browser dashboard can connect without proxy
- ch1-ch13 stream names established as the naming convention for the frontend
- Ready for Phase 146: Camera Dashboard frontend WebRTC consumer

---
*Phase: 145-go2rtc-infrastructure*
*Completed: 2026-03-22*
