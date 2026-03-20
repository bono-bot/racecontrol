---
phase: 12-remote-execution
plan: 01
subsystem: protocol
tags: [exec, security, allowlist, child-process, sanitization]

# Dependency graph
requires:
  - phase: 09-reliable-delivery
    provides: protocol.js MessageType enum, CONTROL_TYPES set
provides:
  - COMMAND_REGISTRY with 13 commands across 3 approval tiers
  - ApprovalTier enum (auto/notify/approve)
  - buildSafeEnv() for sanitized child process environment
  - validateExecRequest() for registry lookup with rejection
  - exec_request, exec_result, exec_approval message types in protocol.js
affects: [12-02-exec-handler, 12-03-exec-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns: [enum-allowlist-for-command-execution, frozen-safe-env, array-args-only]

key-files:
  created: [shared/exec-protocol.js, test/exec-protocol.test.js]
  modified: [shared/protocol.js]

key-decisions:
  - "13 commands in registry: 8 auto, 2 notify, 3 approve -- static args, no parameterization"
  - "buildSafeEnv returns only PATH/SYSTEMROOT/TEMP/TMP/HOME -- no secrets leak"
  - "No shell:true anywhere -- injection impossible by construction"

patterns-established:
  - "Enum allowlist: remote side sends command NAME, never shell string"
  - "Frozen registry + frozen env: immutable at runtime"

requirements-completed: [EXEC-01, EXEC-02, EXEC-08]

# Metrics
duration: 2min
completed: 2026-03-20
---

# Phase 12 Plan 01: Exec Protocol Summary

**Frozen command registry (13 commands, 3 tiers) with sanitized env and array-args-only execution contracts**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-20T07:42:42Z
- **Completed:** 2026-03-20T07:44:38Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created shared/exec-protocol.js with COMMAND_REGISTRY (13 commands), ApprovalTier enum, buildSafeEnv(), validateExecRequest()
- Extended shared/protocol.js with exec_request, exec_result, exec_approval message types (excluded from CONTROL_TYPES for reliable delivery)
- 16 tests covering registry structure, tier classification, env sanitization, validation, and security (no shell:true)

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend protocol.js + create test scaffold (RED)** - `73ce596` (test)
2. **Task 2: Implement shared/exec-protocol.js (GREEN)** - `e8d7c51` (feat)

_TDD: RED then GREEN. No refactor needed -- implementation matches spec exactly._

## Files Created/Modified
- `shared/exec-protocol.js` - Command registry, approval tiers, safe env builder, request validator
- `shared/protocol.js` - Added exec_request, exec_result, exec_approval to MessageType
- `test/exec-protocol.test.js` - 16 tests across 5 describe blocks

## Decisions Made
- 13 commands in registry with static args (no parameterization) -- simplest secure model, parameterization deferred until concrete need
- buildSafeEnv() includes HOME for cross-platform (Bono's Linux VPS) compatibility
- All exec message types excluded from CONTROL_TYPES -- they need reliable delivery via AckTracker

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- exec-protocol.js ready for import by exec-handler.js (Plan 12-02)
- Protocol message types ready for WS message routing in wiring plan (Plan 12-03)
- All 16 tests green, protocol tests zero regressions

---
*Phase: 12-remote-execution*
*Completed: 2026-03-20*
