---
phase: 24-crash-hang-launch-usb-bot-patterns
plan: 02
subsystem: ai-bot
tags: [rust, ai-debugger, ffb, auto-fix, crash-recovery, tdd]

# Dependency graph
requires:
  - phase: 24-01
    provides: PodStateSnapshot 3 new fields + 10 RED test stubs for Wave 0 compliance
provides:
  - fix_frozen_game: billing-gated crash recovery with FFB-before-kill ordering (CRASH-01, CRASH-03)
  - fix_launch_timeout: kills both Content Manager.exe and acmanager.exe (CRASH-02)
  - fix_usb_reconnect: zeros FFB on wheelbase HID reconnect (USB-01)
  - fix_kill_error_dialogs: extended to WerFaultSecure.exe + msedge.exe (UI-01)
  - 3 new try_auto_fix dispatch arms in correct ordering (Pattern 3a, 3b, 3c)
affects: [25-billing-bot-recovery, failure-monitor, ai-debugger-future-extensions]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Billing gate inside fix function (not call site) — DebugMemory replay bypasses call-site guards"
    - "FFB zero BEFORE taskkill ordering enforced by code structure and test"
    - "Pattern specificity ordering in try_auto_fix — more specific arm before generic arm prevents misroute"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ai_debugger.rs

key-decisions:
  - "fix_frozen_game billing gate inside the function — DebugMemory instant_fix() replays fix functions directly, bypassing call-site guards"
  - "fix_usb_reconnect has no billing gate — USB reconnect can happen at any time, safety reset always appropriate"
  - "Pattern 3b dispatch uses 'kill cm' keyword (not 'hang') — matches actual AI suggestion text for CM timeout scenarios"

patterns-established:
  - "FFB-before-kill: always zero_force() before taskkill in any freeze-recovery function"
  - "Billing gate inside fix body: gate billing_active in fix_*() not in try_auto_fix arm"
  - "fix_type string equals function name: fix_frozen_game -> 'fix_frozen_game' for DebugMemory replay key"

requirements-completed: [CRASH-01, CRASH-02, CRASH-03, UI-01]

# Metrics
duration: 6min
completed: 2026-03-16
---

# Phase 24 Plan 02: Wave 1a Fix Functions Summary

**3 new auto-fix functions (fix_frozen_game, fix_launch_timeout, fix_usb_reconnect) + extended fix_kill_error_dialogs turn all 10 Wave 0 RED tests GREEN in ai_debugger.rs**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-16T11:16:21Z
- **Completed:** 2026-03-16T11:22:00Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- fix_frozen_game: billing gate + FFB zero_force() before taskkill — 8Nm Conspit Ares FFB safety (CRASH-01, CRASH-03)
- fix_launch_timeout: kills both "Content Manager.exe" and "acmanager.exe" in separate taskkill calls (CRASH-02)
- fix_usb_reconnect: zeros FFB on HID reconnect to clear stale torque state (USB-01)
- fix_kill_error_dialogs: extended to suppress WerFaultSecure.exe and msedge.exe crash dialogs (UI-01)
- 3 new try_auto_fix dispatch arms (3a frozen+relaunch, 3b launch timeout, 3c wheelbase USB reset) in correct order before generic relaunch arm
- All 10 Wave 0 tests GREEN, 0 regressions in existing test suite

## Task Commits

Each task was committed atomically:

1. **Task 1+2: All Wave 1a implementations** - `a08b765` (feat: implement fix_frozen_game, fix_launch_timeout, fix_usb_reconnect + extend fix_kill_error_dialogs)

Note: Task 1 and Task 2 were implemented together as they both operate on ai_debugger.rs with no logical intermediate commit point between stubs and real implementations.

## Files Created/Modified

- `crates/rc-agent/src/ai_debugger.rs` - Added FfbController import, replaced 3 todo! stubs with real implementations, extended fix_kill_error_dialogs, updated Pattern 3b dispatch keyword

## Decisions Made

- Billing gate inside fix_frozen_game body (not at call site) — required because DebugMemory.instant_fix() replays fix functions directly, bypassing any call-site guards
- fix_usb_reconnect has NO billing gate — USB HID reconnect can happen at any time (even before billing), FFB safety reset is always appropriate
- Pattern 3b dispatch keyword changed from "hang" to "kill cm" — aligns with actual AI suggestion text patterns for Content Manager timeout scenarios
- fix_frozen_game detail string format: "FFB zeroed | killed: {list}" — satisfies test_ffb_zero_before_kill_ordering assertion checking for "FFB" in detail

## Deviations from Plan

None - plan executed exactly as written. Pattern 3b keyword update ("hang" to "kill cm") was specified in the plan's action step and not a deviation.

## Issues Encountered

None.

## Next Phase Readiness

- All 10 Wave 0 tests GREEN — Wave 1a complete
- Phase 24-03 can proceed with failure_monitor.rs spawn + PodStateSnapshot population (Wave 1b)
- fix_frozen_game and fix_launch_timeout are pub(crate) ready for failure_monitor.rs to call
- No blockers

---
*Phase: 24-crash-hang-launch-usb-bot-patterns*
*Completed: 2026-03-16*
