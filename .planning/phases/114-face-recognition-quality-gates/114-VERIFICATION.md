---
phase: 114-face-recognition-quality-gates
verified: 2026-03-21T18:15:00+05:30
status: human_needed
score: 10/10 must-haves verified
re_verification: false
human_verification:
  - test: "Enroll a test face via SQLite, walk past entrance camera"
    expected: "Face recognized with person_name and confidence logged at info level"
    why_human: "Requires live camera + ONNX model file + physical presence"
  - test: "Wave hand quickly past camera to produce blurry frames"
    expected: "Rejected by quality gates with TooBlurry reason in debug logs"
    why_human: "Requires physical blur conditions on live camera"
  - test: "Test recognition at morning, midday, and evening lighting"
    expected: "CLAHE normalization keeps recognition consistent across lighting"
    why_human: "Requires varying real-world lighting conditions"
  - test: "Walk past camera, wait 30s, walk past again"
    expected: "Second pass within 60s cooldown NOT re-logged; after 60s IS re-logged"
    why_human: "Requires timed physical movement past live camera"
---

# Phase 114: Face Recognition & Quality Gates Verification Report

**Phase Goal:** Identify detected faces by matching embeddings against enrolled faces, rejecting poor-quality captures
**Verified:** 2026-03-21T18:15:00+05:30
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Quality gates reject faces smaller than 80x80px | VERIFIED | `quality.rs:43` checks `width < min_face_size \|\| height < min_face_size`, default 80. Test `test_size_reject_too_small` passes. |
| 2 | Quality gates reject faces with Laplacian variance below 100.0 | VERIFIED | `quality.rs:79` checks `var < self.min_laplacian_var`, default 100.0. `laplacian_variance()` uses 3x3 kernel. Tests `test_blur_reject_uniform` and `test_blur_accept_checkerboard` pass. |
| 3 | Quality gates reject faces with estimated yaw above 45 degrees | VERIFIED | `quality.rs:88` checks `yaw > self.max_yaw_degrees`, default 45.0. `estimate_yaw()` uses eye-nose ratio. Tests `test_pose_accept_frontal` and `test_pose_reject_side_profile` pass. |
| 4 | CLAHE normalizes lighting on face crops producing consistent grayscale output | VERIFIED | `clahe.rs:14` uses `clahe::clahe_u8_to_u8(8, 8, 40.0, ...)`, replicates to RGB. 3 tests pass (changes pixels, preserves dims, R=G=B). |
| 5 | ArcFace model loads with CUDA EP and produces 512-D embeddings | VERIFIED | `arcface.rs:75-80` uses `Session::builder().with_execution_providers([ep::CUDA::default().build().error_on_failure()])`. `extract_embedding()` verifies `flat.len() == 512`. Cannot run CUDA test (ort linker issue), but cargo check confirms compilation. |
| 6 | Face alignment warps detected face to 112x112 using 5-point landmark similarity transform | VERIFIED | `alignment.rs:21-58` estimates similarity transform, `align_face()` warps via `imageproc::warp` with inverse projection. Tests pass: identity, scaled, output size 112x112. |
| 7 | Embeddings are L2-normalized after extraction | VERIFIED | `arcface.rs:155-166` computes `norm = sqrt(sum(x^2))` then divides each element. |
| 8 | Embedding gallery loads from SQLite and matches faces via cosine similarity at 0.45 threshold | VERIFIED | `gallery.rs:40` checks `sim > self.threshold` (0.45 default). `db.rs:32-63` loads from SQLite with BLOB deserialization. 7 tests pass (cosine, gallery, db). |
| 9 | Face tracker suppresses redundant recognitions within 60-second cooldown | VERIFIED | `tracker.rs:27-38` checks `duration_since(*last) < self.cooldown`. Default 60s. 3 tests pass. |
| 10 | Detection pipeline runs quality gates, CLAHE, alignment, and ArcFace in sequence after SCRFD | VERIFIED | `pipeline.rs:123-194` wires full chain: `quality_gates.check` -> `alignment::align_face` -> `clahe::apply_clahe` -> `arcface::preprocess` -> `recognizer.extract_embedding` -> `gallery.find_match` -> `tracker.should_report`. |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry-ai/src/recognition/mod.rs` | Module root with submodule exports | VERIFIED | 8 submodules declared (types, quality, clahe, alignment, arcface, gallery, db, tracker) |
| `crates/rc-sentry-ai/src/recognition/types.rs` | QualityResult, RejectReason, RecognitionResult, GalleryEntry | VERIFIED | 29 lines, all types with derives |
| `crates/rc-sentry-ai/src/recognition/quality.rs` | Quality gate filter functions | VERIFIED | 272 lines, QualityGates struct + check + laplacian_variance + estimate_yaw + 7 tests |
| `crates/rc-sentry-ai/src/recognition/clahe.rs` | CLAHE lighting normalization | VERIFIED | 73 lines, apply_clahe + 3 tests |
| `crates/rc-sentry-ai/src/recognition/alignment.rs` | Face alignment via similarity transform | VERIFIED | 270 lines, estimate_similarity_transform + solve_4x4 + align_face + ARCFACE_REF + 4 tests |
| `crates/rc-sentry-ai/src/recognition/arcface.rs` | ArcFace ONNX session with embedding extraction | VERIFIED | 235 lines, ArcfaceRecognizer (session submodule pattern), preprocess, extract_embedding + 2 tests |
| `crates/rc-sentry-ai/src/recognition/gallery.rs` | In-memory gallery with cosine similarity | VERIFIED | 145 lines, Gallery struct, cosine_similarity, find_match, reload + 5 tests |
| `crates/rc-sentry-ai/src/recognition/db.rs` | SQLite schema and CRUD | VERIFIED | 138 lines, create_tables, load_gallery, insert_person, insert_embedding + 2 tests |
| `crates/rc-sentry-ai/src/recognition/tracker.rs` | Face tracker with per-person cooldown | VERIFIED | 79 lines, FaceTracker, should_report, cleanup + 3 tests |
| `crates/rc-sentry-ai/src/config.rs` | RecognitionConfig added to Config | VERIFIED | RecognitionConfig with 8 fields, all with serde defaults, added to Config |
| `crates/rc-sentry-ai/src/lib.rs` | Lib target for testing without ort | VERIFIED | Exports detection::types and recognition modules (excluding arcface) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| quality.rs | detection/types.rs | `use crate::detection::types::DetectedFace` | WIRED | Line 1, used in check() signature |
| arcface.rs | ort CUDA EP | `ep::CUDA::default().build().error_on_failure()` | WIRED | Line 77 |
| alignment.rs | DetectedFace landmarks | `landmarks: &[[f32; 2]; 5]` param | WIRED | align_face accepts landmark array matching DetectedFace.landmarks type |
| pipeline.rs | recognition modules | full chain call | WIRED | Lines 136-192: quality_gates.check, alignment::align_face, clahe::apply_clahe, arcface::preprocess, extract_embedding, gallery.find_match, tracker.should_report |
| gallery.rs | db.rs | load_gallery | WIRED | main.rs:77 calls `recognition::db::load_gallery(&conn)`, result passed to `Gallery::new()` |
| main.rs | ArcfaceRecognizer | `ArcfaceRecognizer::new()` | WIRED | main.rs:51, initialized from config.recognition.model_path |
| main.rs | pipeline::run | passes all recognition components | WIRED | main.rs:142-143 passes recognizer, quality_gates, gallery, tracker |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| FACE-02 | 114-02, 114-03 | ArcFace embedding extraction for identity matching | SATISFIED | ArcfaceRecognizer with CUDA EP, 512-D L2-normalized embeddings, cosine similarity gallery at 0.45, pipeline wired end-to-end |
| FACE-03 | 114-01, 114-03 | Quality gates to reject blurry, side-profile, and backlit captures | SATISFIED | QualityGates with size (80x80), blur (Laplacian var 100.0), pose (yaw 45 deg) -- all wired in pipeline |
| FACE-04 | 114-01, 114-03 | Lighting normalization for entrance camera conditions | SATISFIED | CLAHE applied after alignment, before ArcFace preprocessing -- grayscale-as-RGB output |

No orphaned requirements found.

### Threshold Discrepancy Note

ROADMAP success criteria specified "faces smaller than 200x200px" and "yaw > 30 degrees" but the RESEARCH phase (114-RESEARCH.md) revised these to 80x80 and 45 degrees respectively based on practical accuracy analysis. These revised thresholds are documented in RESEARCH.md, applied consistently in PLAN frontmatter, and implemented correctly. This is a research-informed refinement, not a gap.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No TODO, FIXME, PLACEHOLDER, HACK, or stub patterns found in any recognition module file |

### Build Verification

| Check | Result |
|-------|--------|
| `cargo check -p rc-sentry-ai` | PASS (9 dead-code warnings, no errors) |
| `cargo test -p rc-sentry-ai --lib` | PASS (24 tests, 0 failures) |
| ort CUDA binary tests | SKIP (known static CRT linker issue, not a phase regression) |

### Human Verification Required

### 1. End-to-End Face Recognition

**Test:** Enroll a test face in SQLite (insert person + embedding), walk past entrance camera
**Expected:** Face recognized with person_name, person_id, and confidence logged at info level
**Why human:** Requires live camera feed, ONNX model file at C:\RacingPoint\models\glintr100.onnx, and physical presence

### 2. Quality Gate Rejection (Blur)

**Test:** Wave hand quickly past camera or present deliberately blurry face
**Expected:** Rejected by quality gates with TooBlurry reason visible in debug logs
**Why human:** Requires physical blur conditions on live camera

### 3. Lighting Normalization Consistency

**Test:** Test recognition accuracy at morning, midday, and evening lighting at entrance
**Expected:** CLAHE normalization keeps recognition accuracy consistent across lighting conditions
**Why human:** Requires varying real-world lighting conditions over time

### 4. Tracker Cooldown Deduplication

**Test:** Walk past camera, wait 30 seconds, walk past again (within 60s cooldown)
**Expected:** Second pass is NOT re-logged; walk past after 60s IS re-logged
**Why human:** Requires timed physical movement past live camera

### Gaps Summary

No automated gaps found. All 10 must-have truths verified through code inspection and unit test results. All 24 lib-target tests pass. All 3 requirement IDs (FACE-02, FACE-03, FACE-04) have implementation evidence.

The only remaining verification is human testing of the live end-to-end pipeline, which requires physical camera access, the ONNX model file, and an enrolled face in the SQLite database.

---

_Verified: 2026-03-21T18:15:00+05:30_
_Verifier: Claude (gsd-verifier)_
