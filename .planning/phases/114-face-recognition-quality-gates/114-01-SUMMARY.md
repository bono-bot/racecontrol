---
phase: 114-face-recognition-quality-gates
plan: 01
subsystem: ai
tags: [face-recognition, quality-gates, clahe, laplacian, yaw-estimation, image-processing]

# Dependency graph
requires:
  - phase: 113-face-detection-pipeline
    provides: DetectedFace struct with bbox and landmarks
provides:
  - QualityGates filter chain (size, blur, pose rejection)
  - RejectReason enum for quality gate outcomes
  - RecognitionResult and GalleryEntry types for downstream recognition
  - CLAHE lighting normalization for face crops
  - Laplacian variance blur detection function
  - Yaw estimation from 5-point landmarks
  - Lib target for unit testing without ort linker dependency
affects: [114-02, 114-03, recognition-pipeline]

# Tech tracking
tech-stack:
  added: [clahe 0.1]
  patterns: [quality-gate-filter-chain, lib-target-for-testing]

key-files:
  created:
    - crates/rc-sentry-ai/src/recognition/types.rs
    - crates/rc-sentry-ai/src/recognition/quality.rs
    - crates/rc-sentry-ai/src/recognition/clahe.rs
    - crates/rc-sentry-ai/src/lib.rs
  modified:
    - crates/rc-sentry-ai/src/recognition/mod.rs
    - crates/rc-sentry-ai/Cargo.toml

key-decisions:
  - "Added lib target to Cargo.toml to enable unit testing without ort linker dependency"
  - "Lib target selectively exports recognition submodules, excluding arcface to avoid ort linking"
  - "Yaw estimation uses linear (1-ratio)*90 mapping from eye-nose ratio, matching RESEARCH.md recommendation"

patterns-established:
  - "Quality gate filter chain: pure functions returning Result<(), RejectReason>, run in sequence"
  - "Lib target pattern: separate lib.rs excluding ort-dependent modules for testability"
  - "CLAHE always applied: convert to grayscale, enhance, replicate to 3-channel RGB"

requirements-completed: [FACE-03, FACE-04]

# Metrics
duration: 8min
completed: 2026-03-21
---

# Phase 114 Plan 01: Quality Gates & CLAHE Summary

**Quality gate filter chain rejecting faces by size (<80x80), blur (Laplacian var <100), and pose (yaw >45deg), plus CLAHE lighting normalization producing grayscale-as-RGB face crops**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T16:49:08Z
- **Completed:** 2026-03-21T16:57:22Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Quality gates reject faces below 80x80px, with Laplacian variance below 100.0, and with yaw above 45 degrees
- CLAHE normalization converts face crops to contrast-enhanced grayscale-as-RGB images for ArcFace input
- 10 new unit tests (7 quality + 3 CLAHE), all passing via lib target
- Lib target pattern established for testing without ort linker dependency

## Task Commits

Each task was committed atomically (TDD: test then feat):

1. **Task 1: Recognition module types and quality gates**
   - `4cf5621` (test) - failing tests for quality gates and recognition types
   - `6626fec` (feat) - implement quality gates with size, blur, and pose rejection

2. **Task 2: CLAHE lighting normalization**
   - `c26dd9c` (test) - failing tests for CLAHE lighting normalization
   - `bc5103b` (feat) - implement CLAHE lighting normalization for face crops

3. **Lib fix:** `c851530` (fix) - exclude ort-dependent arcface from lib test target

## Files Created/Modified
- `crates/rc-sentry-ai/src/recognition/types.rs` - RejectReason, RecognitionResult, GalleryEntry types
- `crates/rc-sentry-ai/src/recognition/quality.rs` - QualityGates struct with check_size, check_blur, check_pose + laplacian_variance + estimate_yaw
- `crates/rc-sentry-ai/src/recognition/clahe.rs` - apply_clahe function for lighting normalization
- `crates/rc-sentry-ai/src/recognition/mod.rs` - Updated with types, quality, clahe submodule exports
- `crates/rc-sentry-ai/src/recognition/alignment.rs` - Stub placeholder (pre-existing declaration)
- `crates/rc-sentry-ai/src/recognition/arcface.rs` - Stub placeholder (pre-existing declaration)
- `crates/rc-sentry-ai/src/lib.rs` - New lib target for unit testing without ort linking
- `crates/rc-sentry-ai/Cargo.toml` - Added lib target, clahe 0.1 dependency

## Decisions Made
- Added `[lib]` target to Cargo.toml to enable `cargo test --lib` without hitting ort linker errors (pre-existing environment issue with ort 2.0 on Windows)
- Lib.rs selectively defines recognition module inline, excluding arcface submodule that pulls in ort
- Used `clahe::clahe_u8_to_u8` with 8x8 tiles and clip_limit 40.0 per RESEARCH.md recommendation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created lib target to work around ort linker errors**
- **Found during:** Task 1 (TDD RED phase)
- **Issue:** `cargo test -p rc-sentry-ai` fails at link time with 45+ unresolved external symbols from ort/ONNX Runtime. Pre-existing environment issue unrelated to this plan's changes.
- **Fix:** Added `[lib]` section to Cargo.toml and created src/lib.rs that exports only testable modules (detection::types, recognition minus arcface). Tests run via `cargo test --lib`.
- **Files modified:** Cargo.toml, src/lib.rs
- **Verification:** `cargo test -p rc-sentry-ai --lib` passes all 14 tests
- **Committed in:** 4cf5621 (initial), c851530 (arcface exclusion fix)

**2. [Rule 3 - Blocking] Created stub files for pre-existing module declarations**
- **Found during:** Task 1 (TDD RED phase)
- **Issue:** recognition/mod.rs already declared `pub mod alignment; pub mod arcface;` but files did not exist, causing compilation errors
- **Fix:** Created alignment.rs and arcface.rs stub files
- **Files modified:** alignment.rs, arcface.rs
- **Verification:** `cargo check -p rc-sentry-ai` passes
- **Committed in:** 4cf5621

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary to enable compilation and testing. No scope creep.

## Issues Encountered
- Pre-existing ort 2.0 linker issue on Windows prevents `cargo test -p rc-sentry-ai` (binary target). Resolved by adding lib target for testable code isolation. This is an environment issue that should be investigated separately.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Quality gates ready for integration into detection pipeline
- CLAHE ready to be called after face crop extraction, before ArcFace alignment
- Types (RecognitionResult, GalleryEntry) ready for gallery and tracker implementation
- Lib target pattern available for future recognition module tests

---
*Phase: 114-face-recognition-quality-gates*
*Completed: 2026-03-21*
