---
phase: 14-graceful-degradation
plan: 01
subsystem: messaging
tags: [state-machine, fallback, offline-queue, email, websocket]

# Dependency graph
requires:
  - phase: 09-protocol-foundation
    provides: MessageQueue WAL persistence (enqueue/getPending/acknowledge/compact)
  - phase: 13-observability
    provides: Email fallback infrastructure validation
provides:
  - ConnectionMode three-state manager (REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE)
  - sendCritical() routing for exec_result, task_request, recovery
  - Offline queue drain with WAL acknowledge + compact on mode upgrade
  - Email availability probe with start/stop
affects: [14-02-wiring, james-index, metrics, heartbeat]

# Tech tracking
tech-stack:
  added: []
  patterns: [three-state-machine, critical-message-routing, offline-drain, DI-for-all-delivery-functions]

key-files:
  created:
    - shared/connection-mode.js
    - test/connection-mode.test.js
  modified: []

key-decisions:
  - "All delivery functions injected via constructor DI -- no imports of send_email.js or CommsClient in module"
  - "CRITICAL_TYPES is a frozen Set of 3 types: exec_result, task_request, recovery"
  - "Optimistic email default (true) -- assumes email available until probe says otherwise"
  - "Drain always calls compact() even on empty queue (no-op, consistent behavior)"

patterns-established:
  - "Three-state mode machine: WS > email > offline priority via #recalculate()"
  - "sendCritical pattern: route by mode, drop non-critical in degraded modes"

requirements-completed: [GD-01, GD-02, GD-03]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 14 Plan 01: ConnectionMode State Machine Summary

**Three-state ConnectionMode manager with sendCritical routing and offline queue drain via DI-injected delivery functions**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T09:06:06Z
- **Completed:** 2026-03-20T09:08:54Z
- **Tasks:** 1 (TDD: RED -> GREEN)
- **Files modified:** 2

## Accomplishments
- ConnectionMode state machine correctly implements REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE priority
- sendCritical routes exec_result, task_request, recovery through correct delivery path per mode
- Offline queue drain replays WAL-buffered messages on mode upgrade with ACK + compact
- Email probe with start/stop for periodic availability checking
- 22 unit tests covering all transitions, routing, drain, and probe behavior
- Full test suite (421 tests) passes with zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: ConnectionMode state machine + sendCritical + drain (TDD)** - `cae637d` (feat)

**Plan metadata:** [pending final commit]

## Files Created/Modified
- `shared/connection-mode.js` - Three-state mode manager with email probe, critical message routing, and drain logic
- `test/connection-mode.test.js` - 22 unit tests for all state transitions, sendCritical routing, drain, and probe

## Decisions Made
- All delivery functions (sendTrackedFn, sendViaEmailFn, probeEmailFn, messageQueue) injected via constructor DI for full testability
- CRITICAL_TYPES frozen Set includes only exec_result, task_request, recovery -- heartbeats and status messages are not routed through fallback
- Optimistic email availability default (true) so initial mode is REALTIME until WS disconnects
- Drain calls compact() even on empty queue for consistent behavior
- Non-critical message types silently dropped (return false) in degraded modes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- ConnectionMode class ready to wire into james/index.js (Plan 14-02)
- Plan 14-02 will integrate with CommsClient state events, extend metrics/heartbeat, and add human verification
- Email fallback depends on Gmail OAuth renewal (known blocker, graceful handling built in)

---
*Phase: 14-graceful-degradation*
*Completed: 2026-03-20*
