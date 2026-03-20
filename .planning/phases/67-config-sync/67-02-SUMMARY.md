---
phase: 67-config-sync
plan: 02
subsystem: api
tags: [rust, axum, sync, config, venue, snapshot, serde]

# Dependency graph
requires:
  - phase: 67-01
    provides: "ConfigSanitizer that produces config_snapshot payload shape (venue/pods/branding/_meta)"
provides:
  - "VenueConfigSnapshot struct in state.rs with all venue/pods/branding/meta fields"
  - "AppState.venue_config: RwLock<Option<VenueConfigSnapshot>> field"
  - "parse_config_snapshot() helper function in routes.rs (testable, pure JSON parse)"
  - "config_snapshot branch in sync_push handler -- stores parsed snapshot in AppState"
  - "3 unit tests verifying parse correctness, defaults, and serde roundtrip"
affects: [68-config-serve, any-phase-reading-venue-config-from-AppState]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Extract JSON parsing to named helper function for testability (no AppState needed in tests)"]

key-files:
  created: []
  modified:
    - "crates/racecontrol/src/state.rs"
    - "crates/racecontrol/src/api/routes.rs"

key-decisions:
  - "parse_config_snapshot extracted as pub(crate) fn for testability -- sync_push calls it rather than inlining"
  - "config_snapshot uses total += 1 (single record semantics, not per-field count) -- consistent with other upserts"
  - "Structured tracing on receipt: venue name, pod count, hash prefix (first 8 chars of hash)"

patterns-established:
  - "Parse-helper pattern: extract JSON-to-struct conversion into standalone fn, call from async handler"
  - "venue_config field follows existing RwLock<Option<T>> pattern for nullable shared state"

requirements-completed: [SYNC-03]

# Metrics
duration: 8min
completed: 2026-03-20
---

# Phase 67 Plan 02: Config Sync -- Cloud Receive Summary

**VenueConfigSnapshot struct + parse_config_snapshot() added to cloud racecontrol, wiring James config into AppState via /sync/push config_snapshot branch with 3 passing unit tests**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-20T13:11:17Z
- **Completed:** 2026-03-20T13:20:07Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added VenueConfigSnapshot struct to state.rs (18 fields: venue name/location/timezone, pod count/discovery/healer settings, branding color/theme, source/pushed_at/hash/received_at)
- Added venue_config: RwLock<Option<VenueConfigSnapshot>> to AppState with None initialization
- Extracted parse_config_snapshot() helper function in routes.rs for testable JSON parsing
- Added config_snapshot branch in sync_push handler: parses via helper, logs structured trace (venue, pods, hash prefix), stores in AppState.venue_config
- 3 unit tests: full payload parse, defaults for empty payload, serde roundtrip

## Task Commits

Each task was committed atomically:

1. **Task 1: Add VenueConfigSnapshot to AppState and handle config_snapshot in sync_push** - `e7366cb` (feat)
2. **Task 2: Unit test for config_snapshot handler** - `f5a9a71` (test)

## Files Created/Modified
- `crates/racecontrol/src/state.rs` - VenueConfigSnapshot struct + venue_config field on AppState
- `crates/racecontrol/src/api/routes.rs` - parse_config_snapshot() helper + config_snapshot branch in sync_push + 3 unit tests

## Decisions Made
- parse_config_snapshot extracted as `pub(crate) fn` so tests can call it without a full AppState or Axum test harness
- config_snapshot counts as 1 record toward total (not per-field) -- consistent with how pod upserts work
- Structured tracing truncates hash to first 8 chars for readability in log output

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing test failure `config::tests::config_fallback_preserved_when_no_env_vars` unrelated to this plan's changes -- confirmed pre-existing via git stash check. Not fixed (out of scope).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- cloud racecontrol now stores the latest venue config in AppState.venue_config after each config sync push
- Ready for Phase 68: expose venue_config via API endpoint so Bono can read back the stored snapshot
- Existing sync_push behavior (laps, track_records, personal_bests, billing_sessions, etc.) completely unchanged

---
*Phase: 67-config-sync*
*Completed: 2026-03-20*
