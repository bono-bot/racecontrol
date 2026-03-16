---
phase: 26-lap-filter-pin-security-telemetry-multiplayer
plan: 03
subsystem: auth
tags: [pin, security, hashmap, appstate, rust, tokio, rwlock]

# Dependency graph
requires:
  - phase: 26-01
    provides: Wave 0 RED stubs for PIN-01/PIN-02 as todo!() in auth/mod.rs
provides:
  - AppState.customer_pin_failures: RwLock<HashMap<String, u32>> — per-pod customer failure counter
  - AppState.staff_pin_failures: RwLock<HashMap<String, u32>> — per-pod staff failure counter
  - CUSTOMER_PIN_LOCKOUT_THRESHOLD = 5 in auth/mod.rs
  - validate_pin() lockout check + counter increment + success reset
  - validate_employee_pin() staff counter increment + success reset (no lockout ceiling)
affects: [26-04, any plan touching auth or PIN validation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Structural counter separation: two distinct HashMaps in AppState prevent accidental counter merge across customer/staff paths"
    - "Lockout gate pattern: read-lock check BEFORE DB query avoids wasted DB round-trip on locked pod"
    - "Match-then-increment pattern: unwrap DB result to match block so counter increment is before return Err"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/auth/mod.rs

key-decisions:
  - "PIN counters: strict type separation — customer and staff counters never share state (confirmed in 26-01 decisions, now structurally enforced)"
  - "No lockout ceiling for staff — PIN-02 is absolute: validate_employee_pin() never reads customer_pin_failures"
  - "In-memory only counters — reset on server restart is acceptable; no DB persistence needed"
  - "Counter check BEFORE DB lookup — avoids wasted query cost on locked pod"

patterns-established:
  - "PIN-01: customer_pin_failures keyed by pod_id, threshold 5, reset on success"
  - "PIN-02: staff_pin_failures keyed by pod_id, no ceiling, reset on success, never touches customer counter"

requirements-completed: [PIN-01, PIN-02]

# Metrics
duration: 4min
completed: 2026-03-16
---

# Phase 26 Plan 03: PIN-01/02 Separate Customer and Staff Failure Counters Summary

**Two structurally separate RwLock<HashMap> counters in AppState enforce that customer PIN lockout can never block staff debug PIN access — customer exhausts 5 attempts, staff still unlocks freely**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-16T13:27:37Z
- **Completed:** 2026-03-16T13:31:21Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- AppState gained `customer_pin_failures` and `staff_pin_failures` as separate `RwLock<HashMap<String, u32>>` fields — the separation is structural by construction
- `validate_pin()` now gates on `CUSTOMER_PIN_LOCKOUT_THRESHOLD = 5` before DB lookup; increments customer counter on token miss; resets on success
- `validate_employee_pin()` increments staff counter on wrong PIN, resets on success, and has a PIN-02 invariant comment confirming it NEVER touches `customer_pin_failures`
- 3 Wave 0 PIN stubs turned GREEN: `customer_and_staff_counters_are_separate`, `customer_failures_do_not_affect_staff_counter`, `staff_pin_succeeds_when_customer_counter_maxed`
- Full suite: 261 tests pass, 8 expected RED stubs from other plans (LAP/TELEM/MULTI) remain

## Task Commits

Each task was committed atomically:

1. **Task 1: AppState new fields + validate_pin counter logic (PIN-01, PIN-02)** - `c4e47f5` (feat)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - Added `customer_pin_failures` and `staff_pin_failures` fields to AppState struct and initialized both in `AppState::new()`
- `crates/racecontrol/src/auth/mod.rs` - Added `CUSTOMER_PIN_LOCKOUT_THRESHOLD`, lockout check + counter increment in `validate_pin()`, staff counter in `validate_employee_pin()`, and replaced 3 todo!() stubs with GREEN test implementations

## Decisions Made

- Counter check placed BEFORE DB lookup in `validate_pin()` — avoids wasted DB round-trip on locked pod. Cheap read-lock check gates the expensive query.
- Used match-block pattern for token lookup result instead of chaining `.ok_or_else()` — allows counter increment to happen in the failure branch before returning Err without borrow conflicts on `pod_id`.
- Staff counter has no lockout ceiling — PIN-02 requirement is absolute. No threshold constant defined for staff; the check is simply never performed.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PIN-01 and PIN-02 are fully implemented and verified GREEN
- 26-02 (LAP filter) and 26-04 (TELEM/MULTI) stubs remain RED and are targeted by their respective plans
- All auth tests pass; no regressions from PIN counter additions

---
*Phase: 26-lap-filter-pin-security-telemetry-multiplayer*
*Completed: 2026-03-16*
