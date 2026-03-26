---
phase: 196-game-launcher-structural-rework
plan: 01
subsystem: game-launcher
tags: [rust, billing, traits, tdd, dispatch, json-validation, toctou]

requires:
  - phase: 195-launch-observability
    provides: launch_events table schema + metrics::LaunchEvent types used in game_launcher.rs

provides:
  - GameLauncherImpl trait with validate_args(), make_launch_message(), cleanup_on_failure()
  - AcLauncher, F1Launcher, IRacingLauncher, DefaultLauncher implementations
  - launcher_for(sim_type) static dispatch function
  - Fixed billing gate checking both active_timers AND waiting_for_game
  - Paused session rejection (PausedManual/PausedDisconnect/PausedGamePause)
  - TOCTOU mitigation via re-check inside active_games write lock
  - Invalid JSON launch_args rejection via validate_args (LAUNCH-06)
  - 9 new unit tests covering all new behaviors

affects: [197-game-launcher-error-recovery, any future sim-type addition requiring launcher_for update]

tech-stack:
  added: []
  patterns:
    - "Static trait dispatch: launcher_for(sim_type) returns &'static dyn GameLauncherImpl"
    - "TOCTOU mitigation: acquire write lock, re-verify precondition, then mutate"
    - "Billing gate: check BOTH active_timers AND waiting_for_game before rejecting"
    - "JSON validation before billing gate: fail fast on malformed args before touching state"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/game_launcher.rs

key-decisions:
  - "validate_args called BEFORE billing gate — invalid JSON rejected immediately without touching billing state"
  - "launcher_for returns &'static dyn — avoids Box allocation for hot path; launchers are ZSTs"
  - "TOCTOU re-check uses read locks inside the write lock scope — acceptable since reads are non-blocking"
  - "Error message preserved as 'no active billing session' for both first gate and TOCTOU to avoid breaking existing tests"
  - "launch_events and recovery_events tables added to make_state() test helper for metrics::record_launch_event compatibility"

patterns-established:
  - "Per-game launcher trait: adding new SimType requires adding to launcher_for() match arm + impl"
  - "Billing gate order: JSON validate -> manifest validate -> billing gate -> double-launch check -> TOCTOU + insert"

requirements-completed: [LAUNCH-01, LAUNCH-02, LAUNCH-03, LAUNCH-04, LAUNCH-06]

duration: 10min
completed: 2026-03-26
---

# Phase 196 Plan 01: Game Launcher Structural Rework Summary

**GameLauncherImpl trait with 4 per-game impls, fixed billing gate (deferred billing + paused rejection), TOCTOU mitigation, and invalid JSON rejection — all backed by 19 passing unit tests**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-26T03:54:09Z
- **Completed:** 2026-03-26T04:04:00Z
- **Tasks:** 1 (TDD — RED+GREEN combined, all tests passed on first compile)
- **Files modified:** 1

## Accomplishments

- GameLauncherImpl trait defined with 3 methods; AcLauncher/F1Launcher/IRacingLauncher/DefaultLauncher all implement it (LAUNCH-01)
- Billing gate expanded from `active_timers`-only to also check `waiting_for_game`, fixing deferred billing sessions that were incorrectly rejected (LAUNCH-02)
- Paused sessions (PausedManual/PausedDisconnect/PausedGamePause) now explicitly rejected at billing gate with clear error message (LAUNCH-03)
- TOCTOU window narrowed: billing re-checked inside `active_games.write()` lock before tracker insertion (LAUNCH-04)
- Invalid JSON in launch_args now rejected via `validate_args()` BEFORE billing gate — fail fast without touching state (LAUNCH-06)
- Added 9 new unit tests; all 19 `game_launcher` tests + full 514-test crate suite pass with zero failures

## Task Commits

1. **Task 1: GameLauncherImpl trait + billing gate fix + JSON validation** - `d6cbdbfb` (feat)

## Files Created/Modified

- `crates/racecontrol/src/game_launcher.rs` - Added trait + 4 impls + launcher_for() + refactored launch_game() billing gate + TOCTOU block + 9 new tests + make_state() table additions

## Decisions Made

- `validate_args` is called first (before manifest validation and billing gate) so malformed JSON fails immediately without touching any shared state
- `launcher_for()` returns `&'static dyn GameLauncherImpl` — all four launchers are zero-sized types, no heap allocation needed
- TOCTOU re-check uses `read` locks inside the `write` lock scope: Tokio's `RwLock` allows multiple concurrent readers even while a write lock is held by the current task, so this is safe and non-deadlocking
- The error message for the initial billing gate (`"no active billing session"`) was preserved unchanged to avoid breaking existing tests that assert on that string — the TOCTOU path uses a distinct `"billing session expired during launch"` message
- `launch_events` and `recovery_events` tables added to `make_state()` test helper — the `metrics::record_launch_event()` call in `launch_game()` writes to these tables and would error silently without them

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `git stash pop` mid-verification caused an accidental revert of the working tree — resolved by restoring from stash (stash had my changes captured). No code was lost, tests re-confirmed after restore.

## Self-Check

Files verified:
- `crates/racecontrol/src/game_launcher.rs` — FOUND (1568 lines, all 4 impls present)

Commits verified:
- `d6cbdbfb` — FOUND (feat(196-01): GameLauncherImpl trait + billing gate fixes + JSON validation)

Test results:
- `cargo test -p racecontrol-crate -- game_launcher`: 19 passed, 0 failed
- `cargo test -p racecontrol-crate`: 514 passed, 0 failed

## Self-Check: PASSED

## Next Phase Readiness

- Phase 196-02 (error recovery / cleanup_on_failure integration) can proceed — trait is in place
- Any new SimType added to `rc-common/src/types.rs` will need a corresponding arm in `launcher_for()` — the compiler will warn if the match is non-exhaustive

---
*Phase: 196-game-launcher-structural-rework*
*Completed: 2026-03-26*
