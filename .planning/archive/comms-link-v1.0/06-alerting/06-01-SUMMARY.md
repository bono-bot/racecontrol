---
phase: 06-alerting
plan: 01
subsystem: alerting
tags: [whatsapp, evolution-api, cooldown, websocket, events]

# Dependency graph
requires:
  - phase: 03-heartbeat
    provides: HeartbeatMonitor james_down/james_up events
  - phase: 05-watchdog-hardening
    provides: EscalatingCooldown pattern reference, self_test_passed event
provides:
  - AlertManager class for WhatsApp alerting via Evolution API
  - AlertCooldown fixed-window suppression for alert deduplication
  - sendEvolutionText HTTP helper with DI transport injection
  - recovery message type in shared protocol
affects: [06-02, 08-coordination]

# Tech tracking
tech-stack:
  added: []
  patterns: [fixed-window-cooldown, injectable-transport, null-sentinel-reset]

key-files:
  created:
    - bono/alert-manager.js
    - test/alerting.test.js
  modified:
    - shared/protocol.js

key-decisions:
  - "Fixed-window cooldown (not escalating) for alert suppression -- simpler semantics for notifications vs restart gating"
  - "Null sentinel for AlertCooldown reset instead of 0 -- avoids false suppression when clock values are small"
  - "Down message shows 'last seen Xs ago' instead of crash attempt count -- Bono doesn't have crash context at james_down time"
  - "sendEvolutionText uses injectable transportFn (not global mock) for zero-dependency testing"

patterns-established:
  - "AlertCooldown null-sentinel pattern: #lastAlertTime = null means 'never sent', canSend() returns true"
  - "Fire-and-forget sendFn with .catch() for non-blocking alert delivery"
  - "Graceful degradation: AlertManager disables itself when udayNumber missing, no crash"

requirements-completed: [AL-01, AL-02, AL-04]

# Metrics
duration: 3min
completed: 2026-03-12
---

# Phase 6 Plan 1: AlertManager Summary

**AlertManager with WhatsApp down/recovery alerts via Evolution API, AlertCooldown fixed-window suppression, and protocol recovery message type**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-12T12:44:00Z
- **Completed:** 2026-03-12T12:46:54Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 3

## Accomplishments
- AlertCooldown with fixed 5-minute window suppression, null-sentinel reset for immediate re-alerting after recovery
- AlertManager sends WhatsApp down-alerts (with last-seen context) and recovery alerts (with downtime/restart count) via DI-injected sendFn
- sendEvolutionText HTTP POST helper with correct Evolution API endpoint, headers, body format, and 10s timeout
- Protocol extended with `recovery` message type for James-to-Bono back-online signals
- 19 new tests covering all behaviors, 116 total tests passing (zero regressions)

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Failing tests + protocol extension** - `f513d6b` (test)
2. **Task 1 GREEN: AlertManager, AlertCooldown, sendEvolutionText implementation** - `ebd336e` (feat)

## Files Created/Modified
- `bono/alert-manager.js` - AlertCooldown, AlertManager, sendEvolutionText (211 lines)
- `test/alerting.test.js` - 19 tests across 4 describe blocks (376 lines)
- `shared/protocol.js` - Added `recovery: 'recovery'` to MessageType enum

## Decisions Made
- **Fixed-window vs escalating cooldown:** Used fixed 5-minute window for alert suppression. Alert flooding has different semantics from restart gating -- you want "one alert per window" not "escalating delays between alerts."
- **Null sentinel for reset:** AlertCooldown uses `null` (not `0`) for `#lastAlertTime` to distinguish "never sent" from "sent at epoch 0". This avoids false suppression when injected `nowFn` returns small test values.
- **Down message format:** `James DOWN HH:MM (last seen Xs ago)` -- Bono doesn't have crash attempt count at heartbeat-timeout time. The crash context (count, restarts) arrives with the recovery message. This is honest about what data is available.
- **sendEvolutionText injectable transport:** Tests inject a mock transport object matching the `http.request()` signature. Production auto-selects `http` or `https` based on URL protocol.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] AlertCooldown reset() returned false with small clock values**
- **Found during:** Task 1 GREEN phase
- **Issue:** `reset()` set `#lastAlertTime = 0`, but `canSend()` computed `(nowFn() - 0) >= windowMs`. With test clock at 2000ms and window at 5000ms, `2000 >= 5000` was false -- reset didn't actually make canSend() return true.
- **Fix:** Changed `#lastAlertTime` from `0` to `null` sentinel. `canSend()` returns `true` immediately when `#lastAlertTime === null`. `reset()` sets it to `null`.
- **Files modified:** `bono/alert-manager.js`
- **Verification:** Test "reset() makes canSend() return true immediately" passes
- **Committed in:** ebd336e (GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential correctness fix. Null sentinel is cleaner than the 0-based approach from research.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required. Evolution API credentials will be configured via environment variables in Plan 06-02 wiring.

## Next Phase Readiness
- AlertManager, AlertCooldown, sendEvolutionText ready for wiring in Plan 06-02
- Plan 06-02 will: wire AlertManager to HeartbeatMonitor events in bono/index.js, add James recovery signal sending, add email fallback in watchdog-runner.js
- Evolution API credentials (EVOLUTION_URL, EVOLUTION_INSTANCE, EVOLUTION_API_KEY, UDAY_WHATSAPP) needed from Bono before production use

---
*Phase: 06-alerting*
*Completed: 2026-03-12*
