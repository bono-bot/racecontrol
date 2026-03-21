---
phase: 05-watchdog-hardening
plan: 02
subsystem: watchdog
tags: [comms-client, heartbeat, email-notification, fire-and-forget, di-wiring, node-test]

# Dependency graph
requires:
  - phase: 05-watchdog-hardening
    provides: "EscalatingCooldown + self_test_passed/self_test_failed events from Plan 01"
  - phase: 01-websocket-connection
    provides: "CommsClient with connect()/disconnect()/state API"
  - phase: 03-heartbeat
    provides: "HeartbeatSender with start()/stop() API"
provides:
  - "watchdog-runner.js as full integration hub (ClaudeWatchdog + CommsClient + HeartbeatSender + email)"
  - "wireRunner() exported function for testable event wiring via DI"
  - "Automatic WebSocket re-establishment after successful restart"
  - "Fire-and-forget email notification to bono@racingpoint.in on restart"
affects: [06-alerting, 08-coordination]

# Tech tracking
tech-stack:
  added: []
  patterns: ["wireRunner DI pattern: testable integration wiring exported alongside production entry point", "fire-and-forget execFile with error-only callback logging"]

key-files:
  created: ["test/watchdog-runner.test.js"]
  modified: ["james/watchdog-runner.js"]

key-decisions:
  - "wireRunner() exported for DI-based testing -- production entry point calls it with real instances, tests call with mocks"
  - "Cooldown attemptCount and delay read BEFORE reset() so email body reflects restart-time state"
  - "isMainModule detection via process.argv[1] path check (ESM has no require.main)"

patterns-established:
  - "Integration hub pattern: wireRunner({ watchdog, client, heartbeat, execFileFn, sendEmailPath }) separates wiring from instantiation"
  - "Fire-and-forget pattern: execFile with error-only callback logging, never blocking the event loop"

requirements-completed: [WD-06, WD-07]

# Metrics
duration: 2min
completed: 2026-03-12
---

# Phase 5 Plan 02: Watchdog Runner Integration Summary

**watchdog-runner.js as integration hub wiring ClaudeWatchdog + CommsClient + HeartbeatSender + email notification via DI-testable wireRunner() function**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-12T06:23:09Z
- **Completed:** 2026-03-12T06:25:11Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments
- Rewrote watchdog-runner.js as a full integration hub (176 lines) with exported wireRunner() for testable wiring
- On self_test_passed: resets escalating cooldown, reconnects WebSocket (if disconnected), sends fire-and-forget email to bono@racingpoint.in
- On self_test_failed: logs warning, does NOT reset cooldown (escalation continues)
- Graceful degradation when COMMS_PSK not set (watchdog monitors without WebSocket/email)
- 8 new tests, all 97 project tests pass (zero regressions)

## Task Commits

Each task was committed atomically:

1. **RED: Failing tests for runner integration** - `fe550b6` (test)
2. **GREEN: Implementation passing all tests** - `632f918` (feat)

_TDD plan: RED -> GREEN cycle, no REFACTOR needed (implementation was clean)_

## Files Created/Modified
- `james/watchdog-runner.js` - Rewrote as integration hub: wireRunner() + production entry point with CommsClient, HeartbeatSender, email, graceful shutdown
- `test/watchdog-runner.test.js` - 8 unit tests covering self_test_passed/failed wiring, cooldown reset, WebSocket reconnect, email args, graceful degradation, fire-and-forget error handling

## Decisions Made
- wireRunner() exported for DI-based testing -- production code calls with real instances, tests call with mocks (avoids module import mocking)
- Cooldown attemptCount and delay read BEFORE reset() call so email body reflects the state at restart time, not post-reset zeros
- isMainModule detection uses process.argv[1] path suffix check since ESM has no require.main equivalent

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 5 watchdog hardening is complete (both plans done)
- Old PowerShell watchdog can be retired (Phase 4 decision confirmed)
- Phase 6 (Alerting) can proceed -- watchdog + comms + heartbeat + email all integrated
- Blocker for Phase 6: Evolution API instance name and API key needed from Bono

## Self-Check: PASSED

- [x] james/watchdog-runner.js exists (176 lines, >= 80 minimum)
- [x] test/watchdog-runner.test.js exists (204 lines)
- [x] Commit fe550b6 (RED) verified
- [x] Commit 632f918 (GREEN) verified
- [x] All 97 tests pass (8 new + 89 existing)

---
*Phase: 05-watchdog-hardening*
*Completed: 2026-03-12*
