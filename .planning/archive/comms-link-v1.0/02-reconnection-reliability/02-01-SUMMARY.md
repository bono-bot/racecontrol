---
phase: 02-reconnection-reliability
plan: 01
subsystem: websocket
tags: [reconnect, backoff, exponential-backoff, message-queue, offline-queue, ws]

# Dependency graph
requires:
  - phase: 01-websocket-connection
    provides: CommsClient with connect/disconnect/send and ConnectionStateMachine with RECONNECTING state
provides:
  - Auto-reconnect with exponential backoff (1s-30s cap) and jitter
  - Offline message queue with bounded size and in-order replay on reconnect
  - queueSize getter and maxQueueSize constructor option on CommsClient
affects: [03-heartbeat-presence, 04-watchdog-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns: [exponential-backoff-with-jitter, bounded-queue-with-oldest-drop, intentional-close-flag]

key-files:
  created:
    - test/reconnect.test.js
    - test/queue.test.js
  modified:
    - james/comms-client.js
    - test/connection.test.js

key-decisions:
  - "Only auto-reconnect after established connection dropped, not on failed initial connect (prevents auth rejection retry loops)"
  - "Queue flushes before emitting 'open' to prevent message interleaving"
  - "Intentional close flag (not close code parsing) to distinguish user disconnect from network drops"

patterns-established:
  - "Exponential backoff: delay = min(1000 * 2^attempt, 30000) + random(0-500ms)"
  - "Bounded queue: drop oldest on overflow, shift+send on flush"
  - "Intentional close flag: set in disconnect(), cleared in open handler"

requirements-completed: [WS-02, WS-05]

# Metrics
duration: 6min
completed: 2026-03-12
---

# Phase 2 Plan 1: Reconnect + Queue Summary

**Auto-reconnect with exponential backoff (1s-30s cap, jitter) and bounded offline message queue (100 msgs, oldest-drop, in-order replay)**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-12T02:28:18Z
- **Completed:** 2026-03-12T02:34:34Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- CommsClient auto-reconnects with exponential backoff (1s, 2s, 4s, 8s, 16s, 30s cap) + random jitter on unintentional connection loss
- Messages sent while disconnected are queued (bounded at 100, oldest dropped on overflow) and replayed in order on reconnect
- No reconnect after intentional disconnect() or failed initial connection (auth rejection stays DISCONNECTED)
- 11 new TDD tests covering reconnect states, backoff timing, queue behavior, replay order, and no-duplicate guarantees

## Task Commits

Each task was committed atomically:

1. **Task 1: RED -- Write failing tests for reconnect and message queue** - `1c8a514` (test)
2. **Task 2: GREEN -- Implement reconnect and message queue in CommsClient** - `e619085` (feat)

## Files Created/Modified
- `test/reconnect.test.js` - 5 tests for WS-02: RECONNECTING state, reconnect after restart, exponential backoff, backoff reset, no reconnect after disconnect()
- `test/queue.test.js` - 6 tests for WS-05: queue on send, queueSize getter, in-order replay, bounded overflow, empty after flush, no duplicates
- `james/comms-client.js` - Added #scheduleReconnect, #flushQueue, #queue, #intentionalClose, queueSize getter, maxQueueSize option
- `test/connection.test.js` - Updated "transitions to DISCONNECTED when server stops" to expect RECONNECTING (correct behavior with auto-reconnect)

## Decisions Made
- Only auto-reconnect if previously CONNECTED or already RECONNECTING -- failed initial connections (auth rejection, server unreachable before first open) stay DISCONNECTED to avoid pointless retry loops with bad credentials
- Queue flushed before emitting 'open' event to prevent user code from interleaving new sends with replayed messages
- Used intentional close flag (#intentionalClose) rather than close code parsing to distinguish user-initiated disconnect from network drops -- more reliable and simpler

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated connection lifecycle test for new RECONNECTING behavior**
- **Found during:** Task 2 (GREEN implementation)
- **Issue:** Existing test "transitions to DISCONNECTED when server stops" expected DISCONNECTED, but with auto-reconnect the correct behavior is RECONNECTING. Test also had unhandled error from reconnect attempts.
- **Fix:** Changed test to expect RECONNECTING state with correct previous=CONNECTED, added error handler for reconnect attempts
- **Files modified:** test/connection.test.js
- **Verification:** All 38 tests pass
- **Committed in:** e619085 (Task 2 commit)

**2. [Rule 1 - Bug] Guard against reconnect on failed initial connection**
- **Found during:** Task 2 (GREEN implementation)
- **Issue:** Auth rejection (invalid PSK) caused client to enter RECONNECTING instead of staying DISCONNECTED, because the close handler treated all non-intentional closes as reconnectable. Retrying with the same bad PSK is pointless.
- **Fix:** Added guard in close handler: if state is still DISCONNECTED (never reached CONNECTED), stay DISCONNECTED and don't schedule reconnect
- **Files modified:** james/comms-client.js
- **Verification:** Auth tests pass (invalid/missing PSK stays DISCONNECTED), reconnect tests pass (established-then-dropped enters RECONNECTING)
- **Committed in:** e619085 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes essential for correctness. No scope creep. Auth rejection behavior preserved while adding reconnect.

## Issues Encountered
None -- TDD flow worked cleanly. RED phase confirmed all 11 tests fail, GREEN phase made all 38 pass.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Reconnect + queue foundation is complete for heartbeat/presence (Phase 3) and watchdog recovery (Phase 4)
- CommsClient API extended with queueSize getter and maxQueueSize constructor option
- No new dependencies introduced

## Self-Check: PASSED

- All 5 files exist (2 created, 2 modified, 1 summary)
- Both commits found (1c8a514, e619085)
- 38 tests pass (27 existing + 11 new)

---
*Phase: 02-reconnection-reliability*
*Completed: 2026-03-12*
