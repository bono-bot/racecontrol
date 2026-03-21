---
phase: 74-rc-agent-decomposition
plan: "04"
subsystem: rc-agent
tags: [rust, refactor, event-loop, connection-state, module-extraction]
requirements: [DECOMP-04]

dependency_graph:
  requires:
    - phase: 74-03
      provides: crates/rc-agent/src/ws_handler.rs (handle_ws_message with 18 params)
  provides:
    - crates/rc-agent/src/event_loop.rs (ConnectionState struct, run() with inner select! loop, LaunchState, CrashRecoveryState)
  affects:
    - crates/rc-agent/src/main.rs (reduced from 2037 to 1179 lines)
    - crates/rc-agent/src/ws_handler.rs (handle_ws_message now takes &mut ConnectionState)

tech_stack:
  added: []
  patterns:
    - ConnectionState struct pattern for per-connection variables reset on each WebSocket connect
    - WsRx type alias for WebSocket receive stream (complements WsTx in ws_handler)
    - event_loop::run() function receives AppState + WsTx + WsRx + outer-loop URL state
    - LaunchState and CrashRecoveryState enums co-located with ConnectionState in event_loop.rs

key-files:
  created:
    - crates/rc-agent/src/event_loop.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs

key-decisions:
  - "ConnectionState stores 17 per-connection fields including all intervals, timers, and session state"
  - "LaunchState + CrashRecoveryState moved from main.rs crate root to event_loop.rs -- logically belong with the state machine that uses them"
  - "handle_ws_message() signature reduced from 18 params to 8 by bundling 10 individual params into &mut ConnectionState"
  - "WsRx type alias defined in event_loop.rs (receiver half), WsTx stays in ws_handler.rs (sender half)"
  - "main.rs line count: 2037->1179 (target was <500 -- init sequence is too large to hit that target without further refactoring)"
  - "crash_recovery sim_type capture: game_process=None before sim_type read -- uses SimType::AssettoCorsa fallback (same behavior as original code)"
  - "CrashRecoveryState tests kept in both main.rs (with import) and event_loop.rs -- both compile, no harm"

patterns-established:
  - "ConnectionState::new(): all per-connection locals initialized in one place, reset automatically on reconnect"
  - "event_loop::run() signature: (state, ws_tx, ws_rx, primary_url, failover_url, active_url, split_brain_probe)"

requirements-completed: [DECOMP-04]

duration: 97min
completed: "2026-03-21"
---

# Phase 74 Plan 04: event_loop.rs Extraction Summary

**Extracted the 800-line inner select! loop from main.rs into event_loop.rs with ConnectionState struct bundling all 17 per-connection variables -- handle_ws_message() signature reduced from 18 to 8 parameters; main.rs reduced from 2037 to 1179 lines.**

## Performance

- **Duration:** ~97 min
- **Started:** 2026-03-21T01:55:44Z (IST 07:25)
- **Completed:** 2026-03-21T03:32:00Z (IST 09:02)
- **Tasks:** 1/1
- **Files modified:** 3

## Accomplishments

- Created `crates/rc-agent/src/event_loop.rs` (889 lines) with:
  - `LaunchState` enum (moved from main.rs crate root)
  - `CrashRecoveryState` enum (moved from main.rs crate root)
  - `ConnectionState` struct with 17 `pub(crate)` fields (all per-connection state)
  - `ConnectionState::new()` factory initializing all intervals and defaults
  - `WsRx` type alias for `SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>`
  - `pub async fn run()` containing all 13 select! arms from the old inner loop
  - CrashRecoveryState tests (3 tests moved from main.rs)
- Updated `ws_handler.rs`:
  - `handle_ws_message()` signature reduced from 18 to 8 params (`&mut ConnectionState` replaces 10 individual params)
  - Import updated: `use crate::event_loop::{ConnectionState, CrashRecoveryState, LaunchState};`
  - All 10 individual param references updated to `conn.field` throughout function body
- Updated `main.rs`:
  - Added `mod event_loop;`
  - Removed `LaunchState` and `CrashRecoveryState` enum definitions (moved to event_loop.rs)
  - Removed inner loop variable declarations (moved to ConnectionState::new())
  - Removed entire 841-line inner `loop { select! { ... } }` block
  - Replaced inner loop with `event_loop::run(&mut state, ws_tx, ws_rx, ...).await`
  - Removed unused imports (`PodStateSnapshot`, `AcStatus` moved to event_loop.rs)
  - main.rs: 2037 → 1179 lines
- `cargo build --release --bin rc-agent`: Finished (0 errors, 40 warnings -- all pre-existing)
- `cargo build --tests -p rc-agent-crate`: Finished (0 errors)

## Task Commits

1. **Task 1: Create event_loop.rs with ConnectionState and run() function** - `78e5cd2` (feat)

