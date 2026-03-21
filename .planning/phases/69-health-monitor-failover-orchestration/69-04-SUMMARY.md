---
phase: 69-health-monitor-failover-orchestration
plan: 04
subsystem: infra
tags: [failover, whatsapp, evolution-api, email, comms-link, notification]

# Dependency graph
requires:
  - phase: 69-health-monitor-failover-orchestration
    provides: FailoverOrchestrator + HealthMonitor + secondary watchdog (plans 69-01 through 69-03)
provides:
  - notify_failover command in COMMAND_REGISTRY (tier AUTO, inline Evolution API call)
  - EXEC_REASON env var injection in ExecHandler for caller-specified notification text
  - buildSafeEnv() includes Evolution API vars (EVOLUTION_URL/INSTANCE/API_KEY/UDAY_WHATSAPP)
  - Bono watchdog WhatsApp via sendEvolutionText directly (broken alertManager call replaced)
  - Email notification on both failover paths (James orchestrator + Bono watchdog) via send-email.js
  - shared/send-email.js: stdlib-only email sender (sendmail primary, raw SMTP fallback)
affects: [comms-link deployment, Bono VPS restart, ORCH-04 verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "notify_failover: inline node -e script in COMMAND_REGISTRY args reads env vars for Evolution API"
    - "EXEC_REASON passed as env var from ExecHandler#execute to child process for dynamic notification text"
    - "Fire-and-forget email via execFile shell-out to shared/send-email.js (email failure never blocks failover)"
    - "sendmail primary + raw SMTP fallback in send-email.js (stdlib-only, no npm deps)"

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/shared/send-email.js
  modified:
    - C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js
    - C:/Users/bono/racingpoint/comms-link/bono/index.js
    - C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js
    - C:/Users/bono/racingpoint/comms-link/james/exec-handler.js

key-decisions:
  - "notify_failover tier AUTO (not NOTIFY) -- the command itself delivers the notification; NOTIFY tier would double-notify"
  - "EXEC_REASON injected as env var in ExecHandler#execute so notify_failover inline script gets the failover reason string"
  - "buildSafeEnv() extended with Evolution API vars conditionally (undefined on James's Windows machine, only set on Bono's VPS)"
  - "notifyFn in bonoExecHandler replaced with direct sendEvolutionText call -- fixes NOTIFY-tier WhatsApp for all commands"
  - "send-email.js: sendmail primary (available on Hostinger VPS), raw SMTP to localhost:25 fallback"
  - "Email on both paths (James orchestrator + Bono watchdog) -- ORCH-04 criterion requires email AND WhatsApp"

patterns-established:
  - "EXEC_REASON pattern: ExecHandler merges reason from exec_request into safeEnv before executing command"

requirements-completed: [ORCH-04]

# Metrics
duration: 18min
completed: 2026-03-21
---

# Phase 69 Plan 04: Failover Notification Gap Closure Summary

**Three ORCH-04 notification gaps closed: notify_failover registered in COMMAND_REGISTRY, Bono watchdog fixed to call sendEvolutionText directly, email added to both failover paths via stdlib-only send-email.js**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-21T01:20:00Z (approx)
- **Completed:** 2026-03-21T01:38:00Z (approx)
- **Tasks:** 1
- **Files modified:** 5 (4 modified + 1 created)

## Accomplishments

- `notify_failover` command added to COMMAND_REGISTRY with tier AUTO -- ExecHandler on Bono VPS can now execute it; inline node script calls Evolution API using env vars
- `EXEC_REASON` env var injected into child process by ExecHandler so the `reason` field from James's exec_request becomes the WhatsApp notification text
- `buildSafeEnv()` extended to pass EVOLUTION_URL/INSTANCE/API_KEY/UDAY_WHATSAPP to child processes (conditionally -- only adds vars that are set, so no-op on James's Windows machine)
- Bono watchdog `alertManager.handleNotification` replaced with direct `sendEvolutionText` call -- eliminates TypeError that prevented all watchdog notifications
- `bonoExecHandler` notifyFn fixed to call `sendEvolutionText` directly -- fixes NOTIFY-tier WhatsApp for all commands, not just failover
- `shared/send-email.js` created: stdlib-only Node.js email sender (sendmail primary, raw SMTP to localhost:25 fallback), accepts `recipient subject body` CLI args
- Email notification added to both failover paths: James's `failover-orchestrator.js` (after notify_failover exec_request) and Bono's watchdog (after sendEvolutionText)

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix all three notification gaps -- COMMAND_REGISTRY + Bono watchdog + email** - `7ce7abf` (feat)

## Files Created/Modified

- `C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js` - Added notify_failover to COMMAND_REGISTRY; extended buildSafeEnv() with Evolution API vars
- `C:/Users/bono/racingpoint/comms-link/shared/send-email.js` - New stdlib-only email sender script
- `C:/Users/bono/racingpoint/comms-link/bono/index.js` - Fixed notifyFn + watchdog notification (sendEvolutionText direct + email shell-out)
- `C:/Users/bono/racingpoint/comms-link/james/failover-orchestrator.js` - Added email shell-out after notify_failover exec_request
- `C:/Users/bono/racingpoint/comms-link/james/exec-handler.js` - Added EXEC_REASON env var injection in #execute method

## Decisions Made

- `notify_failover` uses tier AUTO not NOTIFY -- the command itself is the notification delivery mechanism; NOTIFY tier would trigger notifyFn with a generic "Executed command: notify_failover" message which is not useful
- `EXEC_REASON` injected as env var in `ExecHandler#execute` -- this is the correct minimal change to pass the reason string to the child process without changing the COMMAND_REGISTRY contract
- `buildSafeEnv()` extended conditionally -- only adds Evolution API vars when they're set in the parent process environment, so the change is safe on James's Windows machine (all vars absent = no change to safeEnv)
- Email is fire-and-forget on both paths -- email failure (network/sendmail issue) must never block failover orchestration
- `send-email.js` uses sendmail first because Hostinger VPS Linux servers have sendmail/postfix installed by default; raw SMTP fallback handles edge cases

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added EXEC_REASON env var injection in ExecHandler**
- **Found during:** Task 1 (notify_failover COMMAND_REGISTRY implementation)
- **Issue:** The plan's Approach A assumes ExecHandler passes reason as EXEC_REASON env var to child process. ExecHandler did not do this -- `#execute()` only used `this.#safeEnv` with no per-execution overrides.
- **Fix:** Updated `ExecHandler#execute()` signature to accept `reason` parameter; merges `EXEC_REASON: reason` into a new frozen env object before spawning child. Also passed reason through from `handleExecRequest` to `#execute` calls.
- **Files modified:** james/exec-handler.js
- **Verification:** Syntax check passes; reason propagation path confirmed by code review
- **Committed in:** 7ce7abf (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 - missing critical functionality required for notify_failover to work correctly)
**Impact on plan:** Auto-fix was necessary for EXEC_REASON to reach the child process. Without it, notify_failover would have sent "FAILOVER ACTIVATED" generic text instead of the specific reason string from the orchestrator.

## Issues Encountered

None - all fixes applied cleanly on first attempt.

## Next Phase Readiness

- ORCH-04 fully satisfied: Uday gets WhatsApp AND email on failover via both paths (James primary + Bono watchdog)
- Phase 69 all 4 plans complete
- Bono VPS needs pm2 restart to pick up bono/index.js and shared/exec-protocol.js changes -- notified via INBOX.md
- Verify EVOLUTION_URL, EVOLUTION_INSTANCE, EVOLUTION_API_KEY, UDAY_WHATSAPP env vars are set in Bono's pm2 ecosystem file

---
*Phase: 69-health-monitor-failover-orchestration*
*Completed: 2026-03-21*
