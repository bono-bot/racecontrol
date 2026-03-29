---
phase: 256-game-specific-hardening
plan: 02
subsystem: rc-agent
tags: [game-hardening, session-enforcement, crash-detection, forza, non-ac]
dependency_graph:
  requires: [256-01]
  provides: [GAME-03, GAME-08]
  affects: [rc-agent, rc-common, racecontrol]
tech_stack:
  added: []
  patterns:
    - SessionEnforcer: tick()-based duration enforcement with one-shot Warn at T-60s
    - ProcessMonitor: is_process_alive polling for non-child processes
    - tokio::time::Interval per monitor type (1s/5s) added to ConnectionState
key_files:
  created:
    - crates/rc-agent/src/session_enforcer.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ac_server.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/multiplayer.rs
decisions:
  - "GameState::Crashed does not exist — plan's interface comment was wrong. Used GameState::Error for crash/expiry reports."
  - "ProcessMonitor added to ConnectionState (not AppState) — tied to single game launch within a connection"
  - "Steam URL launch: ProcessMonitor deferred creation to game_check_interval when find_game_pid() returns the PID (no conn access in spawn closure)"
  - "duration_minutes: None added to all 7 racecontrol call sites — backward compatible, no enforcement for AC/F1/iRacing etc."
  - "SessionEnforcer::terminate uses taskkill /F /PID — same pattern as game_process::kill_process"
metrics:
  duration: 23min
  completed_date: "2026-03-29"
  tasks: 2
  files: 11
---

# Phase 256 Plan 02: Forza Session Enforcer + Non-AC Crash Detection Summary

One-liner: FH5/Forza session duration enforcement with 1-min warning (GAME-03) and generic non-AC game crash detection via process exit polling (GAME-08).

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Create session_enforcer.rs — SessionEnforcer + ProcessMonitor (TDD) | 7c2c2658 | session_enforcer.rs, main.rs |
| 2 | Integrate into LaunchGame and event loop | 86bb4d91 | event_loop.rs, ws_handler.rs, protocol.rs, 7 racecontrol files |

## What Was Built

### session_enforcer.rs (new module)

**SessionEnforcer** (GAME-03):
- `new(sim_type, pid, duration_secs)` — wall-clock Instant::now() start
- `tick(&mut self) -> SessionAction` — returns Continue/Warn{remaining_secs}/Terminate
  - Warn: emitted ONCE when remaining <= 60s
  - Terminate: emitted every tick once elapsed >= duration (not one-shot — keeps firing until cleared)
- `terminate(pid) -> Result<()>` — `taskkill /F /PID` with CREATE_NO_WINDOW

**ProcessMonitor** (GAME-08):
- `new(pid, sim_type)` — stores pid for polling
- `check() -> ProcessStatus` — Running or Exited{exit_code}
- Uses platform-specific is_process_alive (Windows: GetExitCodeProcess STILL_ACTIVE check; Linux: /proc/pid)

### Protocol changes (rc-common)

- `CoreToAgentMessage::LaunchGame` gains `duration_minutes: Option<u32>` (serde default = None)
- `AgentMessage` gains `SessionExpiryWarning { pod_id, sim_type, remaining_secs }` variant

### event_loop.rs integration

Two new `select!` branches added to `ConnectionState`:

1. **process_monitor_interval (5s)** — GAME-08 crash detection:
   - Polls `monitor.check()` every 5s when `conn.process_monitor.is_some()`
   - On Exited: sends `GameStateUpdate(Error)` + triggers ai_debugger::analyze_crash
   - Handles billing-active path (crash recovery) vs no-billing path (safe state)
   - Clears process_monitor, session_enforcer, game_process, launch_state

2. **session_enforcer_interval (1s)** — GAME-03 duration enforcement:
   - Polls `enforcer.tick()` every 1s when `conn.session_enforcer.is_some()`
   - On Warn: sends `AgentMessage::SessionExpiryWarning` to server
   - On Terminate: calls `SessionEnforcer::terminate(pid)` + sends `GameStateUpdate(Error)` + clears state

### ws_handler.rs integration

- `LaunchGame` match now captures `duration_minutes`
- On direct-exe launch (pid known immediately): creates ProcessMonitor + conditionally SessionEnforcer
- StopGame: clears `conn.process_monitor` and `conn.session_enforcer` (prevents false crash after controlled stop)

### Steam URL launch path

ProcessMonitor creation deferred to `game_check_interval` in event_loop.rs — when `game.state == Launching && child.is_none()` and `find_game_pid()` returns the PID, `conn.process_monitor` is created if not already set.

## Test Coverage

13 unit tests in `session_enforcer::tests`:
- tick() timing: continue at start, continue before T-60, warn at T-60, warn is one-shot, terminate at expiry, terminate repeatedly, remaining_secs accurate
- ProcessMonitor: nonexistent PID → Exited, current process → Running, sim_type stored
- Enum equality for SessionAction and ProcessStatus
- Forza Motorsport also gets enforcement (not just FH5)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] GameState::Crashed does not exist**
- **Found during:** Task 2 — cargo build error
- **Issue:** Plan's interface comment listed `GameState { Idle, Launching, Loading, Running, Paused, Crashed, Error }` — `Crashed` and `Paused` variants do not exist in rc-common types.rs
- **Fix:** Used `GameState::Error` for both crash detection and session expiry reports
- **Files modified:** event_loop.rs
- **Commit:** 86bb4d91

**2. [Rule 2 - Missing functionality] All racecontrol LaunchGame call sites needed duration_minutes**
- **Found during:** Task 2 — cargo build failed due to struct initializer completeness
- **Issue:** 7 call sites across game_launcher.rs, ac_server.rs, routes.rs, auth/mod.rs, billing.rs, multiplayer.rs
- **Fix:** Added `duration_minutes: None` to all sites (backward compatible — no enforcement on AC/F1/iRacing)
- **Files modified:** 6 racecontrol source files
- **Commit:** 86bb4d91

## Known Stubs

None. Both monitors are fully wired — ProcessMonitor polls actual process state, SessionEnforcer fires actual taskkill.

## Self-Check

- [x] session_enforcer.rs exists: `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/session_enforcer.rs`
- [x] Commits exist: 7c2c2658, 86bb4d91
- [x] All 13 tests pass: `cargo test --bin rc-agent session_enforcer`
- [x] cargo build --release rc-agent: Finished release profile
- [x] GAME-03 and GAME-08 markers present in event_loop.rs

## Self-Check: PASSED
