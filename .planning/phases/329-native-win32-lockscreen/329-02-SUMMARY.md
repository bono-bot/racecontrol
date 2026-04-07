---
phase: 329-native-win32-lockscreen
plan: "02"
subsystem: rc-agent/native_lock
tags: [win32, gdi, keyboard, qr, pin-entry, countdown-timer]
dependency_graph:
  requires: ["329-01"]
  provides: ["keyboard-handler", "qr-renderer", "interactive-painters"]
  affects: ["lock_screen.rs", "native_lock/*"]
tech_stack:
  added: ["qrcode crate integration for GDI rendering"]
  patterns: ["WM_CHAR/WM_KEYDOWN handling", "GDI Ellipse for dot indicators", "state transition detection via variant name tracking"]
key_files:
  created:
    - crates/rc-agent/src/native_lock/keyboard.rs
    - crates/rc-agent/src/native_lock/qr.rs
  modified:
    - crates/rc-agent/src/native_lock/window.rs
    - crates/rc-agent/src/native_lock/painter.rs
    - crates/rc-agent/src/native_lock/mod.rs
    - crates/rc-agent/src/lock_screen.rs
decisions:
  - "PIN dots rendered as GDI Ellipse circles (filled white for entered, grey outline for empty) rather than text-based dots for crisp rendering at 7680x1440"
  - "State transition detection uses variant name string comparison to reset PIN buffer on PinEntry entry"
  - "QR module_size clamped 4-8 based on screen height for responsive scaling across monitor configs"
  - "Timer color warning: red at 60s, yellow at 300s, white otherwise"
metrics:
  duration_seconds: 425
  completed: "2026-04-07T14:38:00+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 4
  tests_added: 10
  tests_passing: 10
requirements:
  - WIN-03
  - WIN-04
---

# Phase 329 Plan 02: PIN Entry, QR Code, and ActiveSession Painters Summary

PIN keyboard handler with 4-6 digit entry, auto-submit at 6, QR code GDI renderer using qrcode crate, and ActiveSession MM:SS countdown timer with progress bar and color warnings.

## Task Results

### Task 1: Keyboard handler and QR renderer modules
- **Commit:** `febe2dcb`
- **keyboard.rs:** PinInputState with push_digit, pop, is_complete (4+), is_full (6), take, display_dots, clear. 10 unit tests all passing.
- **qr.rs:** paint_qr_code renders QR modules as FillRect grid with white quiet zone. GDI brushes created and deleted within function (no handle leak).
- **mod.rs:** Registered keyboard and qr modules, re-exported PinInputState.

### Task 2: Wire keyboard and add interactive state painters
- **Commit:** `bb9a1e59`
- **window.rs:** WM_CHAR handles digit input in PinEntry state, auto-submits at 6 digits via event_tx.blocking_send(PinEntered). WM_KEYDOWN handles VK_BACK (backspace) and VK_RETURN (submit at 4+ digits). Focus enforcement on every WM_TIMER tick reclaims foreground when in PinEntry. State transition detection resets pin_input buffer when entering PinEntry.
- **painter.rs:** paint_pin_entry renders card with driver name, tier, 6 dot placeholders (filled/empty), error text. paint_qr_display renders centered QR code with responsive module_size. paint_active_session renders MM:SS countdown in 96pt, progress bar, color warnings (red at 60s, yellow at 300s).
- **mod.rs + lock_screen.rs:** event_tx threaded from LockScreenManager through NativeLockScreen::show() to window thread.

## Verification

1. `cargo build --release --bin rc-agent` -- SUCCESS (45s, 103 warnings all pre-existing)
2. `cargo test -p rc-agent-crate -- pin_input` -- 10/10 tests passing
3. Grep confirms WM_CHAR and WM_KEYDOWN handlers in window.rs
4. Grep confirms paint_qr_code called from painter.rs

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Directory naming: lock_screen/ vs native_lock/**
- **Found during:** Initial file discovery
- **Issue:** Plan referenced `lock_screen/` directory but actual 329-01 output is at `native_lock/`
- **Fix:** Used correct `native_lock/` paths throughout
- **Files modified:** All task files

**2. [Rule 3 - Blocking] Crate name: rc-agent vs rc-agent-crate**
- **Found during:** Task 1 test run
- **Issue:** Cargo package name is `rc-agent-crate`, not `rc-agent`
- **Fix:** Used `cargo test -p rc-agent-crate` for test commands

**3. [Rule 2 - Missing functionality] PIN dot rendering as GDI circles instead of text**
- **Found during:** Task 2 painter implementation
- **Issue:** Plan suggested text-based dots but GDI text rendering of Unicode circles would look poor at high resolution
- **Fix:** Used GDI Ellipse for crisp filled/outline circles at any resolution
- **Files modified:** painter.rs

## Known Stubs

None -- all paint functions are fully implemented for PinEntry, QrDisplay, and ActiveSession states.

## Self-Check: PASSED

- keyboard.rs: FOUND
- qr.rs: FOUND
- Commit febe2dcb: FOUND
- Commit bb9a1e59: FOUND
