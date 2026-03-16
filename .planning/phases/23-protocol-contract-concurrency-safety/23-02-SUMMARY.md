---
phase: 23-protocol-contract-concurrency-safety
plan: 02
subsystem: infra
tags: [rust, axum, watchdog, concurrency, predicate, tdd]

# Dependency graph
requires:
  - phase: 23-protocol-contract-concurrency-safety
    provides: "WatchdogState enum in racecontrol/src/state.rs"
provides:
  - "pub fn is_pod_in_recovery(&WatchdogState) -> bool in pod_healer.rs"
  - "4 unit tests covering all WatchdogState variants for is_pod_in_recovery"
affects:
  - "24-bot-expansion — crash_handler and usb_handler must call is_pod_in_recovery before acting"
  - "25-billing-recovery — any bot task acting on a pod must gate on this predicate"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure predicate function taking &WatchdogState (not Arc<AppState>) — no async, no AppState dependency, fully unit-testable"
    - "TDD Red-Green cycle: write failing tests first, then implement the function"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "is_pod_in_recovery() lives in racecontrol crate (not rc-common) — WatchdogState is server-local, moving it would require touching AppState, pod_monitor, pod_healer"
  - "RecoveryFailed returns false — watchdog gave up, bots may still attempt fixes (different semantic from Restarting/Verifying)"
  - "No #[allow(dead_code)] — function is pub and compiler will warn when Phase 24 code uses it"

patterns-established:
  - "Concurrency guard pattern: bot tasks call is_pod_in_recovery(&wd_state) before acting on a pod"
  - "Pure predicate mirrors should_skip_for_watchdog_state() — co-located in pod_healer.rs for watchdog integration coherence"

requirements-completed: [PROTO-03]

# Metrics
duration: 10min
completed: 2026-03-16
---

# Phase 23 Plan 02: Protocol Contract — is_pod_in_recovery() Predicate Summary

**Pure concurrency guard predicate is_pod_in_recovery(&WatchdogState) -> bool added to pod_healer.rs, blocking Phase 24 bot tasks from acting on pods in active watchdog recovery cycles**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-16T09:56:25Z
- **Completed:** 2026-03-16T10:06:49Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `pub fn is_pod_in_recovery(&WatchdogState) -> bool` to `pod_healer.rs` — returns true for Restarting and Verifying, false for Healthy and RecoveryFailed
- 4 unit tests covering all WatchdogState variants pass: restarting (true), verifying (true), healthy (false), recovery_failed (false)
- All 242 racecontrol-crate tests green (238 baseline + 4 new), zero regressions
- TDD cycle executed correctly: RED (4 compile errors for missing function), GREEN (function added, all pass)

## Task Commits

1. **Task 1: Add is_pod_in_recovery() predicate to pod_healer.rs with 4 unit tests** - `b9fdfee` (feat)

## Files Created/Modified
- `crates/racecontrol/src/pod_healer.rs` - Added `pub fn is_pod_in_recovery()` (line 775) and 4 unit tests in the existing `#[cfg(test)]` block

## Decisions Made
- is_pod_in_recovery() is `pub` not `pub(crate)` — Phase 24 bot modules in the same crate can call it without restriction
- RecoveryFailed intentionally returns false: this state means the watchdog exhausted its retry budget. The bot should still be allowed to attempt a fix — different semantic from the in-progress Restarting/Verifying states
- No refactor step needed — function is a clean one-liner `matches!()` mirroring `should_skip_for_watchdog_state()`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- PROTO-03 complete: is_pod_in_recovery() is pub, callable from Phase 24 crash_handler and usb_handler modules
- Phase 24 bot tasks must call `is_pod_in_recovery(&wd_state)` before acting on any pod — the guard is in place
- Phase 23 plan 01 (PodFailureReason enum) also complete — both protocol contracts are ready for Phase 24

## Self-Check: PASSED

- FOUND: `crates/racecontrol/src/pod_healer.rs` (contains `pub fn is_pod_in_recovery`)
- FOUND: commit `b9fdfee` feat(23-02): add is_pod_in_recovery() predicate with 4 unit tests
- All 4 `is_pod_in_recovery` tests pass
- All 283 tests (242 unit + 41 integration) green, zero regressions

---
*Phase: 23-protocol-contract-concurrency-safety*
*Completed: 2026-03-16*
