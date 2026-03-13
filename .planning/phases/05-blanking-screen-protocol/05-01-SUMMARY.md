---
phase: 05-blanking-screen-protocol
plan: 01
subsystem: ui
tags: [lock-screen, rust, kiosk, screen-transitions, ac-launcher]

# Dependency graph
requires:
  - phase: 04-deployment-pipeline
    provides: stable rc-agent codebase with billing session handling and game launch
provides:
  - LaunchSplash lock screen state with branded Racing Point HTML render
  - show_launch_splash() method on LockScreenManager
  - DIALOG_PROCESSES constant (5 entries: WerFault, WerFaultSecure, ApplicationFrameHost, SystemSettings, msiexec)
  - Corrected SessionEnded, BillingStopped, SubSessionEnded, crash-recovery handler ordering (lock screen before game kill)
  - LaunchGame handler shows branded splash from cached driver_name before spawn_blocking
affects: 05-02-plan (QR/kiosk UX), any future screen state work

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Lock screen before game kill: always show lock_screen state then sleep 500ms BEFORE calling game.stop() or enforce_safe_state()"
    - "Driver name caching: BillingStarted caches current_driver_name; LaunchGame reads it for splash; SessionEnded/BillingStopped clear it"
    - "DIALOG_PROCESSES constant: single source of truth for process kill list in both enforce_safe_state() and cleanup_after_session()"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/debug_server.rs

key-decisions:
  - "LaunchSplash placed between AwaitingAssistance and ScreenBlanked in enum — it is an active customer-facing state, not idle"
  - "health_response_body returns ok for LaunchSplash by default (existing match excludes only Hidden/Disconnected/ConfigError)"
  - "is_idle_or_blanked() is already correct — only matches Hidden|ScreenBlanked|Disconnected; LaunchSplash falls through as active"
  - "500ms sleep between lock screen show and game.stop() — gives Edge kiosk window time to initialize before game window disappears"
  - "current_driver_name cleared on SessionEnded, BillingStopped, and crash recovery to prevent stale name on next customer"
  - "debug_server.rs required LaunchSplash arm (Rule 3 auto-fix — non-exhaustive match prevented compilation)"
  - "render_launch_splash_page uses format! not template replacement — simpler for single-use dynamic HTML with driver name"

patterns-established:
  - "Lock screen before game kill: All session-ending handlers follow show_lock_screen -> sleep(500ms) -> game.stop() -> enforce_safe_state()"
  - "Driver name propagation: BillingStarted -> current_driver_name -> LaunchGame splash, cleared on session end"

requirements-completed: [SCREEN-01, SCREEN-02]

# Metrics
duration: 28min
completed: 2026-03-13
---

# Phase 5 Plan 01: Blanking Screen Protocol Summary

**LaunchSplash state with branded HTML + corrected session-end ordering so customers never see Windows desktop during game launch or session end**

## Performance

- **Duration:** 28 min
- **Started:** 2026-03-13T14:00:00Z
- **Completed:** 2026-03-13T14:28:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Added `LaunchSplash { driver_name, message }` variant to `LockScreenState` enum with full render, method, and health support
- Extended `DIALOG_PROCESSES` constant to 5 entries (WerFault, WerFaultSecure, ApplicationFrameHost, SystemSettings, msiexec) used by both `enforce_safe_state()` and `cleanup_after_session()`
- Fixed session-end ordering in 4 handlers (SessionEnded, BillingStopped, SubSessionEnded, crash recovery): lock screen now shows BEFORE game.stop()
- Wired `show_launch_splash()` into LaunchGame handler using driver name cached from BillingStarted
- 5 new tests added; all 52 tests pass (47 existing + 5 new)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add LaunchSplash state, render function, and extend dialog kill list** - `37ba5f0` (feat)
2. **Task 2: Fix session-end transition ordering and wire LaunchSplash into game launch** - `6dec739` (feat)

**Plan metadata:** (docs commit — see state update below)

## Files Created/Modified
- `crates/rc-agent/src/lock_screen.rs` - Added LaunchSplash variant, show_launch_splash(), render_launch_splash_page(), 4 new tests
- `crates/rc-agent/src/ac_launcher.rs` - Added DIALOG_PROCESSES constant, updated enforce_safe_state() and cleanup_after_session(), 1 new test
- `crates/rc-agent/src/main.rs` - Fixed 4 session-end handlers + added current_driver_name caching + show_launch_splash() before spawn_blocking
- `crates/rc-agent/src/debug_server.rs` - Added LaunchSplash arm to state_name match (auto-fix, non-exhaustive error)

## Decisions Made
- LaunchSplash is an active customer-facing state — health returns "ok", is_idle_or_blanked() returns false
- 500ms sleep gives Edge kiosk time to initialize before game window disappears (empirical: Edge launches fast enough)
- current_driver_name cleared at session end to prevent stale name for next customer
- DIALOG_PROCESSES is a `pub const &[&str]` — testable, single source of truth, no duplication between enforce_safe_state and cleanup_after_session

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added LaunchSplash arm to debug_server.rs state_name match**
- **Found during:** Task 1 (after adding LaunchSplash to enum)
- **Issue:** debug_server.rs had a non-exhaustive match on LockScreenState that didn't handle LaunchSplash — prevented compilation
- **Fix:** Added `LockScreenState::LaunchSplash { .. } => "launch_splash"` arm
- **Files modified:** `crates/rc-agent/src/debug_server.rs`
- **Verification:** Compilation succeeded, all tests pass
- **Committed in:** 37ba5f0 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Auto-fix was necessary for correctness — adding a new enum variant requires exhaustive matches. No scope creep.

## Issues Encountered
None beyond the debug_server non-exhaustive match (handled as Rule 3 auto-fix above).

## Next Phase Readiness
- Phase 5 Plan 01 complete — screen transitions are now clean
- Plan 02 (QR/kiosk UX improvements) can proceed
- Pods need rc-agent redeployment to pick up these fixes

---
*Phase: 05-blanking-screen-protocol*
*Completed: 2026-03-13*
