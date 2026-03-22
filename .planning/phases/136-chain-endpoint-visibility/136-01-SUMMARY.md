---
phase: 136-chain-endpoint-visibility
plan: 01
subsystem: api
tags: [websocket, chain, exec-result-broker, comms-link, relay]

# Dependency graph
requires:
  - phase: 133-delegation
    provides: delegate_result handler pattern used as reference for chain_result handler
  - phase: 134-chain-orchestrator
    provides: ChainOrchestrator and /relay/chain/run HTTP endpoint with waitFor(chainId)
provides:
  - chain_result WS message handler in james/index.js that routes through ExecResultBroker
  - /relay/chain/run HTTP endpoint resolves synchronously without 504 timeout
affects: [relay-chain-run, exec-result-broker, chain-endpoint-visibility]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WS result handler routes through ExecResultBroker.handleResult({ execId: msg.payload?.chainId, ...msg.payload }) — same pattern as delegate_result"

key-files:
  created: []
  modified:
    - C:/Users/bono/racingpoint/comms-link/james/index.js

key-decisions:
  - "chain_result handler placed immediately after delegate_result handler (line ~647) — before the catch-all console.log fallthrough"
  - "execId mapped from msg.payload?.chainId to match the waitFor(chainId) call in /relay/chain/run"

patterns-established:
  - "Pattern: incoming WS result messages route through ExecResultBroker.handleResult({ execId: msg.payload?.<idField>, ...msg.payload })"

requirements-completed: [CHAIN-10, CHAIN-11]

# Metrics
duration: 8min
completed: 2026-03-22
---

# Phase 136 Plan 01: Chain Endpoint Visibility Summary

**Missing chain_result WS handler added to james/index.js — /relay/chain/run now resolves synchronously via ExecResultBroker instead of 504-timing-out**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-22T04:00:00Z
- **Completed:** 2026-03-22T04:08:00Z
- **Tasks:** 1 of 1
- **Files modified:** 1

## Accomplishments

- Added `msg.type === 'chain_result'` handler in james/index.js after the delegate_result handler
- Handler calls `execResultBroker.handleResult({ execId: msg.payload?.chainId, ...msg.payload })` — resolves the `waitFor(chainId)` promise in `/relay/chain/run`
- Verified with node --check (syntax clean) and position assertions (handleResult call is inside the chain_result block, not the delegate_result block)
- exec_result and delegate_result handlers left completely untouched (CHAIN-11 regression-safe)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add chain_result WS message handler to james/index.js** - `cfb80eb` (feat)

**Supporting commits:**
- `c6daa04` - chore: LOGBOOK update
- `874a5e5` - chore: INBOX notification to Bono

## Files Created/Modified

- `C:/Users/bono/racingpoint/comms-link/james/index.js` - Added chain_result WS handler (lines 647-652, 10 lines inserted)

## Decisions Made

- Placed handler immediately after delegate_result (line ~647), before the catch-all `console.log('Received:', ...)` — minimal diff, correct position in the if-else chain
- Used identical broker call pattern to delegate_result: `execResultBroker.handleResult({ execId: msg.payload?.chainId, ...msg.payload })`
- The plan's automated verification script had an `indexOf` bug (found first occurrence of handleResult in delegate_result, not the new one in chain_result). Verified with a corrected check using `indexOf(pattern, chain_result_idx)` — both pass.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

The plan's inline verification script used `indexOf` which found the first `handleResult` call (in delegate_result, line 643) rather than the one inside the new chain_result handler (line 651). This is a false failure in the verification script — the code is correct. A corrected check confirms the second `handleResult` call is at char 24541, after the chain_result handler start at char 24330.

## User Setup Required

None - no external service configuration required. James daemon should be restarted to load the updated james/index.js. Bono notified via comms-link.

## Next Phase Readiness

- CHAIN-10 and CHAIN-11 resolved: `/relay/chain/run` will no longer 504 when Bono sends back chain_result
- No additional changes needed on Bono's side — Bono already sends chain_result WS messages; James simply wasn't handling them
- Phase 136 plan 01 complete; remaining plans in 136-chain-endpoint-visibility can proceed

---
*Phase: 136-chain-endpoint-visibility*
*Completed: 2026-03-22*
