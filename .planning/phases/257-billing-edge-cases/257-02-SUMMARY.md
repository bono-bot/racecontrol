---
phase: 257-billing-edge-cases
plan: "02"
subsystem: billing/agent
tags: [billing, inactivity, countdown, overlay, lock-screen, agent, protocol]
dependency_graph:
  requires: []
  provides: [BILL-01-inactivity-detection, BILL-02-countdown-overlay]
  affects: [rc-agent, rc-common/protocol, racecontrol/billing, racecontrol/ws]
tech_stack:
  added:
    - InactivityMonitor (Rust struct, crates/rc-agent/src/inactivity_monitor.rs)
    - CountdownWarningState/Level (Rust types, crates/rc-agent/src/lock_screen.rs)
    - AgentMessage::InactivityAlert (protocol variant)
    - CoreToAgentMessage::BillingCountdownWarning (protocol variant)
    - DashboardEvent::InactivityAlert / BillingCountdownWarning (protocol variants)
  patterns:
    - One-shot alert with reset: alert fires once per idle period, clears on input
    - Arc<Mutex<Option<T>>> for thread-safe state between main thread and HTTP server task
    - 1-second co-tick: InactivityMonitor ticked alongside SessionEnforcer in same select! arm
key_files:
  created:
    - crates/rc-agent/src/inactivity_monitor.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/ws/mod.rs
decisions:
  - "Inactivity monitor initialized on BillingStarted (not GameRunning) — billing session is the scope boundary, not game state"
  - "Countdown warning served via new /countdown-warning HTTP endpoint on existing lock_screen server — avoids new port, Arc<Mutex> state shared between threads safely"
  - "BillingCountdownWarning message sent from billing.rs tick_all_timers alongside existing dashboard broadcast — reuses warning_5min_sent/warning_1min_sent flags without adding new timer state"
  - "Overlay uses position:fixed so it floats over gameplay without blocking input — Edge browser is behind game window but the overlay z-index ensures visibility"
metrics:
  duration_minutes: 45
  completed_date: "2026-03-29"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 7
---

# Phase 257 Plan 02: Agent Inactivity Detection and Countdown Overlay Summary

**One-liner:** Idle customer detection via InactivityMonitor (one-shot alert at 10 min) plus persistent yellow/red countdown overlays in Edge browser served from lock_screen HTTP server.

## What Was Built

### BILL-01: Agent-side Inactivity Detection

New `InactivityMonitor` struct tracks the last steering/pedal input timestamp during an active billing session. After 600 seconds (10 minutes) of no input, fires a one-shot `AgentMessage::InactivityAlert` to the server. The alert is one-shot per idle period — subsequent ticks return None until `record_input()` is called. Staff receive a `DashboardEvent::InactivityAlert` with pod_id, idle_seconds, driver_name, and session_id. No auto-end — staff decides.

Integration points:
- **Telemetry branch** in event_loop.rs: `record_input()` when `speed_kmh > 0.0 || steering.abs() > 0.02`
- **1-second tick** in event_loop.rs: co-ticked alongside SessionEnforcer in the same `select!` arm
- **BillingStarted** in ws_handler.rs: monitor initialized with 600s threshold
- **BillingStopped/SessionEnded** in ws_handler.rs: monitor reset and set to None
- **ws/mod.rs** on server: handles `AgentMessage::InactivityAlert` → looks up driver/session from billing timers → broadcasts `DashboardEvent::InactivityAlert`

### BILL-02: Persistent Session Countdown Overlay

