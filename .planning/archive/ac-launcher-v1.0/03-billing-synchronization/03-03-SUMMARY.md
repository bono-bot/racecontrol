---
phase: 03-billing-synchronization
plan: 03
subsystem: billing
tags: [rust, billing, websocket, auth, lifecycle, game-status, launch-timeout]

# Dependency graph
requires:
  - phase: 03-billing-synchronization
    provides: "AcStatus enum, GameStatusUpdate protocol, BillingTimer count-up model, compute_session_cost()"
provides:
  - "handle_game_status_update() dispatching Live/Pause/Off/Replay to billing lifecycle"
  - "WaitingForGameEntry struct tracking pods awaiting AC LIVE status"
  - "defer_billing_start() replacing direct billing start at auth time"
  - "check_launch_timeouts() with 3-min timeout, retry, and cancel-on-double-failure"
  - "WebSocket GameStatusUpdate match arm wired to billing dispatch"
  - "All 4 auth call sites decoupled from immediate billing start"
affects: [03-billing-synchronization, overlay, agent-status-polling, rc-agent-launch-state]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Deferred billing: auth stores WaitingForGameEntry, billing starts only on AcStatus::Live"
    - "Launch timeout: 3-min window per attempt, 2 attempts max, cancel with no charge on double failure"
    - "Idempotent state transitions: duplicate Live on Active is no-op, Pause on no timer is no-op"

key-files:
  created: []
  modified:
    - "crates/rc-core/src/billing.rs"
    - "crates/rc-core/src/ws/mod.rs"
    - "crates/rc-core/src/auth/mod.rs"

key-decisions:
  - "Deferred billing uses placeholder ID (deferred-UUID) returned to kiosk/PWA; real billing session created on Live"
  - "Reservation linking deferred from auth to actual billing start in start_billing_session()"
  - "AcStatus::Replay treated same as Pause for billing (customer watching replay is not driving)"
  - "AcStatus::Off ends active billing session as EndedEarly (game exit = session end)"
  - "Launch timeout retry sends CoreToAgentMessage::LaunchGame to trigger agent-side retry"
  - "check_launch_timeouts_from_manager() helper for unit testing without AppState"

patterns-established:
  - "Deferred billing pattern: auth -> WaitingForGameEntry -> GameStatusUpdate(Live) -> start_billing_session()"
  - "Game status dispatch: ws/mod.rs delegates to billing::handle_game_status_update() for all AcStatus transitions"
  - "Launch timeout in tick loop: checked every 1s, attempt 1 retries, attempt 2 cancels with no charge"

requirements-completed: [BILL-01, BILL-02]

# Metrics
duration: 10min
completed: 2026-03-14
---

# Phase 3 Plan 3: Core Billing Lifecycle and Auth Decoupling Summary

**Billing starts only on AC STATUS=LIVE via handle_game_status_update(), all 4 auth paths decoupled from immediate billing, 3-min launch timeout with retry/cancel logic**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-13T18:50:18Z
- **Completed:** 2026-03-13T19:00:21Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- handle_game_status_update() dispatches Live (start/resume billing), Pause (pause billing), Off (end session), Replay (pause billing) with full idempotency
- WaitingForGameEntry struct + BillingManager.waiting_for_game map for deferred billing state
- defer_billing_start() replaces direct billing start at all 4 auth call sites (PIN, QR, Start Now, Kiosk)
- check_launch_timeouts() with 3-min timeout, retry on attempt 1, cancel with no charge on attempt 2
- Launch timeout checks wired into tick_all_timers loop for automatic detection
- WebSocket GameStatusUpdate match arm upgraded from placeholder to real billing dispatch
- 10 new billing lifecycle tests, all 225 workspace tests passing (68 rc-common + 144 rc-core unit + 13 integration)

## Task Commits

Each task was committed atomically:

1. **Task 1: Core billing lifecycle functions** - `7d26624` (feat)
2. **Task 2: Wire WebSocket handler + decouple auth** - `33ea897` (feat)

## Files Created/Modified
- `crates/rc-core/src/billing.rs` - Added WaitingForGameEntry, waiting_for_game on BillingManager, handle_game_status_update(), defer_billing_start(), check_launch_timeouts(), launch timeout in tick loop, 10 new tests
- `crates/rc-core/src/ws/mod.rs` - Replaced GameStatusUpdate placeholder with real billing dispatch via handle_game_status_update()
- `crates/rc-core/src/auth/mod.rs` - All 4 call sites (validate_pin, validate_qr, start_now, validate_pin_kiosk) now use defer_billing_start() instead of start_billing_session()

## Decisions Made
- Placeholder billing session ID ("deferred-UUID") returned to callers; real session created when AcStatus::Live arrives
- Reservation linking deferred from auth-time to actual billing start (link_reservation_to_billing temporarily unused)
- AcStatus::Off triggers EndedEarly on the billing session (game exit = session ends)
- AcStatus::Replay treated same as Pause (customer not actively driving)
- Launch timeout retry sends LaunchGame to agent; agent-side LaunchState machine handles the actual AC restart
- check_launch_timeouts_from_manager() enables unit testing without full AppState

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Core-side billing lifecycle fully wired for Phase 3 Plan 02 (agent sends GameStatusUpdate, core handles it)
- Auth decoupling complete -- billing only starts when AC reports STATUS=LIVE
- Launch timeout handling prevents stuck sessions if AC fails to reach LIVE status
- Phase 4 (Safety Enforcement) can build on this billing lifecycle foundation

## Self-Check: PASSED

- [x] crates/rc-core/src/billing.rs -- FOUND
- [x] crates/rc-core/src/ws/mod.rs -- FOUND
- [x] crates/rc-core/src/auth/mod.rs -- FOUND
- [x] 03-03-SUMMARY.md -- FOUND
- [x] Commit 7d26624 -- FOUND (Task 1)
- [x] Commit 33ea897 -- FOUND (Task 2)
- [x] 225 workspace tests passing (68 rc-common + 144 rc-core unit + 13 integration)

---
*Phase: 03-billing-synchronization*
*Completed: 2026-03-14*
