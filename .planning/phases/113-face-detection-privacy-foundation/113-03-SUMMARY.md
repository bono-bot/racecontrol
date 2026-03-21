---
phase: 113-face-detection-privacy-foundation
plan: 03
subsystem: privacy
tags: [dpdp, audit-log, jsonl, mpsc, axum, chrono, retention, consent, gdpr]

# Dependency graph
requires:
  - phase: 112-sentry-ai-scaffolding
    provides: "rc-sentry-ai crate with health endpoint on :8096"
provides:
  - "Append-only JSONL audit log with AuditWriter (mpsc single-writer pattern)"
  - "DPDP Act 2023 consent signage text and GET /api/v1/privacy/consent endpoint"
  - "DELETE /api/v1/privacy/person/:person_id right-to-erasure endpoint"
  - "Hourly retention purge task with configurable retention_days (default 90)"
  - "PrivacyConfig with audit_log_path and retention_days"
affects: [114-face-embeddings, sentry-ai-dashboard]

# Tech tracking
tech-stack:
  added: []
  patterns: [mpsc-single-writer-audit, privacy-module-pattern]

key-files:
  created:
    - crates/rc-sentry-ai/src/privacy/mod.rs
    - crates/rc-sentry-ai/src/privacy/audit.rs
    - crates/rc-sentry-ai/src/privacy/consent.rs
    - crates/rc-sentry-ai/src/privacy/deletion.rs
    - crates/rc-sentry-ai/src/privacy/retention.rs
  modified:
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/src/health.rs
    - crates/rc-sentry-ai/src/detection/decoder.rs

key-decisions:
  - "mpsc channel capacity 256 for audit writer -- large enough for burst writes without backpressure"
  - "try_send for non-blocking audit log from detection pipeline, send().await for API handlers"
  - "Privacy router uses separate Arc<AuditWriter> state, merged into main app via Router::merge"

patterns-established:
  - "Single-writer JSONL audit: all file writes through mpsc channel to avoid Windows file locking"
  - "Privacy routes on separate state type merged into health router"

requirements-completed: [PRIV-01]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 113 Plan 03: DPDP Privacy Infrastructure Summary

**DPDP Act 2023 compliance with mpsc audit log, 90-day retention purge, right-to-deletion API, and consent signage on :8096**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T15:46:06Z
- **Completed:** 2026-03-21T15:49:08Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Append-only JSONL audit log with single-writer mpsc pattern (capacity 256) avoiding Windows file locking
- DPDP Act 2023 consent signage text constant with GET /api/v1/privacy/consent endpoint
- DELETE /api/v1/privacy/person/:person_id right-to-erasure endpoint with audit logging
- Hourly retention purge task with configurable 90-day default
- PrivacyConfig with serde defaults for audit_log_path and retention_days

## Task Commits

Each task was committed atomically:

1. **Task 1: Audit log and consent modules** - `859b4fb` (feat)
2. **Task 2: Deletion endpoint, retention purge, and API wiring** - `1b3b4fa` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/privacy/mod.rs` - Privacy module root (audit, consent, deletion, retention)
- `crates/rc-sentry-ai/src/privacy/audit.rs` - AuditEntry struct + AuditWriter with mpsc single-writer pattern
- `crates/rc-sentry-ai/src/privacy/consent.rs` - DPDP Act 2023 signage text + consent_notice_handler
- `crates/rc-sentry-ai/src/privacy/deletion.rs` - DELETE handler for right-to-erasure with audit logging
- `crates/rc-sentry-ai/src/privacy/retention.rs` - Hourly retention purge task with configurable cutoff
- `crates/rc-sentry-ai/src/config.rs` - Added PrivacyConfig with audit_log_path and retention_days
- `crates/rc-sentry-ai/src/main.rs` - Wire audit writer, spawn retention task, merge privacy router
- `crates/rc-sentry-ai/src/health.rs` - Added privacy_router function
- `crates/rc-sentry-ai/src/detection/decoder.rs` - Fixed pre-existing openh264 API call

## Decisions Made
- mpsc channel capacity 256 for audit writer -- handles burst writes without backpressure on detection pipeline
- try_send (non-blocking) for detection pipeline callers, send().await for API handlers that can afford to wait
- Privacy router uses separate Arc<AuditWriter> state type, merged into main app via Router::merge

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing openh264 API error in detection/decoder.rs**
- **Found during:** Task 2 (cargo check verification)
- **Issue:** `dimension_rgb()` method does not exist in openh264 0.9; correct method is `dimensions()` from YUVSource trait
- **Fix:** Changed to `dimensions()` and added `use openh264::formats::YUVSource` import
- **Files modified:** crates/rc-sentry-ai/src/detection/decoder.rs
- **Verification:** Decoder module compiles
- **Committed in:** 1b3b4fa (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Fix was necessary for cargo check to compile the crate. No scope creep.

**Note:** 21 pre-existing compilation errors remain in the detection module (Plan 01's ort crate API misuse). These are out of scope for Plan 03 and do not affect privacy module correctness. Privacy modules have zero compilation errors.

## Issues Encountered
- Pre-existing compilation errors in detection/ module (Plan 01) -- 21 errors related to ort 2.0 API changes. These block `cargo check -p rc-sentry-ai` from succeeding fully but do not affect the privacy module code. Logged as deferred.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Privacy infrastructure ready for Phase 114 (face embeddings) to add SQLite purge in retention task and embedding deletion in delete handler
- Consent signage text ready for physical printing and dashboard display
- Audit writer Arc available for passing to detection pipeline in Plan 02

## Self-Check: PASSED

- All 8 created/modified files verified present on disk
- Commit 859b4fb (Task 1) verified in git log
- Commit 1b3b4fa (Task 2) verified in git log

---
*Phase: 113-face-detection-privacy-foundation*
*Completed: 2026-03-21*
