---
phase: 02-watchdog-hardening
plan: 01
subsystem: infra
tags: [rust, watchdog, state-machine, email-alerts, lock-screen, http]

# Dependency graph
requires:
  - phase: 01-state-wiring-config-hardening
    provides: EscalatingBackoff, EmailAlerter, AppState base structure

provides:
  - WatchdogState enum (Healthy/Restarting/Verifying/RecoveryFailed) in racecontrol/state.rs
  - pod_watchdog_states and pod_needs_restart fields in AppState, pre-populated for pods 1-8
  - PodRestarting, PodVerifying, PodRecoveryFailed DashboardEvent variants
  - format_alert_body extended with failure_type, last_heartbeat, next_action params
  - GET /health endpoint on rc-agent lock screen HTTP server (port 18923)

affects:
  - 02-02 (pod_monitor rewrite — uses WatchdogState, pod_watchdog_states, new DashboardEvent variants)
  - 02-03 (pod_healer — reads pod_watchdog_states and pod_needs_restart)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "WatchdogState FSM defined in racecontrol (not rc-common) — core-local concern, not shared protocol"
    - "health_response_body() extracted as pure fn for testability — async TCP handler delegates to it"
    - "TDD: tests written before implementation in all tasks"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/email_alerts.rs
    - crates/racecontrol/src/pod_monitor.rs
    - crates/rc-agent/src/lock_screen.rs

key-decisions:
  - "WatchdogState defined in racecontrol not rc-common — it is a core-side FSM, not a shared protocol type"
  - "health_response_body() is a pure function (not inline in async handler) — enables unit testing without TCP"
  - "/health returns HTTP 200 always; JSON body distinguishes ok/degraded — server liveness is the primary signal"
  - "verify_restart gains last_seen: Option<DateTime<Utc>> param so email context is correct at alert time"

patterns-established:
  - "Pure helper pattern: extract testable logic from async handlers into pure functions"
  - "Pre-populate all 8 pod entries on startup — avoids Option<> noise in watchdog hot path"

requirements-completed: [WD-01, WD-04, WD-03, ALERT-01]

# Metrics
duration: 6min
completed: 2026-03-13
---

# Phase 2 Plan 01: Shared Contracts Summary

**WatchdogState FSM enum, AppState watchdog fields, DashboardEvent watchdog variants, enriched format_alert_body with heartbeat/next-action context, and GET /health on rc-agent port 18923**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-13T00:00:00Z
- **Completed:** 2026-03-13T00:06:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- WatchdogState enum with 4 variants (Healthy, Restarting, Verifying, RecoveryFailed) added to AppState, pre-populated for pods 1-8 at startup
- DashboardEvent extended with PodRestarting, PodVerifying, PodRecoveryFailed — all serde roundtrip tested
- format_alert_body extended to carry failure_type, last_heartbeat, and next_action — all 3 pod_monitor callers updated, verify_restart gains last_seen param
- GET /health endpoint added to rc-agent lock screen HTTP server at port 18923 with health_response_body() pure helper and 6 unit tests

## Task Commits

1. **Task 1: WatchdogState, AppState fields, DashboardEvent variants, format_alert_body** - `55519cd` (feat)
2. **Task 2: rc-agent /health endpoint** - `7694106` (feat)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - WatchdogState enum, pod_watchdog_states/pod_needs_restart fields, create_initial_watchdog_states()/create_initial_needs_restart() helpers, 5 new tests
- `crates/rc-common/src/protocol.rs` - PodRestarting/PodVerifying/PodRecoveryFailed DashboardEvent variants, 3 serde roundtrip tests
- `crates/racecontrol/src/email_alerts.rs` - format_alert_body extended with failure_type/last_heartbeat/next_action, 4 new tests
- `crates/racecontrol/src/pod_monitor.rs` - 3 format_alert_body callers updated, verify_restart gains last_seen param
- `crates/rc-agent/src/lock_screen.rs` - GET /health handler in serve_lock_screen(), health_response_body() pure helper, 6 unit tests

## Decisions Made

- WatchdogState lives in racecontrol (not rc-common): it is internal to core's watchdog FSM, not a shared protocol type that agents need to understand
- health_response_body() extracted as pure function for testability — the async TCP handler just calls it and writes the result
- /health always returns HTTP 200; the JSON body (ok/degraded) provides additional context but the primary failure signal is "server not reachable at all"
- verify_restart gains last_seen parameter to pass accurate heartbeat timestamp into failure alert email

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- All shared contracts are in place for Plan 02 (pod_monitor rewrite)
- WatchdogState, pod_watchdog_states, pod_needs_restart ready for pod_monitor to write FSM transitions
- DashboardEvent variants ready to broadcast watchdog events to dashboard
- /health endpoint ready for post-restart verification via pod-agent /exec curl

---
*Phase: 02-watchdog-hardening*
*Completed: 2026-03-13*
