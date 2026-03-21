---
phase: 115-face-enrollment-system
plan: 01
subsystem: database
tags: [rusqlite, serde, face-enrollment, gallery, crud]

requires:
  - phase: 114-sentry-ai-debugger
    provides: "Base recognition module with db.rs, gallery.rs, types.rs"
provides:
  - "Full person CRUD (get, list, update, delete) + embedding_count in db.rs"
  - "Idempotent phone column migration in create_tables"
  - "Gallery::add_entry() and Gallery::remove_person() for targeted updates"
  - "Enrollment DTOs: CreatePersonRequest, UpdatePersonRequest, PersonResponse, PhotoUploadResponse, ErrorResponse"
  - "enrollment_status() helper function"
affects: [115-02, 115-03, enrollment-api, face-enrollment]

tech-stack:
  added: []
  patterns: ["idempotent ALTER TABLE migration via SELECT probe", "PRAGMA foreign_keys per-connection for CASCADE"]

key-files:
  created:
    - crates/rc-sentry-ai/src/enrollment/mod.rs
    - crates/rc-sentry-ai/src/enrollment/types.rs
  modified:
    - crates/rc-sentry-ai/src/recognition/db.rs
    - crates/rc-sentry-ai/src/recognition/gallery.rs
    - crates/rc-sentry-ai/src/lib.rs

key-decisions:
  - "Phone column migration uses SELECT probe for idempotency instead of parsing table_info"
  - "PRAGMA foreign_keys = ON set both in create_tables batch and before delete_person for safety"
  - "enrollment_status threshold: >= 3 embeddings = complete"

patterns-established:
  - "Idempotent migration: probe column existence with SELECT before ALTER TABLE"
  - "Gallery mutation: add_entry/remove_person for surgical updates without full reload"

requirements-completed: [ENRL-01, ENRL-02]

duration: 3min
completed: 2026-03-21
---

# Phase 115 Plan 01: Data Layer + Enrollment Types Summary

**Full person CRUD with phone migration, Gallery add/remove methods, and enrollment request/response DTOs for face enrollment API**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T17:21:36Z
- **Completed:** 2026-03-21T17:24:21Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Extended db.rs with PersonInfo struct and 5 new CRUD functions (get_person, list_persons, update_person, delete_person, embedding_count)
- Added idempotent phone column migration and PRAGMA foreign_keys for CASCADE deletes
- Added Gallery::add_entry() and Gallery::remove_person() for targeted gallery updates without full reload
- Created enrollment module with all DTOs (CreatePersonRequest, UpdatePersonRequest, PersonResponse, PhotoUploadResponse, DuplicateWarning, ErrorResponse)
- All 38 tests pass with zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend db.rs with full person CRUD + phone migration** - `33ab964` (feat)
2. **Task 2: Extend Gallery with add_entry/remove_person + create enrollment types** - `1c290c7` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/recognition/db.rs` - PersonInfo struct, phone migration, 5 new CRUD functions, 7 new tests
- `crates/rc-sentry-ai/src/recognition/gallery.rs` - add_entry() and remove_person() methods, 3 new tests
- `crates/rc-sentry-ai/src/enrollment/mod.rs` - Module declaration for enrollment types
- `crates/rc-sentry-ai/src/enrollment/types.rs` - All enrollment DTOs with serde derives, 4 tests
- `crates/rc-sentry-ai/src/lib.rs` - Exposed enrollment module

## Decisions Made
- Phone column migration uses SELECT probe for idempotency (simpler than parsing PRAGMA table_info)
- PRAGMA foreign_keys = ON set in both create_tables and delete_person for safety
- enrollment_status threshold: >= 3 embeddings = "complete", < 3 = "partial"
- insert_person signature changed to accept phone parameter (breaking change handled in existing tests)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All CRUD functions ready for Plan 02 HTTP handler implementation
- Gallery mutation methods ready for enrollment/deletion sync
- All DTOs defined for request parsing and response serialization
- enrollment module exposed in lib target for import

---
*Phase: 115-face-enrollment-system*
*Completed: 2026-03-21*
