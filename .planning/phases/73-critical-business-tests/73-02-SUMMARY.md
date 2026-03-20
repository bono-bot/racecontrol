---
phase: 73-critical-business-tests
plan: 02
subsystem: testing
tags: [tokio, test-util, billing_guard, failure_monitor, mpsc, watch, async-tests]

# Dependency graph
requires:
  - phase: 73-critical-business-tests/73-01
    provides: FfbBackend trait seam, mockall dev-dependency, tokio test-util in dev-deps

provides:
  - "billing_guard: 6 async timer+channel tests using tokio::time::pause() — BILL-02/BILL-03 verified via mpsc"
  - "failure_monitor: 6 requirement-named tests — CRASH-01/CRASH-02 condition logic traced to requirement IDs"

affects: [74-rc-agent-decomposition]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "tokio::time::pause() + yield_now() pattern: yield BEFORE advance() to let spawned tasks start; then advance to record timer baseline; then advance past threshold to fire anomaly"
    - "tokio::time::Instant instead of std::time::Instant for debounce timers — enables mock clock control in tests"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/billing_guard.rs
    - crates/rc-agent/src/failure_monitor.rs

key-decisions:
  - "Use tokio::time::Instant (not std::time::Instant) for game_gone_since and idle_since debounce timers — required for tokio test-util mock clock to control elapsed() in tests"
  - "Two-phase advance pattern: yield_now x5 first (to let spawned task start), then advance(5s) to record timer baseline, then advance past threshold to trigger anomaly"
  - "failure_monitor CRASH-01 tests cover condition logic up to the OS API gate only — is_game_process_hung() is Windows hardware behavior not testable without a real hung process"

patterns-established:
  - "Tokio test-util initialization order: pause() → spawn() → yield x5 → advance(baseline) → yield x5 → advance(threshold) → yield x15 → assert"

requirements-completed: [TEST-01, TEST-02]

# Metrics
duration: 15min
completed: 2026-03-20
---

# Phase 73 Plan 02: Critical Business Tests (Timer + Channel) Summary

**6 async billing_guard tests verify BILL-02/BILL-03 actually send AgentMessage through mpsc after 60s/300s; 6 requirement-named failure_monitor tests trace CRASH-01/CRASH-02 to condition guards**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-20T13:34:07Z (IST: 2026-03-20 19:04:07)
- **Completed:** 2026-03-20T13:49:07Z (IST: 2026-03-20 19:19:07)
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- 6 async tokio::time::pause() tests in billing_guard: BILL-02 fires after 60s (SessionStuckWaitingForGame), BILL-03 fires after 300s (IdleBillingDrift), both suppressed by recovery_in_progress and billing_paused
- 6 requirement-named tests in failure_monitor: CRASH-01 (UDP silence condition guard, 3 variants) and CRASH-02 (launch timeout condition guard, 3 variants) with traceability comments
- Switched debounce timers from std::time::Instant to tokio::time::Instant — makes elapsed() controlled by mock clock in tests with no production behavior change
- All 17 billing_guard tests pass (11 existing + 6 new); all 20 failure_monitor tests pass (14 existing + 6 new)

## Task Commits

1. **Task 1: Add billing_guard timer + channel tests (TEST-01)** - `bc2cfc9` (test)
2. **Task 2: Add failure_monitor requirement-named tests (TEST-02)** - `076f905` (test)

## Files Created/Modified
- `crates/rc-agent/src/billing_guard.rs` - 6 async timer tests + tokio::time::Instant for debounce timers
- `crates/rc-agent/src/failure_monitor.rs` - 6 requirement-named CRASH-01/CRASH-02 condition tests

## Decisions Made
- Switched `game_gone_since` and `idle_since` from `std::time::Instant` to `tokio::time::Instant`. Reason: the test-util mock clock only controls `tokio::time::*` functions. With `std::time::Instant`, elapsed() returns real wall time during tests and timer-based tests cannot be deterministic. Production behavior is identical — tokio::time::Instant tracks real time when not paused.
- The critical tokio test-util pattern: the spawned task must be polled ONCE before calling advance() — otherwise the task starts at the post-advance time and records timer baselines at the wrong moment. Fixed by yielding 5 times before the first advance().

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Switched debounce timers to tokio::time::Instant for testability**
- **Found during:** Task 1 (billing_guard timer tests)
- **Issue:** Production code used `std::time::Instant` for `game_gone_since` and `idle_since`. `tokio::time::pause()` only mocks `tokio::time::*` — `std::time::Instant::elapsed()` returns real wall time. Timer-based tests would fail non-deterministically or require real sleeps (60s, 300s).
- **Fix:** Changed both debounce timers to `tokio::time::Instant`. The `get_or_insert_with(tokio::time::Instant::now)` call now uses mock time, and `elapsed()` returns mock-clock elapsed time during tests.
- **Files modified:** `crates/rc-agent/src/billing_guard.rs` (lines 58-60, 91, 159)
- **Verification:** Tests pass; release build compiles cleanly
- **Committed in:** `bc2cfc9` (Task 1 commit)

**2. [Rule 1 - Bug] Yield-before-advance pattern required for spawned task initialization**
- **Found during:** Task 1 debugging — timer tests failing despite correct logic
- **Issue:** Calling `advance()` before the spawned task was polled caused the task to start AFTER the clock advance, recording timer baselines at the post-advance time (elapsed = 0s always).
- **Fix:** Added `for _ in 0..5 { yield_now().await }` before the first `advance(5)` call in each test. This lets the spawned task start, create its interval, and block on the first tick before any clock advance.
- **Files modified:** `crates/rc-agent/src/billing_guard.rs` (all 6 async tests)
- **Verification:** Debug output confirmed tick timing; all 6 tests pass
- **Committed in:** `bc2cfc9` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 Rule 1 bugs)
**Impact on plan:** Both fixes necessary for timer-based tests to work. No scope creep — all changes are test-correctness fixes within billing_guard.rs.

## Issues Encountered
- tokio test-util mock clock timing required careful investigation: `advance()` alone is insufficient because spawned tasks aren't polled until they appear in the executor run queue. Required understanding of single-threaded tokio executor scheduling to find the yield-before-advance pattern.

## Next Phase Readiness
- Phase 74 (rc-agent decomposition) is unblocked: all billing_guard and failure_monitor characterization tests are green
- TEST-01 and TEST-02 complete; TEST-03 (FfbBackend mock tests from 73-01) already complete
- Phase 73 requirements fully satisfied

---
*Phase: 73-critical-business-tests*
*Completed: 2026-03-20*
