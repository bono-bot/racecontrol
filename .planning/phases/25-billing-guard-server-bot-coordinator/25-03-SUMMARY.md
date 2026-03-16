---
phase: 25-billing-guard-server-bot-coordinator
plan: "03"
subsystem: billing
tags: [rust, bot, billing, websocket, email-alerts, pod-healer]

# Dependency graph
requires:
  - phase: 25-01
    provides: "BILL-01 characterization tests + FailureMonitorState.driving_state field enabling billing_guard.rs compilation"
  - phase: 23
    provides: "PodFailureReason enum with SessionStuckWaitingForGame + IdleBillingDrift variants; AgentMessage BillingAnomaly/HardwareFailure/TelemetryGap variants"
provides:
  - "bot_coordinator.rs: server-side routing for BillingAnomaly, HardwareFailure, TelemetryGap agent messages"
  - "handle_billing_anomaly() with is_pod_in_recovery() guard and SessionStuckWaitingForGame/IdleBillingDrift dispatch"
  - "recover_stuck_session() calling only end_billing_session_public() — no separate StopGame"
  - "alert_staff_idle_drift() sending email via state.email_alerter, never ending session"
  - "handle_hardware_failure() and handle_telemetry_gap() stubs for BOT-01 completeness"
affects:
  - "25-04: wiring wave — ws/mod.rs stub replacement calls these handlers"
  - "26: TELEM-01 — handle_telemetry_gap() stub becomes full implementation"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Bot coordinator pattern: server owns all session-ending logic; agent reports, server acts"
    - "Recovery guard: is_pod_in_recovery() checked at handler entry before any action"
    - "Session resolution: server resolves session_id from active_timers, ignores agent-provided id which may be stale"

key-files:
  created:
    - crates/racecontrol/src/bot_coordinator.rs
  modified:
    - crates/racecontrol/src/lib.rs

key-decisions:
  - "Used correct PodFailureReason variants (SessionStuckWaitingForGame/IdleBillingDrift) — plan had stale names (BillingStuckSession/IdleDriftDetected)"
  - "recover_stuck_session() resolves session from active_timers server-side; agent-provided billing_session_id prefixed with _ (unused)"
  - "handle_hardware_failure and handle_telemetry_gap take _state as unused param — forward-compatible for Phase 26 impl"

patterns-established:
  - "Bot handler entry guard: always check is_pod_in_recovery() first, return early if true"
  - "Billing anomaly dispatch: match on PodFailureReason, route recover vs alert, wildcard arm logs and returns"
  - "Staff alert pattern: subject + body format string, write-lock email_alerter, call send_alert()"

requirements-completed:
  - BOT-01
  - BILL-02
  - BILL-03

# Metrics
duration: 8min
completed: 2026-03-16
---

# Phase 25 Plan 03: Bot Coordinator Summary

**Server-side bot message router with recovery guard, stuck-session auto-end, idle-drift staff alert, and hardware/telemetry stubs — 5 new tests, 299 total passing**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-16T12:24:53Z
- **Completed:** 2026-03-16T12:32:00Z
- **Tasks:** 1 (TDD: RED+GREEN combined, plan was already GREEN-ready)
- **Files modified:** 2

## Accomplishments
- Created `crates/racecontrol/src/bot_coordinator.rs` (~200 lines) with three public handlers and two private helpers
- `handle_billing_anomaly()` guards on `is_pod_in_recovery()` then dispatches to recover vs alert
- `recover_stuck_session()` resolves session from `active_timers`, calls only `end_billing_session_public()` — no direct StopGame
- `alert_staff_idle_drift()` sends email via `state.email_alerter` and NEVER calls `end_billing_session_public()`
- 5 unit tests all pass; full racecontrol suite 299 tests (258 lib + 41 integration), zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Create bot_coordinator.rs with routing handlers and unit tests (TDD)** - `74339bc` (feat)

**Plan metadata:** (docs commit follows)

_Note: TDD — RED and GREEN executed in single pass since the implementation was well-specified. Tests verified GREEN immediately._

## Files Created/Modified
- `crates/racecontrol/src/bot_coordinator.rs` - Server-side bot message router: handle_billing_anomaly, handle_hardware_failure, handle_telemetry_gap, recover_stuck_session, alert_staff_idle_drift
- `crates/racecontrol/src/lib.rs` - Added `pub mod bot_coordinator;` declaration (alphabetical between bono_relay and accounting)

## Decisions Made
- Corrected stale variant names from plan: `BillingStuckSession` → `SessionStuckWaitingForGame`, `IdleDriftDetected` → `IdleBillingDrift` (Rule 1 auto-fix)
- `_billing_session_id` parameter kept as unused with underscore prefix — server resolves from active_timers for correctness
- `handle_hardware_failure` and `handle_telemetry_gap` take `_state: &Arc<AppState>` (underscore prefix) for forward-compatibility with Phase 26 full implementation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed stale PodFailureReason variant names**
- **Found during:** Task 1 (creating bot_coordinator.rs)
- **Issue:** Plan interfaces section referenced `PodFailureReason::BillingStuckSession` and `PodFailureReason::IdleDriftDetected` which do not exist in the enum. Actual variants are `SessionStuckWaitingForGame` and `IdleBillingDrift`
- **Fix:** Used correct enum variants throughout implementation and tests
- **Files modified:** crates/racecontrol/src/bot_coordinator.rs
- **Verification:** `cargo test -p racecontrol-crate` compiles and passes (259 tests)
- **Committed in:** 74339bc (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - stale interface names in plan)
**Impact on plan:** Necessary for correctness. No scope creep.

## Issues Encountered
- None beyond the stale variant name deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `bot_coordinator.rs` is ready for Plan 04 wiring: `ws/mod.rs` stubs will be replaced with calls to `handle_billing_anomaly()`, `handle_hardware_failure()`, and `handle_telemetry_gap()`
- `recover_stuck_session()` has a `// Cloud sync fence: Plan 04 adds the relay_available wait here` comment marking the hook point for the sync fence
- `handle_telemetry_gap()` stub is ready for Phase 26 TELEM-01 full implementation

## Self-Check: PASSED

- crates/racecontrol/src/bot_coordinator.rs: FOUND
- .planning/phases/25-billing-guard-server-bot-coordinator/25-03-SUMMARY.md: FOUND
- Commit 74339bc: FOUND

---
*Phase: 25-billing-guard-server-bot-coordinator*
*Completed: 2026-03-16*
