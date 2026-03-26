---
phase: 197-launch-resilience-ac-hardening
plan: 01
subsystem: game-launcher
tags: [rust, axum, sqlite, sqlx, metrics, whatsapp, race-engineer, dynamic-timeout, error-taxonomy]

requires:
  - phase: 196-game-launcher-structural-rework
    provides: GameTracker, launch_game, handle_game_state_update, check_game_health foundations
provides:
  - Dynamic launch timeout from historical SQLite data (query_dynamic_timeout)
  - Typed exit_code field on GameLaunchInfo protocol type
  - ErrorTaxonomy classification via typed exit_code (priority over string parsing)
  - Atomic Race Engineer with single write lock (no TOCTOU duplicate relaunches)
  - Null launch_args guard in Race Engineer + relaunch_game
  - WhatsApp staff alert after 2 exhausted retries (Evolution API)
  - Timeout-triggered Race Engineer relaunch via handle_game_state_update
  - stop_game() sim_type logging fix
  - 10 new unit tests covering all new behaviors
affects:
  - Phase 198 (AC hardening follow-up)
  - Any phase using GameLaunchInfo protocol type (rc-agent, racecontrol, rc-sentry)

tech-stack:
  added: []
  patterns:
    - "Atomic read+increment under single write lock (no TOCTOU): avoid separate read-lock/write-lock for Race Engineer dedup"
    - "Historical median+2*stdev timeout: dynamic timeout adapts to real data, floor at 30s"
    - "exit_code priority over string: typed exit codes beat heuristic string parsing for ErrorTaxonomy"
    - "handle_game_state_update as Race Engineer entry point: both crash and timeout share same relaunch path"

key-files:
  created: []
  modified:
    - crates/rc-common/src/types.rs - Added exit_code: Option<i32> to GameLaunchInfo
    - crates/racecontrol/src/metrics.rs - Added query_dynamic_timeout() + 5 tests
    - crates/racecontrol/src/game_launcher.rs - All Task 1+2 changes + 9 new tests
    - crates/racecontrol/src/ws/mod.rs - Added dynamic_timeout_secs: None to reconciliation tracker
    - crates/rc-agent/src/event_loop.rs - Added exit_code: None to GameLaunchInfo constructions
    - crates/rc-agent/src/ws_handler.rs - Added exit_code: None to GameLaunchInfo constructions

key-decisions:
  - "Use extract_launch_fields() (existing) instead of new extract_car_track_from_args() — avoids duplicate helper"
  - "Route timeout through handle_game_state_update() not direct broadcast — shared Race Engineer path"
  - "Dynamic timeout query uses sim_type+car+track for precision, falls back to game defaults with <3 samples"
  - "Race Engineer single write lock: atomic check+increment prevents concurrent duplicate relaunches"
  - "serde default+skip_serializing_if on exit_code: backward compatible — old rc-agents produce null, new ones produce exit code"

patterns-established:
  - "Atomic Race Engineer: single write().await block reads count AND increments AND returns decision"
  - "ErrorTaxonomy hierarchy: exit_code beats string message; string message beats Unknown"
  - "Dynamic timeout floor: timeout_secs.max(30) prevents overly short timeouts from history"

requirements-completed: [LAUNCH-08, LAUNCH-09, LAUNCH-11, LAUNCH-12, LAUNCH-13, LAUNCH-14, LAUNCH-15, LAUNCH-16, LAUNCH-17, LAUNCH-18, LAUNCH-19]

duration: 90min
completed: 2026-03-26
---

# Phase 197 Plan 01: Launch Resilience AC Hardening Summary

**Server-side launch resilience: dynamic timeout from launch history, typed exit_code error taxonomy, atomic Race Engineer with WhatsApp staff alerts after 2 failed retries.**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-03-26 (continuation session)
- **Completed:** 2026-03-26
- **Tasks:** 2/2
- **Files modified:** 6

## Accomplishments

- Dynamic launch timeout: `query_dynamic_timeout()` computes median+2*stdev from last 10 successful launches per sim/car/track, floor 30s, fallback to AC=120s/others=90s for <3 samples. Stored per-tracker, used in `check_game_health()`.
- Typed crash classification: `exit_code: Option<i32>` flows from rc-agent through protocol to racecontrol. `classify_error_taxonomy()` checks exit_code first — `0xC0000005` → `ProcessCrash { exit_code: -1073741819 }` — then falls back to string heuristics.
- Atomic Race Engineer: replaced TOCTOU read+write pair with single write lock that atomically checks count, increments, and returns relaunch decision — prevents duplicate relaunches from rapid duplicate Error events.
- WhatsApp staff alert (`send_staff_launch_alert`) fires after 2 exhausted retries via Evolution API to 917075778180 with pod/game/error taxonomy details.
- Timeout now routes through `handle_game_state_update()` — Race Engineer auto-relaunch triggers on timeout, not just on crash events.
- `stop_game()` logs actual sim_type instead of empty string, fixing LAUNCH-19.
- Null launch_args guard in both `relaunch_game()` and Race Engineer block — clear error directing staff to kiosk relaunch.

