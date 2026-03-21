---
phase: 115-face-enrollment-system
verified: 2026-03-21T18:15:00+05:30
status: gaps_found
score: 9/10 must-haves verified
re_verification: false
gaps:
  - truth: "Duplicate detection prevents enrolling the same person twice by checking new embeddings against existing ones"
    status: partial
    reason: "Implementation warns about duplicates but does NOT reject/prevent enrollment. Photo upload returns 200 with DuplicateWarning in response body instead of blocking the enrollment. Success criterion SC4 says 'prevents enrolling the same person twice' but code allows it."
    artifacts:
      - path: "crates/rc-sentry-ai/src/enrollment/service.rs"
        issue: "Lines 189-203: duplicate_warning is informational only. process_photo succeeds regardless of duplicate match, embedding is persisted and gallery updated even when duplicate is found."
    missing:
      - "Option A: Return EnrollmentError (e.g., 409 Conflict) when similarity > duplicate_threshold for a different person, preventing the embedding from being saved"
      - "Option B: If warn-only is intentional, update ROADMAP success criterion SC4 to say 'warns about' instead of 'prevents'"
---

# Phase 115: Face Enrollment System Verification Report

**Phase Goal:** Staff can add, update, and remove face profiles to build the recognition database
**Verified:** 2026-03-21T18:15:00+05:30
**Status:** gaps_found
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | persons table has phone column (idempotent migration) | VERIFIED | db.rs:42-49 -- SELECT probe + ALTER TABLE, tested in test_phone_column_migration |
| 2 | DB functions exist for get_person, list_persons, update_person, delete_person, embedding_count | VERIFIED | db.rs:105-181 -- all 5 functions with full implementations and tests |
| 3 | Gallery has add_entry() and remove_person() methods | VERIFIED | gallery.rs:63-72 -- both methods implemented with RwLock write, tested |
| 4 | Enrollment request/response DTOs are defined with serde derives | VERIFIED | types.rs:1-76 -- CreatePersonRequest, UpdatePersonRequest, PersonResponse, PhotoUploadResponse, DuplicateWarning, ErrorResponse all present |
| 5 | Staff can create/list/get/update/delete persons via REST API | VERIFIED | routes.rs:21-41 -- all 6 routes wired in enrollment_router, handlers at lines 43-334 |
| 6 | Photo upload runs quality gates, detection, alignment, CLAHE, ArcFace | VERIFIED | service.rs:97-249 -- full pipeline: decode -> SCRFD detect -> quality gates -> align_face -> apply_clahe -> ArcFace preprocess -> extract_embedding -> persist -> gallery sync |
| 7 | Multi-angle enrollment tracks embedding count, status at >= 3 | VERIFIED | types.rs:65-76 enrollment_status/enrollment_status_with_threshold, config.rs default min_embeddings_complete=3 |
| 8 | All enrollment operations are audit-logged | VERIFIED | routes.rs:68-74 (create), 240-246 (update), 297-303 (delete), service.rs:233-241 (photo) |
| 9 | Enrollment wired into main.rs with shared detector/recognizer/gallery | VERIFIED | main.rs merges enrollment_router, creates EnrollmentState with shared Arc refs, graceful degradation warn |
| 10 | Duplicate detection prevents enrolling the same person twice | FAILED | service.rs:189-203 warns but does not reject; embedding is persisted and gallery updated regardless |

