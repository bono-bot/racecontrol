---
phase: 118-live-camera-feeds
plan: 01
subsystem: api
tags: [mjpeg, streaming, axum, cors, h264, jpeg, camera]

requires:
  - phase: 117-sentry-ai-alerts
    provides: "rc-sentry-ai service with FrameBuffer, health endpoints, and detection pipeline"
provides:
  - "MJPEG streaming endpoint for live camera feeds at /api/v1/cameras/:name/stream"
  - "Camera list endpoint at /api/v1/cameras with status and stream URLs"
  - "CORS-enabled cross-origin access for dashboard at :3200"
affects: [118-02, dashboard, monitoring]

tech-stack:
  added: [tower-http, bytes]
  patterns: [per-connection-h264-decoder, mjpeg-multipart-streaming]

key-files:
  created:
    - crates/rc-sentry-ai/src/mjpeg.rs
  modified:
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/Cargo.toml

key-decisions:
  - "Per-connection H.264 decoder for MJPEG: FrameBuffer stores raw NAL data, so each stream client gets its own stateful decoder"
  - "FPS capped to 10 for browser clients to prevent overwhelming network/rendering"
  - "JPEG quality 70 for balance between visual quality and bandwidth"

patterns-established:
  - "MJPEG streaming pattern: unfold stream with H.264 decode + JPEG encode per frame"
  - "Camera status derivation from FrameBuffer timestamp age (connected/reconnecting/disconnected/offline)"

requirements-completed: [MNTR-01]

duration: 2min
completed: 2026-03-22
---

# Phase 118 Plan 01: MJPEG Streaming Summary

**MJPEG streaming endpoints serving live camera feeds via H.264-to-JPEG transcoding with per-connection decoders and CORS for dashboard access**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T18:40:27Z
- **Completed:** 2026-03-21T18:42:47Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- MJPEG streaming module with multipart/x-mixed-replace content type for native browser img tag rendering
- Camera list endpoint returning configured cameras with real-time status from FrameBuffer
- Per-connection H.264 decoder that gracefully handles P-frames before first keyframe
- CORS layer for cross-origin dashboard access from :3200

## Task Commits

Each task was committed atomically:

1. **Task 1: Create MJPEG streaming module** - `a04543d` (feat)
2. **Task 2: Wire MJPEG router and add dependencies** - `b975fb7` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/mjpeg.rs` - MJPEG streaming handler, camera list endpoint, CORS layer
- `crates/rc-sentry-ai/src/main.rs` - mod mjpeg declaration, MjpegState construction, router merge
- `crates/rc-sentry-ai/Cargo.toml` - Added tower-http (cors) and bytes dependencies
- `Cargo.lock` - Updated lockfile

## Decisions Made
- Used per-connection H.264 decoder because FrameBuffer stores raw NAL units (not JPEG). Each MJPEG stream client gets its own openh264 decoder instance to maintain H.264 state (P-frame dependencies).
- Capped stream FPS to 10 to avoid overwhelming browser rendering and network bandwidth.
- Set JPEG quality to 70 for reasonable visual quality at lower bandwidth.
- Used CORS allow-any-origin since rc-sentry-ai is LAN-only and dashboard at :3200 needs cross-origin img access.
- Skipped frames when decoder waiting for keyframe (graceful degradation vs error).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added bytes crate dependency**
- **Found during:** Task 1 (MJPEG module creation)
- **Issue:** mjpeg.rs uses `bytes::Bytes` for efficient stream frame output, but `bytes` was not an explicit dependency
- **Fix:** Added `bytes = "1"` to Cargo.toml (was transitively available via axum, but explicit is safer)
- **Files modified:** crates/rc-sentry-ai/Cargo.toml
- **Verification:** cargo check passes
- **Committed in:** b975fb7

**2. [Rule 2 - Missing Critical] H.264-to-JPEG transcoding for FrameBuffer data**
- **Found during:** Task 1 (reading stream.rs to determine FrameBuffer data format)
- **Issue:** Plan noted "check if FrameData.data is JPEG or raw pixels". It is neither -- it is H.264 NAL units from retina RTSP. MJPEG requires JPEG frames.
- **Fix:** Added per-connection FrameDecoder (openh264) to decode H.264 to RGB, then image crate JpegEncoder to produce JPEG. Handles keyframe wait gracefully.
- **Files modified:** crates/rc-sentry-ai/src/mjpeg.rs
- **Verification:** cargo check passes, decoder pattern matches existing detection/pipeline.rs usage
- **Committed in:** a04543d

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both auto-fixes necessary for correctness. H.264 transcoding was anticipated by the plan's contingency note. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- MJPEG endpoints ready for dashboard integration in 118-02
- Service at :8096 now serves /api/v1/cameras and /api/v1/cameras/:name/stream

---
*Phase: 118-live-camera-feeds*
*Completed: 2026-03-22*
