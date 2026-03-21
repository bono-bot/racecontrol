---
phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks
plan: 02
subsystem: agent
tags: [rust, pre-flight, hardware-check, billing-safety, rc-agent, tokio, sysinfo, hidapi]

requires:
  - phase: 97-01
    provides: PreFlightPassed/PreFlightFailed AgentMessage variants + PreflightConfig struct

provides:
  - pre_flight::run() — concurrent check runner with 5s hard timeout
  - pre_flight::CheckResult, CheckStatus, PreFlightResult — public types
  - HID check via FfbBackend::zero_force()
  - ConspitLink two-stage check (process + config.json) with auto-fix
  - Orphan game PID-targeted kill (fix is the check)
  - Pre-flight gate in ws_handler.rs BillingStarted arm
  - billing_active.store(true) protected — never set on pre-flight failure

affects:
  - 97-03 (racecontrol MaintenanceRequired FSM receives PreFlightFailed messages)
  - 98 (ClearMaintenance handler, MaintenanceRequired lock screen state)

tech-stack:
  added: []
  patterns:
    - "tokio::join! with timeout(Duration::from_secs(5)) wrapping concurrent checks"
    - "spawn_blocking for sysinfo::refresh_processes (100-300ms Windows block)"
    - "Orphan game kill: taskkill /F /PID {pid} — never name-based"
    - "ConspitLink auto-fix: spawn + 2s sleep + re-scan in one spawn_blocking call"
    - "mockall mock! defined locally in pre_flight::tests — MockTestBackend is private to ffb_controller::tests"

key-files:
  created:
    - crates/rc-agent/src/pre_flight.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "MockHidBackend defined locally in pre_flight::tests — MockTestBackend from ffb_controller.rs is inside a private mod tests{} and cannot be imported cross-module; local mock! macro is cleaner"
  - "Orphan game state captured before AppState borrow in tokio::join! — game_pid and has_game_process extracted as plain values before the concurrent join to avoid lifetime issues with &AppState across await points"
  - "billing_active.store(true) at line 167 in ws_handler.rs — confirmed AFTER pre_flight gate block (lines 141-165); customers on failed pod never billed"
  - "fix_conspit wrapped in timeout(Duration::from_secs(3)) — 2s sleep + re-scan inside spawn_blocking, outer 3s hard cap prevents runaway"

patterns-established:
  - "Pre-flight gate pattern: check enabled flag -> run() -> match Pass/MaintenanceRequired -> gate billing state"
  - "Send PreFlightFailed to server on failure, return Continue (no session started)"

requirements-completed: [PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01]

duration: 5min
completed: 2026-03-21
---

# Phase 97 Plan 02: pre_flight.rs + ws_handler Gate Summary

**Concurrent pre-flight check runner (HID, ConspitLink, orphan game) with auto-fix + billing_active.store(true) moved inside pre-flight Pass branch so customers are never charged on a maintenance-blocked pod**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-21T04:09:33Z (IST: 09:39)
- **Completed:** 2026-03-21T04:14:50Z (IST: 09:44)
- **Tasks:** 2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

- `crates/rc-agent/src/pre_flight.rs` created with:
  - `pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult`
  - `pub enum CheckStatus { Pass, Warn, Fail }`
  - `pub struct CheckResult { name, status, detail }`
  - `pub enum PreFlightResult { Pass, MaintenanceRequired { failures } }`
  - Three concurrent checks via `tokio::join!` with 5s hard timeout
  - HID: `FfbBackend::zero_force()` — Pass/Fail/Fail on Ok(true)/Ok(false)/Err
  - ConspitLink: `spawn_blocking` sysinfo scan + config.json JSON validation
  - Orphan game: PID-targeted `taskkill /F /PID {pid}` — kill IS the fix
  - ConspitLink auto-fix: spawn process, wait 2s, re-scan (one attempt, 3s timeout)
  - 6 unit tests: HID pass/fail/error, orphan no-proc/billing-active/no-pid
- `mod pre_flight;` added to `main.rs`
- `ws_handler.rs` BillingStarted arm restructured:
  - `billing_active.store(true, ...)` moved AFTER pre-flight gate
  - `failure_monitor_tx.send_modify` also after gate
  - On `MaintenanceRequired`: sends `AgentMessage::PreFlightFailed` to server, returns `Continue`
  - `preflight.enabled=false` bypasses gate entirely (PF-07 backward compat)

## Task Commits

1. **Task 1: Create pre_flight.rs module** - `1064f1f` (feat)
2. **Task 2: Wire pre-flight gate into BillingStarted** - `40467d8` (feat)

## Files Created/Modified

- `crates/rc-agent/src/pre_flight.rs` — New module (391 lines): 3 checks + auto-fix + 6 tests
- `crates/rc-agent/src/main.rs` — `mod pre_flight;` declaration added
- `crates/rc-agent/src/ws_handler.rs` — BillingStarted arm restructured with pre-flight gate

## Decisions Made

- MockHidBackend defined locally in `pre_flight::tests` — `MockTestBackend` from `ffb_controller.rs` lives inside `mod tests {}` which is `#[cfg(test)]` and private to that module; local `mock!` avoids cross-module visibility issues
- Orphan game state (`game_pid`, `has_game_process`) extracted as plain values before `tokio::join!` to avoid `&AppState` lifetime across await points
- `billing_active.store(true)` confirmed at line 167 — after pre_flight gate block (lines 141-165). Customers on a failed pod are never billed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Design] MockTestBackend not importable from pre_flight::tests**
- **Found during:** Task 1 (writing unit tests)
- **Issue:** Plan specified `use crate::ffb_controller::MockTestBackend` but the `mock!` macro in ffb_controller.rs is inside a `#[cfg(test)] mod tests { ... }` block — private, not re-exported
- **Fix:** Defined `mock! { pub HidBackend {} ... }` locally in `pre_flight::tests`, generating `MockHidBackend`. Functionally identical, correct Rust visibility
- **Files modified:** crates/rc-agent/src/pre_flight.rs
- **Impact:** Zero — tests cover identical behavior, mock is correct

## Verification Results

- `cargo test -p rc-agent-crate pre_flight` — 6 tests pass
- `cargo build --bin rc-agent` — compiles with 0 errors
- `cargo build --bin racecontrol` — compiles with 0 errors
- `cargo build --bin rc-sentry` — compiles, stdlib-only constraint intact
- `billing_active.store(true)` at line 167, after gate at lines 141-165 (confirmed)
- `billing_active.store(false)` at lines 200, 225 — BillingStopped and SessionEnded (unaffected)

## Self-Check: PASSED

- `crates/rc-agent/src/pre_flight.rs` — exists (created in Task 1)
- `mod pre_flight` in `crates/rc-agent/src/main.rs` — present
- `pre_flight::run` in `crates/rc-agent/src/ws_handler.rs` — present
- `billing_active.store(true)` after gate — verified line 167 > gate lines 141-165
- Commits `1064f1f` and `40467d8` — exist (pushed to remote)

---
*Phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks*
*Completed: 2026-03-21*
