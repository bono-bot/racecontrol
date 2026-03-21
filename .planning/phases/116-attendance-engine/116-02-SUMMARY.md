---
phase: 116-attendance-engine
plan: 02
subsystem: attendance
tags: [sqlite, shifts, clock-in, clock-out, staff-tracking]

requires:
  - phase: 116-01
    provides: attendance_log table, AttendanceConfig, engine loop, broadcast channel
provides:
  - staff_shifts SQLite table with UNIQUE(person_id, day)
  - upsert_shift clock-in/clock-out state machine
  - is_staff role check against persons table
  - process_staff_recognition orchestrator
  - shift query functions (by day, by person)
affects: [116-03-attendance-api]

tech-stack:
  added: []
  patterns: [synchronous shift tracking inside spawn_blocking alongside attendance insert]

key-files:
  created:
    - crates/rc-sentry-ai/src/attendance/shifts.rs
  modified:
    - crates/rc-sentry-ai/src/attendance/db.rs
    - crates/rc-sentry-ai/src/attendance/engine.rs
    - crates/rc-sentry-ai/src/attendance/mod.rs
    - crates/rc-sentry-ai/src/lib.rs

key-decisions:
  - "Shift minutes computed from clock_in/clock_out string diff using chrono NaiveDateTime"
  - "is_staff queries persons table directly (no caching) since it runs inside spawn_blocking"
  - "min_shift_hours passed through but unused in Plan 02 (Plan 03 uses it for API flagging)"

patterns-established:
  - "Shift tracking co-located in same spawn_blocking as attendance insert for atomicity"

requirements-completed: [ATTN-02]

duration: 3min
completed: 2026-03-21
---

# Phase 116 Plan 02: Staff Shift Tracking Summary

**Staff clock-in/clock-out state machine with SQLite persistence and automatic shift tracking on recognition events**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T17:47:09Z
- **Completed:** 2026-03-21T17:50:14Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- staff_shifts table with UNIQUE(person_id, day) constraint for one shift per person per day
- Automatic clock-in on first staff recognition of the day, clock-out updated on every subsequent recognition
- is_staff check against persons.role field to filter non-staff recognition events
- 14 attendance tests passing (10 db + 4 shifts)

## Task Commits

Each task was committed atomically:

1. **Task 1: Staff shifts SQLite schema and shift state machine** - `114efa0` (feat)
2. **Task 2: Wire shift tracking into attendance engine loop** - `e5bf207` (feat)

## Files Created/Modified
- `crates/rc-sentry-ai/src/attendance/shifts.rs` - Staff recognition processor (is_staff gate + upsert_shift)
- `crates/rc-sentry-ai/src/attendance/db.rs` - staff_shifts table, ShiftEntry/ShiftAction types, upsert/query functions, is_staff
- `crates/rc-sentry-ai/src/attendance/engine.rs` - Shift tracking wired into spawn_blocking after attendance insert
- `crates/rc-sentry-ai/src/attendance/mod.rs` - Added pub mod shifts
- `crates/rc-sentry-ai/src/lib.rs` - Added shifts module to lib target for test coverage

## Decisions Made
- Shift minutes computed from clock_in/clock_out string diff using chrono NaiveDateTime parsing
- is_staff queries persons table directly (no caching) -- acceptable since it runs inside spawn_blocking and shift checks are deduped
- min_shift_hours parameter threaded through but not acted on in Plan 02 (Plan 03 API will flag incomplete shifts)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added shifts module to lib.rs**
- **Found during:** Task 1
- **Issue:** shifts.rs tests would not run because lib.rs (used for unit testing to avoid ONNX linker issues) did not include the shifts module
- **Fix:** Added `pub mod shifts;` to the attendance module in lib.rs
- **Files modified:** crates/rc-sentry-ai/src/lib.rs
- **Verification:** cargo test --lib passes with all 14 attendance tests
- **Committed in:** 114efa0 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for test execution. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Shift data is being persisted on every staff recognition event
- Query functions ready for Plan 03 attendance API (get_shifts_for_day, get_shifts_for_person, get_shift)
- min_shift_hours available for incomplete shift flagging in API responses

---
*Phase: 116-attendance-engine*
*Completed: 2026-03-21*
