---
phase: 01-session-types-race-mode
plan: 01
subsystem: ac-launcher
tags: [rust, serde, ini-builder, session-types, ai-opponents, rand]

# Dependency graph
requires: []
provides:
  - AiCarSlot struct for AI opponent configuration
  - Extended AcLaunchParams with session_type, ai_cars, starting_position, formation_lap, weekend fields
  - Composable write_race_ini() via 18 section writers
  - build_race_ini_string() for testable INI generation
  - parse_ini() test helper for section-level assertions
  - AI_DRIVER_NAMES pool (60 names) + pick_ai_names() shuffler
  - Practice (TYPE=1) and Hotlap (TYPE=4) session generation
  - effective_ai_cars() with MAX_AI_SINGLE_PLAYER cap (19)
affects: [01-02-PLAN, phase-02-ai-race-mode]

# Tech tracking
tech-stack:
  added: [rand 0.8]
  patterns: [composable-ini-builder, section-writer-functions, build-string-then-write]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Used String session_type instead of rc-common SessionType enum for forward compatibility with trackday/weekend modes not in the enum"
  - "Composable INI builder uses writeln! macros into String rather than format! template for maintainability"
  - "effective_ai_cars() generates trackday defaults and caps all modes at 19 AI (AC 20-slot limit)"
  - "AI name pool is 60 international names shuffled per session for variety"

patterns-established:
  - "Composable section writers: each write_*_section() takes &mut String and appends one INI section"
  - "build_race_ini_string() for testable generation, write_race_ini() for disk I/O wrapper"
  - "parse_ini() test helper for section-level assertions in unit tests"
  - "All new AcLaunchParams fields use serde(default) for backward compatibility with existing JSON payloads"

requirements-completed: [SESS-01, SESS-03, SESS-08]

# Metrics
duration: 7min
completed: 2026-03-13
---

# Phase 1 Plan 01: Session Types & Composable INI Builder Summary

**AiCarSlot struct, extended AcLaunchParams with 6 session-type fields, composable write_race_ini() with 18 section writers, Practice (TYPE=1) and Hotlap (TYPE=4) generation, 60-name AI driver pool**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-13T06:19:46Z
- **Completed:** 2026-03-13T06:27:02Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Extended AcLaunchParams with session_type, ai_cars, starting_position, formation_lap, weekend_practice_minutes, weekend_qualify_minutes -- all backward compatible via serde defaults
- Refactored monolithic format! write_race_ini() into 18 composable section writers with build_race_ini_string() for testing
- Practice generates TYPE=1/SPAWN_SET=PIT, Hotlap generates TYPE=4/SPAWN_SET=START
- SESS-08 verified: CARS = 1 + actual ai_cars.len(), no phantom AI ever generated
- 17 unit tests covering deserialization, name pool, INI generation, and no-fallback behavior

## Task Commits

Each task was committed atomically:

1. **Task 1: Define contracts and add rand dependency** - `ec341f9` (feat)
2. **Task 2: Refactor write_race_ini into composable builder** - `9708a48` (feat)

_Note: TDD tasks -- tests written first (RED), then implementation (GREEN)_

## Files Created/Modified
- `crates/rc-agent/src/ac_launcher.rs` - AiCarSlot, extended AcLaunchParams, composable INI builder, AI name pool, 17 tests
- `crates/rc-agent/Cargo.toml` - Added rand 0.8 dependency
- `crates/rc-agent/src/main.rs` - Updated AcLaunchParams struct literals with new fields

## Decisions Made
- Used String for session_type (not rc-common::SessionType enum) because the plan specifies "trackday" and "weekend" modes that don't exist in the enum -- forward compatible without modifying shared types
- Composable section writers use writeln! macros into a String buffer rather than a single format! template -- each section is independently testable and modifiable
- effective_ai_cars() handles Track Day defaults (12 mixed GT3/Supercars at AI level 85) and caps all modes at 19 AI opponents (AC's 20-slot limit including player)
- AI name pool contains 60 internationally diverse names shuffled per session using rand

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated main.rs callsite struct literals**
- **Found during:** Task 1
- **Issue:** Two places in main.rs construct AcLaunchParams directly (not via serde), missing the 6 new fields
- **Fix:** Added session_type, ai_cars, starting_position, formation_lap, weekend_practice_minutes, weekend_qualify_minutes with default values to both struct literals
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo build -p rc-agent succeeds
- **Committed in:** ec341f9 (Task 1 commit)

**2. [Rule 2 - Missing Critical] Added Track Day defaults and AI cap**
- **Found during:** Task 2
- **Issue:** Plan mentions "write_ai_car_sections and weekend path can be stubs" but the composable builder implemented the full session dispatch including trackday defaults and AI count capping for robustness
- **Fix:** Added TRACKDAY_CAR_POOL, generate_trackday_ai(), effective_ai_cars() with MAX_AI_SINGLE_PLAYER=19 cap
- **Files modified:** crates/rc-agent/src/ac_launcher.rs
- **Verification:** All 17 tests pass
- **Committed in:** 9708a48 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both auto-fixes necessary for compilation and robustness. The Track Day/Weekend implementation goes slightly beyond what the plan specified as "stubs" but provides a complete foundation for Plan 02.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Composable builder architecture is ready for Plan 02 to add AI grid generation
- AiCarSlot, effective_ai_cars(), write_ai_car_sections() are all implemented and tested
- All session type dispatch paths exist in write_session_blocks()
- build_race_ini_string() and parse_ini() test infrastructure ready for Plan 02 tests

## Self-Check: PASSED

- 01-01-SUMMARY.md: FOUND
- ec341f9 (Task 1): FOUND
- 9708a48 (Task 2): FOUND
- ac_launcher.rs: FOUND
- Cargo.toml: FOUND
- All 17 tests: PASSED
- Full crate build: PASSED

---
*Phase: 01-session-types-race-mode*
*Completed: 2026-03-13*
