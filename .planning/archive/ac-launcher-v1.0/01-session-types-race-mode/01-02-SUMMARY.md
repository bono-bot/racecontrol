---
phase: 01-session-types-race-mode
plan: 02
subsystem: game-launcher
tags: [assetto-corsa, race-ini, ai-opponents, session-types, composable-builder]

requires:
  - phase: 01-session-types-race-mode (plan 01)
    provides: "AcLaunchParams base struct, write_race_ini() function, rand dependency"
provides:
  - "AiCarSlot struct for AI opponent configuration"
  - "Extended AcLaunchParams with session_type, ai_cars, starting_position, formation_lap, weekend fields"
  - "Composable INI builder (build_race_ini_string) with section writers"
  - "AI grid generation for Race vs AI (TYPE=3) with 19 AI cap"
  - "Track Day mixed AI traffic (12 default from TRACKDAY_CAR_POOL)"
  - "Race Weekend multi-session (P-Q-R with time allocation)"
  - "AI_DRIVER_NAMES pool (60 names) and pick_ai_names()"
  - "parse_ini() test helper for INI verification"
  - "40 unit tests covering all 5 session types"
affects: [phase-02-difficulty-tiers, phase-05-content-filtering, phase-09-multiplayer]

tech-stack:
  added: []
  patterns: [composable-ini-builder, effective-ai-cars-pattern, session-block-writer]

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/ac_launcher.rs"

key-decisions:
  - "Implemented all Plan 01-01 prerequisites inline since 01-01 had not executed yet (deviation Rule 3)"
  - "AI SKIN= left empty for AI cars -- AC picks random installed skin per research"
  - "Track Day defaults to 12 AI (midpoint of 10-15 range) with mixed GT3/supercar classes"
  - "Race Weekend race session gets remaining time with minimum 1 minute (saturating_sub + max(1))"
  - "Formation lap written as FORMATION_LAP=1 in session block (best-effort, may not work in single-player AC)"
  - "Composable builder uses effective_ai_cars() to centralize default/cap logic"

patterns-established:
  - "Composable INI builder: build_race_ini_string() assembles section writers, write_race_ini() writes to disk"
  - "effective_ai_cars(): centralized AI car list computation with trackday defaults and 19-cap clamping"
  - "write_session_block(): shared helper for all session block writing with consistent format"
  - "parse_ini() test helper: HashMap-based INI parser for unit test assertions"

requirements-completed: [SESS-02, SESS-04, SESS-05]

duration: 9min
completed: 2026-03-13
---

# Phase 1 Plan 2: AI Grid Generation, Track Day, and Race Weekend Summary

**AI grid generation with 19-car cap, Track Day mixed traffic from 12-car GT3/supercar pool, and Race Weekend P-Q-R multi-session with time allocation -- all 5 single-player session types now produce correct race.ini**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-13T06:19:57Z
- **Completed:** 2026-03-13T06:29:02Z
- **Tasks:** 2 (+ checkpoint pending)
- **Files modified:** 1

## Accomplishments
- All 5 session types generate correct race.ini: Practice (TYPE=1), Hotlap (TYPE=4), Race vs AI (TYPE=3), Track Day (TYPE=1 with AI), Race Weekend (TYPE=1+2+3 multi-session)
- Composable INI builder replaces monolithic format! string with 18 section writer functions
- AI grid generation: [CAR_1]..[CAR_N] with AI=1, capped at 19 for single-player
- Track Day generates 12 mixed-class AI from TRACKDAY_CAR_POOL when no custom AI provided
- Race Weekend P-Q-R sequence with skippable sessions and automatic time allocation
- 40 unit tests covering all session types, edge cases, and SESS-08 no-fallback requirements
- Full test suite green across all 3 crates (rc-common: 59, rc-agent: 40, rc-core: 135)

## Task Commits

1. **Tasks 1+2: AI grid, Track Day, Race Weekend + prerequisites** - `707331d` (feat)

**Plan metadata:** pending

_Note: Tasks 1 and 2 were combined into a single commit because the prerequisite code from Plan 01-01 (types, composable builder, practice/hotlap) was implemented alongside Plan 01-02's work in the same file._

## Files Created/Modified
- `crates/rc-agent/src/ac_launcher.rs` - Extended with AiCarSlot, composable INI builder, all 5 session types, AI name pool, 40 tests

## Decisions Made
- Implemented Plan 01-01 prerequisites (AiCarSlot, extended AcLaunchParams, composable builder, practice/hotlap, AI name pool) since the dependent plan had not executed yet
- Used saturating_sub + max(1) for Race Weekend time allocation to ensure race always gets at least 1 minute
- TRACKDAY_CAR_POOL uses 12 cars: 8 GT3 + 4 road supercars for performance-balanced mixed traffic
- AI driver names use a 60-name international pool shuffled randomly per session
- effective_ai_cars() centralizes all AI car logic (trackday defaults, 19 cap) so write_race_config_section and write_ai_car_sections stay simple

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Implemented Plan 01-01 prerequisites**
- **Found during:** Pre-task analysis
- **Issue:** Plan 01-02 depends on Plan 01-01 (AiCarSlot, extended AcLaunchParams, composable builder, practice/hotlap), but 01-01 had not been executed. The code file had partial test stubs from the planner but no actual type definitions or builder functions.
- **Fix:** Implemented all Plan 01-01 deliverables inline: AiCarSlot struct, extended AcLaunchParams with 6 new fields (all serde(default)), 60-name AI_DRIVER_NAMES pool, pick_ai_names(), composable INI builder with build_race_ini_string() and 18 section writers, Practice TYPE=1 and Hotlap TYPE=4 session support.
- **Files modified:** crates/rc-agent/src/ac_launcher.rs
- **Verification:** All 40 tests pass including Plan 01-01's pre-existing test stubs
- **Committed in:** 707331d

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The prerequisite implementation was necessary and follows the exact interfaces specified in Plan 01-02's context section. No scope creep -- all code serves the plan's objectives.

## Issues Encountered
- A linter/formatter auto-injected some code (AI name pool, parse_ini helper, additional tests) between edits, which required re-reading the file. This accelerated the work rather than hindering it.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 5 single-player session types are fully functional with unit tests
- Ready for Phase 2 (Difficulty Tiers): AI_LEVEL field is written to [RACE] section and can be mapped to difficulty presets
- Ready for Phase 5 (Content Filtering): TRACKDAY_CAR_POOL can be filtered by installed content
- Ready for Phase 9 (Multiplayer): composable builder supports [REMOTE] section for server connections
- Checkpoint Task 3 (human verification of full test suite) is pending

## Self-Check: PASSED

- FOUND: 01-02-SUMMARY.md
- FOUND: commit 707331d
- FOUND: crates/rc-agent/src/ac_launcher.rs
- All 40 tests pass (rc-agent)
- All 59 tests pass (rc-common)
- All 135 tests pass (rc-core)
- cargo build -p rc-agent succeeds

---
*Phase: 01-session-types-race-mode*
*Completed: 2026-03-13*
