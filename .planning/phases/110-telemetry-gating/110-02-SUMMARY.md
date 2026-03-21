---
phase: 110-telemetry-gating
plan: "02"
subsystem: rc-agent / telemetry / socket lifecycle
tags: [telemetry, udp, socket, anti-cheat, HARD-04]
dependency_graph:
  requires: []
  provides: [UDP socket lifecycle gated to Running state]
  affects: [crates/rc-agent/src/event_loop.rs, crates/rc-agent/src/sims/f1_25.rs]
tech_stack:
  added: []
  patterns: [GameState::Running guard before UDP bind, matches! macro for SimType dispatch]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/sims/f1_25.rs
decisions:
  - "UDP gating placed in telemetry tick before adapter.connect() — minimal code change, no new state required"
  - "iRacing (port 6789) and LMU (port 5555) confirmed to use rF2 shared memory in rc-agent, not UDP sockets — no gating needed"
  - "Disconnect log upgraded to include port number for cleaner audit trail during post-game scans"
metrics:
  duration_minutes: 15
  completed_date: "2026-03-21T21:50:00+05:30"
  tasks_completed: 1
  files_modified: 2
---

# Phase 110 Plan 02: Telemetry Gating — UDP Socket Lifecycle Summary

**One-liner:** F1 25 UDP socket on port 20777 now binds only when GameState::Running and drops on game exit with explicit port-20777 log.

## What Was Built

The telemetry tick in `event_loop.rs` was updated to check `GameState::Running` before calling `adapter.connect()` for F1 25 (UDP adapter). Previously, the socket would be bound as soon as an F125Adapter existed — even before the game process reached Running state. Now:

1. `is_udp_adapter` check via `matches!(adapter.sim_type(), SimType::F125)` identifies adapters that bind real UDP sockets.
2. If game is not Running (`state.game_process.as_ref().map(|gp| gp.state == GameState::Running).unwrap_or(false)` is false), the telemetry tick skips `connect()` entirely.
3. `F125Adapter::disconnect()` now logs `"F1 25 UDP socket closed (port 20777) — game exit cleanup"` making it auditable.
4. A unit test `test_udp_connect_requires_running_state` (HARD-04) verifies the SimType gating logic for all four relevant sim types.

All existing disconnect paths in `event_loop.rs` (process exit, crash recovery, session end) and `ws_handler.rs` (StopGame, SessionEnded) already call `adapter.disconnect()` — no gaps found.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Gate UDP adapter connect to Running state, enhance disconnect log | c727b70 | event_loop.rs (in 1d2507d), f1_25.rs |

Note: The event_loop.rs changes (UDP gating logic + test) were included in the Plan 01 commit `1d2507d` as part of the same telemetry gating work. The f1_25.rs disconnect log enhancement is the Plan 02 commit `c727b70`.

## Deviations from Plan

None — plan executed exactly as written.

## Verification Results

- `grep is_udp_adapter crates/rc-agent/src/event_loop.rs` — 5 matches (HARD-04 guard present)
- `grep "UDP socket closed" crates/rc-agent/src/sims/f1_25.rs` — 1 match (line 574)
- `cargo check --release` — Finished with 0 errors
- `test_udp_connect_requires_running_state` — PASSED
- All f1_25 tests (14 tests) — PASSED
- All event_loop tests (crash_recovery + UDP gating) — PASSED

## Self-Check: PASSED

- [x] crates/rc-agent/src/event_loop.rs — modified (in commit 1d2507d)
- [x] crates/rc-agent/src/sims/f1_25.rs — modified (in commit c727b70)
- [x] commit 1d2507d exists (git log verified)
- [x] commit c727b70 exists (git log verified)