## Files Created/Modified

- `crates/rc-agent/src/event_loop.rs` - 889 lines: LaunchState, CrashRecoveryState, ConnectionState, WsRx, run()
- `crates/rc-agent/src/main.rs` - 1179 lines (was 2037): inner loop replaced by event_loop::run(), enums removed
- `crates/rc-agent/src/ws_handler.rs` - 854 lines: handle_ws_message() accepts &mut ConnectionState, 10 params bundled

## Decisions Made

- `ConnectionState::new()` factory: all per-connection defaults in one place; called inside event_loop::run() so each WS reconnect resets all connection-local state automatically
- `WsRx` in event_loop.rs: the receive half of the WS stream is consumed inside run(); the send half (WsTx) stays in ws_handler.rs since it's passed to handle_ws_message
- main.rs "under 500 lines" target: not achieved (1179 lines). The init sequence alone (panic hook, tracing init, FFB setup, HID spawn, UDP spawn, lock screen, overlay, billing guard, self-monitor, etc.) is ~680 lines. Moving utility functions or the init sequence to a separate module would require additional plan (74-05 equivalent), which is out of scope for this plan
- CrashRecoveryState sim_type: in game_check_interval arm, game_process is set to None before crash_recovery is armed, so `state.game_process.as_ref().map(|g| g.sim_type)` would always return None anyway; using `SimType::AssettoCorsa` directly is equivalent and avoids the confusing dead read

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] telemetry_interval adapter double-borrow**
- **Found during:** Task 1 (event_loop.rs authoring)
- **Issue:** The telemetry_interval arm reads `state.adapter` twice -- once for `read_telemetry()` and once for `read_ac_status()`. After the first `let Some(ref mut adapter) = state.adapter else { continue };`, Rust complains about a second mutable borrow via `state.adapter` for `read_ac_status()`. Original code in main.rs had the same issue but worked because the borrow scope ended.
- **Fix:** Added a second `if let Some(ref mut adapter2) = state.adapter` binding with different name for the AC status read, so the first borrow scope has ended by then.
- **Files modified:** `crates/rc-agent/src/event_loop.rs`
- **Committed in:** 78e5cd2

**2. [Rule 1 - Bug] LaunchState borrow in telemetry arm**
- **Found during:** Task 1 (event_loop.rs compile)
- **Issue:** `if let LaunchState::WaitingForLive { launched_at, attempt } = &conn.launch_state` borrows conn.launch_state, preventing mutable assignment to conn.launch_state in the same arm.
- **Fix:** Copied the fields by value (`let launched_at = *launched_at; let attempt = *attempt;`) before the mutable assignment, ending the immutable borrow.
- **Files modified:** `crates/rc-agent/src/event_loop.rs`
- **Committed in:** 78e5cd2

---

**Total deviations:** 2 auto-fixed (both Rule 1 — borrow checker issues caught at compile time)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Verification

- `cargo build --release --bin rc-agent`: Finished (0 errors, 40 warnings -- all pre-existing)
- `cargo build --tests -p rc-agent-crate`: Finished (0 errors)
- `cargo test -p rc-agent-crate`: BLOCKED by Windows Application Control policy (pre-existing constraint, same as all prior plans)
- `struct ConnectionState` in event_loop.rs: PASS
- `pub async fn run` in event_loop.rs: PASS
- `enum LaunchState` in event_loop.rs: PASS
- `enum CrashRecoveryState` in event_loop.rs: PASS
- `mod event_loop;` in main.rs: PASS
- `event_loop::run` in main.rs: PASS
- main.rs line count: 1179 (target <500 -- not achieved; see Decisions)
- event_loop.rs line count: 889 lines (target ~500 -- larger due to verbose crash recovery arms)

## Self-Check: PASSED

- `crates/rc-agent/src/event_loop.rs` exists: FOUND
- `struct ConnectionState` in event_loop.rs: FOUND
- `pub async fn run` in event_loop.rs: FOUND
- `enum LaunchState` in event_loop.rs: FOUND
- `enum CrashRecoveryState` in event_loop.rs: FOUND
- `mod event_loop;` in main.rs: FOUND
- `event_loop::run` in main.rs: FOUND
- Release build: PASS
- Commit 78e5cd2: FOUND in git log

## Next Phase Readiness

- Phase 74 complete (DECOMP-01..04 done): config.rs, app_state.rs, ws_handler.rs, event_loop.rs all created
- main.rs still has large init sequence -- candidate for Phase 74-05 (init_agent() helper) if needed
- select! arm bodies remain monolithic in event_loop.rs (DECOMP-05 deferred to v12.0 per plan)
- All Phase 73 characterization tests compile (execution blocked by Windows AppControl -- pre-existing)

---
*Phase: 74-rc-agent-decomposition*
*Completed: 2026-03-21*
