---
phase: 114-face-recognition-quality-gates
plan: 03
subsystem: ai
tags: [face-recognition, cosine-similarity, sqlite, arcface, gallery, tracker, pipeline]

requires:
  - phase: 114-01
    provides: "Quality gates (size, blur, pose) and CLAHE lighting normalization"
  - phase: 114-02
    provides: "ArcFace embedding extraction and face alignment"
provides:
  - "Embedding gallery with cosine similarity matching at 0.45 threshold"
  - "SQLite persistence for persons and face embeddings (BLOB storage)"
  - "Face tracker with 60-second per-person cooldown"
  - "RecognitionConfig TOML section with all threshold values"
  - "Full recognition pipeline: SCRFD -> quality -> CLAHE -> align -> ArcFace -> gallery -> tracker"
affects: [face-enrollment, identity-api, recognition-alerts]

tech-stack:
  added: [rusqlite-0.32-bundled]
  patterns: [cosine-similarity-matching, blob-embedding-storage, cooldown-tracker, graceful-degradation]

key-files:
  created:
    - crates/rc-sentry-ai/src/recognition/gallery.rs
    - crates/rc-sentry-ai/src/recognition/db.rs
    - crates/rc-sentry-ai/src/recognition/tracker.rs
  modified:
    - crates/rc-sentry-ai/src/recognition/mod.rs
    - crates/rc-sentry-ai/src/lib.rs
    - crates/rc-sentry-ai/Cargo.toml
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/detection/pipeline.rs
    - crates/rc-sentry-ai/src/main.rs

key-decisions:
  - "Used rusqlite 0.32 with bundled SQLite for zero-dependency deployment"
  - "Embeddings stored as little-endian f32 BLOBs (2048 bytes per 512-D vector)"
  - "Gallery uses tokio RwLock for concurrent read access during matching"
  - "Tracker uses std Mutex (not tokio) since operations are sub-microsecond"
  - "Recognition is None-gated: pipeline skips recognition when ArcFace init fails"

patterns-established:
  - "Cosine similarity for L2-normalized embeddings (dot product shortcut)"
  - "Periodic cleanup pattern for tracker memory management (every 5 min)"
  - "Graceful degradation: recognition disabled does not affect detection pipeline"

requirements-completed: [FACE-02, FACE-03, FACE-04]

duration: 4min
completed: 2026-03-21
---

# Phase 114 Plan 03: Recognition Pipeline Wiring Summary

**Cosine similarity gallery with SQLite persistence, face tracker cooldown, and full pipeline integration from SCRFD through ArcFace to identity logging**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T17:01:49Z
- **Completed:** 2026-03-21T17:05:44Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- In-memory embedding gallery matches faces via cosine similarity at 0.45 threshold with SQLite backend
- Face tracker suppresses redundant recognitions within 60-second cooldown per person
- Full pipeline chain wired: SCRFD detect -> quality gates -> CLAHE -> alignment -> ArcFace -> gallery match -> tracker -> log
- RecognitionConfig added with all threshold values, model paths, and DB path
- 10 new unit tests (gallery: 5, db: 2, tracker: 3), all 24 lib tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Embedding gallery, SQLite persistence, and face tracker** - `056327f` (feat)
2. **Task 2: Config extension and pipeline integration** - `a275a62` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/recognition/gallery.rs` - In-memory gallery with cosine similarity matching and RwLock
- `crates/rc-sentry-ai/src/recognition/db.rs` - SQLite schema, CRUD for persons/embeddings, BLOB serialization
- `crates/rc-sentry-ai/src/recognition/tracker.rs` - Per-person cooldown tracker with cleanup
- `crates/rc-sentry-ai/src/recognition/mod.rs` - Added gallery, db, tracker module declarations
- `crates/rc-sentry-ai/src/lib.rs` - Added gallery, db, tracker to lib test target
- `crates/rc-sentry-ai/Cargo.toml` - Added rusqlite 0.32 with bundled feature
- `crates/rc-sentry-ai/src/config.rs` - Added RecognitionConfig struct with defaults
- `crates/rc-sentry-ai/src/detection/pipeline.rs` - Wired full recognition chain after SCRFD detection
- `crates/rc-sentry-ai/src/main.rs` - Initialize ArcFace, gallery, tracker, quality gates; periodic cleanup task

## Decisions Made
- Used rusqlite 0.32 with bundled SQLite for zero-dependency deployment on pods
- Stored embeddings as little-endian f32 BLOBs (2048 bytes per 512-D vector) for portable binary format
- Gallery uses tokio::sync::RwLock for concurrent read access during multi-camera matching
- Tracker uses std::sync::Mutex since operations are sub-microsecond and never async
- Recognition is Option-gated: pipeline gracefully skips when ArcFace init fails or recognition disabled

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- cargo test with bin target fails due to ort/MSVC static CRT linker issue (known, documented in objective). Used --lib flag for all test runs.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Full recognition pipeline is operational end-to-end
- Gallery loads from SQLite at startup; enrollment API needed for adding new persons
- Ready for face enrollment endpoints and recognition alert integration

---
*Phase: 114-face-recognition-quality-gates*
*Completed: 2026-03-21*