**Score:** 9/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-sentry-ai/src/recognition/db.rs` | Extended CRUD + phone migration | VERIFIED | 361 lines, PersonInfo struct, 5 CRUD functions, idempotent phone migration, 10 tests |
| `crates/rc-sentry-ai/src/recognition/gallery.rs` | add_entry and remove_person | VERIFIED | 212 lines, both methods implemented with proper RwLock, 7 tests |
| `crates/rc-sentry-ai/src/enrollment/types.rs` | Request/response DTOs | VERIFIED | 144 lines, 6 types + enrollment_status functions, 4 tests |
| `crates/rc-sentry-ai/src/enrollment/mod.rs` | Module declarations | VERIFIED | 3 lines: pub mod routes, service, types |
| `crates/rc-sentry-ai/src/enrollment/routes.rs` | Axum HTTP handlers | VERIFIED | 335 lines, 6 handlers, enrollment_router builder with body limit |
| `crates/rc-sentry-ai/src/enrollment/service.rs` | Photo processing pipeline | VERIFIED | 250 lines, EnrollmentState, EnrollmentError, process_photo with full ML pipeline |
| `crates/rc-sentry-ai/src/config.rs` | EnrollmentConfig | VERIFIED | Lines 170-228, 7 config fields with defaults, stricter than live thresholds |
| `crates/rc-sentry-ai/src/lib.rs` | Enrollment module exposed | VERIFIED | Line 21-23: pub mod enrollment with pub mod types |
| `crates/rc-sentry-ai/src/main.rs` | Router merge + state init | VERIFIED | EnrollmentState created, enrollment_router merged into app |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| db.rs | persons table | ALTER TABLE phone migration | WIRED | Line 46: ALTER TABLE persons ADD COLUMN phone |
| types.rs | db.rs | PersonResponse maps to PersonInfo | WIRED | routes.rs maps PersonInfo fields to PersonResponse in all handlers |
| routes.rs | service.rs | handlers call service::process_photo | WIRED | Line 330: service::process_photo(&state, id, &body) |
| service.rs | db.rs | spawn_blocking for DB ops | WIRED | Lines 115-123 (get_person), 209-219 (insert_embedding, embedding_count) |
| service.rs | gallery.rs | gallery.add_entry after DB write | WIRED | Lines 222-229: gallery.add_entry(GalleryEntry{...}) |
| routes.rs | gallery.rs | gallery.remove_person on delete | WIRED | Line 295: state.gallery.remove_person(id) |
| main.rs | routes.rs | merge enrollment_router | WIRED | Line 218: .merge(enrollment::routes::enrollment_router(enrollment_state)) |
| routes.rs | audit.rs | AuditEntry logging | WIRED | Lines 68, 240, 297 in routes.rs; line 233 in service.rs |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-----------|-------------|--------|----------|
| ENRL-01 | 115-01, 115-02 | Face profile management (add/remove/update face photos) | SATISFIED | Full CRUD API (create/read/update/delete persons + photo upload), all wired and substantive |
| ENRL-02 | 115-01, 115-02 | Multi-angle enrollment capture for better recognition accuracy | SATISFIED | embedding_count tracking, enrollment_status threshold (>=3 = complete), quality gates with stricter thresholds (face_size=120, laplacian=150, yaw=30) |

No orphaned requirements found -- REQUIREMENTS-v16.md maps only ENRL-01 and ENRL-02 to Phase 115.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | Zero TODOs, FIXMEs, placeholders, or empty implementations found |

### Human Verification Required

### 1. Photo Upload ML Pipeline End-to-End

**Test:** POST a real face photo to /api/v1/enrollment/persons/:id/photos with SCRFD and ArcFace models loaded
**Expected:** Returns 200 with embedding_id, embedding_count incremented, enrollment_status updates
**Why human:** ML model loading and ONNX inference cannot be verified without actual model files and runtime

### 2. Quality Gate Rejection

**Test:** Upload a blurry, small, or extreme-angle face photo
**Expected:** Returns 422 with quality_rejected error and RejectReason details
**Why human:** Quality threshold behavior depends on actual image characteristics

### 3. Duplicate Detection Warning Behavior

**Test:** Upload face of person A under person B's profile
**Expected:** Returns 200 with duplicate_warning containing person A's ID and similarity score
**Why human:** Requires two enrolled persons with actual face embeddings

### Gaps Summary

**One gap found:** The duplicate detection implementation (service.rs:189-203) warns about potential duplicates but does not prevent enrollment. The ROADMAP success criterion SC4 states "Duplicate detection prevents enrolling the same person twice" -- the word "prevents" implies the enrollment should be blocked, not just warned about.

The implementation persists the embedding and updates the gallery regardless of whether a duplicate is detected. The DuplicateWarning is returned in the response body as informational data only.

This is a semantic gap -- the code is functional and well-structured, but the behavior does not match the stated success criterion. Either the code should reject duplicates (return an error status) or the success criterion should be updated to reflect warn-only behavior.

All other aspects of the phase are fully verified: person CRUD, photo processing pipeline, gallery sync, audit logging, config, and main.rs wiring are all substantive and properly connected.

---

_Verified: 2026-03-21T18:15:00+05:30_
_Verifier: Claude (gsd-verifier)_
