---
phase: 11-reliable-delivery-wiring
plan: 01
subsystem: messaging
tags: [websocket, ack-tracker, deduplication, message-queue, wal, reliable-delivery]

# Dependency graph
requires:
  - phase: 09-protocol-foundation
    provides: AckTracker, DeduplicatorCache, MessageQueue, protocol.js CONTROL_TYPES
provides:
  - sendRaw() method on CommsClient for pre-serialized message delivery
  - AckTracker + DeduplicatorCache + MessageQueue wired into James daemon
  - INBOX.md demoted to audit-only (appendAuditLog, no programmatic reads)
  - Task request timeout tracking (pendingTasks Map, 5-min default)
  - AckTracker pending replay on reconnect
  - HTTP relay endpoints for queue inspection and task sending
affects: [11-02-PLAN, 12-remote-execution, 13-observability]

# Tech tracking
tech-stack:
  added: []
  patterns: [sendTracked wrapper for AckTracker, appendAuditLog for write-only INBOX, top-level await for ESM startup]

key-files:
  created:
    - test/reliable-delivery.test.js
  modified:
    - james/comms-client.js
    - james/index.js

key-decisions:
  - "INBOX.md is write-only audit log via appendAuditLog -- never read programmatically"
  - "sendTracked() wraps createMessage + AckTracker.track for ergonomic tracked sends"
  - "Task timeout default 5 min (TASK_TIMEOUT_MS env var) matching plan spec"
  - "Top-level await for ESM data dir init + WAL load"

patterns-established:
  - "sendTracked pattern: createMessage -> JSON.parse -> ackTracker.track for all data messages"
  - "appendAuditLog pattern: human-readable INBOX entries, machine store via MessageQueue WAL"
  - "Dedup guard at top of message handler: msg_ack first, then isDuplicate check, then record"

requirements-completed: [TQ-05, BDR-01, BDR-02, BDR-03]

# Metrics
duration: 9min
completed: 2026-03-20
---

# Phase 11 Plan 01: Reliable Delivery Wiring Summary

**sendRaw() on CommsClient + AckTracker/DeduplicatorCache/MessageQueue wired into James daemon with audit-only INBOX and task timeout tracking**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-20T06:32:00Z
- **Completed:** 2026-03-20T06:41:11Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- CommsClient.sendRaw() for pre-serialized message delivery (AckTracker retries)
- James daemon wired with AckTracker (ACK + retry), DeduplicatorCache (dedup incoming), MessageQueue (WAL persistence)
- INBOX.md demoted from programmatic read target to write-only audit log
- Task request timeout tracking with pendingTasks Map (5-min default, configurable)
- AckTracker pending messages replayed on reconnect
- HTTP relay endpoints: /relay/queue/peek, /relay/queue/ack, /relay/task
- 324 tests pass (17 new + 307 existing), 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add sendRaw() to CommsClient + test scaffold** - `16e4ee5` (feat, TDD)
2. **Task 2: Wire reliable delivery into james/index.js** - `b290559` (feat)

## Files Created/Modified
- `james/comms-client.js` - Added sendRaw() method for pre-serialized message delivery
- `james/index.js` - AckTracker, DeduplicatorCache, MessageQueue wiring + audit log + task timeout + queue HTTP endpoints
- `test/reliable-delivery.test.js` - 17 tests covering sendRaw, AckTracker, DeduplicatorCache, MessageQueue

## Decisions Made
- INBOX.md is write-only audit log via appendAuditLog -- never read programmatically (MessageQueue WAL is the machine-readable store)
- sendTracked() wraps createMessage + AckTracker.track for ergonomic tracked sends
- Task timeout default 5 min (TASK_TIMEOUT_MS env var) matching plan spec
- Top-level await for ESM data dir init + WAL load (project uses "type": "module")
- AckTracker retry timer cleanup (reset()) in tests to prevent process hang

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- AckTracker's internal retry timers kept the test process alive after tests completed. Fixed by calling tracker.reset() in tests that create long-lived trackers. Not a production issue -- only affects test cleanup.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- James side fully wired for reliable delivery
- Ready for Phase 11-02 (Bono side wiring)
- sendRaw() + AckTracker + DeduplicatorCache + MessageQueue all verified with 324 passing tests

---
*Phase: 11-reliable-delivery-wiring*
*Completed: 2026-03-20*
