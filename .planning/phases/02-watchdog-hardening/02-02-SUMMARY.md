---
phase: 02-watchdog-hardening
plan: 02
subsystem: infra
tags: [rust, watchdog, state-machine, fsm, dashboard-events, email-alerts, ws-liveness]

# Dependency graph
requires:
  - phase: 02-watchdog-hardening
    plan: 01
    provides: WatchdogState enum, pod_watchdog_states/pod_needs_restart AppState fields, DashboardEvent watchdog variants, format_alert_body with failure context

provides:
  - Rewritten check_all_pods: WatchdogState-aware restart with skip-during-recovery guard
  - Rewritten verify_restart: Verifying state on entry, partial-recovery-as-failure, RecoveryFailed on timeout
  - is_ws_alive() helper using sender.is_closed() -- eliminates contains_key() WS liveness bug
  - backoff_label() helper (30s/2m/10m/30m human-readable)
  - determine_failure_reason() and failure_type_from_reason() pure helpers for testability
  - 30 new tests covering all WatchdogState transitions and lifecycle invariants

affects:
  - 02-03 (pod_healer -- reads pod_watchdog_states and pod_needs_restart, same fields pod_monitor now writes)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "is_closed() for WS channel liveness -- sender.is_closed() is the only reliable signal after receiver is dropped"
    - "Pure helper extraction: determine_failure_reason() + failure_type_from_reason() for testable decision logic"
    - "WatchdogState FSM skip guard: match Restarting|Verifying -> continue at top of pod loop"
    - "Partial recovery is FAILED: no special-case return for process+ws ok, lock fail"
    - "needs_restart flag consumed via HashMap::remove().unwrap_or(false) -- read+clear atomically"

key-files:
  created: []
  modified:
    - crates/rc-core/src/pod_monitor.rs

key-decisions:
  - "partial recovery (process+WS ok, lock screen fail) is FAILED per CONTEXT.md -- lock screen is essential for customer flow"
  - "is_closed() replaces contains_key() for WS liveness -- contains_key only checks map presence, not channel health"
  - "WatchdogState skip check placed AFTER billing guard -- billing takes precedence over all other signals"
  - "Pure helper functions determine_failure_reason/failure_type_from_reason extracted so failure path logic is testable without network"
  - "check_lock_screen URL updated to /health endpoint (from / root) -- aligns with Plan 01 lock_screen.rs changes"

patterns-established:
  - "Pure helper pattern: extract all decision logic from async handlers into pure functions for unit testing"
  - "Consume-and-clear pattern: HashMap::remove().unwrap_or(false) for one-shot flags"

requirements-completed: [WD-01, WD-03, WD-04, ALERT-01, ALERT-02]

# Metrics
duration: 18min
completed: 2026-03-13
---

# Phase 2 Plan 02: Pod Monitor Restart Lifecycle Summary

**WatchdogState-aware restart lifecycle in pod_monitor: escalating backoff with double-restart prevention, partial-recovery-as-failure, is_closed() WS liveness, PodRestarting/PodVerifying/PodRecoveryFailed dashboard events, and email alerts on verification failure**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-13T00:10:00Z
- **Completed:** 2026-03-13T00:28:00Z
- **Tasks:** 2 (both in pod_monitor.rs, committed together)
- **Files modified:** 1

## Accomplishments

- Rewrote `check_all_pods`: skips Restarting/Verifying pods (no double-restart), consumes `needs_restart` flag from pod_healer, sets WatchdogState::Restarting + broadcasts PodRestarting on successful restart, resets WatchdogState to Healthy on natural recovery
- Rewrote `verify_restart`: sets Verifying on entry + broadcasts PodVerifying, treats partial recovery (no lock screen) as FAILED per CONTEXT.md, resets backoff + sets Healthy on full recovery, sets RecoveryFailed + broadcasts PodRecoveryFailed + sends email alert on failure
- Replaced all `contains_key()` WS liveness checks with `is_ws_alive()` using `sender.is_closed()`
- 30 new tests covering all lifecycle scenarios with zero network calls (pure helper approach)

