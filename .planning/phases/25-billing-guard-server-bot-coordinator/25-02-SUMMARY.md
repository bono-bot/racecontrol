---
phase: 25-billing-guard-server-bot-coordinator
plan: "02"
subsystem: rc-agent + billing anomaly detection
tags: [tdd, wave-1a, bill-02, bill-03, billing-guard, failure-monitor]
dependency_graph:
  requires:
    - "25-01: FailureMonitorState.driving_state field (compile gate)"
  provides:
    - "billing_guard::spawn() background task for BILL-02 and BILL-03 detection"
    - "7 unit tests for billing anomaly detection conditions"
  affects:
    - "crates/rc-agent/src/billing_guard.rs (new file)"
    - "crates/rc-agent/src/main.rs (mod billing_guard; declaration)"
tech_stack:
  added: []
  patterns:
    - "Tokio background task with task-local debounce state (same as failure_monitor)"
    - "watch::Receiver<FailureMonitorState> clone for independent polling"
    - "try_send for non-blocking anomaly message dispatch"
key_files:
  created:
    - crates/rc-agent/src/billing_guard.rs
  modified:
    - crates/rc-agent/src/main.rs
decisions:
  - "Used existing PodFailureReason variants (SessionStuckWaitingForGame, IdleBillingDrift) instead of plan's speculative names — rc-common enum was already defined with different names in Phase 23"
  - "spawn() call deferred to Plan 04 as planned — mod declaration alone causes no breakage"
  - "state_rx parameter is not mut — watch::Receiver::borrow() takes &self in Tokio"
metrics:
  duration_min: 4
  completed_date: "2026-03-16"
  tasks_completed: 1
  files_modified: 2
---

# Phase 25 Plan 02: billing_guard.rs — Agent-Side Billing Anomaly Detector Summary

New independent Tokio task in rc-agent that detects stuck sessions (BILL-02: billing active + game_pid=None >= 60s) and idle drift (BILL-03: billing active + not DrivingState::Active >= 300s), sends AgentMessage::BillingAnomaly, never calls end_session.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create billing_guard.rs with spawn() and unit tests (TDD) | a635a9b | crates/rc-agent/src/billing_guard.rs, crates/rc-agent/src/main.rs |

## What Was Built

**billing_guard.rs (~150 lines):**
- `spawn(state_rx, agent_msg_tx, pod_id)` — fires and forgets a Tokio task
- Poll loop: 5s interval (same as failure_monitor `POLL_INTERVAL_SECS`)
- BILL-02 detection: `billing_active=true && game_pid.is_none()` for >= 60s → `AgentMessage::BillingAnomaly { reason: SessionStuckWaitingForGame }`
- BILL-03 detection: `billing_active=true && !matches!(driving_state, Some(Active))` for >= 300s → `AgentMessage::BillingAnomaly { reason: IdleBillingDrift }`
- `stuck_fired` / `idle_fired` boolean debounce — prevents duplicate sends per incident, resets when condition clears
- `recovery_in_progress=true` resets all timers and skips the loop iteration
- `billing_session_id: "unknown"` — server resolves via `active_timers` lookup
- `tracing::warn!` logged on each anomaly send

**main.rs change:**
- `mod billing_guard;` added alphabetically after `mod billing_guard;` (between `ai_debugger` and `content_scanner`)

**7 unit tests (pure condition assertions, no async):**
1. `stuck_session_condition_requires_billing_and_no_pid` — billing=true + pid=None triggers condition
2. `no_stuck_session_when_billing_inactive` — billing=false suppresses condition
3. `no_stuck_session_when_game_running` — game_pid=Some suppresses condition
4. `idle_drift_condition_driving_inactive` — billing=true + DrivingState::Idle triggers condition
5. `idle_drift_suppressed_when_recovery_in_progress` — recovery_in_progress=true present in state
6. `stuck_threshold_is_60s` — STUCK_SESSION_THRESHOLD_SECS == 60
7. `idle_threshold_is_300s` — IDLE_DRIFT_THRESHOLD_SECS == 300

## Test Results

```
billing_guard::tests: 7 passed
failure_monitor::tests: 8 passed (no regressions)
rc-agent-crate cargo check: PASS (29 warnings, all pre-existing)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] PodFailureReason variant name mismatch**
- **Found during:** Task 1, compilation step
- **Issue:** Plan's interface block specified `PodFailureReason::BillingStuckSession` and `PodFailureReason::IdleDriftDetected`, but the actual rc-common enum (defined in Phase 23) uses `SessionStuckWaitingForGame` and `IdleBillingDrift`
- **Fix:** Updated billing_guard.rs to use the real variant names from rc-common/src/types.rs
- **Files modified:** crates/rc-agent/src/billing_guard.rs
- **Commit:** a635a9b

## Self-Check: PASSED

Files exist:
- crates/rc-agent/src/billing_guard.rs — FOUND (created)
- crates/rc-agent/src/main.rs — FOUND (modified, mod billing_guard; added)

Commits exist:
- a635a9b feat(25-02): add billing_guard.rs with spawn() and 7 unit tests — FOUND
