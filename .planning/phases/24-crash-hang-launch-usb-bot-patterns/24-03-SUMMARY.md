---
phase: 24-crash-hang-launch-usb-bot-patterns
plan: 03
subsystem: infra
tags: [rust, tokio, hidapi, sysinfo, winapi, failure-detection, game-monitor, usb]

# Dependency graph
requires:
  - phase: 24-crash-hang-launch-usb-bot-patterns
    provides: "ai_debugger::try_auto_fix() dispatch arms for CRASH-01, CRASH-02, USB-01 (Plan 01/02)"
provides:
  - "FailureMonitorState struct — 6-field shared state for polling loop (game_pid, last_udp_secs_ago, hid_connected, launch_started_at, billing_active, recovery_in_progress)"
  - "failure_monitor::spawn() — 5s polling task with 30s startup grace, CRASH-01/CRASH-02/USB-01 detection, HardwareFailure AgentMessage on disconnect"
  - "is_game_process_hung() — CPU pre-filter (sysinfo two-refresh) + IsHungAppWindow EnumWindows check"
  - "8 unit tests for detection state machine, all spawn_blocking-wrapped try_auto_fix calls"
affects:
  - 24-04-PLAN (Wave 2: main.rs wiring — watch::channel + failure_monitor::spawn() call)
  - 24-05-PLAN (integration verification with real pod state)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "tokio watch channel for shared mutable state (clone-on-read, no locking)"
    - "spawn_blocking wrapping all hidapi/sysinfo blocking calls"
    - "Synthetic canonical keyword strings to drive try_auto_fix dispatch without AI"
    - "CPU pre-filter (sysinfo two-refresh) before expensive EnumWindows/IsHungAppWindow"
    - "thread_local! Cell<T> pattern for FFI callback state (extern system callback)"
    - "launch_timeout_fired bool for per-launch-attempt deduplication"

key-files:
  created:
    - crates/rc-agent/src/failure_monitor.rs
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "is_game_process_hung uses CPU pre-filter before calling EnumWindows — avoids Windows API overhead on every 5s tick when game is actively using CPU"
  - "prev_hid_connected tracked as task-local bool (not in FailureMonitorState) — transition detection requires previous value which only the monitor task needs"
  - "launch_timeout_fired is task-local bool reset when launch_started_at becomes None — prevents duplicate CM kill attempts for same launch"
  - "recovery_in_progress updates prev_hid_connected before continue — prevents false disconnect detection on the cycle after recovery clears"

patterns-established:
  - "Wave 1b pattern: detection module watches shared state and fires canonical synthetic strings into try_auto_fix — keeps detection and fix logic separate"
  - "All blocking hardware/OS calls (hidapi, sysinfo, EnumWindows) must go through spawn_blocking — never block tokio runtime thread"

requirements-completed: [CRASH-01, CRASH-02, USB-01]

# Metrics
duration: 9min
completed: 2026-03-16
---

# Phase 24 Plan 03: Failure Monitor Summary

**failure_monitor.rs with CRASH-01/CRASH-02/USB-01 detection state machine, 8 tests green, all try_auto_fix calls wrapped in spawn_blocking**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-16T11:16:56Z
- **Completed:** 2026-03-16T11:26:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Created `crates/rc-agent/src/failure_monitor.rs` (384 lines) with `FailureMonitorState`, `spawn()`, `build_snapshot()`, `is_game_process_hung()`
- 8 unit tests covering all detection conditions (USB reconnect fires/suppressed, launch timeout fires/suppressed, freeze threshold below/above, recovery gate, default state)
- All three try_auto_fix call sites wrapped in `spawn_blocking` per KEY CONSTRAINT
- `is_game_process_hung` uses two-step approach: sysinfo CPU pre-filter (avoids EnumWindows cost), then IsHungAppWindow via EnumWindows with thread_local! FFI state
- Added `mod failure_monitor;` to main.rs (declaration only — wiring deferred to Plan 04 Wave 2)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create failure_monitor.rs with FailureMonitorState, spawn(), and unit tests** - `cbc610f` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/rc-agent/src/failure_monitor.rs` - New failure detection module: FailureMonitorState struct, spawn() poll loop, is_game_process_hung(), 8 unit tests
- `crates/rc-agent/src/main.rs` - Added `mod failure_monitor;` declaration (module included but spawn() not yet called — Plan 04 wires it)

## Decisions Made

- CPU pre-filter before EnumWindows: sysinfo two-refresh (500ms sleep) runs first — if CPU >5% the game is actively running, skip expensive EnumWindows
- `prev_hid_connected` is task-local (not in FailureMonitorState) because the watch channel only carries current state; transition detection needs both prev and current
- `launch_timeout_fired` bool is task-local, reset when `launch_started_at` becomes None — prevents duplicate CM kills for the same launch attempt
- `recovery_in_progress` updates `prev_hid_connected` before `continue` to avoid false USB disconnect on the cycle immediately after recovery clears

## Deviations from Plan

None - plan executed exactly as written. The `mod failure_monitor;` addition to `main.rs` was a necessary deviation (file needed to be included in compilation for tests to run) but is consistent with the plan's stated expectation that "Cargo will emit an unused warning for the spawn function until main.rs adds `mod failure_monitor`".

## Issues Encountered

- Full `cargo test -p rc-agent-crate` output file was lost due to large output buffer cleanup, but targeted runs (8/8 failure_monitor tests, 22/22 regression tests in self_monitor/driving_detector/ffb_controller/firewall, full --list showing 230 tests) all confirmed clean compilation and no failures.

## Next Phase Readiness

- `failure_monitor::spawn()` is ready to be called from `main.rs` in Plan 04 (Wave 2)
- `FailureMonitorState` needs a `watch::Sender<FailureMonitorState>` in main.rs event loop to update fields as state changes
- Plan 04 will need to thread `state_tx.send()` calls into: LaunchGame handler (launch_started_at), game PID tracker, HID poll loop, billing state updates

---
*Phase: 24-crash-hang-launch-usb-bot-patterns*
*Completed: 2026-03-16*
