---
phase: 05-watchdog-hardening
plan: 01
subsystem: watchdog
tags: [escalating-cooldown, self-test, event-emitter, tdd, node-test]

# Dependency graph
requires:
  - phase: 04-watchdog-core
    provides: "ClaudeWatchdog class with crash detection, zombie kill, auto-restart"
provides:
  - "EscalatingCooldown class with ready()/recordAttempt()/reset() API"
  - "ClaudeWatchdog self_test_passed/self_test_failed events"
  - "ClaudeWatchdog cooldown getter for external reset by event consumers"
affects: [05-watchdog-hardening, 06-alerting]

# Tech tracking
tech-stack:
  added: []
  patterns: ["escalating cooldown gating", "self-test event emission after restart"]

key-files:
  created: []
  modified: ["james/watchdog.js", "test/watchdog.test.js"]

key-decisions:
  - "Cooldown NOT reset inside ClaudeWatchdog -- consumer (runner) owns reset policy via self_test_passed event"
  - "Cooldown injected via constructor DI and exposed via getter for external access"
  - "Existing Restart Guard tests updated with always-ready cooldown to isolate restarting-flag behavior from cooldown behavior"

patterns-established:
  - "Escalating cooldown pattern: steps array with clamping, nowFn DI for testability"
  - "Self-test event pattern: separate events (self_test_passed/failed) alongside backward-compat events"

requirements-completed: [WD-04, WD-05]

# Metrics
duration: 3min
completed: 2026-03-12
---

# Phase 5 Plan 01: EscalatingCooldown + ClaudeWatchdog Self-Test Summary

**Escalating cooldown class (5s/15s/30s/60s/5min steps) gating watchdog restarts, plus self-test event emission after post-spawn verification**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-12T06:15:57Z
- **Completed:** 2026-03-12T06:19:20Z
- **Tasks:** 1 (TDD: RED + GREEN)
- **Files modified:** 2

## Accomplishments
- EscalatingCooldown class exported from watchdog.js with ready()/recordAttempt()/reset() API and 5-step escalation
- ClaudeWatchdog poll loop gated by cooldown.ready() -- prevents restart thrashing on persistent failures
- Self-test events (self_test_passed/self_test_failed) emitted after 3s post-spawn verification
- 14 new tests added (7 EscalatingCooldown unit + 2 cooldown integration + 5 self-test), all 89 project tests pass

## Task Commits

Each task was committed atomically:

1. **RED: Failing tests for EscalatingCooldown + self-test** - `2af06f3` (test)
2. **GREEN: Implementation passing all tests** - `bf671ff` (feat)

_TDD plan: RED -> GREEN cycle, no REFACTOR needed (implementation was minimal and clean)_

## Files Created/Modified
- `james/watchdog.js` - Added EscalatingCooldown class, cooldown integration in poll(), self-test events in restart()
- `test/watchdog.test.js` - Added 14 new tests across 3 describe blocks (EscalatingCooldown, Cooldown Integration, Self-Test Events)

## Decisions Made
- Cooldown is NOT reset inside ClaudeWatchdog -- the consumer (watchdog-runner.js in Plan 02) listens for self_test_passed and calls cooldown.reset() externally. This keeps ClaudeWatchdog unaware of the runner's reset policy.
- Cooldown injected via constructor DI and exposed via getter, so the runner can access it for reset.
- Two existing Restart Guard tests were updated to inject an always-ready cooldown (steps: [0]) so they continue to test the restarting-flag behavior in isolation from cooldown behavior.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed existing Restart Guard tests broken by cooldown integration**
- **Found during:** GREEN phase
- **Issue:** Two existing tests ("clears restarting flag after restart completes" and "clears restarting flag after restart fails") expected rapid back-to-back restarts, but the new cooldown gating blocked the second attempt
- **Fix:** Injected always-ready cooldown (steps: [0]) into those two tests to isolate restarting-flag behavior
- **Files modified:** test/watchdog.test.js
- **Verification:** All 89 tests pass
- **Committed in:** bf671ff (GREEN phase commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Necessary fix to maintain backward compatibility of existing tests while adding cooldown gating. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- EscalatingCooldown and self-test events are ready for Plan 02 (runner integration)
- Plan 02 runner will: listen for self_test_passed to reset cooldown, re-establish CommsClient + HeartbeatSender after restart, send email notification to Bono

---
*Phase: 05-watchdog-hardening*
*Completed: 2026-03-12*
