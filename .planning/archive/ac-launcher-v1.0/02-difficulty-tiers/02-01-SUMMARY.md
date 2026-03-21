---
phase: 02-difficulty-tiers
plan: 01
subsystem: game-launcher
tags: [rust, serde, ini-builder, ai-difficulty, tdd]

# Dependency graph
requires:
  - phase: 01-session-types-race-mode
    provides: "AcLaunchParams struct, composable INI builder, effective_ai_cars(), generate_trackday_ai()"
provides:
  - "DifficultyTier enum with 5 variants (Rookie, Amateur, SemiPro, Pro, Alien)"
  - "tier_for_level(u32) -> Option<DifficultyTier> mapping function"
  - "Session-wide ai_level: u32 on AcLaunchParams (default 87)"
  - "AI_LEVEL in race.ini driven by params.ai_level"
  - "All AI slots inherit session-wide ai_level"
affects: [03-weather-time, 08-ui-components]

# Tech tracking
tech-stack:
  added: [serde::Serialize on DifficultyTier]
  patterns: [session-wide-parameter-override, tier-to-range-mapping]

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/ac_launcher.rs"
    - "crates/rc-agent/src/main.rs"

key-decisions:
  - "DifficultyTier controls AI_LEVEL only -- assists remain completely independent (user decision)"
  - "AI_AGGRESSION deferred -- uncertain CSP support across versions"
  - "Default ai_level is 87 (Semi-Pro midpoint) for backward compatibility"
  - "Session-wide ai_level overrides all per-car ai_level values (single difficulty for all AI)"
  - "DIFF-03 and DIFF-04 (assist presets per tier) SUPERSEDED by user decision"

patterns-established:
  - "Session-wide parameter pattern: top-level field on AcLaunchParams with serde default, propagated through effective_ai_cars() to all slots"
  - "Tier enum pattern: range(), midpoint(), display_name(), all() methods with tier_for_level() pure mapping function"

requirements-completed: [DIFF-01, DIFF-02, DIFF-03, DIFF-04, DIFF-05]

# Metrics
duration: 8min
completed: 2026-03-13
---

# Phase 2 Plan 1: Difficulty Tiers Summary

**DifficultyTier enum (5 tiers: Rookie/Amateur/SemiPro/Pro/Alien) with session-wide ai_level on AcLaunchParams controlling AI_LEVEL in race.ini**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-13T08:00:37Z
- **Completed:** 2026-03-13T08:08:31Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- DifficultyTier enum with 5 racing-themed variants, each mapping to an AI_LEVEL range (70-100)
- Session-wide ai_level field on AcLaunchParams (default 87/Semi-Pro) replaces per-car AI_LEVEL derivation
- All AI car slots (including trackday-generated AI) inherit session-wide ai_level
- 10 new TDD tests covering tier boundaries, midpoints, ranges, display names, backward compat, INI output, and slot inheritance
- Assists (AcAids) verified completely independent of difficulty tier selection

## Task Commits

Each task was committed atomically:

1. **Task 1: DifficultyTier enum and tier_for_level (TDD)** - `f7ca1a8` (feat)
2. **Task 2: Session-wide ai_level on AcLaunchParams + INI wiring (TDD)** - `03b91ff` (feat)

_Note: TDD tasks each had RED (compile fail) then GREEN (all pass) phases._

## Files Created/Modified
- `crates/rc-agent/src/ac_launcher.rs` - Added DifficultyTier enum, tier_for_level(), ai_level field on AcLaunchParams, updated write_race_config_section, effective_ai_cars, generate_trackday_ai, 10 new tests
- `crates/rc-agent/src/main.rs` - Added ai_level: 87 to both fallback AcLaunchParams struct literals

## Decisions Made
- DifficultyTier controls AI_LEVEL only; assists are independent (per user decision -- DIFF-03/DIFF-04 superseded)
- AI_AGGRESSION not written anywhere (deferred due to uncertain CSP support)
- Default ai_level set to 87 (Semi-Pro midpoint) for backward compatibility with existing JSON payloads
- Session-wide ai_level overrides per-car ai_level values in all modes (race, trackday, weekend)
- Named default function `default_session_ai_level` (returns 87) to avoid conflict with existing `default_ai_level` (returns 90 for AiCarSlot)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated existing test_write_race_ini_race_ai_level to match new behavior**
- **Found during:** Task 2 (GREEN phase)
- **Issue:** Existing test asserted AI_LEVEL came from ai_cars[0].ai_level (old behavior). After wiring session-wide ai_level, the test failed because params.ai_level defaults to 87, not 75 from the per-car slot.
- **Fix:** Updated test JSON to include explicit ai_level:75 at session level, and changed assertion message from "AI_LEVEL from first AI car" to "AI_LEVEL from session-wide ai_level"
- **Files modified:** crates/rc-agent/src/ac_launcher.rs (test module)
- **Verification:** All 110 rc-agent tests pass
- **Committed in:** 03b91ff (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug in existing test)
**Impact on plan:** Expected consequence of changing AI_LEVEL source from per-car to session-wide. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DifficultyTier enum is pub-exported and ready for UI integration (Phase 8)
- tier_for_level() available for display mapping in kiosk/PWA
- All 304 tests pass across 3 crates (59 rc-common + 110 rc-agent + 135 rc-core)
- AI_AGGRESSION remains deferred as a known gap (documented in Phase 2 research)

## Self-Check: PASSED

- SUMMARY.md exists at `.planning/phases/02-difficulty-tiers/02-01-SUMMARY.md`
- Commit f7ca1a8 (Task 1) verified in git log
- Commit 03b91ff (Task 2) verified in git log
- All 304 tests pass across 3 crates

---
*Phase: 02-difficulty-tiers*
*Completed: 2026-03-13*