## Task Commits

1. **Task 1 + Task 2: Rewrite pod_monitor restart lifecycle** - `4bdd2c7` (feat)

## Files Created/Modified

- `crates/rc-core/src/pod_monitor.rs` - Full restart lifecycle rewrite with WatchdogState management, is_ws_alive(), backoff_label(), determine_failure_reason(), failure_type_from_reason(), 30 new tests

## Decisions Made

- Partial recovery (process+WS connected, lock screen fail) is FAILED: customers cannot use a pod without the lock screen. Alert fires, WatchdogState set to RecoveryFailed, backoff NOT reset
- is_closed() is the correct WS liveness signal: contains_key() only checks map presence but a stale sender entry can linger in the map after the receiver (ws handler) has dropped
- WatchdogState skip check placed AFTER billing guard: billing takes precedence — a pod with active billing is never restarted regardless of watchdog state
- check_lock_screen URL updated from `http://127.0.0.1:18923/` to `http://127.0.0.1:18923/health` to align with the /health endpoint added in Plan 01 (Rule 1 auto-fix, minor URL correction)
- Pure helper functions extracted for testability: determine_failure_reason() and failure_type_from_reason() allow testing the failure path without async network calls

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated check_lock_screen URL to /health endpoint**
- **Found during:** Task 2 (verify_restart implementation)
- **Issue:** Original code checked `http://127.0.0.1:18923/` (root) but Plan 01 added a dedicated `/health` endpoint at `GET /health`. Checking root would not trigger the health_response_body() logic added in Plan 01
- **Fix:** Updated PowerShell command in check_lock_screen() to hit `http://127.0.0.1:18923/health`
- **Files modified:** crates/rc-core/src/pod_monitor.rs
- **Verification:** Build clean, test logic correct
- **Committed in:** 4bdd2c7 (combined task commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** URL fix necessary for correctness -- checking root would silently succeed even if /health wasn't registered. No scope creep.

## Issues Encountered

- `last_lock_ok` variable generated a compiler warning for "assigned but never read" due to Rust's analysis of `continue` control flow. Resolved by also using `last_lock_ok` in `determine_failure_reason()` call after the loop (the value IS needed for the partial recovery case) and assigning `false` in the early `continue` branch to keep assignments explicit.

## Next Phase Readiness

- pod_monitor now correctly implements the full WatchdogState FSM lifecycle
- pod_healer (Plan 03) can safely read pod_watchdog_states and set pod_needs_restart -- pod_monitor will honor both signals
- DashboardEvent broadcasts are complete: frontend can display Restarting/Verifying/RecoveryFailed states in real-time
- Email alerts fire on verification failure with enriched body (failure_type, last_heartbeat, next_action)

---
*Phase: 02-watchdog-hardening*
*Completed: 2026-03-13*

## Self-Check: PASSED

- [x] SUMMARY.md created: `.planning/phases/02-watchdog-hardening/02-02-SUMMARY.md`
- [x] Commit 4bdd2c7 exists: `feat(02-02): rewrite pod_monitor restart lifecycle with WatchdogState management`
- [x] All 83 rc-core tests pass (44 unit + 9 pod_healer + 30 pod_monitor)
- [x] All 33 rc-common tests pass
- [x] cargo build -p rc-core succeeds (no new warnings from pod_monitor)
- [x] contains_key() no longer used for WS liveness in pod_monitor.rs
- [x] STATE.md updated with 02-02 decisions
- [x] ROADMAP.md updated (Phase 2: 3/3 plans complete, Status: Complete)
- [x] REQUIREMENTS.md: ALERT-02 marked complete (WD-01/WD-03/WD-04/ALERT-01 already done in 02-01)
