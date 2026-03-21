---
phase: 132-chain-orchestration
plan: "01"
subsystem: comms-link
tags: [v18.0, chain-orchestration, exec-result-broker, tdd]

# Dependency graph
requires:
  - phase: 131-shell-relay
    provides: ShellRelayHandler, exec routing on both sides
provides:
  - ExecResultBroker class: shared pending-promise pattern for exec_request/exec_result pairs
  - ChainOrchestrator class: sequential multi-step exec chains with stdout piping, abort-on-failure, continue_on_error, chain-level timeout
  - TDD test suites: 7 tests for ExecResultBroker, 9 tests for ChainOrchestrator
affects: [shared/exec-result-broker.js, shared/chain-orchestrator.js, test/exec-result-broker.test.js, test/chain-orchestrator.test.js]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ExecResultBroker: Map<execId, {resolve, reject, timer}> pending-promise pattern extracted from FailoverOrchestrator"
    - "ChainOrchestrator: Promise.race for chain-level timeout against sequential step loop"
    - "stdout piping: prevStdout accumulated and passed as previousStdout in each exec_request payload"
    - "chainId prefix 'ch_', execId prefix 'ex_' -- consistent with existing FailoverOrchestrator pattern"

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/shared/exec-result-broker.js
    - C:/Users/bono/racingpoint/comms-link/shared/chain-orchestrator.js
    - C:/Users/bono/racingpoint/comms-link/test/exec-result-broker.test.js
    - C:/Users/bono/racingpoint/comms-link/test/chain-orchestrator.test.js
  modified: []

key-decisions:
  - "ExecResultBroker is a pure standalone utility with zero imports -- no dependency on protocol.js or any other comms-link module"
  - "ChainOrchestrator uses Promise.race between stepLoopPromise and a chainTimeout promise -- avoids AbortController complexity while cleanly racing both"
  - "Chain status OK when all steps ran (even if some had continue_on_error failures) -- FAILED only when a non-continue_on_error step exits non-zero"
  - "shutdown() on ExecResultBroker only clears timers, does not reject pending promises -- caller is responsible for not awaiting after shutdown"

requirements-completed: [CHAIN-01, CHAIN-02, CHAIN-03, CHAIN-04, CHAIN-05]

# Metrics
duration_minutes: 8
tasks_completed: 2
files_created: 4
files_modified: 0
tests_written: 16
completed_date: "2026-03-22"
---

# Phase 132 Plan 01: Chain Orchestration Summary

**ExecResultBroker (pending-promise utility) and ChainOrchestrator (sequential exec chains with stdout piping, abort-on-failure, continue_on_error, and chain-level timeout) -- fully TDD'd with 16 tests green.**

---

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-22T04:05:00Z
- **Completed:** 2026-03-22T04:13:00Z
- **Tasks:** 2
- **Files:** 4 (all created)

## Accomplishments

- `shared/exec-result-broker.js`: ExecResultBroker class extracted from FailoverOrchestrator's pending-map pattern
  - `waitFor(execId, timeoutMs)`: returns Promise, sets timer that rejects + deletes on timeout
  - `handleResult(payload)`: resolves pending entry by execId, silent no-op on miss or null payload
  - `shutdown()`: clears all pending timers, empties map
  - Zero external dependencies -- pure Node.js built-ins only

- `shared/chain-orchestrator.js`: ChainOrchestrator class for sequential multi-step execution
  - `execute({ steps, chainTimeoutMs? })`: sequences steps, passes stdout forward, handles failures
  - stdout piping: `prevStdout` accumulated per step, sent as `previousStdout` in each exec_request
  - Abort-on-failure: stops loop if `exitCode !== 0` and `!step.continue_on_error`
  - `continue_on_error: true`: step failure does not abort chain, next step still runs
  - Chain-level timeout: `Promise.race` between step loop and `chainTimeoutMs` timer
  - `chain_result` shape: `{ chainId, status: 'OK'|'FAILED'|'TIMEOUT', steps, totalDurationMs, abortReason? }`
  - chainId format: `ch_` + first 8 chars of UUID

- `test/exec-result-broker.test.js`: 7 tests -- resolve, no-op, timeout, late-result, concurrent, shutdown, null payload
- `test/chain-orchestrator.test.js`: 9 tests -- CHAIN-01 through CHAIN-05 requirements covered, plus edge cases

## Task Commits

1. **Task 1 RED: TDD ExecResultBroker (tests)** - `56887e3` (test)
2. **Task 1 GREEN: ExecResultBroker implementation** - `87fbe78` (feat)
3. **Task 2 RED: TDD ChainOrchestrator (tests)** - `05411ce` (test)
4. **Task 2 GREEN: ChainOrchestrator implementation** - `696aaaa` (feat)

## Files Created

- `shared/exec-result-broker.js` - ExecResultBroker: waitFor/handleResult/shutdown, Map-based pending-promise pattern
- `shared/chain-orchestrator.js` - ChainOrchestrator: sequential steps, stdout piping, abort-on-failure, continue_on_error, chain timeout
- `test/exec-result-broker.test.js` - 7 TDD tests: resolve, no-op, timeout, late-result, concurrent, shutdown, null payload
- `test/chain-orchestrator.test.js` - 9 TDD tests: CHAIN-01 to CHAIN-05 + edge cases (empty steps, single step, chainId format)

## Decisions Made

- ExecResultBroker has zero external imports -- pure Node.js `node:crypto` not even needed (UUID is caller's responsibility). Actually even simpler: it has NO imports at all, purely in-memory Map operations.
- ChainOrchestrator uses `Promise.race` between the full sequential step loop and a chain timeout setTimeout -- cleanest approach without AbortController complexity.
- Chain status is `OK` if all steps ran (even with `continue_on_error` failures). Status is `FAILED` only when execution stops due to a non-`continue_on_error` step failure.
- `shutdown()` on ExecResultBroker clears timers but does not actively reject pending promises -- this matches the existing FailoverOrchestrator pattern and avoids unhandled rejection noise.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None -- no external configuration needed. Plan 02 will wire these classes into james/index.js and bono/index.js.

## Next Phase Readiness

- ExecResultBroker ready to be used by Plan 02 (wire chain_request handling into index.js)
- ChainOrchestrator ready for DI injection in both james/index.js and bono/index.js
- All 16 tests green, no regressions in existing test suite

## Self-Check: PASSED

All 4 files found on disk. All 4 task commits verified in git history.

---
*Phase: 132-chain-orchestration*
*Completed: 2026-03-22*
