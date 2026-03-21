---
phase: 06-mid-session-controls
plan: 01
subsystem: agent
tags: [sendinput, hid, openffboard, shared-memory, overlay, protocol, winapi]

# Dependency graph
requires:
  - phase: 04-safety-enforcement
    provides: FfbController with HID vendor commands and zero_force()
  - phase: 01-session-types-race-mode
    provides: INI builder and AC launch pipeline
provides:
  - SetAssist / SetFfbGain / QueryAssistState protocol messages
  - SendInput keyboard simulation for AC assists (ABS, TC, transmission)
  - HID FFB gain control via OpenFFBoard CLASS_AXIS power command
  - Shared memory assist state reading (ABS, TC, auto_shifter)
  - Overlay toast notification system (3s, replace behavior)
  - Wired handlers in main.rs for all 5 mid-session control messages
affects: [06-mid-session-controls, 07-kiosk-pwa-controls]

# Tech tracking
tech-stack:
  added: [winapi SendInput, HID CLASS_AXIS 0x0A01]
  patterns: [shared-memory-confirm-after-sendinput, overlay-toast-replace, trait-default-method-for-sim-specific]

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/src/ffb_controller.rs
    - crates/rc-agent/src/sims/assetto_corsa.rs
    - crates/rc-agent/src/sims/mod.rs
    - crates/rc-agent/src/overlay.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "SendInput helpers are in ac_launcher::mid_session submodule (colocated with AC launch code)"
  - "set_gain() uses send_vendor_cmd_to_class() with CLASS_AXIS parameter (not modifying existing send_vendor_cmd)"
  - "read_assist_state() implemented as SimAdapter trait default method for dyn dispatch"
  - "Stability control excluded -- AC has no keyboard shortcut for it (user decision DIFF-09)"
  - "SetFfb handler: numeric values use HID gain, non-numeric presets fall back to legacy INI"
  - "last_ffb_percent cached in main.rs scope (default 70%) since FFB has no shared memory readback"

patterns-established:
  - "SendInput pattern: bring_game_to_foreground() then send INPUT structs via winapi"
  - "Shared memory confirm: apply change via SendInput, sleep 100ms, read back from shared memory"
  - "Overlay toast: 3-second duration, new toast replaces existing (no stacking)"
  - "Trait default method: SimAdapter::read_assist_state() returns None, AC adapter overrides"

requirements-completed: [DIFF-06, DIFF-07, DIFF-08, DIFF-09, DIFF-10]

# Metrics
duration: 18min
completed: 2026-03-14
---

# Phase 6 Plan 01: Mid-Session Control Engine Summary

**SendInput keyboard simulation for AC assists (ABS/TC/transmission), HID FFB gain control via OpenFFBoard CLASS_AXIS, shared memory assist state confirmation, and overlay toast notifications -- all wired in main.rs with protocol messages**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-14T05:25:00Z
- **Completed:** 2026-03-14T05:44:29Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- 6 new protocol message variants enabling core-to-agent mid-session control communication
- Instant assist changes via SendInput (Ctrl+A/T/G) replacing INI-write-on-next-launch approach
- HID FFB gain control mapping 10-100% to 16-bit values via OpenFFBoard CLASS_AXIS power command
- Shared memory readback (ABS at offset 252, TC at 204, auto_shifter at 264) for confirming changes
- Overlay toast system with 3-second auto-dismiss and replace-no-stack behavior
- 20 new tests across 5 files covering protocol serde, HID buffer format, SendInput buffer layout, shared memory offsets, and toast behavior

## Task Commits

Each task was committed atomically:

1. **Task 1: Protocol + SendInput + FFB gain + shared memory + overlay toast** - `d5ad18a` (feat)
2. **Task 2: Wire mid-session handlers in main.rs** - `8cff1ec` (feat)

