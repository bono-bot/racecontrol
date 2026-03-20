---
phase: 12-remote-execution
plan: 03
subsystem: protocol
tags: [exec, websocket, http-relay, approval-flow, child-process]

# Dependency graph
requires:
  - phase: 12-02-exec-handler
    provides: ExecHandler class with 3-tier approval flow
  - phase: 12-01-exec-protocol
    provides: Command registry, buildSafeEnv, validateExecRequest
  - phase: 11-reliable-delivery-wiring
    provides: sendTracked, AckTracker, DeduplicatorCache, wireBono pattern
provides:
  - ExecHandler wired into james/index.js with message routing
  - HTTP relay routes for exec approval (pending, approve, reject, history)
  - sendExecRequest function in bono/index.js for remote command execution
  - Symmetric exec_request rejection on Bono side (deferred feature)
affects: [phase-13-observability, phase-14-graceful-degradation]

# Tech tracking
tech-stack:
  added: []
  patterns: [http-relay-approval, symmetric-exec-rejection, sendTracked-for-exec]

key-files:
  created:
    - test/exec-wiring.test.js
  modified:
    - james/index.js
    - bono/index.js

key-decisions:
  - "sendExecRequest uses ackTracker.track for reliable delivery of exec_request messages"
  - "Bono-side exec_request handling gracefully rejects with 'not implemented' (deferred per research)"
  - "HTTP relay routes follow existing relay pattern: GET pending, POST approve/:id, POST reject/:id, GET history"

patterns-established:
  - "HTTP relay routes for approval flows: GET listing + POST action/:id pattern"
  - "Symmetric message rejection: receive unknown capability, respond with structured rejection"

requirements-completed: [EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07, EXEC-08]

# Metrics
duration: 4min
completed: 2026-03-20
---

# Phase 12 Plan 03: Exec Wiring Summary

**ExecHandler wired into both daemons with HTTP relay approval routes and reliable exec_request delivery via AckTracker**

## Performance

- **Duration:** 4 min (continuation from checkpoint)
- **Started:** 2026-03-20T07:50:00Z
- **Completed:** 2026-03-20T08:01:00Z
- **Tasks:** 3 (2 auto + 1 human-verify checkpoint)
- **Files modified:** 3

## Accomplishments
- ExecHandler instantiated in james/index.js with exec_request/exec_approval message routing
- HTTP relay routes added: GET /relay/exec/pending, POST /relay/exec/approve/:id, POST /relay/exec/reject/:id, GET /relay/exec/history
- sendExecRequest added to bono/index.js using ackTracker for reliable delivery
- Bono gracefully rejects inbound exec_request (symmetric, deferred feature)
- ExecHandler.shutdown() called on daemon exit for clean cleanup
- Human verified: all tests pass, no shell:true anywhere, env sanitized, all 13 commands in registry

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire ExecHandler into james/index.js + HTTP relay routes** - `ccf4fe4` (feat)
2. **Task 2: Add exec_request sending + exec_result handling to bono/index.js** - `b36c322` (feat)
3. **Task 3: Verify remote execution end-to-end** - checkpoint:human-verify (approved, no code changes)

## Files Created/Modified
- `james/index.js` - ExecHandler instantiation, exec_request/exec_approval message handlers, HTTP relay routes, shutdown cleanup
- `bono/index.js` - sendExecRequest function, exec_result handler, symmetric exec_request rejection, COMMAND_REGISTRY export
- `test/exec-wiring.test.js` - Integration tests for message routing and HTTP relay approval endpoints

## Decisions Made
- sendExecRequest uses ackTracker.track for reliable delivery (consistent with sendTracked pattern from Phase 11)
- Bono-side exec_request handling returns structured rejection instead of silently dropping (per research open question #1)
- HTTP relay routes follow existing createServer pattern in james/index.js for consistency

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 12 (Remote Execution) is fully complete: protocol, handler, and wiring all done
- Ready for Phase 13 (Observability): health snapshots, metrics counters, JSON endpoint
- Email fallback E2E validation (OBS-04) will need working Gmail OAuth

## Self-Check: PASSED

- james/index.js: FOUND
- bono/index.js: FOUND
- test/exec-wiring.test.js: FOUND
- Commit ccf4fe4: FOUND
- Commit b36c322: FOUND

---
*Phase: 12-remote-execution*
*Completed: 2026-03-20*
