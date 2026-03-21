---
phase: 117-alerts-notifications
plan: 03
subsystem: detection
tags: [face-recognition, alerts, jpeg, rate-limiting, broadcast]

requires:
  - phase: 117-alerts-notifications (plan 01)
    provides: AlertEvent enum with UnknownPerson variant, alerts broadcast channel, AlertsConfig
provides:
  - UnknownFaceEvent struct for pipeline-to-alert-engine communication
  - Unknown person detection in pipeline (else branch on gallery miss)
  - Rate-limited unknown person alerts with JPEG face crop saving
  - Full unknown_tx broadcast wiring from pipeline through main.rs to unknown engine
affects: [117-alerts-notifications, sentry-dashboard, enrollment]

tech-stack:
  added: []
  patterns: [per-camera rate limiting via HashMap<String, Instant>, spawn_blocking for JPEG I/O]

key-files:
  created:
    - crates/rc-sentry-ai/src/alerts/unknown.rs
  modified:
    - crates/rc-sentry-ai/src/alerts/types.rs
    - crates/rc-sentry-ai/src/alerts/mod.rs
    - crates/rc-sentry-ai/src/detection/pipeline.rs
    - crates/rc-sentry-ai/src/main.rs

key-decisions:
  - "Clone aligned face before CLAHE to preserve raw RGB for crop saving"
  - "Use JpegEncoder with configurable quality instead of save_buffer for JPEG quality control"
  - "IST timestamps in face crop filenames for local operator readability"
  - "Sanitize camera names in filenames (replace non-alphanumeric with underscore)"

patterns-established:
  - "Per-camera rate limiting: HashMap<String, Instant> with periodic cleanup"
  - "spawn_blocking for file I/O in async context"
  - "Graceful degradation: emit alert without crop_path if JPEG save fails"

requirements-completed: [ALRT-03]

duration: 5min
completed: 2026-03-22
---

# Phase 117 Plan 03: Unknown Person Detection Summary

**Unknown face detection with rate-limited alerts and 112x112 JPEG crop saving to C:\RacingPoint\face-crops\**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T18:25:45Z
- **Completed:** 2026-03-21T18:30:26Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Pipeline now detects unrecognized faces (no gallery match above 0.45) and broadcasts UnknownFaceEvent
- Unknown person engine saves 112x112 JPEG face crops with IST-timestamped filenames
- Per-camera rate limiting prevents alert spam (default 5 minutes between alerts per camera)
- Full broadcast wiring from pipeline through main.rs to unknown engine, feeding into existing alert WebSocket and toast notification system

## Task Commits

Each task was committed atomically:

1. **Task 1: Unknown face event type and broadcast from pipeline** - `c84e34d` (feat)
2. **Task 2: Unknown engine with rate limiting and face crop saving** - `e2d1e10` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/alerts/types.rs` - Added UnknownFaceEvent struct
- `crates/rc-sentry-ai/src/alerts/unknown.rs` - Full unknown person alert engine with rate limiting and JPEG saving
- `crates/rc-sentry-ai/src/alerts/mod.rs` - Registered unknown module
- `crates/rc-sentry-ai/src/detection/pipeline.rs` - Added unknown_tx parameter and else branch for gallery miss
- `crates/rc-sentry-ai/src/main.rs` - Created unknown broadcast channel and spawned unknown engine

## Decisions Made
- Cloned aligned face before CLAHE processing to preserve raw RGB pixels for JPEG crop
- Used image::codecs::jpeg::JpegEncoder with quality parameter instead of save_buffer (which doesn't support quality)
- IST timestamps in filenames for operator convenience
- Camera names sanitized in filenames to avoid filesystem issues
- Graceful degradation: if JPEG save fails, alert is still emitted with crop_path: None

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed type mismatch: aligned face is ImageBuffer, not Vec<u8>**
- **Found during:** Task 1 (pipeline else branch)
- **Issue:** `alignment::align_face` returns `RgbImage` (ImageBuffer), but UnknownFaceEvent expects Vec<u8>
- **Fix:** Used `.into_raw()` to extract raw bytes from the cloned ImageBuffer
- **Files modified:** crates/rc-sentry-ai/src/detection/pipeline.rs
- **Verification:** cargo check passes
- **Committed in:** c84e34d (Task 1 commit)

**2. [Rule 1 - Bug] Used JpegEncoder instead of save_buffer for quality control**
- **Found during:** Task 2 (JPEG saving)
- **Issue:** `image::save_buffer` does not accept a quality parameter; the quality config field would be unused
- **Fix:** Used `image::codecs::jpeg::JpegEncoder::new_with_quality` with `encode()` method
- **Files modified:** crates/rc-sentry-ai/src/alerts/unknown.rs
- **Verification:** cargo check passes, no unused variable warnings
- **Committed in:** e2d1e10 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Issues Encountered
- Plan 02 ran in parallel and modified alerts/mod.rs (adding toast module) -- resolved by reading current state before editing
- Plan 02 also modified main.rs concurrently -- no conflicts due to targeted edits at different locations

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Unknown person alerts flow through existing broadcast to WebSocket clients and toast notifications
- Face crops directory (C:\RacingPoint\face-crops\) created automatically on first run
- Ready for dashboard UI integration to display unknown person alerts with crop images

---
*Phase: 117-alerts-notifications*
*Completed: 2026-03-22*
