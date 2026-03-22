---
phase: 137-browser-watchdog
plan: 02
subsystem: rc-agent
tags: [browser-watchdog, edge, lock-screen, event-loop, tokio, windows]

# Dependency graph
requires:
  - phase: 137-01
    provides: count_edge_processes(), close_browser() with safe-mode gate, LockScreenState enum, browser_process field

provides:
  - is_browser_alive() on LockScreenManager (windows + non-windows)
  - is_browser_expected() on LockScreenManager (watchdog state check — not Hidden)
  - launch_browser() and close_browser() made pub for direct watchdog access
  - browser_watchdog_interval (30s) in ConnectionState
  - Watchdog tick handler in event loop select! block (BWDOG-01, BWDOG-02, BWDOG-04)

affects: [rc-agent event loop, lock_screen, v17.0-ai-debugger]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Watchdog intervals follow existing ConnectionState tick pattern (30s interval in select!)"
    - "is_browser_expected() gates watchdog on Hidden state — any other state means browser should be alive"
    - "Stacking check runs before liveness check — worse condition handled first, continue avoids redundant check"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "Added is_browser_expected() as a separate method instead of reusing is_active() — existing is_active() returns false for ScreenBlanked (considers it idle), but watchdog must fire for ScreenBlanked since a browser IS expected"
  - "Watchdog gates on is_browser_expected() (not Hidden), not is_active() — semantic difference avoids missed relaunches during screen-blanked state"
  - "Stacking check (edge_count > 5) runs before liveness check — stacking is the more severe condition and should be remediated first"

patterns-established:
  - "Browser watchdog: safe_mode gate -> is_browser_expected gate -> stacking check -> liveness check"
  - "Auto-recovery uses continue after stacking relaunch to avoid double-relaunch"

requirements-completed:
  - BWDOG-01
  - BWDOG-02

# Metrics
duration: 15min
completed: 2026-03-22
---

# Phase 137 Plan 02: Browser Watchdog Event Loop Summary

**30-second browser watchdog in rc-agent event loop that auto-recovers from Edge crashes and process stacking (>5 msedge.exe), gated by safe mode and Hidden state**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-22T04:00:00Z
- **Completed:** 2026-03-22T04:15:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `is_browser_alive()` to detect child process exit via `try_wait()` (clears stale handle on exit)
- Added `is_browser_expected()` returning true for all states except Hidden (correct watchdog gate)
- Made `launch_browser()` and `close_browser()` pub on both cfg variants for direct watchdog access
- Wired `browser_watchdog_interval` (30s) into ConnectionState and event loop select! block
- Watchdog detects stacking (>5 msedge.exe) and liveness failure, relaunches automatically
- All 459 rc-agent tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add is_browser_alive(), is_browser_expected(), pub launch/close_browser** - `12624c1` (feat)
2. **Task 2: Wire browser_watchdog_interval into event loop** - `e9c42f1` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/rc-agent/src/lock_screen.rs` - Added is_browser_alive() (windows+non-windows), is_browser_expected(), made launch_browser()/close_browser() pub
- `crates/rc-agent/src/event_loop.rs` - Added browser_watchdog_interval field+init, watchdog tick handler in select!

## Decisions Made
- Used `is_browser_expected()` instead of reusing existing `is_active()` — the existing method returns false for ScreenBlanked (treated as idle), but the watchdog must fire for ScreenBlanked since the browser is expected to be visible. Adding a distinct method with clear semantics prevents future confusion.
- Stacking check runs before liveness check to handle the more severe condition first, with `continue` to avoid double-relaunch.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Used is_browser_expected() instead of is_active() for watchdog gate**
- **Found during:** Task 1 (reviewing existing is_active() implementation)
- **Issue:** Plan said "add is_active() if not already present" — it was present, but returns false for ScreenBlanked (considers it idle), which would suppress the watchdog during screen-blanked state where a browser IS expected
- **Fix:** Added dedicated `is_browser_expected()` method returning `!matches!(*state, LockScreenState::Hidden)` and used it in the event loop instead of `is_active()`
- **Files modified:** crates/rc-agent/src/lock_screen.rs, crates/rc-agent/src/event_loop.rs
- **Verification:** cargo check + 459 tests pass
- **Committed in:** 12624c1 (Task 1 commit), e9c42f1 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - semantic correctness)
**Impact on plan:** Necessary for correctness — watchdog would silently skip during ScreenBlanked state using existing is_active(). No scope creep.

## Issues Encountered
None

## Next Phase Readiness
- Browser watchdog is live — rc-agent will auto-recover Edge crashes every 30s
- BWDOG-01 (liveness) and BWDOG-02 (stacking >5) both implemented
- BWDOG-04 (safe mode gate) inherited from Plan 01 close_browser() gate
- Ready for Plan 03 if any further watchdog work planned

---
*Phase: 137-browser-watchdog*
*Completed: 2026-03-22*
