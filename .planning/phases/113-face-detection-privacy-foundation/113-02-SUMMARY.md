---
phase: 113-face-detection-privacy-foundation
plan: 02
subsystem: detection
tags: [scrfd, onnx, face-detection, pipeline, h264, tokio]

requires:
  - phase: 113-01
    provides: "SCRFD detector, H.264 decoder, DetectedFace types"
provides:
  - "Detection pipeline loop (decode -> preprocess -> detect per camera)"
  - "DetectionConfig with model_path, confidence_threshold, nms_threshold, enabled"
  - "DetectionStats shared with health endpoint"
  - "Health endpoint /health includes detection metrics"
affects: [113-03, 113-04, privacy-pipeline, blur-pipeline]

tech-stack:
  added: []
  patterns:
    - "Per-camera detection tasks spawned as long-lived tokio tasks"
    - "Single ScrfdDetector shared via Arc across all camera detection tasks"
    - "DetectionStats with AtomicU64 counters and RwLock for last_detection timestamp"

key-files:
  created:
    - crates/rc-sentry-ai/src/detection/pipeline.rs
  modified:
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/detection/mod.rs
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/src/health.rs

key-decisions:
  - "Pipeline calls ScrfdDetector::detect directly (already async with internal spawn_blocking) instead of wrapping in another spawn_blocking"
  - "DetectionStats always created and passed to health endpoint even when detection disabled (shows zeroes)"

patterns-established:
  - "Detection pipeline polls FrameBuffer with 50ms sleep to avoid busy-wait"
  - "One FrameDecoder per camera (H.264 stateful decoder, not shareable)"

requirements-completed: [FACE-01]

duration: 2min
completed: 2026-03-21
---

# Phase 113 Plan 02: Detection Pipeline Summary

**Live detection pipeline wiring: per-camera decode->preprocess->detect loop with config-gated SCRFD init and health endpoint stats**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T15:56:02Z
- **Completed:** 2026-03-21T15:57:53Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- DetectionConfig added to Config with model_path, confidence_threshold, nms_threshold, enabled (all with defaults)
- Detection pipeline module orchestrates per-camera decode->preprocess->detect loop as long-lived tokio tasks
- SCRFD session created once and shared via Arc across all camera detection tasks
- Health endpoint reports detection metrics (frames_processed, faces_detected, last_detection_secs_ago)

## Task Commits

Each task was committed atomically:

1. **Task 1: Detection config and pipeline module** - `38a7055` (feat)
2. **Task 2: Wire pipeline into main.rs and update health endpoint** - `7bce148` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/detection/pipeline.rs` - Detection loop: poll FrameBuffer, decode H.264, preprocess, run SCRFD, log results
- `crates/rc-sentry-ai/src/config.rs` - Added DetectionConfig struct with serde defaults
- `crates/rc-sentry-ai/src/detection/mod.rs` - Added pipeline module export
- `crates/rc-sentry-ai/src/main.rs` - SCRFD init, per-camera task spawning, detection_stats in AppState
- `crates/rc-sentry-ai/src/health.rs` - detection_stats in AppState, detection metrics in /health JSON

## Decisions Made
- Pipeline calls ScrfdDetector::detect() directly with .await since it already uses spawn_blocking internally, avoiding double-blocking wrapper
- DetectionStats always created even when detection is disabled, so health endpoint always has the detection key (with zeroes)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adapted pipeline to async ScrfdDetector::detect API**
- **Found during:** Task 1 (pipeline.rs creation)
- **Issue:** Plan assumed ScrfdDetector::detect was synchronous and wrapped it in spawn_blocking. Actual implementation is already async with internal spawn_blocking.
- **Fix:** Pipeline calls detector.detect(...).await directly instead of spawn_blocking wrapper
- **Files modified:** crates/rc-sentry-ai/src/detection/pipeline.rs
- **Verification:** cargo check passes
- **Committed in:** 38a7055 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary adaptation to match actual ScrfdDetector API. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Detection pipeline ready for blur/privacy overlay in subsequent plans
- Health endpoint detection stats available for monitoring
- Future phases can add broadcast channel for face results consumption

---
*Phase: 113-face-detection-privacy-foundation*
*Completed: 2026-03-21*
