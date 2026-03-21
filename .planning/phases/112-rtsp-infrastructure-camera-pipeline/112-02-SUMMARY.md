---
phase: 112-rtsp-infrastructure-camera-pipeline
plan: 02
subsystem: infra
tags: [rtsp, retina, tokio, camera, go2rtc, frame-buffer]

requires:
  - phase: 112-01
    provides: "go2rtc relay installed and configured with camera streams"
provides:
  - "rc-sentry-ai crate in workspace with config, frame buffer, and RTSP stream extraction"
  - "Per-camera retina RTSP sessions with independent reconnect loops"
  - "FrameBuffer shared state for downstream consumers (health endpoint, face detection)"
affects: [112-03, 112-04, 113]

tech-stack:
  added: [retina 0.4, futures 0.3, url 2, reqwest 0.12]
  patterns: [per-camera-tokio-task, arc-rwlock-frame-buffer, rtsp-reconnect-loop]

key-files:
  created:
    - crates/rc-sentry-ai/Cargo.toml
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/frame.rs
    - crates/rc-sentry-ai/src/stream.rs
    - C:\RacingPoint\rc-sentry-ai.toml
  modified:
    - Cargo.toml

key-decisions:
  - "Used Arc<RwLock<HashMap>> for FrameBuffer to allow concurrent read access from health endpoint"
  - "Store raw H.264 NAL units in FrameBuffer, defer pixel decoding to Phase 113"
  - "Rate limit frame extraction via tokio::time::sleep at configured FPS per camera"

patterns-established:
  - "Per-camera tokio::spawn task with infinite reconnect loop and 5s backoff"
  - "TOML config at C:\\RacingPoint\\rc-sentry-ai.toml matching monorepo config pattern"

requirements-completed: [CAM-02]

duration: 4min
completed: 2026-03-21
---

# Phase 112 Plan 02: rc-sentry-ai Crate Summary

**rc-sentry-ai crate with retina RTSP frame extraction, per-camera reconnect loops, and shared FrameBuffer for 3 Dahua cameras via go2rtc relay**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T15:08:18Z
- **Completed:** 2026-03-21T15:11:53Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Created rc-sentry-ai crate in workspace with retina, axum, tokio dependencies
- TOML config parsing for 3 cameras (entrance, reception, reception_wide) with relay settings
- Per-camera retina RTSP sessions with TCP transport connecting to go2rtc relay at :8554
- FrameBuffer with Arc<RwLock<HashMap>> storing latest H.264 NAL frame per camera
- Independent tokio::spawn task per camera with automatic reconnect (5s backoff on error)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create rc-sentry-ai crate scaffold with config and frame buffer** - `d79a9b0` (feat)
2. **Task 2: Implement per-camera retina RTSP stream extraction** - `a614213` (feat)

## Files Created/Modified
- `Cargo.toml` - Added rc-sentry-ai to workspace members
- `crates/rc-sentry-ai/Cargo.toml` - Crate manifest with retina, axum, tokio, futures deps
- `crates/rc-sentry-ai/src/main.rs` - Entry point: config load, tracing init, per-camera task spawning
- `crates/rc-sentry-ai/src/config.rs` - Config/ServiceConfig/RelayConfig/CameraConfig structs with TOML parsing
- `crates/rc-sentry-ai/src/frame.rs` - FrameBuffer and FrameData with update/get/status methods
- `crates/rc-sentry-ai/src/stream.rs` - camera_loop and connect_and_stream with retina RTSP + reconnect
- `C:\RacingPoint\rc-sentry-ai.toml` - Service config with 3 cameras pointing at go2rtc relay

## Decisions Made
- Used `Arc<RwLock<HashMap>>` for FrameBuffer (tokio RwLock, not std) for async-safe concurrent access
- Store raw H.264 NAL units, not decoded pixels -- downstream Phase 113 handles GPU decode
- Rate limit via `tokio::time::sleep(1/fps)` after each frame to avoid busy-looping
- `SessionGroup` wrapped in `Arc` as required by retina 0.4.19 API

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] SessionGroup requires Arc wrapper**
- **Found during:** Task 2 (RTSP stream implementation)
- **Issue:** retina 0.4.19 `session_group()` method expects `Arc<SessionGroup>`, not `SessionGroup`
- **Fix:** Wrapped `SessionGroup::default()` in `Arc::new()`
- **Files modified:** crates/rc-sentry-ai/src/stream.rs
- **Verification:** `cargo build -p rc-sentry-ai` exits 0
- **Committed in:** a614213 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor API correction, no scope change.

## Issues Encountered
None beyond the Arc wrapper deviation above.

## User Setup Required
None - rc-sentry-ai.toml already placed at C:\RacingPoint\. go2rtc must be running (Plan 01) for streams to connect.

## Next Phase Readiness
- FrameBuffer API ready for health endpoint consumption (Plan 03)
- stream.rs camera_loop running per-camera tasks ready for real go2rtc relay
- Config structs extensible for future camera additions

## Self-Check: PASSED

All 7 files verified present. Both task commits (d79a9b0, a614213) found in git log.

---
*Phase: 112-rtsp-infrastructure-camera-pipeline*
*Completed: 2026-03-21*
