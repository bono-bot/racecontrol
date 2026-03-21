---
phase: 06-alerting
plan: 02
subsystem: alerting
tags: [recovery-signal, email-fallback, websocket, watchdog, evolution-api, whatsapp]

# Dependency graph
requires:
  - phase: 05-watchdog-hardening
    provides: wireRunner, EscalatingCooldown, self_test_passed event
  - phase: 06-alerting plan 01
    provides: AlertManager, AlertCooldown, sendEvolutionText, recovery message type
provides:
  - James-side recovery signal via WebSocket (client.send('recovery', ...))
  - James-side email fallback to Uday + Bono on crash escalation at cooldown cap
  - James-side recovery email on prior alert email
  - Bono-side AlertManager wired to HeartbeatMonitor and incoming recovery messages
  - wireBono() testable wiring function for bono/index.js
affects: [08-coordination]

# Tech tracking
tech-stack:
  added: []
  patterns: [closure-state-tracking, one-email-per-cycle-suppression, isMainModule-guard]

key-files:
  created:
    - test/alerting-integration.test.js
  modified:
    - james/watchdog-runner.js
    - bono/index.js
    - test/watchdog-runner.test.js

key-decisions:
  - "wireBono() extracted as testable wiring function -- follows wireRunner() pattern from james side"
  - "isMainModule guard added to bono/index.js for safe test imports -- production entry point only runs as main module"
  - "Email fallback sends to BOTH usingh@racingpoint.in and bono@racingpoint.in per CONTEXT.md locked decision"
  - "Recovery signal uses optional chaining (client?.send) for graceful degradation when client is null"
  - "One email per escalation cycle via alertEmailSent closure flag -- prevents email flooding during crash loop"

patterns-established:
  - "Closure state in wireRunner: lastCrashTimestamp and alertEmailSent managed via closure variables for cross-event tracking"
  - "isMainModule guard pattern in bono/index.js: production code only runs when file is main module, enabling safe test imports"
  - "Dual-recipient email pattern: loop over recipients array for consistent multi-recipient fire-and-forget emails"

requirements-completed: [AL-02, AL-03, AL-04]

# Metrics
duration: 4min
completed: 2026-03-12
---

# Phase 6 Plan 2: Recovery Signal and Email Fallback Summary

**James-side recovery signal via WebSocket + email fallback to Uday on crash escalation + Bono-side AlertManager wiring to HeartbeatMonitor and recovery messages**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-12T12:49:56Z
- **Completed:** 2026-03-12T12:54:00Z
- **Tasks:** 2 (1 TDD + 1 auto)
- **Files modified:** 4

## Accomplishments
- James sends recovery message to Bono via WebSocket after every successful restart with crashCount, downtimeMs, restartCount, pid, exePath
- James sends alert email to Uday + Bono ONLY when WS disconnected AND cooldown at 5-minute cap, with one-email-per-cycle suppression
- James sends recovery email to both recipients when prior alert email was sent, then resets suppression flag
- Bono's index.js wires AlertManager to HeartbeatMonitor james_down events and incoming recovery WebSocket messages
- wireBono() extracted as testable wiring function following wireRunner() DI pattern
- 134 tests pass (116 existing + 11 new watchdog-runner + 7 integration), zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Failing tests for recovery signal and email fallback** - `96f52e5` (test)
2. **Task 1 GREEN: Recovery signal, email fallback, recovery email in watchdog-runner** - `ea47bf0` (feat)
3. **Task 2: Wire AlertManager into bono/index.js with integration tests** - `60b074c` (feat)

## Files Created/Modified
- `james/watchdog-runner.js` - Recovery signal, email fallback to Uday+Bono, recovery email, closure state tracking (263 lines)
- `bono/index.js` - wireBono() extracted, AlertManager wired to HeartbeatMonitor and recovery messages, isMainModule guard (101 lines)
- `test/watchdog-runner.test.js` - 11 new tests: recovery signal, email fallback, suppression, recovery email (449 lines)
- `test/alerting-integration.test.js` - 7 integration tests for wireBono wiring correctness (119 lines)

## Decisions Made
- **wireBono() extraction:** bono/index.js was 43 lines but needed testability for AlertManager wiring. Extracted wireBono() following the wireRunner() DI pattern from james side. Plan left this to judgment -- extraction was the right call for 7 integration tests.
- **isMainModule guard:** Added to bono/index.js so test imports don't trigger process.exit(1) from missing COMMS_PSK. Same pattern as watchdog-runner.js.
- **Optional chaining for recovery signal:** Used `client?.send('recovery', ...)` instead of explicit null check -- cleaner when client can be null (no COMMS_PSK).
- **Dual-recipient loop:** Email fallback and recovery email both iterate over a recipients array rather than duplicating execFileFn calls.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added isMainModule guard to bono/index.js**
- **Found during:** Task 2
- **Issue:** Importing bono/index.js in tests triggered the production entry point which calls process.exit(1) when COMMS_PSK is not set
- **Fix:** Wrapped production code in isMainModule guard (same pattern as james/watchdog-runner.js)
- **Files modified:** bono/index.js
- **Verification:** `node -e "import('./bono/index.js')"` succeeds without COMMS_PSK, tests pass
- **Committed in:** 60b074c (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for testability. No scope creep -- the isMainModule guard pattern already existed in the codebase.

## Issues Encountered
None

## User Setup Required
None - Evolution API credentials (EVOLUTION_URL, EVOLUTION_INSTANCE, EVOLUTION_API_KEY, UDAY_WHATSAPP) remain as env var placeholders. Coordinate with Bono before production use.

## Next Phase Readiness
- Phase 6 (Alerting) is now complete. Both plans done.
- All alerting infrastructure is in place: AlertManager, email fallback, recovery signals, WhatsApp wiring
- Production activation requires Evolution API credentials from Bono
- Ready for Phase 7 (if it exists) or Phase 8 (Coordination)

## Self-Check: PASSED

All 5 files verified present. All 3 commits verified in git log.

---
*Phase: 06-alerting*
*Completed: 2026-03-12*
