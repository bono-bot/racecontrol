---
phase: 05-watchdog-hardening
plan: 02
subsystem: infra
tags: [watchdog, backoff, pod-monitor, pod-healer, email-alerts, restart-verification]

# Dependency graph
requires:
  - "05-01: EscalatingBackoff state machine, EmailAlerter module, WatchdogConfig expansion, AppState shared fields"
provides:
  - "pod_monitor.rs with escalating backoff (30s/2m/10m/30m), post-restart verification (5/15/30/60s), email alerts"
  - "pod_healer.rs using shared backoff, deferring restarts to pod_monitor, email alerts for persistent issues"
  - "Coordinated watchdog: pod_monitor owns restarts, pod_healer owns diagnostics/healing, both share backoff state"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: ["Shared backoff state between watchdog subsystems via AppState.pod_backoffs", "Post-restart verification with progressive health checks (process, WebSocket, lock screen)", "Restart deferral pattern: healer detects, monitor restarts"]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/pod_monitor.rs
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "pod_monitor owns all rc-agent restarts; pod_healer defers via issues list instead of restarting independently"
  - "Post-restart verification uses 4-stage progressive delay (5/15/30/60s) to avoid false negatives on slow startups"
  - "Partial recovery (WebSocket OK, lock screen Session 0) does NOT trigger email or reset backoff -- known Session 0 limitation"
  - "Active billing guard added to pod_monitor to prevent restarts during customer sessions"

patterns-established:
  - "Restart ownership: single subsystem (pod_monitor) owns restart decisions, others defer"
  - "Progressive verification: check at increasing intervals to balance responsiveness with accuracy"

requirements-completed: [WD-01, WD-02, WD-03, WD-05]

# Metrics
duration: 6min
completed: 2026-03-12
---

# Phase 5 Plan 2: Watchdog Integration Summary

**Escalating backoff and post-restart verification wired into pod_monitor.rs; pod_healer.rs refactored to share backoff state and defer restarts**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-12T05:04:01Z
- **Completed:** 2026-03-12T05:10:06Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- pod_monitor.rs rewritten: replaces fixed 120s cooldown with shared EscalatingBackoff (30s/2m/10m/30m), spawns async post-restart verification tasks at 5/15/30/60s, sends email alerts on exhaustion or verification failure
- pod_healer.rs refactored: removes HealCooldown/HEAL_COOLDOWN_SECS, reads shared backoff from AppState, defers rc-agent restarts to pod_monitor, sends email for pods with 3+ persistent issues
- Active billing guard added to pod_monitor to prevent restarts during customer sessions
- Partial recovery handling: WebSocket-connected but lock screen in Session 0 is logged but does not trigger false email alerts

## Task Commits

Each task was committed atomically:

1. **Task 1: Rewrite pod_monitor.rs with escalating backoff and post-restart verification** - `3448a6c` (feat)
2. **Task 2: Modify pod_healer.rs to use shared backoff and defer restarts** - `1a82cd7` (feat)

## Files Created/Modified
- `crates/racecontrol/src/pod_monitor.rs` - Escalating backoff, verify_restart() with 4-stage progressive health check, email alerts on exhaustion/failure, active billing guard
- `crates/racecontrol/src/pod_healer.rs` - Shared backoff via AppState, restart deferral to pod_monitor, email alerts for persistent issues (3+)

## Decisions Made
- pod_monitor is the single owner of rc-agent restarts; pod_healer logs issues but never executes restart_rc_agent -- prevents duplicate restarts and backoff confusion
- Post-restart verification uses 4 progressive delays (5s, 15s, 30s, 60s) to balance early detection of healthy restarts with tolerance for slow boots
- Partial recovery (WebSocket connected but lock screen unresponsive in Session 0) is documented but does NOT trigger email -- this is a known Windows Session 0 limitation that resolves on reboot
- Active billing guard was added to pod_monitor even though the original code lacked it -- the plan's research identified this as an anti-pattern to prevent

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing rc-agent compilation errors (installed_games field removal, LockScreenManager signature change) prevented `cargo test --workspace` from succeeding, but these are in the rc-agent crate and unrelated to this plan's scope. rc-common and racecontrol tests all pass (30 + 31 unit + 13 integration = 74 tests).

## User Setup Required
None - no external service configuration required. Email alerting uses the configuration from Plan 01 (disabled by default, configurable via racecontrol.toml).

## Next Phase Readiness
- Phase 5 (Watchdog Hardening) is now complete: all foundation primitives (Plan 01) and integration wiring (Plan 02) are done
- The escalating backoff, post-restart verification, email alerting, and coordinated restart ownership are all operational
- No blockers for future phases

## Self-Check: PASSED

All 2 modified files verified present. Both task commits (3448a6c, 1a82cd7) confirmed in git log.

---
*Phase: 05-watchdog-hardening*
*Completed: 2026-03-12*
