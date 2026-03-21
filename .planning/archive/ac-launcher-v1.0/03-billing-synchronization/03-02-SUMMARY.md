---
phase: 03-billing-synchronization
plan: 02
subsystem: ui, agent
tags: [overlay, taxi-meter, ac-status, shared-memory, launch-timeout, gdi, win32]

# Dependency graph
requires:
  - phase: 03-billing-synchronization
    plan: 01
    provides: AcStatus enum, BillingTick Optional fields, GameStatusUpdate variant
provides:
  - SimAdapter::read_ac_status() trait method with default None impl
  - AssettoCorsaAdapter reads graphics::STATUS from shared memory, maps to AcStatus
  - OverlayData taxi meter fields (elapsed_seconds, cost_paise, paused, waiting_for_game)
  - OverlayManager activate_v2() and update_billing_v2() methods
  - format_cost() helper (paise to Rs.X with floor division)
  - SessionTimerSection taxi meter rendering with PAUSED badge, WAITING FOR GAME, rate upgrade prompt
  - 30-min VALUE RATE UNLOCKED celebration (10-second green text on tier crossing)
  - LaunchState machine (3-min timeout, auto-retry once, cancel on 2nd fail)
  - AC STATUS polling with 1-second debounce in main loop
  - BillingTick v2 handler with Optional field destructuring and legacy fallback
affects: [03-billing-synchronization]

# Tech tracking
tech-stack:
  added: []
  patterns: [taxi-meter-overlay, status-debounce, launch-state-machine, v2-with-legacy-fallback]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/sims/mod.rs
    - crates/rc-agent/src/sims/assetto_corsa.rs
    - crates/rc-agent/src/overlay.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Taxi meter detection: use elapsed_seconds > 0 || waiting_for_game || paused to switch between new and legacy rendering"
  - "LaunchState tracks attempt count (1 or 2) with 3-min timeout per attempt"
  - "BillingStarted uses activate_v2 when allocated_seconds >= 10800 (open-ended billing cap)"
  - "STATUS debounce at 1 second prevents flapping from rapid ESC press (Pitfall 3)"
  - "STATUS polling guarded by game_process.is_some() to avoid stale shared memory (Pitfall 1)"
  - "format_cost uses floor division (paise / 100) for customer-friendly rounding"

patterns-established:
  - "v2-with-legacy-fallback: New methods (activate_v2, update_billing_v2) coexist with old methods during rolling deploy"
  - "status-debounce: AC STATUS must be stable for 1 second before reporting to core"
  - "launch-state-machine: LaunchState transitions on game launch, STATUS=LIVE, session end, and timeout"

requirements-completed: [BILL-01, BILL-06]

# Metrics
duration: 13min
completed: 2026-03-14
---

# Phase 3 Plan 02: Agent Overlay Taxi Meter + AC STATUS Polling + LaunchState Machine Summary

**Taxi meter overlay with elapsed time + running cost, AC STATUS reading via shared memory, 1-second debounce, 30-min celebration, and 3-minute launch timeout with auto-retry**

## Performance

- **Duration:** 13 min
- **Started:** 2026-03-13T18:50:00Z
- **Completed:** 2026-03-14T19:03:18Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- SimAdapter trait extended with read_ac_status() and AC adapter reads graphics::STATUS offset 4
- Overlay taxi meter renders elapsed time + cost in real-time with PAUSED badge, WAITING FOR GAME state, rate upgrade prompt, and 30-min VALUE RATE UNLOCKED celebration
- Main loop polls AC STATUS with 1-second debounce, sends GameStatusUpdate to core on transitions
- LaunchState machine tracks 3-min launch timeout: auto-retry once, cancel on second failure (no charge)
- BillingTick handler uses update_billing_v2 when new Optional fields are present, falls back to legacy
- 8 new tests covering format_cost, taxi meter state, billing v2, celebrations, AC status read

## Task Commits

