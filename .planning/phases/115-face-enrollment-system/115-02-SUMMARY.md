---
phase: 115-face-enrollment-system
plan: 02
subsystem: api
tags: [axum, enrollment, face-recognition, scrfd, arcface, sqlite, audit, rest-api]

# Dependency graph
requires:
  - phase: 115-face-enrollment-system (plan 01)
    provides: enrollment types, extended DB CRUD, gallery add/remove methods
provides:
  - Enrollment HTTP API (6 endpoints) at :8096
  - Photo upload pipeline (SCRFD->quality->align->CLAHE->ArcFace->duplicate->persist->gallery)
  - EnrollmentConfig with stricter quality thresholds
  - EnrollmentState shared state for all handlers
affects: [115-face-enrollment-system]

# Tech tracking
tech-stack:
  added: []
  patterns: [optional-ml-models-graceful-degradation, enrollment-quality-thresholds-stricter-than-live]

key-files:
  created:
    - crates/rc-sentry-ai/src/enrollment/routes.rs
    - crates/rc-sentry-ai/src/enrollment/service.rs
  modified:
    - crates/rc-sentry-ai/src/enrollment/mod.rs
    - crates/rc-sentry-ai/src/enrollment/types.rs
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/recognition/db.rs
    - crates/rc-sentry-ai/src/main.rs

key-decisions:
  - "Optional detector/recognizer in EnrollmentState for graceful degradation (CRUD works without ML models)"
  - "Enrollment quality thresholds stricter than live detection (face_size=120 vs 80, laplacian=150 vs 100, yaw=30 vs 45)"
  - "Duplicate detection warns but does not reject (returns DuplicateWarning in response)"

patterns-established:
  - "Optional ML models: EnrollmentState uses Option<Arc<T>> for detector/recognizer, returns 503 for photo upload if unavailable"
  - "Enrollment-specific QualityGates: separate config section with stricter thresholds"

requirements-completed: [ENRL-01, ENRL-02]

# Metrics
duration: 4min
completed: 2026-03-21
---

# Phase 115 Plan 02: Enrollment API Summary

**Axum REST API with 6 endpoints for person CRUD and photo upload with full SCRFD->ArcFace ML pipeline, duplicate detection, gallery sync, and audit logging**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T17:26:12Z
- **Completed:** 2026-03-21T17:30:17Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Complete enrollment HTTP API: POST/GET persons, GET/PUT/DELETE persons/:id, POST persons/:id/photos
- Photo upload pipeline chains decode->SCRFD detect->quality gates->align->CLAHE->ArcFace embed->duplicate check->DB persist->gallery sync->audit log
- EnrollmentConfig with stricter quality thresholds for enrollment vs live detection
- Graceful degradation: CRUD endpoints work even without ML models loaded; photo upload returns 503

## Task Commits

Each task was committed atomically:

1. **Task 1: Enrollment service + config + route handlers** - `d0a28f1` (feat)
2. **Task 2: Wire enrollment into main.rs and verify full build** - `a7e1dc8` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/enrollment/service.rs` - EnrollmentState, EnrollmentError, process_photo pipeline
- `crates/rc-sentry-ai/src/enrollment/routes.rs` - 6 Axum handlers with enrollment_router builder
- `crates/rc-sentry-ai/src/enrollment/mod.rs` - Module declarations (routes, service, types)
- `crates/rc-sentry-ai/src/enrollment/types.rs` - Added enrollment_status_with_threshold
- `crates/rc-sentry-ai/src/config.rs` - EnrollmentConfig with 7 config fields and defaults
- `crates/rc-sentry-ai/src/recognition/db.rs` - insert_embedding now returns embedding ID
- `crates/rc-sentry-ai/src/main.rs` - mod enrollment, shared_detector extraction, EnrollmentState init, router merge

## Decisions Made
- Used Optional detector/recognizer in EnrollmentState for graceful degradation (CRUD endpoints work without ML models loaded)
- Set enrollment quality thresholds stricter than live detection: min_face_size=120 (vs 80), min_laplacian_var=150 (vs 100), max_yaw_degrees=30 (vs 45)
- Duplicate detection warns but does not reject (DuplicateWarning in PhotoUploadResponse)
- Extracted SCRFD detector to Option<Arc> at outer scope in main.rs to share between detection pipeline and enrollment

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] insert_embedding return type changed to i64**
- **Found during:** Task 1 (service.rs implementation)
- **Issue:** PhotoUploadResponse requires embedding_id but insert_embedding returned ()
- **Fix:** Changed insert_embedding to return last_insert_rowid() as i64
- **Files modified:** crates/rc-sentry-ai/src/recognition/db.rs
- **Verification:** All 38 lib tests pass, cargo check clean
- **Committed in:** d0a28f1 (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added enrollment_status_with_threshold**
- **Found during:** Task 1 (service.rs implementation)
- **Issue:** enrollment_status used hardcoded threshold of 3, but EnrollmentConfig has configurable min_embeddings_complete
- **Fix:** Added enrollment_status_with_threshold function to types.rs
- **Files modified:** crates/rc-sentry-ai/src/enrollment/types.rs
- **Verification:** All existing tests pass unchanged
- **Committed in:** d0a28f1 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both fixes necessary for correct operation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Enrollment API fully wired and compiling
- Ready for integration testing and end-to-end verification
- All 38 lib tests pass with no regressions

---
*Phase: 115-face-enrollment-system*
*Completed: 2026-03-21*
