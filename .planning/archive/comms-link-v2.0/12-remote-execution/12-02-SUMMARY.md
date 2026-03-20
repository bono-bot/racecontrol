---
phase: 12-remote-execution
plan: 02
subsystem: exec
tags: [exec-handler, approval-flow, 3-tier, dedup, timeout, child-process]

# Dependency graph
requires:
  - phase: 12-remote-execution
    provides: exec-protocol.js COMMAND_REGISTRY, ApprovalTier, buildSafeEnv
provides:
  - ExecHandler class with 3-tier approval flow (auto/notify/approve)
  - Timeout default-deny for approve-tier commands
  - Deduplication via completedExecs Set
  - Output truncation (50KB stdout, 10KB stderr)
affects: [12-03-exec-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns: [di-constructor-for-testability, eventemitter-lifecycle-events, timeout-default-deny]

key-files:
  created: [james/exec-handler.js, test/exec-handler.test.js]
  modified: []

key-decisions:
  - "Use injected commandRegistry instead of imported validateExecRequest for testability with custom registries"
  - "Approval timeout sends timed_out tier result (not rejected) for distinguishable telemetry"
  - "shutdown() clears timers but does not send results for pending approvals"

patterns-established:
  - "DI constructor: execFileFn, sendResultFn, notifyFn, nowFn, approvalTimeoutMs all injectable"
  - "Timeout default-deny: unapproved commands expire with structured error after configurable timeout"

requirements-completed: [EXEC-01, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 12 Plan 02: Exec Handler Summary

**ExecHandler with 3-tier approval flow (auto/notify/approve), timeout default-deny, dedup, and sanitized child_process execution**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T07:47:30Z
- **Completed:** 2026-03-20T07:50:21Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Created ExecHandler class with full 3-tier routing: auto (immediate), notify (immediate + notification), approve (queued with timeout)
- 17 tests covering all tiers, timeout default-deny, dedup, truncation, security (shell:false, safeEnv), events, and lifecycle
- DI constructor for complete testability -- all dependencies injectable

## Task Commits

Each task was committed atomically:

1. **Task 1: Write ExecHandler test scaffold (RED)** - `1c0863b` (test)
2. **Task 2: Implement ExecHandler class (GREEN)** - `13339fd` (feat)

_TDD: RED then GREEN. No refactor needed._

## Files Created/Modified
- `james/exec-handler.js` - ExecHandler class with 3-tier approval, timeout, dedup, truncation
- `test/exec-handler.test.js` - 17 tests across 9 describe blocks

## Decisions Made
- Used injected commandRegistry lookup instead of imported validateExecRequest -- enables testing with custom registries without modifying shared module
- Approval timeout sends tier='timed_out' (distinct from 'rejected') for clear telemetry
- shutdown() silently clears pending approvals without sending results -- avoids confusing late notifications

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed validateExecRequest using global registry instead of injected one**
- **Found during:** Task 2 (GREEN implementation)
- **Issue:** validateExecRequest from exec-protocol.js uses hardcoded COMMAND_REGISTRY, not the injected commandRegistry. Tests with custom TEST_REGISTRY all failed because commands weren't in global registry.
- **Fix:** Replaced validateExecRequest call with direct this.#commandRegistry[command] lookup
- **Files modified:** james/exec-handler.js
- **Verification:** All 17 tests pass
- **Committed in:** 13339fd (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary for testability. No scope creep.

## Issues Encountered

None beyond the deviation noted above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- ExecHandler ready for import by wiring plan (Plan 12-03)
- Events (exec_started, exec_completed, pending_approval, approval_timeout) available for WS message routing
- All 17 exec-handler tests green, 16 exec-protocol tests green (zero regressions)

## Self-Check: PASSED

- FOUND: james/exec-handler.js
- FOUND: test/exec-handler.test.js
- FOUND: 1c0863b (Task 1 RED)
- FOUND: 13339fd (Task 2 GREEN)

---
*Phase: 12-remote-execution*
*Completed: 2026-03-20*
