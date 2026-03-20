---
phase: 14-graceful-degradation
plan: 02
subsystem: messaging
tags: [connection-mode, email-fallback, offline-queue, metrics, heartbeat, wiring]

# Dependency graph
requires:
  - phase: 14-01
    provides: ConnectionMode class, sendCritical routing, CRITICAL_TYPES set
  - phase: 13-observability
    provides: collectMetrics DI pattern, /relay/metrics endpoint
  - phase: 11-reliable-delivery
    provides: sendTracked, messageQueue, ackTracker in james/index.js
provides:
  - ConnectionMode wired into James daemon with real-time mode transitions
  - sendCritical replacing sendTracked for exec_result, task_request, recovery
  - connectionMode field in /relay/metrics endpoint and heartbeat payload
  - Email fallback via execFile send pattern with periodic probe
affects: [bono-metrics-consumer, daily-summary, deployment]

# Tech tracking
tech-stack:
  added: []
  patterns: [sendCritical-wiring, email-fallback-via-execFile, connectionMode-in-metrics]

key-files:
  created:
    - test/graceful-degradation.test.js
  modified:
    - james/index.js
    - james/system-metrics.js
    - test/system-metrics.test.js

key-decisions:
  - "sendViaEmail uses execFile with array args following bono/daily-summary.js pattern"
  - "probeEmail checks SEND_EMAIL_PATH file accessibility via fs.access"
  - "connectionMode.startProbe() called on WS open, stopProbe() on shutdown"
  - "exec_result and task_request routed through sendCritical for automatic fallback"

patterns-established:
  - "sendCritical pattern: replace sendTracked for critical message types in daemon wiring"
  - "connectionModeFn DI parameter in collectMetrics for heartbeat extensibility"

requirements-completed: [GD-01, GD-02, GD-03]

# Metrics
duration: 4min
completed: 2026-03-20
---

# Phase 14 Plan 02: ConnectionMode Wiring Summary

**ConnectionMode wired into James daemon with sendCritical routing for exec_result/task_request, email fallback via execFile, and connectionMode in metrics/heartbeat**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-20T09:10:00Z
- **Completed:** 2026-03-20T09:14:00Z
- **Tasks:** 2 (1 auto + 1 checkpoint)
- **Files modified:** 4

## Accomplishments
- ConnectionMode instantiated in james/index.js with full DI (sendTracked, sendViaEmail, probeEmail, messageQueue)
- exec_result and task_request routed through sendCritical for automatic email/offline fallback when WS is down
- connectionMode field visible in /relay/metrics JSON response and heartbeat payload
- CommsClient state events drive mode transitions in real time via connectionMode.onWsStateChange
- Email probe runs on 60s interval, starts on WS open, stops on shutdown
- Mode transitions logged with [CONN-MODE] prefix
- 16 new integration tests + 2 new system-metrics tests (437 total, 0 failures)
- Human verified: metrics endpoint, mode transitions, full test suite green

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire ConnectionMode into james/index.js + extend system-metrics** - `5904441` (feat)
2. **Task 2: Verify graceful degradation wiring** - checkpoint approved, no code changes

**Plan metadata:** [pending final commit]

## Files Created/Modified
- `james/index.js` - ConnectionMode instantiation, sendCritical wiring, email fallback, mode in metrics
- `james/system-metrics.js` - connectionModeFn DI parameter added to collectMetrics
- `test/graceful-degradation.test.js` - 16 integration tests for wiring: email fallback, WAL drain, mode in metrics
- `test/system-metrics.test.js` - 2 new tests for connectionMode in heartbeat payload

## Decisions Made
- sendViaEmail uses execFile with array args (following bono/daily-summary.js pattern) -- fire-and-forget with error logging
- probeEmail checks SEND_EMAIL_PATH file accessibility via fs.access (R_OK) -- returns boolean
- connectionMode.startProbe() called in client.on('open') handler after ackTracker replay
- connectionMode.stopProbe() added to shutdown() function for clean teardown

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. Email fallback requires SEND_EMAIL_PATH env var and Gmail OAuth renewal (known prerequisite from Phase 13 OBS-04).

## Next Phase Readiness
- v2.0 milestone complete -- all 14 phases and 28 plans delivered
- Graceful degradation fully wired: REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE
- Email fallback functional when SEND_EMAIL_PATH is set and Gmail OAuth is renewed
- All 437 tests pass with zero regressions

## Self-Check: PASSED

All files verified present, all commits verified in git log.

---
*Phase: 14-graceful-degradation*
*Completed: 2026-03-20*
