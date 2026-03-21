---
phase: 03-heartbeat
plan: 01
subsystem: monitoring
tags: [heartbeat, system-metrics, cpu, memory, process-detection, eventemitter]

# Dependency graph
requires:
  - phase: 02-reconnection
    provides: "Reliable WebSocket connection with auto-reconnect and message queuing"
provides:
  - "HeartbeatSender: 15s periodic heartbeat with system metrics over WebSocket"
  - "SystemMetrics: CPU, memory, uptime, Claude Code process detection"
  - "HeartbeatMonitor: 45s timeout tracker with james_down/james_up events"
affects: [alerting, watchdog, coordination]

# Tech tracking
tech-stack:
  added: []
  patterns: [EventEmitter for state change events, dependency injection for testability, CPU delta sampling]

key-files:
  created:
    - james/system-metrics.js
    - james/heartbeat-sender.js
    - bono/heartbeat-monitor.js
    - test/system-metrics.test.js
    - test/heartbeat.test.js
  modified:
    - james/index.js
    - bono/index.js

key-decisions:
  - "HeartbeatSender accepts optional collectFn for testability -- avoids execFile deadlock under mock timers"
  - "CPU delta sampling with module-level state -- returns 0 on first call (no baseline), accurate on subsequent"
  - "Claude detection via tasklist with 5s timeout -- returns null on error (graceful degradation, not crash)"

patterns-established:
  - "DI for async I/O in timer-tested classes: pass collectFn option to avoid mock timer deadlocks"
  - "EventEmitter for monitoring state: james_up/james_down events with payload + timestamp"
  - "Private class fields (#interval, #timeout) for encapsulation in all new modules"

requirements-completed: [HB-01, HB-02, HB-03, HB-04]

# Metrics
duration: 4min
completed: 2026-03-12
---

# Phase 3 Plan 1: Heartbeat Summary

**15s heartbeat with CPU/memory/uptime/Claude-process metrics from James, and 45s DOWN detection with james_down/james_up events on Bono**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T03:33:06Z
- **Completed:** 2026-03-12T03:37:38Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- System metrics collector (CPU delta sampling, memory percentage, OS uptime, Claude Code process detection via tasklist)
- HeartbeatSender sends immediately on connect then every 15s, stops on disconnect (no stale heartbeat queuing)
- HeartbeatMonitor emits james_down after 45s silence and james_up on resume, with isUp/lastPayload getters
- 19 new tests (5 sender + 8 monitor + 6 metrics), 57 total, zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: HeartbeatSender, SystemMetrics, HeartbeatMonitor (TDD RED)** - `5e57675` (test)
2. **Task 1: HeartbeatSender, SystemMetrics, HeartbeatMonitor (TDD GREEN)** - `d77c4d6` (feat)
3. **Task 2: Wire heartbeat into entry points** - `dcfca7d` (feat)

_TDD task had separate RED and GREEN commits._

## Files Created/Modified
- `james/system-metrics.js` - CPU delta sampling, memory, uptime, Claude Code process detection via tasklist
- `james/heartbeat-sender.js` - 15s interval heartbeat sender with start/stop lifecycle and DI collectFn
- `bono/heartbeat-monitor.js` - 45s timeout tracker emitting james_down/james_up EventEmitter events
- `test/system-metrics.test.js` - 6 tests: output shape, field types (cpu, memory, uptime, claudeRunning)
- `test/heartbeat.test.js` - 13 tests: sender interval/lifecycle, monitor timeout/events/lastPayload
- `james/index.js` - Added HeartbeatSender import, start on open, stop on close, cleanup on shutdown
- `bono/index.js` - Added HeartbeatMonitor import, route heartbeat messages, log state changes, cleanup on shutdown

## Decisions Made
- HeartbeatSender accepts optional `collectFn` for dependency injection in tests -- avoids `execFile` deadlocking under node:test mock timers
- CPU uses delta sampling (module-level previous times) -- returns 0 on first call since no baseline exists
- Claude detection uses `tasklist /NH /FI` with 5s timeout -- returns `null` on error/timeout (graceful degradation)
- HeartbeatMonitor extends EventEmitter directly (not wrapping) -- consistent with existing codebase pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added dependency injection to HeartbeatSender for testability**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** `collectMetrics()` calls `execFile('tasklist')` which creates real I/O that deadlocks under `t.mock.timers.enable()`. Mock timers intercept the setTimeout used by execFile, preventing async resolution.
- **Fix:** Added optional `collectFn` parameter to HeartbeatSender constructor (defaults to real `collectMetrics`). Tests pass a synchronous mock.
- **Files modified:** james/heartbeat-sender.js, test/heartbeat.test.js
- **Verification:** All 19 new tests pass, all 38 existing tests pass
- **Committed in:** d77c4d6 (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- added one optional constructor parameter for testability. Production code path unchanged (uses real collectMetrics by default).

## Issues Encountered
None beyond the mock timer deadlock addressed above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Heartbeat infrastructure complete -- Bono can detect James DOWN within 45s
- Foundation ready for Phase 6 (Alerting) to wire WhatsApp/email notifications to james_down/james_up events
- Phase 4 (Watchdog) can proceed independently -- depends only on Phase 1

---
*Phase: 03-heartbeat*
*Completed: 2026-03-12*
