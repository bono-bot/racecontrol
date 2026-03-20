---
phase: 73-critical-business-tests
plan: 01
subsystem: testing
tags: [rust, mockall, ffb, hidapi, trait, unit-tests]

# Dependency graph
requires:
  - phase: 72-rc-sentry-endpoint-expansion
    provides: test infrastructure patterns established for rc-agent
provides:
  - FfbBackend trait seam in ffb_controller.rs
  - mockall 0.13 in rc-agent dev-dependencies
  - 8 mock-based FFB unit tests with no HID hardware dependency
affects: [74-rc-agent-decomposition]

# Tech tracking
tech-stack:
  added: [mockall 0.13, tokio test-util (dev-only)]
  patterns: [trait seam pattern for HID abstraction, mockall mock! macro for unit tests, fully-qualified delegation in trait impl to avoid infinite recursion]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ffb_controller.rs
    - crates/rc-agent/Cargo.toml

key-decisions:
  - "FfbBackend trait delegates via FfbController::method(self) fully-qualified syntax to avoid infinite recursion — trait and inherent methods share same names"
  - "mockall mock tests added inside existing #[cfg(test)] mod tests block, not a separate module"
  - "tokio test-util added to dev-dependencies alongside mockall to fix pre-existing billing_guard test compilation errors"

patterns-established:
  - "Trait seam pattern: pub trait + impl for concrete type + mock! in cfg(test) — standard approach for HID/IO abstraction in rc-agent"
  - "Fully-qualified delegation: FfbController::method(self) in impl FfbBackend for FfbController to prevent infinite recursion when trait and inherent method names match"

requirements-completed: [TEST-03]

# Metrics
duration: 15min
completed: 2026-03-20
---

# Phase 73 Plan 01: Critical Business Tests Summary

**FfbBackend trait seam with mockall-generated mock, 8 passing unit tests — FFB controller now testable without HID hardware**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-20T13:10:00Z
- **Completed:** 2026-03-20T13:25:44Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Added `pub trait FfbBackend: Send + Sync` with 5 methods matching FfbController's FFB command surface
- Implemented `impl FfbBackend for FfbController` via fully-qualified delegation (prevents infinite recursion)
- Added `mockall = "0.13"` to dev-dependencies; 8 mock-based unit tests all pass without real HID hardware
- Production build (`cargo build --release --bin rc-agent`) compiles cleanly with zero regressions
- All 30 FFB-related tests pass (8 new mock tests + 22 pre-existing hardware-absent/buffer-format tests)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add mockall dev-dependency and create FfbBackend trait seam** - `1887334` (feat)

**Plan metadata:** (this SUMMARY.md commit — see final commit)

## Files Created/Modified

- `crates/rc-agent/src/ffb_controller.rs` - Added FfbBackend trait, impl FfbBackend for FfbController, 8 mock tests inside existing test module
- `crates/rc-agent/Cargo.toml` - Added mockall = "0.13" and tokio test-util to dev-dependencies

## Decisions Made

- Fully-qualified `FfbController::method(self)` delegation syntax used in trait impl — same-named inherent and trait methods would cause infinite recursion otherwise
- Mock tests added inside the existing `#[cfg(test)] mod tests` block rather than a separate module — keeps all FFB tests co-located
- `tokio = { version = "1", features = ["test-util"] }` added to dev-deps alongside mockall — this was needed by pre-existing billing_guard tests that were failing to compile without it (auto-detected and added by rust-analyzer linter)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added tokio test-util dev-dependency to fix pre-existing billing_guard test compilation**
- **Found during:** Task 1 (first cargo test run)
- **Issue:** billing_guard.rs tests use `tokio::time::pause()` and `tokio::time::advance()` which require the `test-util` tokio feature — 12 compilation errors prevented running any tests
- **Fix:** Added `tokio = { version = "1", features = ["test-util"] }` to [dev-dependencies] (this was also auto-added by rust-analyzer linter simultaneously)
- **Files modified:** crates/rc-agent/Cargo.toml
- **Verification:** cargo test -p rc-agent-crate ffb compiles and all 30 tests pass
- **Committed in:** 1887334 (part of task commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking pre-existing test compilation failure)
**Impact on plan:** Necessary to allow any tests to run at all. The billing_guard fix was pre-existing debt, not scope creep.

## Issues Encountered

- Pre-existing billing_guard.rs tests required tokio test-util feature — blocked compilation of entire test binary. Fixed inline as Rule 3 deviation.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- TEST-03 complete: FfbBackend trait seam is in place as prerequisite for Phase 74 decomposition
- Phase 74 (rc-agent decomposition) can now safely refactor FfbController knowing the trait seam provides test coverage
- Remaining Phase 73 plans (billing_guard + failure_monitor tests) can proceed independently

---
*Phase: 73-critical-business-tests*
*Completed: 2026-03-20*
