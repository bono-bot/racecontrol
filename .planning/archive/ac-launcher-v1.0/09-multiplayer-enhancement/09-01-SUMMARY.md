---
phase: 09-multiplayer-enhancement
plan: 01
subsystem: api
tags: [assettoserver, ai-fillers, entry-list, multiplayer, websocket, sqlite]

# Dependency graph
requires:
  - phase: 01-session-types-race-mode
    provides: AI_DRIVER_NAMES pool and pick_ai_names() in rc-agent
  - phase: 02-difficulty-tiers
    provides: DifficultyTier midpoints (Rookie->75 to Alien->98)
  - phase: 05-content-validation-filtering
    provides: ContentManifest with pit_count per track config
provides:
  - AI names shared via rc-common::ai_names (available to both rc-agent and rc-core)
  - AcEntrySlot.ai_mode field for AssettoServer AI=fixed support
  - GroupSessionInfo with track/car/ai_count/difficulty_tier for lobby UI
  - generate_extra_cfg_yml() for AssettoServer EnableAi + AiAggression config
  - generate_entry_list_ini() with AI=fixed line support
  - start_ac_server() sends JSON launch_args with game_mode "multi" (fixes raw URI bug)
  - start_ac_lan_for_group() adds AI fillers up to track pit count
  - DB migration: group_sessions.track, group_sessions.car, group_sessions.ai_count columns
affects: [09-02 (lobby UI), 09-03 (synchronized billing), pwa-multiplayer]

# Tech tracking
tech-stack:
  added: [rand in rc-common]
  patterns: [AssettoServer AI=fixed entry list, extra_cfg.yml generation, JSON launch_args for multiplayer]

key-files:
  created:
    - crates/rc-common/src/ai_names.rs
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/lib.rs
    - crates/rc-common/Cargo.toml
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-core/src/ac_server.rs
    - crates/rc-core/src/multiplayer.rs
    - crates/rc-core/src/db/mod.rs

key-decisions:
  - "AI names moved to rc-common (not duplicated) for single source of truth"
  - "AcEntrySlot.ai_mode uses serde(default, skip_serializing_if) for backward compat"
  - "GroupSessionInfo new fields all Option<T> with serde(default) for rolling deploy"
  - "AI filler count = track pit_count - human_count, capped at 19 (AC 20-slot limit)"
  - "AI_LEVEL mapped from difficulty_tier via Phase 2 midpoints, default SemiPro (87)"
  - "extra_cfg.yml written to server_dir root (AssettoServer reads from working directory)"
  - "LaunchGame sends JSON with game_mode multi instead of raw acmanager:// URI"
  - "track/car/ai_count stored on group_sessions table for lobby enrichment"
  - "build_group_session_info reads difficulty_tier from kiosk_experiences table"

patterns-established:
  - "AssettoServer AI config: AI=fixed in entry_list.ini + EnableAi: true in extra_cfg.yml"
  - "Multiplayer launch_args: JSON with game_mode, server_ip, server_http_port fields"
  - "Idempotent ALTER TABLE migrations for adding columns to existing tables"

requirements-completed: [MULT-01, MULT-02, MULT-05, MULT-06]

# Metrics
duration: 12min
completed: 2026-03-14
---

# Phase 9 Plan 01: Server-Side Multiplayer Enhancement Summary

**AI grid fillers via AssettoServer entry list with AI_LEVEL from host difficulty tier, fixed LaunchGame JSON format for multi-pod join, enriched GroupSessionInfo for lobby UI**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-14T04:40:03Z
- **Completed:** 2026-03-14T04:52:49Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Shared 60-name AI driver pool between rc-agent and rc-core via rc-common::ai_names
- AcEntrySlot supports AI=fixed entries for AssettoServer multiplayer AI opponents
- Fixed LaunchGame dispatch: sends JSON with game_mode "multi" instead of raw acmanager:// URI
- AI fillers auto-calculated from track pit count minus human players (capped at 19)
- extra_cfg.yml generated with EnableAi: true and AiAggression mapped from difficulty tier
- GroupSessionInfo enriched with track, car, ai_count, difficulty_tier for lobby display
- DB migration adds track/car/ai_count columns to group_sessions
- 14 new tests across rc-common (8) and rc-core (6), all 288+ tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Move AI names to rc-common and add ai_mode to AcEntrySlot** - `f512274` (feat)
2. **Task 2: AI fillers in entry list, fix LaunchGame JSON, extra_cfg.yml, DB enrichment** - `ae7ef38` (feat)

## Files Created/Modified
- `crates/rc-common/src/ai_names.rs` - AI_DRIVER_NAMES constant and pick_ai_names() shared module
- `crates/rc-common/src/lib.rs` - Added pub mod ai_names
- `crates/rc-common/src/types.rs` - AcEntrySlot.ai_mode, GroupSessionInfo.track/car/ai_count/difficulty_tier
- `crates/rc-common/Cargo.toml` - Added rand dependency for pick_ai_names shuffle
- `crates/rc-agent/src/ac_launcher.rs` - Replaced local AI names with rc-common import
- `crates/rc-core/src/ac_server.rs` - generate_entry_list_ini AI=fixed, generate_extra_cfg_yml, JSON launch_args
- `crates/rc-core/src/multiplayer.rs` - AI fillers in start_ac_lan_for_group, enriched build_group_session_info
- `crates/rc-core/src/db/mod.rs` - ALTER TABLE group_sessions ADD COLUMN track/car/ai_count

## Decisions Made
- AI names moved to rc-common (shared crate) instead of duplicated, ensuring single source of truth
- AcEntrySlot.ai_mode and GroupSessionInfo new fields use serde(default, skip_serializing_if) for full backward compatibility during rolling deploy
- AI filler count calculated as pit_count - human_count, capped at 19 (AC engine limit of 20 total slots)
- AI_LEVEL mapped from host's difficulty tier using Phase 2 midpoints: Rookie->75, Amateur->82, SemiPro->87, Pro->93, Alien->98
- Default AI_LEVEL is 87 (SemiPro midpoint) when no difficulty info available
- extra_cfg.yml uses AiAggression (0.0-1.0 float) mapped from AI_LEVEL/100
- Pod manifest queried for track pit_count (falls back to 24 if unavailable)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- rc-core test compilation initially failed due to pre-existing uncommitted billing.rs changes referencing a missing function. This was a pre-existing issue unrelated to plan changes -- resolved on second compilation attempt (stale cache).

## User Setup Required

None - no external service configuration required. AssettoServer binary path is configured via existing acserver_path in racecontrol.toml (user responsibility, documented in Phase 1).

## Next Phase Readiness
- Server-side AI fillers and corrected launch flow ready for on-site testing
- GroupSessionInfo enrichment ready for Plan 02 (lobby UI enhancement)
- Synchronized billing infrastructure (in billing.rs) already partially implemented (pre-existing uncommitted changes) and ready for Plan 03
- AssettoServer must be installed on Racing-Point-Server (.23) with acserver_path configured

## Self-Check: PASSED

- All 8 created/modified files verified present
- Commit f512274 (Task 1) verified in git log
- Commit ae7ef38 (Task 2) verified in git log
- 93 rc-common tests pass, 195 rc-core tests pass, 167 rc-agent tests pass

---
*Phase: 09-multiplayer-enhancement*
*Completed: 2026-03-14*
