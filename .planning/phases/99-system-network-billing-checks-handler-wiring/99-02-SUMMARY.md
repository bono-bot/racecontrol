---
phase: 99-system-network-billing-checks-handler-wiring
plan: 02
subsystem: agent
tags: [rust, pre-flight, rate-limiting, alert-flood, ws-handler, maintenance-retry, STAFF-04]

requires:
  - phase: 99-01
    provides: 9-way concurrent pre-flight runner with billing_stuck, disk, memory, ws_stability checks

provides:
  - last_preflight_alert: Option<Instant> on AppState for 60s alert rate-limiting
  - BillingStarted handler: PreFlightFailed WS alert suppressed within 60s cooldown
  - Pass branch resets cooldown in both ws_handler.rs and event_loop.rs
  - Maintenance retry loop: confirmed no alert send (only refreshes lock screen)

affects:
  - crates/rc-agent/src/app_state.rs — last_preflight_alert field added
  - crates/rc-agent/src/main.rs — last_preflight_alert: None initialization
  - crates/rc-agent/src/ws_handler.rs — rate-limiting logic + Pass reset
  - crates/rc-agent/src/event_loop.rs — Pass reset + documentation comment

tech-stack:
  added: []
  patterns:
    - "Option<std::time::Instant> on AppState for last-alert tracking — no Arc/Mutex needed (single-threaded select! loop)"
    - "should_alert = last_preflight_alert.map(|t| t.elapsed() > 60s).unwrap_or(true) — None means never alerted"
    - "lock_screen + in_maintenance always fire unconditionally — only WS message is rate-limited"
    - "Reset to None on Pass — ensures first failure after recovery always alerts"
    - "Retry loop does NOT send PreFlightFailed — only initial BillingStarted path does"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/app_state.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/event_loop.rs

key-decisions:
  - "Retry loop does NOT send PreFlightFailed — verified and documented (STAFF-04 comment). Alert flood was already prevented by retry-only-logging design from 98-02; rate-limiter only needed in BillingStarted path."
  - "Pass branch resets cooldown in BOTH call sites (ws_handler + event_loop) — ensures recovery always triggers fresh alert"
  - "Option<Instant> on AppState (not Arc<Mutex>) — safe because AppState is used inside single-threaded tokio select! loop"

metrics:
  duration: 5min
  completed: 2026-03-21
  tasks: 2
  files_modified: 4
---

# Phase 99 Plan 02: Alert Rate-Limiting for PreFlightFailed (STAFF-04) Summary

**60s cooldown on PreFlightFailed WS alerts via Option<Instant> on AppState; lock screen + maintenance flag always fire; retry loop confirmed no-alert by design**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-03-21T05:20:01Z (IST: 10:50)
- **Completed:** 2026-03-21T05:24:54Z (IST: 10:55)
- **Tasks:** 2
- **Files modified:** 4 (0 created, 4 modified)

## Accomplishments

### Task 1: Add last_preflight_alert to AppState

Added `pub(crate) last_preflight_alert: Option<std::time::Instant>` to the `AppState` struct in `app_state.rs`. Initialized to `None` in `main.rs` AppState construction block. This field tracks when the last PreFlightFailed WebSocket alert was sent. None means never alerted (permit send); Some(t) with elapsed <= 60s means cooldown is active (suppress).

### Task 2: Wire rate-limiting into BillingStarted handler + maintenance retry loop

**ws_handler.rs BillingStarted MaintenanceRequired arm:**
- Added `should_alert` check: `last_preflight_alert.map(|t| t.elapsed() > Duration::from_secs(60)).unwrap_or(true)`
- PreFlightFailed WS message only sent if `should_alert` is true
- On alert send: `state.last_preflight_alert = Some(Instant::now())` + `tracing::warn!`
- On suppression: `tracing::info!` with elapsed seconds
- Lock screen (`show_maintenance_required`) and `in_maintenance.store(true)` are ALWAYS executed — not rate-limited

**ws_handler.rs Pass branch:**
- Reset: `state.last_preflight_alert = None` on Pass — ensures next failure after recovery always alerts immediately

**event_loop.rs maintenance retry arm:**
- Verified: retry arm does NOT send PreFlightFailed alerts — it only logs + refreshes lock screen (correct by design from 98-02)
- Added STAFF-04 comment documenting intentional no-alert behavior
- Pass branch: `state.last_preflight_alert = None` reset added

## Task Commits

1. **Task 1: Add last_preflight_alert to AppState** - `71e75b7` (feat)
2. **Task 2: Wire rate-limiting into handlers** - `afed1c2` (feat)

## Files Created/Modified

- `crates/rc-agent/src/app_state.rs` — +3 lines: last_preflight_alert field + doc comment
- `crates/rc-agent/src/main.rs` — +1 line: last_preflight_alert: None initialization
- `crates/rc-agent/src/ws_handler.rs` — +22 lines: cooldown check, should_alert logic, warn/info logging, Pass reset
- `crates/rc-agent/src/event_loop.rs` — +5 lines: Pass reset + STAFF-04 comment in retry arm

## Decisions Made

- Option<Instant> on AppState (not Arc<Mutex>) — safe because AppState is used inside single-threaded tokio select! loop
- Retry loop does NOT send alerts — verified and documented, not a bug. Alert flood prevention was already built into the 98-02 retry design (logs only). Rate-limiter was only needed in the BillingStarted path.
- Reset to None on Pass (both call sites) — ensures recovery -> next failure always alerts immediately

## Deviations from Plan

None - plan executed exactly as written. Retry arm confirmed no-alert design from 98-02 (plan anticipated this with the "if not sending, just verify and document" clause).

## Verification Results

- `cargo build --bin rc-agent` — 0 errors, 61 warnings (all pre-existing)
- `cargo test -p rc-agent-crate pre_flight::tests` — 17 tests pass (0 failures)
- `grep -c "last_preflight_alert" ws_handler.rs` — 4 (>= 3 required)
- `grep -c "last_preflight_alert" app_state.rs` — 1 (match required)
- `grep "should_alert" ws_handler.rs` — match
- `grep "cooldown|suppressed|60" ws_handler.rs` — match
- `grep "last_preflight_alert.*None" ws_handler.rs` — match (reset on Pass)
- `grep "last_preflight_alert.*None" main.rs` — match (initialization)

## Self-Check: PASSED

- `crates/rc-agent/src/app_state.rs` — last_preflight_alert field present (1 occurrence)
- `crates/rc-agent/src/main.rs` — last_preflight_alert: None present
- `crates/rc-agent/src/ws_handler.rs` — should_alert + cooldown + suppressed + reset to None confirmed
- `crates/rc-agent/src/event_loop.rs` — Pass reset + STAFF-04 comment confirmed
- Commit `71e75b7` — exists in git log
- Commit `afed1c2` — exists in git log

---
*Phase: 99-system-network-billing-checks-handler-wiring*
*Completed: 2026-03-21*