Each task was committed atomically:

1. **Task 1: SimAdapter read_ac_status + overlay taxi meter data model + rendering + 30-min celebration** - `7d26624` (feat)
2. **Task 2: Wire main loop -- STATUS polling, GameStatusUpdate, BillingTick v2 to overlay, LaunchState machine** - `13018e1` (feat)

_Note: Task 1 commit was combined with Plan 03-03 commit due to parallel execution (both staged at the same time). All Task 1 code is verified present in 7d26624._

## Files Created/Modified
- `crates/rc-agent/src/sims/mod.rs` - Added read_ac_status() to SimAdapter trait with default None impl
- `crates/rc-agent/src/sims/assetto_corsa.rs` - Implemented read_ac_status() reading graphics::STATUS, maps to AcStatus enum
- `crates/rc-agent/src/overlay.rs` - Taxi meter fields on OverlayData, format_cost(), activate_v2(), update_billing_v2(), 30-min celebration logic, refactored SessionTimerSection paint
- `crates/rc-agent/src/main.rs` - LaunchState enum, AC STATUS polling with 1s debounce, BillingTick v2 handler, activate_v2 for open-ended billing, STATUS/LaunchState resets on all session-end paths

## Decisions Made
- Taxi meter rendering uses `elapsed_seconds > 0 || waiting_for_game || paused` to detect new mode vs legacy countdown
- BillingStarted uses `allocated_seconds >= 10800` as the threshold for open-ended billing (10800 = 3hr hard cap from CONTEXT.md)
- LaunchState tracks attempt count (1 or 2) with independent 3-minute timers per attempt
- AC STATUS debounce set to 1 second to prevent pause/unpause flapping (RESEARCH.md Pitfall 3)
- STATUS polling guarded by game_process.is_some() to prevent stale shared memory reads after game exit (Pitfall 1)
- format_cost uses integer floor division (paise / 100) -- Rs.0 for 99 paise, consistent with RESEARCH.md Pitfall 6

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] BillingTick pattern exhaustiveness error**
- **Found during:** Task 1 (compilation)
- **Issue:** Plan 01 added new Optional fields to BillingTick enum variant, but main.rs pattern match didn't include them, causing E0027 compiler error
- **Fix:** Added `..` rest pattern to BillingTick destructuring to allow compilation while Task 2 properly destructures all fields
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo check passes
- **Committed in:** 7d26624 (part of Task 1 commit)

**2. [Observation] Task 1 commit absorbed by parallel Plan 03-03**
- **Found during:** Task 1 commit attempt
- **Issue:** Plan 03-03 executor committed all staged changes (including Task 1's rc-agent files) in commit 7d26624
- **Impact:** Task 1 code is present and correct but under the 03-03 commit message. Task 2 commit (13018e1) is clean and standalone.
- **Resolution:** Documented as deviation. No code lost, all changes verified present.

---

**Total deviations:** 1 auto-fixed (blocking), 1 observation (parallel commit collision)
**Impact on plan:** Auto-fix was necessary for compilation. Parallel commit collision is cosmetic only -- all code is committed and correct.

## Issues Encountered
- Parallel plan execution (03-02 and 03-03) caused commit collision. Both plans modified rc-agent files. The 03-03 executor committed first and captured Task 1's changes. Resolved by verifying all code present and proceeding with Task 2 as a standalone commit.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Agent-side billing visualization is complete: taxi meter, STATUS polling, launch timeout, 30-min celebration
- Plan 03-03 (billing lifecycle on core side) executes in parallel and handles the core-side GameStatusUpdate dispatch
- All Plan 01 contracts (AcStatus, BillingTick Optional fields, GameStatusUpdate) are fully wired end-to-end between agent and core
- Phase 3 is complete once Plan 03-03 finishes -- ready for Phase 4

---
*Phase: 03-billing-synchronization*
*Plan: 02*
*Completed: 2026-03-14*
