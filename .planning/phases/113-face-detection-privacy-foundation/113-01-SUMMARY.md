---
phase: 113-face-detection-privacy-foundation
plan: 01
subsystem: detection
tags: [scrfd, onnx, ort, cuda, openh264, h264, ndarray, face-detection]

# Dependency graph
requires:
  - phase: 112-sentry-ai-rtsp-camera-pipeline
    provides: FrameBuffer with H.264 NAL data from RTSP camera streams
provides:
  - DetectedFace struct with bbox, confidence, and 5-point landmarks
  - FrameDecoder for stateful H.264 NAL to RGB conversion
  - ScrfdDetector with CUDA EP, preprocessing, inference, and NMS postprocessing
affects: [113-02, 114-face-embeddings]

# Tech tracking
tech-stack:
  added: [ort 2.0.0-rc.12 (CUDA), openh264 0.9, ndarray 0.17, image 0.25]
  patterns: [Arc<Mutex<Session>> for thread-safe ONNX inference, spawn_blocking for GPU-bound work, flat tensor indexing for ort 2.0 output]

key-files:
  created:
    - crates/rc-sentry-ai/src/detection/mod.rs
    - crates/rc-sentry-ai/src/detection/types.rs
    - crates/rc-sentry-ai/src/detection/decoder.rs
    - crates/rc-sentry-ai/src/detection/scrfd.rs
  modified:
    - crates/rc-sentry-ai/Cargo.toml
    - crates/rc-sentry-ai/src/main.rs

key-decisions:
  - "ndarray 0.17 (not 0.16) to match ort 2.0 internal ndarray version"
  - "Arc<Mutex<Session>> instead of Arc<Session> because ort 2.0 session.run() requires &mut self"
  - "Flat tensor indexing for ort 2.0 output (Shape + &[f32] tuple, not ndarray)"
  - "map_err for ort errors since Error<SessionBuilder> is not Send+Sync for anyhow"

patterns-established:
  - "ort 2.0 session pattern: Arc<Mutex<Session>> with blocking_lock in spawn_blocking"
  - "SCRFD postprocessing: classify output tensors by last dimension (1=score, 4=bbox, 10=kps)"

requirements-completed: [FACE-01]

# Metrics
duration: 7min
completed: 2026-03-21
---

# Phase 113 Plan 01: Detection Foundation Summary

**SCRFD-10GF ONNX face detector with CUDA EP and openh264 H.264 decoder -- core detection building blocks**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-21T15:46:00Z
- **Completed:** 2026-03-21T15:53:17Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- DetectedFace struct with bbox, confidence, and 5-point landmarks matching InsightFace format
- FrameDecoder wrapping openh264 for stateful per-camera H.264 decode (~7ms at 1080p)
- ScrfdDetector with CUDA EP (error_on_failure -- no silent CPU fallback), preprocessing (resize + normalize to [1,3,640,640]), and full FPN postprocessing across 3 stride levels with NMS

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dependencies and create detection types** - `5ac03b3` (feat)
2. **Task 2: H.264 decoder and SCRFD detector** - `fc1a82d` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/detection/mod.rs` - Detection module exports (decoder, scrfd, types)
- `crates/rc-sentry-ai/src/detection/types.rs` - DetectedFace struct with bbox/confidence/landmarks
- `crates/rc-sentry-ai/src/detection/decoder.rs` - H.264 NAL to RGB via openh264 (stateful per-camera)
- `crates/rc-sentry-ai/src/detection/scrfd.rs` - SCRFD ONNX inference with CUDA EP, preprocessing, postprocessing, NMS
- `crates/rc-sentry-ai/Cargo.toml` - Added ort, openh264, ndarray, image, chrono, uuid dependencies
- `crates/rc-sentry-ai/src/main.rs` - Registered detection module

## Decisions Made
- Used ndarray 0.17 instead of plan-specified 0.16 because ort 2.0.0-rc.12 depends on ndarray 0.17 internally; using 0.16 causes type mismatch at TensorArrayData trait boundary
- Used Arc<Mutex<Session>> instead of Arc<Session> because ort 2.0 requires &mut self for session.run() -- previous ort versions allowed &self
- Used Tensor::from_array(owned) instead of TensorRef::from_array_view(&ref) to avoid lifetime issues inside spawn_blocking closure
- Mapped ort errors explicitly via map_err since Error<SessionBuilder> does not implement Send+Sync required by anyhow

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ndarray version mismatch with ort 2.0**
- **Found during:** Task 2 (SCRFD detector implementation)
- **Issue:** Plan specified ndarray = "0.16" but ort 2.0.0-rc.12 uses ndarray 0.17 internally, causing OwnedTensorArrayData trait bound failure
- **Fix:** Changed ndarray dependency from 0.16 to 0.17 in Cargo.toml
- **Files modified:** crates/rc-sentry-ai/Cargo.toml
- **Verification:** cargo check -p rc-sentry-ai passes
- **Committed in:** fc1a82d (Task 2 commit)

**2. [Rule 3 - Blocking] ort 2.0 API changes from plan assumptions**
- **Found during:** Task 2 (SCRFD detector implementation)
- **Issue:** Plan assumed ort 1.x API (session.inputs field, inputs! macro with array view, ndarray return from try_extract_tensor). ort 2.0 uses methods (inputs()/outputs()), requires &mut self for run(), returns (Shape, &[f32]) tuple from try_extract_tensor
- **Fix:** Rewrote session management to use Arc<Mutex<Session>> with blocking_lock, Tensor::from_array for input, flat tensor indexing for output, and explicit map_err for non-Send ort errors
- **Files modified:** crates/rc-sentry-ai/src/detection/scrfd.rs
- **Verification:** cargo check -p rc-sentry-ai passes
- **Committed in:** fc1a82d (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary to compile against ort 2.0 API. No scope creep -- same functionality, different API surface.

## Issues Encountered
None beyond the ort 2.0 API changes documented as deviations above.

## User Setup Required
None - no external service configuration required. SCRFD model file (scrfd_10g_bnkps.onnx) must be present at C:\RacingPoint\models\ for runtime use (Plan 02 scope).

## Next Phase Readiness
- Detection module compiled and wired into main.rs
- Plan 02 can now build the live detection pipeline: read from FrameBuffer, decode via FrameDecoder, preprocess + detect via ScrfdDetector
- SCRFD model file download needed before integration testing

---
*Phase: 113-face-detection-privacy-foundation*
*Completed: 2026-03-21*
