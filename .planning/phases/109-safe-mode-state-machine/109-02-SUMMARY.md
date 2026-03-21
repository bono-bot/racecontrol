---
phase: 109-safe-mode-state-machine
plan: 02
subsystem: infra
tags: [safe-mode, anti-cheat, process-guard, kiosk, lock-screen, wmi, event-loop, f1-25, iracing, lmu]

# Dependency graph
requires:
  - phase: 109-01
    provides: SafeMode struct, is_protected_game(), exe_to_sim_type(), spawn_wmi_watcher(), AppState safe_mode fields

provides:
  - Safe mode entry in LaunchGame handler before game spawn (SAFE-01, SAFE-02)
  - WMI channel polling in game_check_interval for externally launched games (SAFE-01)
  - Cooldown timer select! arm — deactivates safe mode 30s after game exit (SAFE-03)
  - process_guard scan skip when safe_mode_active AtomicBool is true (SAFE-04)
  - Ollama analyze_crash suppressed during safe mode (SAFE-05)
  - GPO registry writes (kiosk) deferred during safe mode (SAFE-06)
  - Focus Assist registry write (lock_screen) deferred during safe mode (SAFE-06)
  - SAFE-07 verified: billing, heartbeat, overlay unaffected (zero safe_mode refs)

affects: [phase-109-03, process_guard, kiosk, lock_screen, event_loop, ws_handler]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "wire_safe_mode() method pattern: subsystem managers (KioskManager, LockScreenManager) accept Arc<AtomicBool> after AppState construction"
    - "Call-site guard pattern: Ollama spawn guarded at call site in event_loop (state.safe_mode.active check), not inside analyze_crash"
    - "Cooldown timer follows exit_grace_timer pattern: Pin<Box<Sleep>> + armed bool in AppState, polled in select! arm"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/process_guard.rs
    - crates/rc-agent/src/kiosk.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Ollama suppression via call-site guard (Option B) — state.safe_mode.active checked in event_loop before tokio::spawn, no signature change to analyze_crash"
  - "KioskManager and LockScreenManager use wire_safe_mode() post-construction wiring — avoids changing new() signature while keeping kiosk startup call (main.rs:479) unaffected"
  - "self_heal::repair_registry_key requires no gate — called at startup (line 239 main.rs) before any game or safe mode state"
  - "WRC.exe safe mode activation uses manual field assignment (safe_mode.active=true, game=None) since no SimType::EaWrc variant exists"

patterns-established:
  - "Pattern: Registry write gate — check safe_mode_active.load(Relaxed) before calling registry-writing functions; log and skip if active"
  - "Pattern: Cooldown timer — Pin<Box<Sleep>> + armed bool in AppState, reset via .as_mut().reset() on game exit, fires in select! arm"

requirements-completed: [SAFE-01, SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, SAFE-07]

# Metrics
duration: 25min
completed: 2026-03-21
---

# Phase 109 Plan 02: Safe Mode Integration Summary

**Safe mode wired into all 7 integration points: LaunchGame entry, WMI polling, 30s cooldown timer, process_guard scan skip, Ollama suppression, kiosk GPO deferred, lock_screen Focus Assist deferred**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-21T15:15:13Z
- **Completed:** 2026-03-21T15:40:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- LaunchGame handler enters safe mode before game spawn for all protected games (F1 25, iRacing, LMU, EVO)
- WMI channel polled every 2s game_check_interval tick; handles externally launched games including WRC (no SimType)
- Cooldown timer fires 30s after game exit via select! arm; safe_mode_active AtomicBool stays true during cooldown
- process_guard scan loop skips entirely when safe_mode_active is true (SAFE-04)
- Ollama analyze_crash suppressed at call site when safe_mode.active is true (SAFE-05)
- KioskManager and LockScreenManager defer registry writes during safe mode via wire_safe_mode() pattern (SAFE-06)
- SAFE-07 verified: billing_guard, udp_heartbeat, overlay have zero safe_mode references

## Task Commits

1. **Task 1: Wire safe mode into event_loop and ws_handler** - `0913e82` (feat)
2. **Task 2: Gate process_guard, kiosk, and lock_screen** - `705b07d` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/rc-agent/src/ws_handler.rs` - Safe mode entry in LaunchGame before game spawn
- `crates/rc-agent/src/event_loop.rs` - WMI polling, cooldown timer select! arm, Ollama suppression, game exit cooldown trigger
- `crates/rc-agent/src/process_guard.rs` - `_safe_mode_active` stub wired into scan loop skip
- `crates/rc-agent/src/kiosk.rs` - `safe_mode_active` field + `wire_safe_mode()` + GPO registry write gate
- `crates/rc-agent/src/lock_screen.rs` - `safe_mode_active` field + `wire_safe_mode()` + Focus Assist registry write gate; test fixtures updated
- `crates/rc-agent/src/main.rs` - Wire safe_mode_active into KioskManager and LockScreenManager after AppState construction

## Decisions Made

- Used call-site guard for Ollama (Option B from plan) — `state.safe_mode.active` checked in event_loop before spawning analyze_crash task; no signature change to the function
- Chose `wire_safe_mode()` post-construction pattern over changing `new()` signature — kiosk startup call at main.rs:479 runs before AppState is built
- `self_heal::repair_registry_key` confirmed startup-only (main.rs:239) — no gate needed; runs before event loop and before any game
- WRC.exe manual safe mode activation since no SimType::EaWrc variant exists — direct field assignment with game=None

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test compile errors in lock_screen.rs test fixtures**
- **Found during:** Task 2 (verification / test run)
- **Issue:** Test code used struct literal initialization of LockScreenManager — failed to compile after adding `safe_mode_active` field
- **Fix:** Added `safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))` to 4 test initializers in lock_screen.rs
- **Files modified:** crates/rc-agent/src/lock_screen.rs (test section)
- **Verification:** `cargo test -- lock_screen` passes (38 tests ok)
- **Committed in:** `705b07d` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - bug)
**Impact on plan:** Test fixture fix was required for correctness. No scope creep.

## Issues Encountered

None - plan executed cleanly. All 7 SAFE requirements wired.

## Next Phase Readiness

- All 7 SAFE requirements fully wired and verified
- cargo build --release passes clean
- All safe_mode tests pass (21/21)
- All lock_screen tests pass (38/38)
- Phase 109-03 (if any) can rely on complete safe mode integration

---
*Phase: 109-safe-mode-state-machine*
*Completed: 2026-03-21*

## Self-Check: PASSED

- All modified files exist on disk
- Task commit `0913e82` found in git log
- Task commit `705b07d` found in git log
- `109-02-SUMMARY.md` exists at expected path