## Task Commits

1. **Task 1: Dynamic timeout + exit_code + error taxonomy** - `42f87b0c` (feat)
2. **Task 2: Atomic Race Engineer + alerts + stop_game fix** - `5019e476` (feat)

## Files Created/Modified

- `crates/rc-common/src/types.rs` - Added `exit_code: Option<i32>` to `GameLaunchInfo` with serde default+skip
- `crates/racecontrol/src/metrics.rs` - Added `query_dynamic_timeout()` + 5 test cases
- `crates/racecontrol/src/game_launcher.rs` - All core changes: atomic Race Engineer, send_staff_launch_alert, check_game_health timeout routing, stop_game fix, null args guards, classify_error_taxonomy, dynamic_timeout_secs field + 9 new tests
- `crates/racecontrol/src/ws/mod.rs` - Added `dynamic_timeout_secs: None` to reconciliation GameTracker
- `crates/rc-agent/src/event_loop.rs` - Added `exit_code: None` to all GameLaunchInfo constructions (5 sites)
- `crates/rc-agent/src/ws_handler.rs` - Added `exit_code: None` to all GameLaunchInfo constructions (7 sites)

## Decisions Made

- Used existing `extract_launch_fields()` for car/track extraction in dynamic timeout query instead of adding a new helper — avoids dead code.
- Timeout in `check_game_health()` now calls `handle_game_state_update()` to share Race Engineer path — one entry point for all Error events regardless of source.
- `exit_code` field uses `#[serde(default, skip_serializing_if = "Option::is_none")]` for backward compatibility — old rc-agents serialize without the field, racecontrol deserializes as `None`.
- Race Engineer single write lock: the old pattern (read lock → check count → release → write lock → increment) had a TOCTOU window. New pattern holds write lock for the entire check+increment+decision.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] rust-analyzer reverted metrics.rs and game_launcher.rs multiple times during partial application**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** rust-analyzer auto-reverted file changes when compile errors existed in other files, causing changes to disappear between edit calls
- **Fix:** Applied all changes atomically via Python script (`fix_game_launcher2.py`) then immediately ran cargo check
- **Files modified:** crates/racecontrol/src/game_launcher.rs, crates/racecontrol/src/metrics.rs
- **Committed in:** 42f87b0c

**2. [Rule 1 - Bug] extract_car_track_from_args() not inserted — function already existed as extract_launch_fields()**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** Plan called for new helper function but `extract_launch_fields()` already provided a superset (adds session_type + hash)
- **Fix:** Changed call site to use existing `extract_launch_fields()` with `(car, track, _, _)` destructuring
- **Files modified:** crates/racecontrol/src/game_launcher.rs
- **Committed in:** 42f87b0c

**3. [Rule 2 - Missing cascade] exit_code field cascade to rc-agent/ws_handler.rs nested LaunchDiagnostics**
- **Found during:** Task 1 (cascade from GameLaunchInfo struct change)
- **Issue:** Python regex for `diagnostics: Some(...)` matched inside `LaunchDiagnostics { ... }` struct literal, inserting `exit_code: None` inside the wrong struct
- **Fix:** Manually corrected ws_handler.rs line 397 to move `exit_code: None` outside the `diagnostics: Some(...)` block
- **Files modified:** crates/rc-agent/src/ws_handler.rs
- **Committed in:** 42f87b0c

---

**Total deviations:** 3 auto-fixed (Rule 1: 1, Rule 2: 1, Rule 3: 1)
**Impact on plan:** All auto-fixes necessary for correctness. No scope changes. Plan executed as designed.

## Issues Encountered

- `cargo test -p racecontrol` package name: correct flag is `-p racecontrol-crate` (the Cargo.toml `name` field).
- Pre-existing flaky test `config_fallback_preserved_when_no_env_vars` fails under parallel execution due to env var pollution — passes when run isolated. Not related to this plan.
- `test_race_engineer_no_maintenance_mode_sentinel_written` required careful design to avoid self-referencing the sentinel string in `include_str!` — used `concat!("MAINTENANCE", "_", "MODE")` to avoid self-match in pattern.

## User Setup Required

None — no external service configuration required. Evolution API WhatsApp alert uses existing config at `state.config.auth.evolution_url/evolution_api_key/evolution_instance` — gracefully skips if not configured.

## Next Phase Readiness

Phase 197 Plan 02 (if exists) or next plans in the AC hardening phase. All server-side launch resilience changes are in place:
- Dynamic timeouts adapt automatically as launch history accumulates
- Crashes and timeouts both trigger Race Engineer with deduplication
- Staff receive WhatsApp alerts after automation exhausted
- Clean typed error taxonomy for diagnostics and metrics

---
*Phase: 197-launch-resilience-ac-hardening*
*Completed: 2026-03-26*
