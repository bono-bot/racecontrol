---
phase: 05-watchdog-hardening
plan: 01
subsystem: infra
tags: [watchdog, backoff, email-alerts, rate-limiting, state-machine]

# Dependency graph
requires: []
provides:
  - "EscalatingBackoff state machine in rc-common (30s/2m/10m/30m cooldown steps)"
  - "EmailAlerter module in rc-core with per-pod (30min) and venue-wide (5min) rate limiting"
  - "Expanded WatchdogConfig with 6 new email/escalation fields"
  - "AppState pod_backoffs and email_alerter shared state fields"
affects: [05-02-PLAN]

# Tech tracking
tech-stack:
  added: []
  patterns: ["EscalatingBackoff state machine with configurable step durations", "Rate-limited email alerting via Node.js shell-out with tokio timeout"]

key-files:
  created:
    - crates/rc-common/src/watchdog.rs
    - crates/rc-core/src/email_alerts.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/rc-core/src/lib.rs
    - crates/rc-core/src/config.rs
    - crates/rc-core/src/state.rs
    - crates/rc-core/src/api/routes.rs

key-decisions:
  - "EscalatingBackoff uses Vec<Duration> steps with clamping to last element for cap behavior"
  - "EmailAlerter enforces dual rate limits: per-pod 30min AND venue-wide 5min must both pass"
  - "Email sending uses 15s tokio timeout with kill_on_drop(true) to avoid blocking watchdog loop"

patterns-established:
  - "Shared state machine in rc-common for cross-crate consumption (EscalatingBackoff)"
  - "Rate-limited alerter pattern: should_send() check + record_sent() on success"

requirements-completed: [WD-01, WD-03, WD-04, WD-06]

# Metrics
duration: 5min
completed: 2026-03-12
---

# Phase 5 Plan 1: Watchdog Foundation Summary

**EscalatingBackoff state machine (30s/2m/10m/30m) with EmailAlerter dual rate limiting (per-pod 30min, venue-wide 5min) and expanded WatchdogConfig/AppState**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-12T04:55:08Z
- **Completed:** 2026-03-12T05:00:26Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- EscalatingBackoff state machine with configurable cooldown steps, ready/record/reset/exhausted API
- EmailAlerter with per-pod and venue-wide rate limiting, async send via Node.js shell-out with timeout
- WatchdogConfig expanded with 6 new fields (email_enabled, email_recipient, email_script_path, email_pod_cooldown_secs, email_venue_cooldown_secs, escalation_steps_secs) and sane defaults
- AppState gains pod_backoffs (HashMap) and email_alerter (RwLock) for shared watchdog state
- 26 total new unit tests (14 watchdog + 12 email_alerts/config)

## Task Commits

Each task was committed atomically:

1. **Task 1: EscalatingBackoff state machine in rc-common** - `02ad967` (feat)
2. **Task 2: EmailAlerter module, WatchdogConfig expansion, and AppState fields** - `c50e67a` (feat)

_Note: TDD tasks with implementation and tests written together._

## Files Created/Modified
- `crates/rc-common/src/watchdog.rs` - EscalatingBackoff state machine with 14 unit tests
- `crates/rc-common/src/lib.rs` - Added `pub mod watchdog` export
- `crates/rc-core/src/email_alerts.rs` - EmailAlerter with rate limiting, send_alert(), format_alert_body()
- `crates/rc-core/src/lib.rs` - Added `pub mod email_alerts` export
- `crates/rc-core/src/config.rs` - WatchdogConfig expanded with 6 new fields + 2 config deserialization tests
- `crates/rc-core/src/state.rs` - AppState gains pod_backoffs and email_alerter fields
- `crates/rc-core/src/api/routes.rs` - Removed stale installed_games field references (pre-existing fix)

## Decisions Made
- EscalatingBackoff uses Vec<Duration> steps with index clamping (not modular arithmetic) so the cap is always the last configured step
- EmailAlerter requires both per-pod AND venue-wide cooldowns to have elapsed before sending -- this prevents spam during multi-pod failures
- Email sending uses tokio::process::Command with kill_on_drop(true) and 15s timeout to prevent blocking the watchdog loop
- Config defaults match operational needs: email disabled by default, recipient defaults to usingh@racingpoint.in

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing installed_games field removal in routes.rs**
- **Found during:** Task 2 (building rc-core)
- **Issue:** PodInfo struct had `installed_games` field removed but 3 references in routes.rs still used it, blocking all rc-core compilation
- **Fix:** Removed the 3 stale `installed_games: Vec::new()` lines from routes.rs
- **Files modified:** crates/rc-core/src/api/routes.rs
- **Verification:** cargo build -p rc-core compiles successfully
- **Committed in:** c50e67a (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Pre-existing compilation error unrelated to plan scope. Minimal fix (3 lines removed). No scope creep.

## Issues Encountered
None beyond the pre-existing installed_games build error documented above.

## User Setup Required
None - no external service configuration required. Email alerting is disabled by default and configurable via racecontrol.toml.

## Next Phase Readiness
- All foundation primitives (EscalatingBackoff, EmailAlerter, expanded config, AppState fields) are ready for Plan 02 integration
- Plan 02 will wire these into pod_monitor.rs and pod_healer.rs
- No blockers

## Self-Check: PASSED

All 7 files verified present. Both task commits (02ad967, c50e67a) confirmed in git log.

---
*Phase: 05-watchdog-hardening*
*Completed: 2026-03-12*
