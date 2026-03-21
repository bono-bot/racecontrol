---
phase: 116-attendance-engine
plan: 01
subsystem: attendance
tags: [sqlite, broadcast, dedup, chrono, tokio]

requires:
  - phase: 115-sentry-enrollment
    provides: "Face recognition pipeline with RecognitionResult type and gallery SQLite DB"
provides:
  - "attendance_log SQLite table with insert/query functions"
  - "AttendanceConfig with dedup_window_secs, present_timeout_secs, min_shift_hours"
  - "Broadcast channel for RecognitionResult events from detection pipeline"
  - "Attendance engine with 5-min cross-camera dedup"
affects: [attendance-api, attendance-reports, shift-tracking]

tech-stack:
  added: []
  patterns: [broadcast-subscriber, cross-camera-dedup, ist-day-boundary]

key-files:
  created:
    - crates/rc-sentry-ai/src/attendance/mod.rs
    - crates/rc-sentry-ai/src/attendance/db.rs
    - crates/rc-sentry-ai/src/attendance/engine.rs
  modified:
    - crates/rc-sentry-ai/src/config.rs
    - crates/rc-sentry-ai/src/detection/pipeline.rs
    - crates/rc-sentry-ai/src/main.rs
    - crates/rc-sentry-ai/src/lib.rs

key-decisions:
  - "Attendance tables share faces.db (gallery_db_path) to avoid second DB file"
  - "IST day boundary via chrono::FixedOffset (no chrono_tz dependency)"
  - "Engine module excluded from lib target (depends on config, binary-only)"

patterns-established:
  - "Broadcast channel pattern: pipeline broadcasts events, consumers subscribe independently"
  - "Cross-camera dedup: HashMap<person_id, Instant> with configurable window"

requirements-completed: [ATTN-01]

duration: 3min
completed: 2026-03-21
---

# Phase 116 Plan 01: Attendance Engine Summary

**Broadcast channel wiring from detection pipeline to attendance engine with 5-min cross-camera dedup and SQLite attendance_log table**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T17:42:13Z
- **Completed:** 2026-03-21T17:45:20Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- attendance_log SQLite table with person_id, camera_id, confidence, day, logged_at columns
- AttendanceConfig with configurable dedup window (default 300s), present timeout, min shift hours
- Detection pipeline broadcasts RecognitionResult via tokio::broadcast channel
- Attendance engine subscribes, deduplicates same person across cameras within 5-min window
- IST day boundary (UTC+5:30) for correct day field computation

## Task Commits

Each task was committed atomically:

1. **Task 1: Attendance SQLite schema and broadcast channel wiring** - `31fcea0` (feat)
2. **Task 2: Attendance engine subscriber with cross-camera dedup** - `90819dc` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/attendance/mod.rs` - Module declaration (db + engine)
- `crates/rc-sentry-ai/src/attendance/db.rs` - SQLite schema, insert, query, get_last_seen with 4 tests
- `crates/rc-sentry-ai/src/attendance/engine.rs` - Broadcast subscriber with dedup and spawn_blocking inserts
- `crates/rc-sentry-ai/src/config.rs` - AttendanceConfig struct with serde defaults
- `crates/rc-sentry-ai/src/detection/pipeline.rs` - Added recognition_tx broadcast sender param
- `crates/rc-sentry-ai/src/main.rs` - Broadcast channel creation, attendance engine spawn, pipeline wiring
- `crates/rc-sentry-ai/src/lib.rs` - Registered attendance::db in lib target

## Decisions Made
- Shared faces.db for attendance tables (no second DB file) -- simplifies deployment
- Used chrono::FixedOffset::east_opt(19800) for IST instead of adding chrono_tz dependency
- Excluded engine from lib target since it depends on config (binary-only module); db remains testable via lib

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Pipeline call site in main.rs needed None for new param**
- **Found during:** Task 1
- **Issue:** Adding recognition_tx param to pipeline::run() broke the existing call in main.rs
- **Fix:** Passed `None` temporarily (Task 2 replaced with `Some(tx)`)
- **Files modified:** crates/rc-sentry-ai/src/main.rs
- **Committed in:** 31fcea0

**2. [Rule 3 - Blocking] Engine excluded from lib target**
- **Found during:** Task 2
- **Issue:** lib.rs declared `pub mod attendance { pub mod engine; }` but engine uses `crate::config` which is binary-only
- **Fix:** Removed engine from lib.rs attendance module, kept db only
- **Files modified:** crates/rc-sentry-ai/src/lib.rs
- **Committed in:** 90819dc

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
- ONNX Runtime linker errors when running tests via binary target (known issue) -- used `--lib` flag for test execution

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Attendance engine fully wired, ready for API endpoints (query attendance_log)
- Future plans can add shift tracking using present_timeout_secs and min_shift_hours config

---
*Phase: 116-attendance-engine*
*Completed: 2026-03-21*
