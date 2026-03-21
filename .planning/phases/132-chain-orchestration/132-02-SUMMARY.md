---
phase: 132-chain-orchestration
plan: "02"
subsystem: comms-link
tags: [v18.0, chain-orchestration, exec-result-broker, wiring]

# Dependency graph
requires:
  - phase: 132-chain-orchestration
    plan: "01"
    provides: ExecResultBroker, ChainOrchestrator
  - phase: 131-shell-relay
    provides: ShellRelayHandler, exec routing on both sides
provides:
  - ExecResultBroker wired into james/index.js and bono/index.js
  - ChainOrchestrator wired into james/index.js and bono/index.js
  - FailoverOrchestrator refactored to use shared broker instead of private #pending Map
  - Integration test suite: 3 tests verifying chain wiring end-to-end
affects:
  - C:/Users/bono/racingpoint/comms-link/james/index.js
  - C:/Users/bono/racingpoint/comms-link/bono/index.js
  - C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
  - C:/Users/bono/racingpoint/comms-link/test/chain-wiring.test.js

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ExecResultBroker instantiated before FailoverOrchestrator and ChainOrchestrator -- dependency ordering enforced"
    - "Both sides make message callback async to allow existing await calls (pre-existing pattern fix)"
    - "exec_result routes through shared broker on both sides -- broker.handleResult replaces direct orchestrator call"
    - "chain_request handler uses .then().catch() pattern (no top-level await in event handler)"

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/test/chain-wiring.test.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
    - C:/Users/bono/racingpoint/comms-link/james/index.js
    - C:/Users/bono/racingpoint/comms-link/bono/index.js

key-decisions:
  - "FailoverOrchestrator completely removes #pending Map, handleExecResult, and #waitForExecResult -- broker is the single source of truth for exec_result resolution"
  - "bono/index.js wss.on message callback made async (same fix as james/index.js) -- pre-existing bug where await was used inside non-async callback"
  - "exec_result on Bono side calls BOTH bonoExecResultBroker.handleResult AND wss.emit -- backward compat preserved for existing event consumers"

requirements-completed: [CHAIN-01, CHAIN-04]

# Metrics
duration_minutes: 12
tasks_completed: 2
files_created: 1
files_modified: 3
tests_written: 3
completed_date: "2026-03-22"
---

# Phase 132 Plan 02: Chain Orchestration Wiring Summary

**ExecResultBroker and ChainOrchestrator wired into both james/index.js and bono/index.js -- exec_result routes through shared broker, chain_request triggers ChainOrchestrator.execute on both sides, FailoverOrchestrator simplified to delegate to broker.**

---

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-22T04:16:00Z
- **Completed:** 2026-03-22T04:28:00Z
- **Tasks:** 2
- **Files:** 4 (1 created, 3 modified)

## Accomplishments

- `james/failover-orchestrator.js`: Refactored to use ExecResultBroker
  - Removed: `#pending = new Map()`, `#waitForExecResult()` private method, `handleExecResult()` public method
  - Added: `#broker` field, `broker` constructor parameter
  - All 6 `await this.#waitForExecResult(...)` calls replaced with `await this.#broker.waitFor(...)`
  - Constructor signature: `{ client, httpPost, alertCooldown, broker }`

- `james/index.js`: Full chain wiring
  - Imports: `ExecResultBroker`, `ChainOrchestrator`
  - `execResultBroker` instantiated before FailoverOrchestrator and ChainOrchestrator
  - FailoverOrchestrator receives `broker: execResultBroker`
  - ChainOrchestrator: `sendFn` uses `connectionMode.sendCritical`, `identity: 'james'`
  - `exec_result` handler: `execResultBroker.handleResult(msg.payload)` (not `failoverOrchestrator.handleExecResult`)
  - `chain_request` handler: calls `chainOrchestrator.execute()`, sends `chain_result`
  - `execResultBroker.shutdown()` added to shutdown function
  - Made `client.on('message', ...)` callback async (pre-existing bug fix)

- `bono/index.js`: Symmetric wiring inside wireBono()
  - Imports: `ExecResultBroker`, `ChainOrchestrator`
  - `bonoExecResultBroker` and `bonoChainOrchestrator` instantiated in wireBono()
  - ChainOrchestrator `sendFn`: sends to first connected `wss.clients` entry
  - `exec_result` handler: calls `bonoExecResultBroker.handleResult(payload)` AND `wss.emit('exec_result', payload)`
  - `chain_request` handler: calls `bonoChainOrchestrator.execute()`, sends `chain_result` to first client
  - `wireBono()` return now includes `bonoChainOrchestrator` and `bonoExecResultBroker`
  - Made `wss.on('message', ...)` callback async (pre-existing bug fix)

- `test/chain-wiring.test.js`: 3 integration tests
  - Test 1: chain_request triggers ChainOrchestrator.execute; auto-resolving exec_results via broker; chain completes OK
  - Test 2: Regular exec_result (non-chain) resolves via broker.waitFor (regression test for FailoverOrchestrator pattern)
  - Test 3: chain_result payload has required shape `{ chainId, status, steps, totalDurationMs }` with correct types

## Task Commits

1. **Task 1: James side wired** - `a966808` (feat)
2. **Task 2: Bono side wired + integration tests** - `aa22050` (feat)

## Files Created

- `test/chain-wiring.test.js` - 3 integration tests covering chain wiring end-to-end

## Files Modified

- `james/failover-orchestrator.js` - Removed #pending Map; uses broker.waitFor throughout
- `james/index.js` - ExecResultBroker + ChainOrchestrator wired; exec_result routed through broker
- `bono/index.js` - Symmetric wiring inside wireBono(); 3 integration tests added

## Decisions Made

- FailoverOrchestrator's private pending-map pattern is fully replaced by the shared broker -- no parallel pending-map infrastructure remains.
- Both sides make the WS message callback `async` to fix a pre-existing bug where `await` was used inside non-async callbacks. This is a bug fix, not a new pattern.
- On Bono side, `exec_result` calls BOTH `bonoExecResultBroker.handleResult()` AND `wss.emit()` to preserve backward compatibility with any consumers that listen to the `exec_result` wss event.
- `wireBono()` return extended with `bonoChainOrchestrator` and `bonoExecResultBroker` for testability -- callers that destructure only `sendTaskRequest` etc. are unaffected.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Pre-existing async/await in non-async WS message callbacks**
- **Found during:** Task 1 verification (james/index.js module load check)
- **Issue:** `client.on('message', (msg) => { ... })` in james/index.js used `await` in two places (registry_register and exec_approval handlers) without the callback being declared `async`. Same issue in bono/index.js `wss.on('message', ...)`.
- **Fix:** Added `async` keyword to both message callbacks.
- **Files modified:** `james/index.js`, `bono/index.js`
- **Commit:** Included in respective task commits

## Issues Encountered

None beyond the pre-existing async bug auto-fixed above.

## User Setup Required

None -- no configuration changes needed. Both sides are wired and ready.

## Next Phase Readiness

- Both james and bono now handle `chain_request` and return `chain_result`
- FailoverOrchestrator simplified -- single broker handles all exec_result routing
- 56 total tests pass across all test suites (no regressions)
- Plan 03 (if planned) can build higher-level chain orchestration features on top of this wiring

## Self-Check: PASSED
