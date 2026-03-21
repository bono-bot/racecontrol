---
phase: 109-safe-mode-state-machine
plan: 01
subsystem: infra
tags: [safe-mode, anti-cheat, wmi, process-guard, state-machine, rust]

requires: []
provides:
  - "SafeMode struct with active/game/cooldown_until fields and enter/start_cooldown/exit transitions"
  - "is_protected_game() and exe_to_sim_type() classification functions"
  - "PROTECTED_EXE_NAMES constant for 6 protected exe names"
  - "spawn_wmi_watcher() PowerShell-based process start detection"
  - "detect_running_protected_game() startup scan using sysinfo"
  - "AppState fields: safe_mode, safe_mode_active, wmi_rx, safe_mode_cooldown_timer, safe_mode_cooldown_armed"
  - "main.rs startup scan + WMI watcher initialization"
affects:
  - "109-02 (event_loop + ws_handler + subsystem gate wiring)"
  - "110 (telemetry gating)"

tech-stack:
  added: []
  patterns:
    - "SafeMode as pure state machine struct — no async, no cross-thread sync. Only AppState holds it."
    - "Arc<AtomicBool> shadow flag pattern for cross-task safe mode queries (mirrors in_maintenance pattern)"
    - "WMI watcher on std::thread with PowerShell Register-WmiEvent, CREATE_NO_WINDOW (0x08000000)"
    - "#[cfg(not(test))] guard on sysinfo scan to keep unit tests hermetic"

key-files:
  created:
    - "crates/rc-agent/src/safe_mode.rs"
  modified:
    - "crates/rc-agent/src/app_state.rs"
    - "crates/rc-agent/src/main.rs"
    - "crates/rc-agent/src/process_guard.rs"

key-decisions:
  - "WRC.exe included in PROTECTED_EXE_NAMES even though no SimType::EaWrc exists — detection fires, exe_to_sim_type returns None"
  - "detect_running_protected_game() stubbed to None in #[cfg(test)] — avoids real sysinfo scans in unit tests"
  - "process_guard::spawn() accepts _safe_mode_active as 5th arg (unused for now) — Plan 02 wires it into the scan loop"
  - "SafeMode::enter() clears cooldown_until — game takes priority over any pending cooldown window"
  - "Cooldown timer uses Box::pin(tokio::time::sleep(86400s)) as parked state in AppState — Plan 02 arms it"

patterns-established:
  - "Safe mode fields live in AppState (not ConnectionState) to survive WebSocket reconnections"
  - "Dual representation: SafeMode struct (rich state) + Arc<AtomicBool> shadow (for cross-thread reads)"

requirements-completed: [SAFE-01, SAFE-02, SAFE-03]

duration: 25min
completed: 2026-03-21
---

# Phase 109 Plan 01: Safe Mode State Machine Foundation Summary

**SafeMode state machine with WMI process watcher, sysinfo startup scan, and full AppState integration for anti-cheat compatible protected game detection**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-21T15:05:30Z
- **Completed:** 2026-03-21T15:30:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Created safe_mode.rs with SafeMode struct, 3 transition methods, WMI watcher, startup scan, and 21 unit tests (all passing)
- Integrated 5 new AppState fields for safe mode state, shadow flag, WMI receiver, cooldown timer, and armed flag
- main.rs initializes all fields, runs startup scan before reconnect loop, spawns WMI watcher, passes safe_mode_active to process_guard

## Task Commits

Each task was committed atomically:

1. **Task 1: Create safe_mode.rs module** - `0921d5c` (feat)
2. **Task 2: Integrate safe_mode into AppState and main.rs** - `ebe1020` (feat)

_Note: TDD task had combined implementation + test in single commit (tests and impl written together, all pass on first run)_

## Files Created/Modified

- `crates/rc-agent/src/safe_mode.rs` — SafeMode struct, is_protected_game(), PROTECTED_EXE_NAMES, exe_to_sim_type(), spawn_wmi_watcher(), detect_running_protected_game(), 21 unit tests
- `crates/rc-agent/src/app_state.rs` — Added safe_mode import and 5 new AppState fields
- `crates/rc-agent/src/main.rs` — mod safe_mode declaration, AppState field initializers, startup scan, WMI watcher spawn
- `crates/rc-agent/src/process_guard.rs` — Added _safe_mode_active: Arc<AtomicBool> parameter to spawn() (Plan 02 wires it)

## Decisions Made

- WRC.exe is in PROTECTED_EXE_NAMES even without a SimType variant — detection is future-proof, exe_to_sim_type returns None gracefully
- The `detect_running_protected_game()` function uses `#[cfg(not(test))]` / `#[cfg(test)]` split to keep unit tests hermetic (no sysinfo in tests)
- process_guard::spawn() extended with `_safe_mode_active` as a preflight stub — avoids a compile break while keeping Plan 02's wiring clean
- SafeMode::enter() always clears cooldown_until so that a new game start during cooldown takes immediate priority

## Deviations from Plan

None - plan executed exactly as written. The process_guard::spawn() 5th parameter approach was explicitly mentioned as acceptable in the plan note.

## Issues Encountered

- Cargo package name is `rc-agent-crate` (not `rc-agent`) — test command in plan used `-p rc-agent` which fails. Correct command is `-p rc-agent-crate`. This is a doc issue only, no code impact.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- safe_mode.rs foundation complete — Plan 02 can wire SafeMode into event_loop select! (WMI channel poll, cooldown timer), ws_handler subsystem gates, and process_guard scan loop
- All acceptance criteria met, build clean, 21 tests passing

---
*Phase: 109-safe-mode-state-machine*
*Completed: 2026-03-21*
