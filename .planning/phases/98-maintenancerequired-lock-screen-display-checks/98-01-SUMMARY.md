---
phase: 98-maintenancerequired-lock-screen-display-checks
plan: 01
subsystem: agent
tags: [rust, lock-screen, maintenance, pre-flight, ws-handler, atomic-bool]

requires:
  - phase: 97-02
    provides: PreFlightResult::MaintenanceRequired + PreFlightFailed AgentMessage + ClearMaintenance CoreToAgentMessage

provides:
  - LockScreenState::MaintenanceRequired variant with branded HTML renderer
  - show_maintenance_required() + is_maintenance_required() methods on LockScreenManager
  - AppState::in_maintenance Arc<AtomicBool> flag
  - ws_handler ClearMaintenance handler restoring pod to idle
  - is_idle_or_blanked() and health_response_body() recognize MaintenanceRequired as degraded/idle

affects:
  - lock_screen.rs — new variant, 3 new methods, render function, health/idle updates
  - app_state.rs — new in_maintenance field
  - main.rs — in_maintenance initialization
  - ws_handler.rs — Phase 98 comment replaced, ClearMaintenance handler added
  - debug_server.rs — state_name match extended (auto-fix)

tech-stack:
  added: []
  patterns:
    - "show_lockdown() pattern copied exactly for show_maintenance_required()"
    - "Arc<AtomicBool> for cross-thread maintenance flag (Ordering::Relaxed)"
    - "failure_strings.clone() before move into AgentMessage — keep original for lock screen"
    - "html_escape() on each failure string in render_maintenance_required_page()"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/app_state.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/src/debug_server.rs

key-decisions:
  - "debug_server.rs state_name match needed MaintenanceRequired arm (Rule 1 auto-fix) — enum exhaustiveness failure caught during compilation"
  - "failure_strings cloned before send into AgentMessage::PreFlightFailed — original used for show_maintenance_required()"
  - "ClearMaintenance inserted before 'other =>' arm — clean separation from unhandled message logging"

metrics:
  duration: 11min
  completed: 2026-03-21
  tasks: 2
  files_modified: 5
---

# Phase 98 Plan 01: MaintenanceRequired Lock Screen + AppState Flag Summary

**MaintenanceRequired LockScreenState variant with branded Racing Red HTML renderer, in_maintenance AtomicBool on AppState, and ws_handler wiring to show maintenance screen on pre-flight failure and clear on ClearMaintenance**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-21T04:35:32Z (IST: 10:05)
- **Completed:** 2026-03-21T04:47:06Z (IST: 10:17)
- **Tasks:** 2
- **Files modified:** 5 (0 created, 5 modified)

## Accomplishments

### Task 1: MaintenanceRequired LockScreenState variant + methods + HTML renderer

- `LockScreenState::MaintenanceRequired { failures: Vec<String> }` variant added as 14th enum variant
- `show_maintenance_required(&mut self, failures: Vec<String>)` — follows show_lockdown() pattern exactly
- `is_maintenance_required(&self) -> bool` — query method using matches! macro
- `is_idle_or_blanked()` extended: `LockScreenState::MaintenanceRequired { .. }` added (pod not serving customer)
- `render_page()` match: `LockScreenState::MaintenanceRequired { failures } => render_maintenance_required_page(failures)` arm added
- `render_maintenance_required_page(failures: &[String]) -> String` — branded HTML with:
  - Racing Red #E10600 "MAINTENANCE REQUIRED" header in Enthocentric font
  - "Staff have been notified. This pod is temporarily unavailable." in white
  - Bullet list of html_escape()-sanitized failure strings
  - "This pod will automatically recover once the issue is resolved." in Gunmetal Grey #5A5A5A
  - 5-second auto-reload via setTimeout
- `health_response_body()`: `MaintenanceRequired { .. }` added to degraded list
- 3 unit tests added and passing:
  - `maintenance_required_renders_html`
  - `health_degraded_for_maintenance_required`
  - `maintenance_required_is_idle_or_blanked`

