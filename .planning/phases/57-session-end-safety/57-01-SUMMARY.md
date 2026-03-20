---
phase: 57-session-end-safety
plan: 01
subsystem: ffb
tags: [openffboard, hid, ffb, safety, hidapi]

# Dependency graph
requires:
  - phase: 46-startup-safety
    provides: FfbController with zero_force, zero_force_with_retry, set_gain, send_vendor_cmd_to_class
provides:
  - FfbController.fxm_reset() method for clearing orphaned DirectInput effects
  - FfbController.set_idle_spring(value) method for centering spring
  - POWER_CAP_80_PERCENT constant (52428) for startup power capping
  - Clone derive on FfbController for spawn_blocking closures
  - 6 unit tests verifying HID buffer format for all new commands
affects: [57-02-session-end-orchestrator, 57-03-call-site-replacement, 61-ffb-preset-tuning]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "HID command constants grouped by class (CLASS_FXM, CLASS_AXIS) with doc comments referencing OpenFFBoard wiki"
    - "New FFB methods follow same Ok(true)/Ok(false)/Err pattern as zero_force()"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ffb_controller.rs

key-decisions:
  - "CLASS_FXM = 0x0A03 and CMD_FXM_RESET = 0x01 per upstream OpenFFBoard wiki — needs empirical validation on Conspit fork"
  - "CMD_IDLESPRING = 0x05 on CLASS_AXIS (0x0A01) — same class used by working set_gain()"
  - "POWER_CAP_80_PERCENT = 52428 is pub const (not const) — Plan 02 will reference it from main.rs"
  - "Clone derived on FfbController — only holds vid/pid, trivially cloneable, needed for spawn_blocking closures in Plan 02"

patterns-established:
  - "Buffer format tests: construct buffer manually with constants, verify exact byte positions match expected LE encoding"
  - "Ramp calculation: (target * step) / total_steps for linear stepped increment"

requirements-completed: [SAFE-02, SAFE-03, SAFE-04, SAFE-05]

# Metrics
duration: 2min
completed: 2026-03-20
---

# Phase 57 Plan 01: FFB HID Command Building Blocks Summary

**fxm_reset + set_idle_spring + Clone + POWER_CAP_80_PERCENT added to FfbController with 6 unit tests verifying HID byte layout**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-20T08:30:07Z
- **Completed:** 2026-03-20T08:32:12Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 1

## Accomplishments
- Added fxm_reset() to clear orphaned DirectInput effects via Effects Manager class 0x0A03
- Added set_idle_spring(value) for centering spring via Axis class CMD_IDLESPRING 0x05
- Added POWER_CAP_80_PERCENT constant (52428) for 80% power cap at startup
- Added Clone derive to FfbController for async spawn_blocking usage
- 6 new unit tests all pass, 5 existing tests unchanged and passing (11 total)

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add failing tests for new HID commands** - `20a4ac8` (test)
2. **Task 1 GREEN: Add constants, Clone, fxm_reset, set_idle_spring** - `9be3c05` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ffb_controller.rs` - Added CLASS_FXM, CMD_FXM_RESET, CMD_IDLESPRING, POWER_CAP_80_PERCENT constants; Clone derive; fxm_reset() and set_idle_spring() methods; 6 new tests

## Decisions Made
- CLASS_FXM = 0x0A03 per upstream OpenFFBoard wiki (needs hardware validation on Conspit fork in Plan 02/03)
- CMD_IDLESPRING = 0x05 on existing CLASS_AXIS (0x0A01) which already works for set_gain()
- POWER_CAP_80_PERCENT is pub (not crate-private) so main.rs can reference it at startup
- Clone derive added since FfbController only holds vid: u16 and pid: u16

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FfbController now has all HID building blocks for Plan 02's safe_session_end() orchestrator
- fxm_reset() and set_idle_spring() ready to be called from the async session-end sequence
- Clone derive enables ffb.clone() for spawn_blocking closures
- POWER_CAP_80_PERCENT ready for startup power capping
- ESTOP path (zero_force, zero_force_with_retry) completely untouched

---
*Phase: 57-session-end-safety*
*Completed: 2026-03-20*
