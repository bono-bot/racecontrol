---
phase: 03-billing-synchronization
plan: 01
subsystem: billing
tags: [rust, serde, billing, pricing, timer, protocol]

# Dependency graph
requires:
  - phase: 02-difficulty-tiers
    provides: "Existing rc-common types and protocol enums, BillingTimer in rc-core"
provides:
  - "AcStatus enum (Off/Replay/Live/Pause) for AC shared memory STATUS field"
  - "BillingSessionStatus::WaitingForGame and PausedGamePause variants"
  - "AgentMessage::GameStatusUpdate for agent-to-core AC STATUS reporting"
  - "BillingTick with Optional elapsed/cost/rate/paused/minutes_to_value_tier fields"
  - "BillingSessionInfo with Optional elapsed_seconds/cost_paise/rate_per_min_paise"
  - "compute_session_cost() pure function with retroactive two-tier pricing"
  - "SessionCost struct for cost calculation results"
  - "BillingTimer count-up model with elapsed_seconds, pause_seconds, max_session_seconds"
  - "BillingTimer::current_cost() and updated to_info() with new Optional fields"
affects: [03-billing-synchronization, overlay, agent-status-polling]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure function cost calculation (compute_session_cost) - no DB dependency per tick"
    - "Retroactive two-tier pricing (standard <30min, value >=30min, entire session)"
    - "Count-up billing timer (elapsed_seconds) replacing countdown (remaining_seconds)"
    - "Optional fields with serde(default, skip_serializing_if) for backward compat"

key-files:
  created: []
  modified:
    - "crates/rc-common/src/types.rs"
    - "crates/rc-common/src/protocol.rs"
    - "crates/rc-core/src/billing.rs"
    - "crates/rc-core/src/ws/mod.rs"
    - "crates/rc-core/tests/integration.rs"

key-decisions:
  - "Backward compat: all new fields on BillingTick and BillingSessionInfo are Option<T> with serde(default) for rolling deploy"
  - "elapsed_seconds mirrors driving_seconds for compat -- both incremented in Active tick"
  - "max_session_seconds defaults to allocated_seconds for legacy timer compat"
  - "minutes_to_next_tier uses integer division (floor) -- at 29:59 shows 1 minute remaining"
  - "PausedGamePause has separate 10-min timeout (600s) independent of disconnect pause"
  - "WaitingForGame status: no elapsed/pause increments during game loading"

patterns-established:
  - "Pure function pattern: compute_session_cost(elapsed_seconds) -> SessionCost for testable pricing"
  - "Count-up timer: BillingTimer.elapsed_seconds counts UP, tick() returns true on auto-end triggers"
  - "Game pause billing: PausedGamePause freezes elapsed, increments pause_seconds"

requirements-completed: [BILL-01, BILL-02]

# Metrics
duration: 13min
completed: 2026-03-14
---

# Phase 3 Plan 1: Billing Type Contracts and Cost Engine Summary

**Retroactive two-tier pricing engine (Rs.23.3/min standard, Rs.15/min value) with count-up BillingTimer, AcStatus enum, and GameStatusUpdate protocol -- all backward compatible**

## Performance

- **Duration:** 13 min
- **Started:** 2026-03-13T18:31:07Z
- **Completed:** 2026-03-13T18:43:58Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Retroactive two-tier pricing: compute_session_cost() returns correct paise for standard (<30min) and value (>=30min) tiers, with retroactive rate drop at 30-min threshold
- BillingTimer refactored from countdown to count-up model with elapsed_seconds, pause_seconds, and max_session_seconds
- AcStatus enum (Off/Replay/Live/Pause) and GameStatusUpdate agent message ready for AC STATUS polling
- All new protocol fields are Optional with serde(default) for zero-downtime rolling deploys
- 22 new tests added (9 rc-common + 13 rc-core billing), all 87 tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Shared type contracts -- AcStatus, protocol messages, BillingSessionInfo** - `40b7474` (feat)
2. **Task 2: compute_session_cost pure function + BillingTimer count-up refactor** - `9618e7b` (feat)

## Files Created/Modified
- `crates/rc-common/src/types.rs` - Added AcStatus enum, WaitingForGame + PausedGamePause variants, Optional fields on BillingSessionInfo
- `crates/rc-common/src/protocol.rs` - Added GameStatusUpdate to AgentMessage, Optional fields on BillingTick
- `crates/rc-core/src/billing.rs` - Added SessionCost, compute_session_cost(), BillingTimer count-up fields, refactored tick() and to_info()
- `crates/rc-core/src/ws/mod.rs` - Added GameStatusUpdate match arm (placeholder), updated BillingTick construction
- `crates/rc-core/tests/integration.rs` - Updated 4 integration tests with new BillingTimer fields

## Decisions Made
- Used integer division for minutes_to_next_tier (floor): at 29:59 shows "1 minute" not "0 minutes" to value tier
- Kept all legacy BillingTimer fields (allocated_seconds, driving_seconds) populated with sensible values for backward compat
- GameStatusUpdate placeholder in ws/mod.rs logs the status change; full lifecycle wiring is Plan 03 scope
- to_info() computes cost via current_cost() on every call (simple and correct, no caching needed)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Non-exhaustive match on AgentMessage in ws/mod.rs**
- **Found during:** Task 1 (after adding GameStatusUpdate variant)
- **Issue:** Adding GameStatusUpdate to AgentMessage caused non-exhaustive pattern match error in ws/mod.rs
- **Fix:** Added placeholder match arm that logs the AC STATUS change (full wiring is Plan 03 scope)
- **Files modified:** crates/rc-core/src/ws/mod.rs
- **Verification:** cargo check -p rc-core passes
- **Committed in:** 40b7474 (Task 1 commit)

**2. [Rule 3 - Blocking] Missing new fields in integration test BillingTimer constructions**
- **Found during:** Task 2 (after adding new fields to BillingTimer)
- **Issue:** 4 integration tests in crates/rc-core/tests/integration.rs constructed BillingTimer without the new fields
- **Fix:** Added elapsed_seconds, pause_seconds, max_session_seconds to all 4 integration test timer constructions
- **Files modified:** crates/rc-core/tests/integration.rs
- **Verification:** cargo test -p rc-core passes
- **Committed in:** 9618e7b (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes were necessary compilation fixes caused by the planned type changes. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All shared types and protocol contracts ready for Plans 02 and 03
- compute_session_cost() available for overlay taxi meter (Plan 02) and billing lifecycle (Plan 03)
- BillingTimer count-up model ready for wiring to AC STATUS polling
- GameStatusUpdate placeholder in ws/mod.rs ready for full lifecycle implementation in Plan 03

## Self-Check: PASSED

- [x] crates/rc-common/src/types.rs -- FOUND
- [x] crates/rc-common/src/protocol.rs -- FOUND
- [x] crates/rc-core/src/billing.rs -- FOUND
- [x] crates/rc-core/src/ws/mod.rs -- FOUND
- [x] Commit 40b7474 -- FOUND (Task 1)
- [x] Commit 9618e7b -- FOUND (Task 2)
- [x] 68 rc-common tests passing (was 59, +9 new)
- [x] 19 rc-core billing tests passing (was 6, +13 new)

---
*Phase: 03-billing-synchronization*
*Completed: 2026-03-14*
