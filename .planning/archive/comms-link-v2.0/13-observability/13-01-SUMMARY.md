---
phase: 13-observability
plan: 01
subsystem: observability
tags: [metrics, heartbeat, tdd, dependency-injection]

requires:
  - phase: 03-heartbeat
    provides: collectMetrics function and HeartbeatSender
provides:
  - MetricsCollector class with uptime, reconnect, ACK latency tracking
  - Extended collectMetrics with queueDepth, ackPending, podStatus, deployState
affects: [13-02, 14-graceful-degradation]

tech-stack:
  added: []
  patterns: [dependency-injection for metrics providers, rolling-window statistics, module-level singleton constants]

key-files:
  created: [james/metrics-collector.js, test/metrics-collector.test.js]
  modified: [james/system-metrics.js, test/system-metrics.test.js]

key-decisions:
  - "Hardcoded version '2.0.0' in deployState rather than reading package.json -- simpler, matches milestone"
  - "MODULE_STARTED_AT set once at import time for consistent startedAt across calls"
  - "DI params use optional chaining with nullish coalescing for clean defaults"

patterns-established:
  - "Metrics DI pattern: collectMetrics accepts injectable provider functions for extensibility"
  - "Rolling window: fixed-size array with shift() for bounded memory latency tracking"

requirements-completed: [OBS-01, OBS-02]

duration: 2min
completed: 2026-03-20
---

# Phase 13 Plan 01: Metrics Collector & Extended Heartbeat Summary

**MetricsCollector class with rolling-window ACK latency stats and collectMetrics extended with queue depth, pod status, and deploy state via DI**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-20T08:34:11Z
- **Completed:** 2026-03-20T08:35:50Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- MetricsCollector class tracks uptime, reconnect count, and ACK latency (avg + p99) with 100-entry rolling window
- collectMetrics extended with queueDepth, ackPending, podStatus, deployState, and metricsSnapshot via dependency injection
- 20 tests total (7 MetricsCollector + 13 system-metrics), all passing
- Fully backward compatible -- existing callers with no args get original 5 fields

## Task Commits

Each task was committed atomically:

1. **Task 1: MetricsCollector class with TDD** - `58935a9` (feat)
2. **Task 2: Extend collectMetrics with queue depth, pod status, deploy state** - `58799f2` (feat)

_Both tasks followed TDD: RED (failing tests) then GREEN (implementation)._

## Files Created/Modified
- `james/metrics-collector.js` - MetricsCollector class with recordReconnect, recordAckLatency, snapshot
- `test/metrics-collector.test.js` - 7 tests for MetricsCollector
- `james/system-metrics.js` - collectMetrics extended with DI params for enriched heartbeat
- `test/system-metrics.test.js` - 7 new tests added (13 total) for extended fields

## Decisions Made
- Hardcoded version '2.0.0' in deployState rather than reading package.json -- simpler, matches milestone
- MODULE_STARTED_AT set once at import time so startedAt is consistent across calls
- DI params use optional chaining with nullish coalescing for clean zero-value defaults

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- MetricsCollector and extended collectMetrics ready for wiring in 13-02
- 13-02 will wire MetricsCollector into HeartbeatSender, add GET /relay/metrics endpoint, validate email fallback

---
*Phase: 13-observability*
*Completed: 2026-03-20*
