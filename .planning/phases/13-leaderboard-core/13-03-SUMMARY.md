---
phase: 13-leaderboard-core
plan: 03
subsystem: notification
tags: [email, track-record, notification, tokio-spawn, fire-and-forget, tdd]

# Dependency graph
requires:
  - phase: 13-leaderboard-core
    provides: laps table with suspect column, track_records table, persist_lap() function
provides:
  - get_previous_record_holder() public function for fetching record holder data
  - Track record "beaten" email notification in persist_lap() (fire-and-forget)
  - format_lap_time() helper for M:SS.mmm display
  - 3 integration tests for notification data ordering
affects: [13-leaderboard-core, 14-events-and-championships]

# Tech tracking
tech-stack:
  added: []
  patterns: [fetch-before-upsert, fire-and-forget-email, nickname-aware-display]

key-files:
  created: []
  modified:
    - crates/rc-core/src/lap_tracker.rs
    - crates/rc-core/tests/integration.rs

key-decisions:
  - "Previous record holder data (name, email) fetched BEFORE the UPSERT to avoid reading back the new holder's data"
  - "Notification is fire-and-forget via tokio::spawn -- failure does not block lap persistence or track record update"
  - "New holder display name uses nickname if show_nickname_on_leaderboard=1 and nickname is NOT NULL (NTF-02)"
  - "NULL email on previous holder silently skips notification with debug-level log"
  - "get_previous_record_holder() extracted as public function for testability"

patterns-established:
  - "Fetch-before-UPSERT: always capture previous state before ON CONFLICT DO UPDATE"
  - "Fire-and-forget email: tokio::spawn with Command::new('node').arg(send_email.js) pattern"
  - "Nickname-aware display: CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END"

requirements-completed: [NTF-01, NTF-02]

# Metrics
duration: 8min
completed: 2026-03-15
---

# Phase 13 Plan 03: Track Record Notification Summary

**Fire-and-forget email notification to previous record holder when their track record is beaten, with fetch-before-UPSERT data ordering and nickname-aware new holder display**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-14T21:38:28Z
- **Completed:** 2026-03-14T21:46:39Z
- **Tasks:** 2 (TDD RED + GREEN)
- **Files modified:** 2

## Accomplishments
- Restructured persist_lap() track record section: previous holder data (name, email, best time) fetched BEFORE the UPSERT to prevent data loss
- Added get_previous_record_holder() as a public helper function testable independently from persist_lap()
- New record holder display name fetched with nickname awareness (NTF-02: show_nickname_on_leaderboard flag)
- Fire-and-forget email notification via tokio::spawn + node send_email.js with full error handling and logging
- Email body includes track, car, old time (M:SS.mmm), new time, new holder name, and public leaderboard link
- 3 integration tests covering: data ordering verification, NULL email skip, first record no-notify
- Full test suite green: rc-core 209 unit + 34 integration = 243 tests pass

## Task Commits

Each task was committed atomically:

1. **TDD RED: Failing notification data ordering tests** - `f916ec2` (test)
2. **TDD GREEN: Notification implementation + get_previous_record_holder** - `d76ff9d` (feat)

_TDD REFACTOR phase not needed -- implementation is clean and well-structured._

## Files Created/Modified
- `crates/rc-core/src/lap_tracker.rs` - Added get_previous_record_holder(), restructured track record section with fetch-before-UPSERT, tokio::spawn email notification, format_lap_time() helper
- `crates/rc-core/tests/integration.rs` - Added 3 notification tests: test_notification_data_before_upsert, test_notification_skip_no_email, test_notification_first_record_no_notify

## Decisions Made
- Previous record holder data (name, email) is fetched BEFORE the UPSERT. This is the critical data ordering requirement -- if fetched after, the ON CONFLICT DO UPDATE would have already replaced the holder with the new driver.
- Notification is fire-and-forget via tokio::spawn. The spawned task handles its own error logging. Failure never affects lap persistence or track record update.
- New holder display name uses the nickname-aware CASE expression (NTF-02), matching the leaderboard display logic.
- NULL email on previous holder silently skips notification with a debug-level log, no crash.
- get_previous_record_holder() is a standalone public function for testability -- tests can call it directly to verify data before/after UPSERT without needing to intercept the email spawn.

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered
None -- plan executed cleanly.

## User Setup Required
None - no external service configuration required. Uses existing send_email.js and watchdog.email_script_path config.

## Next Phase Readiness
- Track record notification system is complete and tested
- get_previous_record_holder() is available for any future code that needs record holder data
- Email notification pattern (tokio::spawn + node send_email.js) can be reused for other notification types
- Ready for 13-04 (next plan in leaderboard core phase)

## Self-Check: PASSED

- All 2 modified files exist on disk
- Both commit hashes (f916ec2, d76ff9d) verified in git log
- All 243 tests pass across rc-core (209 unit + 34 integration)

---
*Phase: 13-leaderboard-core*
*Completed: 2026-03-15*
