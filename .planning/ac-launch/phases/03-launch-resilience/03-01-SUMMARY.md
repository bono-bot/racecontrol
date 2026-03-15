---
phase: 03-launch-resilience
plan: 01
subsystem: game-launch
tags: [rust, diagnostics, ac-launcher, websocket, serde]

# Dependency graph
requires:
  - phase: 02-game-crash-recovery
    provides: GameLaunchInfo protocol + GameTracker + game crash/relaunch pipeline
provides:
  - LaunchDiagnostics struct in rc-common/types.rs with 5 structured fields
  - diagnostics: Option<LaunchDiagnostics> on GameLaunchInfo (serde default for rolling deploy)
  - LaunchResult in ac_launcher.rs carries agent-side LaunchDiagnostics
  - launch_ac() populates cm_attempted, cm_exit_code, cm_log_errors, fallback_used on CM failure
  - All GameStateUpdate messages in main.rs include diagnostics field
affects: [03-02-launch-resilience, dashboard, rc-core, rc-agent]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Agent-side diagnostic struct mirrors protocol struct; converted at send boundary
    - serde(default) + skip_serializing_if = Option::is_none for backward-compatible new fields
    - get_cm_exit_code() uses tasklist to detect CM process absence as exit signal

key-files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-core/src/game_launcher.rs

key-decisions:
  - "Agent-side LaunchDiagnostics is a separate struct from protocol type — converted explicitly at WebSocket send boundary, not shared via rc-common dependency in ac_launcher.rs"
  - "get_cm_exit_code() returns Some(-1) for 'exited but code unknown' (tasklist can't read exit codes), None for 'still running'"
  - "diagnostics: None on all non-CM-path GameLaunchInfo constructions — keeps the field optional and avoids false positives"

patterns-established:
  - "New optional GameLaunchInfo fields use serde(default, skip_serializing_if = Option::is_none) — old rc-core reads existing JSON without diagnostics field without error"
  - "All GameLaunchInfo struct literals must include diagnostics field — compiler enforces completeness across all 9 call sites"

requirements-completed: [LAUNCH-01, LAUNCH-02]

# Metrics
duration: 15min
completed: 2026-03-15
---

# Phase 3 Plan 01: Launch Resilience — Diagnostic Pipeline Summary

**Structured CM diagnostics (cm_attempted, cm_exit_code, cm_log_errors, fallback_used) now flow from launch_ac() through LaunchResult to GameStateUpdate WebSocket messages**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-15T00:00:00Z
- **Completed:** 2026-03-15T00:15:00Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments
- Added `LaunchDiagnostics` struct to rc-common/types.rs with 5 machine-readable fields for dashboard display
- Enhanced `LaunchResult` in ac_launcher.rs with agent-side diagnostics populated on CM failure path
- Added `get_cm_exit_code()` helper that detects CM exit via tasklist process absence
- Threaded diagnostics through all 9 `GameLaunchInfo` construction sites in main.rs and rc-core
- All 282 tests pass (98 rc-common + 184 rc-agent), all 3 crates compile cleanly

## Task Commits

Each task was committed atomically:

1. **Task 1: Add LaunchDiagnostics struct + GameLaunchInfo field** - `8d68795` (feat)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - Added `LaunchDiagnostics` struct + `diagnostics` field on `GameLaunchInfo`
- `crates/rc-agent/src/ac_launcher.rs` - Enhanced `LaunchResult`, added `LaunchDiagnostics`, `get_cm_exit_code()`, populates diag in CM failure path
- `crates/rc-agent/src/main.rs` - All 9 `GameLaunchInfo` constructions updated with `diagnostics` field
- `crates/rc-core/src/game_launcher.rs` - Two `GameLaunchInfo` constructions updated with `diagnostics: None`

## Decisions Made
- Agent-side `LaunchDiagnostics` is a separate (non-serde) struct from the protocol type — converted explicitly when constructing `GameLaunchInfo` for the WebSocket send, avoiding an rc-common dependency inside ac_launcher.rs business logic
- `get_cm_exit_code()` uses `Some(-1)` to represent "CM exited but exact code unknown" (tasklist doesn't expose exit codes), `None` means CM still running
- No behavior changes to the launch sequence — diagnostics are purely additive observability data

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Fixed rc-core game_launcher.rs GameLaunchInfo constructions**
- **Found during:** Task 1 (compiler error after adding required diagnostics field)
- **Issue:** The plan only mentioned rc-agent/src/main.rs but rc-core/src/game_launcher.rs also constructs `GameLaunchInfo` in `to_info()` and the timeout handler — both would fail to compile without the new field
- **Fix:** Added `diagnostics: None` to both constructions in game_launcher.rs
- **Files modified:** crates/rc-core/src/game_launcher.rs
- **Verification:** `cargo build -p rc-core` compiles cleanly
- **Committed in:** 8d68795 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 - missing critical)
**Impact on plan:** Fix was required for compilation. Exactly 2 additional lines added to rc-core. No scope creep.

## Issues Encountered
None — implementation followed the plan exactly. rc-core had two call sites not mentioned in the plan, both trivially fixed with `diagnostics: None`.

## User Setup Required
None - no external service configuration required. Diagnostic data is purely additive.

## Next Phase Readiness
- `LaunchDiagnostics` is now in the protocol and flowing to rc-core — Plan 03-02 can read `info.diagnostics` from `GameStateChanged` WebSocket messages immediately
- rc-core can store/display diagnostics without any further protocol changes
- Rolling deploy compatible: old rc-core instances ignore the new field gracefully via serde(default)

---
*Phase: 03-launch-resilience*
*Completed: 2026-03-15*
