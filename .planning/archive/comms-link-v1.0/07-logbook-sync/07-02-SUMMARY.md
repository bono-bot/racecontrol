---
phase: 07-logbook-sync
plan: 02
subsystem: sync
tags: [websocket, file-sync, ack-gate, conflict-detection, echo-suppression, atomic-write]

# Dependency graph
requires:
  - phase: 07-logbook-sync
    provides: "LogbookWatcher class, atomicWrite(), detectConflict(), getAppendedLines() from Plan 01"
  - phase: 01-websocket-connection
    provides: "CommsClient, createCommsServer, protocol.js (createMessage, parseMessage)"
provides:
  - "wireLogbook() for James: watcher -> file_sync via CommsClient with ack gate"
  - "wireLogbook() for Bono: watcher -> file_sync broadcast via WebSocket server"
  - "Bidirectional file sync with 30s ack timeout, conflict detection, and reconnect convergence"
  - "Production entry point integration: LogbookWatcher created and started in both james and bono"
affects: [08-deployment, logbook-sync]

# Tech tracking
tech-stack:
  added: []
  patterns: [ack-gate-sync, conflict-file-fallback, reconnect-convergence, echo-suppression-bracket]

key-files:
  created:
    - test/logbook-sync.test.js
  modified:
    - james/watchdog-runner.js
    - bono/index.js

key-decisions:
  - "lastSentContent tracked alongside lastSyncedHash so ack handler can establish conflict detection base"
  - "Conflict writes .conflict file and preserves local -- never overwrites local on non-mergeable conflict"
  - "Bono broadcasts file_sync to all connected clients (future-proof for multiple James instances)"
  - "Bono requires LOGBOOK_PATH env var (no default) -- Bono must explicitly configure"
  - "James defaults to C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md per locked decision"

patterns-established:
  - "Ack gate: pendingAck boolean + 30s timeout prevents flood of file_sync messages"
  - "Echo suppression bracket: suppressNextCycle() before write, resumeDetection(hash) after write"
  - "Reconnect convergence: 'open' event on James / 'connection' event on Bono triggers fresh file_sync"
  - "wireLogbook() DI pattern: accepts readFileFn/writeFileFn/renameFn for testability"

requirements-completed: [LS-02, LS-05]

# Metrics
duration: 8min
completed: 2026-03-12
---

# Phase 7 Plan 02: Logbook Sync Wiring Summary

**Bidirectional LOGBOOK.md sync wired into James and Bono entry points with ack-gated flow, append-merge, conflict detection, and reconnect convergence**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-12T14:35:19Z
- **Completed:** 2026-03-12T14:43:42Z
- **Tasks:** 1 (TDD: RED-GREEN)
- **Files modified:** 3

## Accomplishments
- wireLogbook() for James wires LogbookWatcher to CommsClient with ack gate and 30s timeout
- wireLogbook() for Bono wires LogbookWatcher to WebSocket server with broadcast to all clients
- Conflict detection: append-only concurrent edits auto-merged, non-append conflicts write .conflict file
- Echo suppression prevents infinite sync loops (suppress before write, resume after)
- Reconnect convergence: both sides exchange file_sync on connection for immediate state reconciliation
- 16 new integration tests, 178 total (zero failures, zero regressions)

## Task Commits

Each task was committed atomically:

1. **RED: Failing integration tests** - `ab7c38c` (test)
2. **GREEN: wireLogbook implementation for James + Bono** - `d45d28e` (feat)

_TDD flow: RED (failing tests) -> GREEN (implementations passing all 16 tests)_

## Files Created/Modified
- `test/logbook-sync.test.js` - 16 integration tests: sync flow, ack, timeout, reconnect, conflict, echo suppression
- `james/watchdog-runner.js` - wireLogbook() export + production LogbookWatcher integration
- `bono/index.js` - wireLogbook() export + production LogbookWatcher integration

## Decisions Made
- Track `lastSentContent` in closure so ack handler can set `lastSyncedContent` for conflict detection base
- Bono broadcasts to all wss.clients (not just the sender) for future multi-client support
- Bono requires LOGBOOK_PATH env var with no default (explicit configuration required on VPS)
- James defaults LOGBOOK_PATH to `C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md` (per locked decision)
- Mock timers only enabled per-test for timeout tests (not globally) to avoid interfering with async flush

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed lastSyncedContent tracking for conflict detection**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** file_ack handler updated lastSyncedHash but not lastSyncedContent, causing null reference in detectConflict
- **Fix:** Track lastSentContent in changed handler, copy to lastSyncedContent when ack received
- **Files modified:** james/watchdog-runner.js
- **Verification:** Tests 8 and 9 (auto-merge and conflict) now pass
- **Committed in:** d45d28e (GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential fix for conflict detection to work. No scope creep.

## Issues Encountered
- Node.js 22 mock timers do not accept 'clearTimeout' in apis array (auto-mocked with setTimeout) -- restructured tests to enable mock timers only in timeout-specific tests

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 7 (LOGBOOK Sync) is complete -- all 2 plans done
- Both James and Bono entry points now create, wire, start, and stop LogbookWatcher
- Bono deployment requires LOGBOOK_PATH env var to be set
- 178 tests all passing, stable baseline for Phase 8

## Self-Check: PASSED

- All files exist: test/logbook-sync.test.js, james/watchdog-runner.js, bono/index.js, 07-02-SUMMARY.md
- All commits verified: ab7c38c (RED), d45d28e (GREEN)
- Full test suite: 178 tests, 0 failures

---
*Phase: 07-logbook-sync*
*Completed: 2026-03-12*
