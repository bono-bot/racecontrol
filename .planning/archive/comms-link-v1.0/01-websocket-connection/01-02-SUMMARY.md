---
phase: 01-websocket-connection
plan: 02
subsystem: infra
tags: [websocket, psk-auth, timingSafeEqual, keepalive, echo, integration-tests]

# Dependency graph
requires:
  - phase: 01-websocket-connection/01
    provides: shared/protocol.js (createMessage, parseMessage), shared/state.js (ConnectionStateMachine, State)
provides:
  - WebSocket server with PSK authentication and echo handler (bono/comms-server.js)
  - WebSocket client with state machine integration (james/comms-client.js)
  - Entry points for both sides (bono/index.js, james/index.js)
  - Integration test suite covering auth, connection lifecycle, and echo round-trip
affects: [02-reconnection, 03-heartbeat, 04-watchdog, all-future-phases]

# Tech tracking
tech-stack:
  added: [ws@8.19.0 (server+client), node:crypto (timingSafeEqual), node:http (upgrade handler)]
  patterns: [noServer WebSocket upgrade, PSK in Authorization Bearer header, 25s transport-level ping keepalive, EventEmitter forwarding]

key-files:
  created: [bono/comms-server.js, bono/index.js, james/comms-client.js, james/index.js, test/auth.test.js, test/connection.test.js, test/echo.test.js]
  modified: []

key-decisions:
  - "PSK sent via Authorization: Bearer header (not query params) to avoid server log leaks"
  - "Used noServer mode with manual upgrade handler for PSK validation before WebSocket handshake completes"
  - "timingSafeEqual for PSK comparison with length pre-check to avoid timing attacks"
  - "Client guards against same-state DISCONNECTED transition on close event"
  - "Server terminates clients that miss one ping cycle (25s interval)"
  - "Tasks 1 and 2 consolidated in TDD flow -- tests written first, implementation second"

patterns-established:
  - "WebSocket server: http.createServer + noServer upgrade pattern for auth gating"
  - "PSK auth: Authorization Bearer header, validated with crypto.timingSafeEqual"
  - "Client state machine: CommsClient extends EventEmitter, forwards ConnectionStateMachine events"
  - "Integration tests: ephemeral server on port 0, auto-assign port, cleanup in after() hooks"
  - "Server echo: receives type=echo, responds with type=echo_reply preserving payload"

requirements-completed: [WS-01, WS-03]

# Metrics
duration: 3min
completed: 2026-03-12
---

# Phase 1 Plan 02: WebSocket Server + Client Summary

**PSK-authenticated WebSocket server (Bono) and client (James) with timingSafeEqual auth, 25s ping keepalive, echo handler, and 11 integration tests**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-12T01:27:13Z
- **Completed:** 2026-03-12T01:30:32Z
- **Tasks:** 2 (consolidated into single TDD cycle)
- **Files modified:** 7

## Accomplishments
- Built bono/comms-server.js: createCommsServer with noServer upgrade handler, PSK validation via timingSafeEqual, echo reply, and 25-second ping keepalive with dead client termination
- Built james/comms-client.js: CommsClient class extending EventEmitter with PSK in Authorization Bearer header, ConnectionStateMachine integration, send/receive, and old connection cleanup
- 11 integration tests covering PSK accept/reject/missing, connection lifecycle transitions, state getter, state events, echo round-trip with envelope validation, and disconnected send
- All 27 tests pass (16 unit from Plan 01 + 11 integration from Plan 02)
- End-to-end verified: server starts, client connects, echo round-trip works, invalid PSK rejected with 401

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): Integration tests for auth, connection, echo** - `350b368` (test)
2. **Task 1 (GREEN): Server + client implementation** - `8787732` (feat)

_Note: Task 2 (integration tests) was naturally consolidated into Task 1's TDD RED phase -- tests were written first as part of TDD flow, so no separate Task 2 commit was needed._

## Files Created/Modified
- `bono/comms-server.js` - WebSocket server: createCommsServer with PSK auth, echo, 25s ping keepalive
- `bono/index.js` - Bono entry point: reads COMMS_PSK env var, starts server on COMMS_PORT (default 8765)
- `james/comms-client.js` - WebSocket client: CommsClient with PSK header, state machine, send/receive
- `james/index.js` - James entry point: reads COMMS_PSK and COMMS_URL env vars, connects and logs
- `test/auth.test.js` - 3 tests: valid PSK accepted, invalid PSK rejected, missing PSK rejected
- `test/connection.test.js` - 5 tests: initial state, CONNECTED transition, server-stop DISCONNECTED, state getter, state events
- `test/echo.test.js` - 3 tests: echo reply with payload, envelope validation, send-when-disconnected

## Decisions Made
- PSK sent via Authorization: Bearer header (not query params) per research recommendation -- headers don't leak into server logs
- Used noServer mode with manual upgrade handler so PSK is validated before WebSocket handshake completes
- crypto.timingSafeEqual for PSK comparison with Buffer length pre-check to prevent timing attacks
- Client guards against same-state DISCONNECTED transition on close event (ws emits close after error on 401)
- Tasks 1 and 2 consolidated in TDD flow -- standard TDD practice, not a deviation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added error handlers in auth rejection tests**
- **Found during:** Task 1 GREEN phase
- **Issue:** ws library emits 'error' event on 401 response before 'close' event. Without a handler, Node.js throws unhandled error, crashing the test.
- **Fix:** Added `client.on('error', () => {})` in rejects-invalid-PSK and rejects-missing-PSK tests to swallow expected 401 errors
- **Files modified:** test/auth.test.js
- **Verification:** All 3 auth tests pass
- **Committed in:** 8787732 (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary for test correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 1 complete: working authenticated WebSocket channel between James and Bono
- Ready for Phase 2 (Reconnection & Reliability): auto-reconnect with backoff, offline message queuing
- Blocker for end-to-end VPS deployment: Bono must deploy server endpoint on VPS (CO-02)

---
*Phase: 01-websocket-connection*
*Completed: 2026-03-12*