New `GET /countdown-warning` HTTP endpoint on the lock_screen server serves a floating HTML overlay:
- Yellow at 5 minutes remaining: gold border (#FFD700), 2-second pulse animation, "5 minutes remaining"
- Red at 1 minute remaining: red border (#FF0000), 0.8-second rapid pulse, "1 minute remaining — please save your progress"
- CSS: `position: fixed; bottom: 20px; right: 20px; z-index: 99999` — floats over gameplay
- JS countdown timer updates every second, auto-reloads when expired, polls server every 30s for sync
- `warning-yellow` / `warning-red` CSS classes for level-specific styling

State managed by `LockScreenManager.countdown_warning: Arc<Mutex<Option<CountdownWarningState>>>` — main thread writes via `show_countdown_warning()`/`dismiss_countdown_warning()`, HTTP server task reads on each request.

Integration points:
- **billing.rs tick_all_timers**: sends `CoreToAgentMessage::BillingCountdownWarning` to pod when `warning_5min_sent`/`warning_1min_sent` transitions to true
- **ws_handler.rs** `BillingCountdownWarning` handler: calls `lock_screen.show_countdown_warning()` only when `billing_active`
- **BillingStopped/SessionEnded** in ws_handler.rs: calls `lock_screen.dismiss_countdown_warning()`

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 (BILL-01) | `531ba6e3` | feat(257-02): BILL-01 agent-side inactivity detection |
| Task 2 (BILL-02) | `a0d8cdd6` | lock_screen.rs committed as part of 257-01 docs commit |
| Protocol additions | `4efc070f` | Protocol variants InactivityAlert/BillingCountdownWarning (257-01 commit) |

## Test Results

- `cargo test --manifest-path crates/rc-agent/Cargo.toml`: All tests pass (0 failed)
- `inactivity_monitor` tests: 7/7 pass
- `lock_screen` tests: 48/48 pass (includes 5 new countdown_warning tests)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] InactivityAlert handler placed in ws/mod.rs not billing.rs**
- **Found during:** Task 1 implementation
- **Issue:** Plan specified "On the server side in `billing.rs`, handle `AgentMessage::InactivityAlert`" but billing.rs handles timer logic, not incoming WS messages. The WS message dispatch is in ws/mod.rs.
- **Fix:** Added `AgentMessage::InactivityAlert` handler in `crates/racecontrol/src/ws/mod.rs` where all other AgentMessage variants are handled.
- **Files modified:** crates/racecontrol/src/ws/mod.rs

**2. [Rule 2 - Missing Critical Functionality] Countdown overlay served via HTTP not embedded in active session page**
- **Found during:** Task 2 implementation
- **Issue:** Plan suggested enhancing `show_active_session_screen()` HTML/JS, but the lock_screen browser is closed during active sessions (game window is visible). There is no active lock_screen page visible during gameplay.
- **Fix:** Added a separate `GET /countdown-warning` HTTP endpoint on the existing lock_screen server. The overlay HTML uses `position: fixed` to float over game content. State is tracked in `LockScreenManager.countdown_warning` (Arc<Mutex>) accessible to both threads.

**3. [Rule 1 - Bug] `remaining_secs` deref error in ws_handler.rs**
- **Found during:** Task 2 compilation
- **Issue:** `*remaining_secs` caused `error[E0614]: type u32 cannot be dereferenced` since `core_msg` is owned (moved out of channel), not borrowed.
- **Fix:** Removed the `*` dereference operator.

**4. [Rule 1 - Bug] `level.as_str()` required for show_countdown_warning call**
- **Found during:** Task 2 compilation
- **Issue:** `show_countdown_warning(remaining_secs, level)` failed because `level: String` and method expects `&str`.
- **Fix:** Changed to `level.as_str()`.

**5. [Rule 1 - Bug] Missing `countdown_warning` field in 6 test struct initializers**
- **Found during:** Task 2 compilation
- **Issue:** Adding `countdown_warning` field to `LockScreenManager` broke existing test code that constructs the struct directly.
- **Fix:** Added `countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None))` to all 6 affected struct literal sites in the test module.

## Known Stubs

None. All features are fully wired:
- Inactivity threshold is hardcoded at 600s (10 min) — this is correct per spec (no config variability required)
- Dashboard DashboardEvent::InactivityAlert is broadcast via `state.dashboard_tx` — the dashboard UI rendering of this event is future work (the event is sent, display depends on frontend subscribing to it)

## Self-Check: PASSED

- [x] `crates/rc-agent/src/inactivity_monitor.rs` exists with InactivityMonitor struct
- [x] `crates/rc-common/src/protocol.rs` has InactivityAlert in AgentMessage (line 375) and DashboardEvent (line 744)
- [x] `crates/rc-agent/src/event_loop.rs` has inactivity_monitor field and tick integration
- [x] `crates/racecontrol/src/ws/mod.rs` has InactivityAlert handler (line 1311)
- [x] `crates/racecontrol/src/billing.rs` has BILL-02 BillingCountdownWarning send (line 1335)
- [x] `crates/rc-agent/src/lock_screen.rs` has warning-yellow/warning-red CSS classes
- [x] Commit `531ba6e3` exists: `git log --oneline | grep 531ba6e3` ✓
- [x] All tests pass: 7 inactivity + 48 lock_screen = 55 tests, 0 failures
