---
phase: 329-native-win32-lockscreen
plan: 01
subsystem: ui
tags: [win32, gdi, lock-screen, montserrat, native-window]

# Dependency graph
requires: []
provides:
  - "native_lock/ module: NativeLockScreen, LockGdiResources, Win32 window creation"
  - "Embedded Montserrat Regular + Bold TTFs for branded text rendering"
  - "Double-buffered GDI painter for ScreenBlanked, StartupConnecting, Disconnected states"
  - "LockScreenManager wired to use native Win32 window instead of Edge browser"
affects: [329-02-PLAN, 329-03-PLAN, lock-screen, overlay]

# Tech tracking
tech-stack:
  added: [Montserrat TTF fonts (SIL OFL)]
  patterns: [native Win32 GDI lock screen, embedded font via include_bytes + AddFontResourceExW]

key-files:
  created:
    - crates/rc-agent/src/native_lock/mod.rs
    - crates/rc-agent/src/native_lock/font.rs
    - crates/rc-agent/src/native_lock/window.rs
    - crates/rc-agent/src/native_lock/painter.rs
    - crates/rc-agent/assets/fonts/Montserrat-Regular.ttf
    - crates/rc-agent/assets/fonts/Montserrat-Bold.ttf
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/game_process.rs

key-decisions:
  - "Module named native_lock/ instead of lock_screen/ to coexist with lock_screen.rs (Plan 03 renames)"
  - "Font sizes in pixels (not points) matching overlay.rs create_font pattern for consistency"
  - "close_browser() hides window instead of destroying — allows fast re-show without thread respawn"

patterns-established:
  - "Native Win32 lock screen: WS_POPUP | WS_EX_TOPMOST spanning virtual desktop via get_virtual_screen_bounds"
  - "Font embedding: include_bytes! -> temp file -> AddFontResourceExW(FR_PRIVATE) with system font fallback"
  - "State-driven GDI paint: match on LockScreenState, double-buffered (CreateCompatibleDC + BitBlt)"

requirements-completed: [WIN-01, WIN-02]

# Metrics
duration: 25min
completed: 2026-04-07
---

# Phase 329 Plan 01: Native Win32 Lock Screen Foundation Summary

**Native Win32 GDI lock screen with embedded Montserrat fonts replaces Edge browser for ScreenBlanked, StartupConnecting, and Disconnected states**

## Performance

- **Duration:** 25 min
- **Started:** 2026-04-07T08:38:40Z
- **Completed:** 2026-04-07T09:03:24Z
- **Tasks:** 2/2
- **Files modified:** 9

## Accomplishments

### Task 1: Lock screen module with font embedding and GDI resource cache
- Created `native_lock/` module directory with 4 files (mod.rs, font.rs, window.rs, painter.rs)
- Embedded Montserrat Regular (~302KB) and Bold (~302KB) TTFs via `include_bytes!`
- Font installation via `AddFontResourceExW(FR_PRIVATE)` with temp file write and automatic cleanup
- `LockGdiResources` cache with 6 font handles (title/heading/body/pin/timer/small) and 5 brush handles (black/asphalt/red/card/grey)
- `NativeLockScreen` struct with show/hide/destroy/is_alive/request_repaint API
- Made `get_virtual_screen_bounds()` pub(crate) for cross-module access

### Task 2: Win32 window with message loop and state-driven painter
- `spawn_lock_window()` creates dedicated std::thread with Win32 message loop
- Window class "RacingPointLockScreen" with `CS_HREDRAW | CS_VREDRAW`
- `CreateWindowExW(WS_EX_TOPMOST, ..., WS_POPUP | WS_VISIBLE)` spanning full virtual desktop
- `WM_TIMER` at 1-second interval triggers `InvalidateRect` for countdown displays
- `WM_SETCURSOR` hides cursor on ScreenBlanked, shows arrow on other states
- `WM_PAINT` dispatches to `paint_lock_screen()` with double-buffered GDI
- State-driven rendering:
  - `ScreenBlanked`: Pure black FillRect (no text, hidden cursor)
  - `StartupConnecting`: Asphalt background + "RACING POINT" in red + "Connecting to server..." in white
  - `Disconnected`: Asphalt background + "RACING POINT" in red + "Connection lost" in red
  - All other states: Asphalt background + "RACING POINT" branding placeholder
- Wired `LockScreenManager` to use `NativeLockScreen` instead of Edge browser
- Removed ~200 lines of Edge launch code (--app mode, SetWindowPos retry loop, EnumWindows)
- All 770 existing tests pass with zero regressions

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `865ebb09` | native_lock/ module, fonts, GDI resources, NativeLockScreen struct |
| 2 | `26d2acf1` | Wire NativeLockScreen into LockScreenManager, remove Edge code |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fix stray closing brace in game_process.rs**
- **Found during:** Task 1
- **Issue:** Pre-existing uncommitted change left a stray `}` on line 10 of game_process.rs (from removing `hidden_cmd()` function), preventing compilation
- **Fix:** Removed the orphan closing brace
- **Files modified:** crates/rc-agent/src/game_process.rs
- **Commit:** 865ebb09

**2. [Rule 3 - Blocking] Module naming: native_lock/ instead of lock_screen/**
- **Found during:** Task 1 planning
- **Issue:** Rust cannot have both `lock_screen.rs` (file) and `lock_screen/` (directory) as a module. Plan acknowledged this and suggested `native_lock/` as temporary name.
- **Fix:** Used `native_lock/` module name; Plan 03 will rename when lock_screen.rs is converted to lock_screen/mod.rs
- **Files modified:** crates/rc-agent/src/main.rs
- **Commit:** 865ebb09

## Known Stubs

None. All three target states (ScreenBlanked, StartupConnecting, Disconnected) render real content. Other states render a branded placeholder as specified in the plan (Plans 02/03 add real paint functions for those states).

## Verification

1. `cargo build --release --bin rc-agent` -- PASS (zero errors, 120 warnings all pre-existing)
2. `cargo test -p rc-agent-crate` -- PASS (770/770, 0 failures)
3. Binary: `target/release/rc-agent.exe` = 21,243,904 bytes
4. No `msedge` references in native_lock/ module files -- CONFIRMED
5. `include_bytes!` in font.rs -- CONFIRMED (2 TTF embeddings)