## Files Created/Modified
- `crates/rc-common/src/protocol.rs` - 6 new protocol variants (SetAssist, SetFfbGain, QueryAssistState, AssistChanged, FfbGainChanged, AssistState) with 9 serde roundtrip tests
- `crates/rc-agent/src/ac_launcher.rs` - mid_session module with send_ctrl_key, send_ctrl_shift_key, toggle_ac_abs/tc/transmission, 3 tests
- `crates/rc-agent/src/ffb_controller.rs` - set_gain() method with CLASS_AXIS/CMD_POWER constants, send_vendor_cmd_to_class(), 2 tests
- `crates/rc-agent/src/sims/assetto_corsa.rs` - physics offset constants (TC:204, ABS:252, AUTO_SHIFTER_ON:264), read_assist_state() impl, 2 tests
- `crates/rc-agent/src/sims/mod.rs` - read_assist_state() default method added to SimAdapter trait
- `crates/rc-agent/src/overlay.rs` - toast_message/toast_until fields, show_toast() method, toast rendering in paint_all, 4 tests
- `crates/rc-agent/src/main.rs` - 5 handler arms (SetAssist, SetFfbGain, QueryAssistState, updated SetTransmission, updated SetFfb), last_ffb_percent state variable

## Decisions Made
- **SendInput module location:** Placed in ac_launcher::mid_session submodule rather than a separate file, since it's tightly coupled with AC-specific keybindings and uses bring_game_to_foreground() from the parent module.
- **FFB gain method:** Added send_vendor_cmd_to_class() that accepts a class_id parameter instead of modifying existing send_vendor_cmd() which hardcodes CLASS_FFBWHEEL. Preserves backward compatibility for zero_force() and estop.
- **Trait dispatch for read_assist_state:** Implemented as a default trait method on SimAdapter (returns None) with the AC adapter overriding it. This enables `adapter.as_ref().and_then(|a| a.read_assist_state())` through dyn dispatch without downcasting.
- **Stability control excluded:** AC has no keyboard shortcut for stability control. Intentionally omitted per user decision (DIFF-09).
- **SetFfb backward compat:** Numeric strings use HID gain, non-numeric preset strings fall back to legacy INI writing. Smooth transition during rollout.
- **FFB percent caching:** No shared memory readback for FFB gain exists, so last_ffb_percent is cached in main.rs scope (default 70%) and updated on successful set_gain() calls.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed read_assist_state infinite recursion**
- **Found during:** Task 1 (shared memory reading)
- **Issue:** Initially added read_assist_state() as both an inherent method on AssettoCorsaAdapter AND in the SimAdapter trait impl. The trait impl delegated to self.read_assist_state() which would recurse infinitely since trait methods shadow inherent methods on trait objects.
- **Fix:** Removed inherent method entirely, placed full implementation directly in the SimAdapter trait impl block with #[cfg(windows)] / #[cfg(not(windows))] variants.
- **Files modified:** crates/rc-agent/src/sims/assetto_corsa.rs
- **Verification:** cargo test passes, no recursion
- **Committed in:** d5ad18a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for correctness. No scope creep.

## Issues Encountered
- Bash temp file ENOENT errors required redirecting test output to explicit file paths and using run_in_background parameter as workaround.
- Large files (protocol.rs 1273 lines, ac_launcher.rs 2201 lines, overlay.rs 1575 lines) needed offset/limit reads and grep searches to navigate efficiently.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All mid-session control primitives are in place for Plan 02 (core-side API endpoints) and Plan 03 (kiosk/PWA UI)
- Protocol messages are defined and handlers wired -- core just needs to send them over WebSocket
- Overlay toast system ready for any future notification needs beyond assist changes
- 242 total tests passing (85 rc-common + 157 rc-agent), including 20 new tests

## Self-Check: PASSED

- All 7 modified files: FOUND
- Commit d5ad18a (Task 1): FOUND
- Commit 8cff1ec (Task 2): FOUND
- 06-01-SUMMARY.md: FOUND

---
*Phase: 06-mid-session-controls*
*Completed: 2026-03-14*
