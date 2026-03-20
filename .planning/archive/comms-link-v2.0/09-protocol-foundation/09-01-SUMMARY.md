---
phase: 09-protocol-foundation
plan: 01
subsystem: protocol
tags: [ack, retry, deduplication, exponential-backoff, eventemitter]

# Dependency graph
requires: []
provides:
  - AckTracker class with monotonic sequence numbers and exponential backoff retry
  - DeduplicatorCache class with TTL and size eviction
  - msg_ack message type in protocol.js
  - CONTROL_TYPES frozen Set and isControlMessage() helper
affects: [09-02-message-queue, 11-ack-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns: [exponential-backoff-retry, deduplicator-cache, control-message-classification]

key-files:
  created:
    - shared/ack-tracker.js
    - test/ack-tracker.test.js
  modified:
    - shared/protocol.js
    - test/protocol.test.js

key-decisions:
  - "CONTROL_TYPES excludes file_ack and sync_action_ack -- those are data-layer ACKs needing reliable delivery"
  - "AckTracker uses real timers with short timeoutMs for tests (no mock timers -- project lesson from Phase 3)"
  - "DI pattern: sendFn, nowFn injectable for testing (matches process-supervisor.js convention)"

patterns-established:
  - "makeTracker() test helper pattern for AckTracker -- mirrors makeXxx() from process-supervisor tests"
  - "isControlMessage() guard pattern -- call before track() to exclude transport-level messages"

requirements-completed: [REL-01, REL-02, REL-03, REL-04, REL-05, REL-06]

# Metrics
duration: 4min
completed: 2026-03-20
---

# Phase 9 Plan 1: Protocol Foundation Summary

**ACK tracker with monotonic sequence numbers, exponential backoff retry (1x/2x/4x), deduplicator cache with TTL+size eviction, and msg_ack protocol type**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-20T06:06:12Z
- **Completed:** 2026-03-20T06:10:00Z
- **Tasks:** 2 (both TDD: RED-GREEN)
- **Files modified:** 4

## Accomplishments
- Protocol extended with msg_ack type, CONTROL_TYPES frozen Set, and isControlMessage() helper
- AckTracker: monotonic seq (0,1,2...), sendFn DI, exponential backoff retry, ack/retry/timeout events, getPendingMessages() for reconnect replay
- DeduplicatorCache: TTL-based expiry (1hr default), size-based eviction (1000 default), cleanup()
- 26 new tests (7 protocol + 19 ack-tracker), 307 total suite green with zero regressions

## Task Commits

Each task was committed atomically (TDD: test then feat):

1. **Task 1: Protocol extensions** - `7e90827` (test: RED) + `aea93c1` (feat: GREEN)
2. **Task 2: AckTracker + DeduplicatorCache** - `8f0e956` (test: RED) + `41cecb7` (feat: GREEN)

_TDD tasks have two commits each (test then implementation)_

## Files Created/Modified
- `shared/protocol.js` - Added msg_ack to MessageType, CONTROL_TYPES frozen Set, isControlMessage()
- `shared/ack-tracker.js` - AckTracker (EventEmitter) + DeduplicatorCache classes
- `test/protocol.test.js` - 7 new tests for control message classification (13 total)
- `test/ack-tracker.test.js` - 19 tests for AckTracker and DeduplicatorCache

## Decisions Made
- CONTROL_TYPES excludes file_ack and sync_action_ack (data-layer ACKs need tracking)
- Real timers with short timeoutMs for tests (no mock timers -- deadlock with node:test)
- DI pattern (sendFn, nowFn) matches existing process-supervisor.js convention

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- AckTracker and DeduplicatorCache ready for import by Phase 9 Plan 2 (MessageQueue)
- CONTROL_TYPES and isControlMessage() ready for Phase 11 ACK wiring
- All exports stable: AckTracker, DeduplicatorCache, CONTROL_TYPES, isControlMessage, MessageType.msg_ack

---
*Phase: 09-protocol-foundation*
*Completed: 2026-03-20*