### Task 2: in_maintenance AtomicBool + ws_handler wiring

- `app_state.rs`: `pub(crate) in_maintenance: std::sync::Arc<std::sync::atomic::AtomicBool>` field added
- `main.rs`: initialized as `Arc::new(AtomicBool::new(false))` in AppState struct literal
- `ws_handler.rs` PreFlightFailed branch:
  - `failure_strings.clone()` used in AgentMessage send; original moved to `show_maintenance_required()`
  - Phase 98 placeholder comment replaced with actual implementation
- `ws_handler.rs` ClearMaintenance handler:
  - `state.in_maintenance.store(false, Ordering::Relaxed)`
  - `state.lock_screen.show_idle_pin_entry()` returns pod to ready state

## Task Commits

1. **Task 1 RED: Failing tests** - `0dedde2` (test)
2. **Task 1 GREEN: Implementation** - `6ba5372` (feat)
3. **Task 2: AppState + ws_handler wiring** - `cb79088` (feat)

## Files Created/Modified

- `crates/rc-agent/src/lock_screen.rs` — +48 lines: variant, 3 methods, render fn, 3 tests, health/idle updates
- `crates/rc-agent/src/app_state.rs` — +1 field: in_maintenance Arc<AtomicBool>
- `crates/rc-agent/src/main.rs` — +1 line: in_maintenance initialization
- `crates/rc-agent/src/ws_handler.rs` — +8 lines: failure_strings.clone(), show_maintenance_required, in_maintenance.store, ClearMaintenance handler
- `crates/rc-agent/src/debug_server.rs` — +1 line: "maintenance_required" arm in state_name match (auto-fix)

## Decisions Made

- `failure_strings.clone()` before send into AgentMessage — keeps original for show_maintenance_required() without re-collecting from `failures`
- ClearMaintenance handler inserted before `other =>` wildcard arm — ergonomic, no risk of shadowing
- debug_server.rs maintenance_required arm added as Rule 1 auto-fix — exhaustiveness check caught immediately on first compile

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Non-exhaustive pattern in debug_server.rs state_name match**
- **Found during:** Task 1 GREEN step (first compile after adding variant)
- **Issue:** `debug_server.rs` line 80 has an exhaustive match on LockScreenState; adding MaintenanceRequired variant broke compilation
- **Fix:** Added `LockScreenState::MaintenanceRequired { .. } => "maintenance_required"` arm to the state_name match
- **Files modified:** `crates/rc-agent/src/debug_server.rs`
- **Commit:** Included in `6ba5372`
- **Impact:** Zero — debug server correctly reports state name for monitoring

## Verification Results

- `cargo test -p rc-agent-crate lock_screen::tests` — 36 tests pass (35 + 1 self_test)
- `cargo test -p rc-agent-crate pre_flight` — 6 tests pass (pre-existing, unaffected)
- `cargo build --bin rc-agent` — compiles with 0 errors, 42 warnings (all pre-existing)
- `grep -c "MaintenanceRequired" lock_screen.rs` — 14 (>= 8 required)
- `grep -c "Phase 98 will add" ws_handler.rs` — 0 (comment replaced)
- `grep -c "ClearMaintenance" ws_handler.rs` — 2 (>= 1)
- `grep -c "in_maintenance" ws_handler.rs` — 2 (>= 2)

## Self-Check: PASSED

- `crates/rc-agent/src/lock_screen.rs` — MaintenanceRequired variant present (14 occurrences)
- `crates/rc-agent/src/app_state.rs` — in_maintenance field present
- `crates/rc-agent/src/main.rs` — in_maintenance initialization present
- `crates/rc-agent/src/ws_handler.rs` — show_maintenance_required + ClearMaintenance handler present
- `crates/rc-agent/src/debug_server.rs` — maintenance_required arm present
- Commits `0dedde2`, `6ba5372`, `cb79088` — all exist in git log

---
*Phase: 98-maintenancerequired-lock-screen-display-checks*
*Completed: 2026-03-21*
