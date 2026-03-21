---
phase: 110-telemetry-gating
plan: 01
subsystem: telemetry
tags: [anti-cheat, shared-memory, iracing, lmu, ac-evo, eac, eos, feature-flag, hard-03, hard-05]

requires:
  - phase: 109-multi-game
    provides: sim adapter infrastructure (SimAdapter trait, IracingAdapter, LmuAdapter, AssettoCorsaEvoAdapter)

provides:
  - 5-second deferred shared memory connect after game reaches Running state (HARD-03)
  - AC EVO telemetry feature flag (ac_evo_telemetry_enabled, default false) (HARD-05)
  - shm_connect_allowed() helper function with unit tests

affects:
  - event_loop
  - sim-adapters
  - config

tech-stack:
  added: []
  patterns:
    - "shm_connect_allowed(game_running_since) pure function for testable timing logic"
    - "ConnectionState fields game_running_since + shm_defer_logged for deferred connect tracking"
    - "#[serde(default)] bool field in AgentConfig for feature flags (defaults to false via Default::default())"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Defer connect in event loop caller, not inside adapter.connect() itself — keeps adapters simple and testable"
  - "Use game_running_since in ConnectionState (not AppState) — deferred connect is per-connection-lifetime concern"
  - "AC EVO feature flag defaults false until anti-cheat status confirmed at v1.0 (HARD-05)"
  - "shm_connect_allowed() extracted as public(crate) pure function to enable unit tests without mocking"
  - "shm_defer_logged flag prevents log spam while still logging once per connect-attempt window"

patterns-established:
  - "Pattern 1: Anti-cheat timing gate — check shm_connect_allowed() before calling connect() for shm adapters"
  - "Pattern 2: Feature flag as top-level bool in AgentConfig with #[serde(default)] — no Default impl needed"

requirements-completed: [HARD-03, HARD-05]

duration: 35min
completed: 2026-03-21
---

# Phase 110 Plan 01: Telemetry Gating Summary

**5-second deferred SHM connect (HARD-03) and AC EVO feature flag off by default (HARD-05) to avoid triggering EAC/EOS/Javelin anti-cheat scans during game startup**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-21T16:15:00+05:30
- **Completed:** 2026-03-21T16:50:00+05:30
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `game_running_since: Option<Instant>` and `shm_defer_logged: bool` to `ConnectionState`; set when game transitions to Running, reset on exit
- Gates iRacing, LMU, AC EVO, and AC Rally `adapter.connect()` calls: skips connect if less than 5s has elapsed since Running state
- Extracted `shm_connect_allowed()` as a pure function with 3 unit tests (before_5s, after_5s, no_game)
- Added `ac_evo_telemetry_enabled: bool` to `AgentConfig` defaulting to false; AC EVO adapter creation gated in main.rs
- 2 new config tests verify default false and explicit true parsing

## Task Commits

1. **Task 1: Add AC EVO feature flag and deferred connect config** - `185b4c3` (feat)
2. **Task 2: Implement 5-second deferred shared memory connect in event loop** - `1d2507d` (feat)

## Files Created/Modified

- `crates/rc-agent/src/config.rs` - Added `ac_evo_telemetry_enabled: bool` field to `AgentConfig`, two new config tests
- `crates/rc-agent/src/event_loop.rs` - Added `game_running_since`/`shm_defer_logged` to `ConnectionState`, `shm_connect_allowed()` helper, deferred connect logic in telemetry tick, 3 unit tests
- `crates/rc-agent/src/main.rs` - Gated `AssettoCorsaEvo` adapter creation behind `config.ac_evo_telemetry_enabled`

## Decisions Made

- Defer in the event loop caller (not inside `adapter.connect()`) — keeps adapters unchanged and testable
- `game_running_since` lives in `ConnectionState` (per-connection) rather than `AppState` (persistent) because it resets every reconnect
- AC EVO defaults to false (HARD-05): anti-cheat scanning behavior unknown, safer to ship disabled
- Extracted `shm_connect_allowed()` as a named pure function to make timing logic unit-testable without spawning a real event loop

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Test runner produced LNK1104 linker error when running `cargo test -p rc-agent-crate` (rc-agent.exe locked by running process on James's machine). Resolved by killing the process with `taskkill`. All 45 targeted tests (event_loop + config filters) passed cleanly. Release build `cargo build --release --bin rc-agent` completed without errors.

## Next Phase Readiness

- SHM gating is in place. Phase 110-02 (if planned) can add per-sim tuning of the defer window or runtime config for the 5s threshold.
- AC EVO telemetry will be available once anti-cheat status is confirmed by setting `ac_evo_telemetry_enabled = true` in rc-agent.toml.

---
*Phase: 110-telemetry-gating*
*Completed: 2026-03-21*
