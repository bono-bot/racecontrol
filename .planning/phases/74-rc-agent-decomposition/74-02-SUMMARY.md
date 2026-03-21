---
phase: 74-rc-agent-decomposition
plan: "02"
subsystem: rc-agent
tags: [rust, refactor, struct-extraction, app-state]
requirements: [DECOMP-02]

dependency_graph:
  requires:
    - phase: 74-01
      provides: crates/rc-agent/src/config.rs (AgentConfig and all config types)
  provides:
    - crates/rc-agent/src/app_state.rs (AppState struct with 34 pub(crate) fields)
  affects:
    - crates/rc-agent/src/main.rs
    - "74-03 (ws_handler.rs extraction — will use &mut AppState)"
    - "74-04 (event_loop.rs extraction — will use AppState as parameter)"

tech_stack:
  added: []
  patterns:
    - AppState struct pattern for bundling pre-loop agent state across WS reconnections
    - crash_recovery (bool startup flag) renamed to crash_recovery_startup to avoid name collision with CrashRecoveryState inner-loop local
    - QueryAssistState local variable renamed to assist_msg to avoid shadowing AppState

key-files:
  created:
    - crates/rc-agent/src/app_state.rs
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "AppState fields all pub(crate) not pub — crate-internal, not public API (matches config.rs pattern)"
  - "crash_recovery bool renamed crash_recovery_startup in AppState — avoids collision with CrashRecoveryState inner-loop local"
  - "SelfHealResult type name (not HealResult) — self_heal.rs uses SelfHealResult struct name"
  - "AiDebugSuggestion imported from rc_common::types (not ai_debugger) — already a shared type"
  - "ws_tx, ws_rx remain loop-local (borrow conflict rule from RESEARCH.md Pitfall)"
  - "reconnect_attempt, startup_complete_logged, startup_report_sent, ws_disconnected_at remain outer-loop locals (control flow, not reconnect-surviving state)"
  - "active_url, primary_url, failover_url, split_brain_probe remain outer-loop locals (URL switching state, not in AppState per plan)"
  - "launch_state remains inner-loop local (re-initialized each connection; moves to event_loop.rs in Plan 74-04)"
  - "QueryAssistState uses assist_msg local instead of state to avoid shadowing outer AppState binding"

patterns-established:
  - "AppState: all pre-loop variables that survive WebSocket reconnections bundled into single struct"
  - "state.field prefix pattern throughout reconnect loop — consistent with Rust struct access"

requirements-completed: [DECOMP-02]

duration: 58min
completed: "2026-03-21"
---

# Phase 74 Plan 02: AppState Extraction Summary

**Bundled 34 pre-loop agent variables into AppState struct in app_state.rs; all reconnect loop references updated to state.field pattern — enabling event_loop::run() to receive a single parameter in Plan 74-04.**

## Performance

- **Duration:** ~58 min
- **Started:** 2026-03-21T07:00:00Z (IST 12:30)
- **Completed:** 2026-03-21T07:58:00Z (IST 13:28)
- **Tasks:** 1/1
- **Files modified:** 2

## Accomplishments

- Created `crates/rc-agent/src/app_state.rs` with AppState struct containing 34 `pub(crate)` fields
- Added `mod app_state;` and `use app_state::AppState;` to main.rs
- Inserted AppState construction after all pre-loop variable initialization (before reconnect loop)
- Updated all 200+ references in the reconnect loop (outer + inner) to use `state.field` pattern
- Resolved naming conflict: `crash_recovery` bool renamed `crash_recovery_startup`; `QueryAssistState` local renamed `assist_msg`
- Fixed import errors: `SelfHealResult` (not `HealResult`), `AiDebugSuggestion` from `rc_common::types`

## Task Commits

1. **Task 1: Create app_state.rs with AppState struct and update main.rs** - `4c7a591` (feat)

## Files Created/Modified

- `crates/rc-agent/src/app_state.rs` - AppState struct with 34 pre-loop fields (pod_id, config, ffb, detector, adapter, kiosk, lock_screen, overlay, channels, heartbeat_status, etc.)
- `crates/rc-agent/src/main.rs` - Added mod app_state + AppState construction; all reconnect loop references updated to state.field

## Decisions Made

- `SelfHealResult` (not `HealResult`) — self_heal.rs defines it as `pub struct SelfHealResult`
- `AiDebugSuggestion` from `rc_common::types` not `ai_debugger` — shared type already in common
- `crash_recovery` (startup bool) renamed to `crash_recovery_startup` — avoids collision with `CrashRecoveryState` inner-loop local
- `QueryAssistState` response local renamed `assist_msg` — avoids shadowing `state` (AppState binding)
- `ws_tx`/`ws_rx` stay loop-local (borrow conflict per RESEARCH.md — can't be in AppState)
- `active_url`/`primary_url`/`failover_url`/`split_brain_probe` stay outer-loop locals (URL switching, not reconnect-surviving state per plan)
- `launch_state` stays inner-loop local (per plan; moves to `event_loop.rs` in Plan 74-04)
- `reconnect_attempt`/`startup_complete_logged`/`startup_report_sent`/`ws_disconnected_at` stay outer-loop locals (control flow)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Wrong type name for HealResult**
- **Found during:** Task 1 (app_state.rs compilation)
- **Issue:** Plan used `self_heal::HealResult` but self_heal.rs defines `pub struct SelfHealResult`
- **Fix:** Used `SelfHealResult` in app_state.rs imports and struct field
- **Files modified:** `crates/rc-agent/src/app_state.rs`
- **Committed in:** 4c7a591

**2. [Rule 1 - Bug] Wrong module path for AiDebugSuggestion**
- **Found during:** Task 1 (app_state.rs compilation)
- **Issue:** Plan used `crate::ai_debugger::AiDebugSuggestion` (private) but it lives in `rc_common::types`
- **Fix:** Imported from `rc_common::types::AiDebugSuggestion`
- **Files modified:** `crates/rc-agent/src/app_state.rs`
- **Committed in:** 4c7a591

**3. [Rule 1 - Bug] Variable name shadowing in QueryAssistState handler**
- **Found during:** Task 1 (inner loop update)
- **Issue:** `let state = rc_common::protocol::AgentMessage::AssistState` shadowed the `state: AppState` binding
- **Fix:** Renamed local to `assist_msg`
- **Files modified:** `crates/rc-agent/src/main.rs`
- **Committed in:** 4c7a591

---

**Total deviations:** 3 auto-fixed (all Rule 1 — type/name bugs caught at compile time)
**Impact on plan:** All fixes necessary for compilation. No scope creep.

## Verification

- `cargo build --bin rc-agent`: Finished (0 errors, 43 warnings — pre-existing)
- `cargo build --bin rc-sentry`: Finished (0 errors, no tokio contamination)
- `cargo build --tests -p rc-agent-crate`: Finished (0 errors)
- Test binary execution blocked by Windows Application Control policy (pre-existing)
- All 5 acceptance criteria: PASS (grep confirms pub struct AppState, pod_id, game_process, heartbeat_status fields, mod app_state; in main.rs)
- 34 `pub(crate)` fields (requirement: 30+): PASS

## Self-Check: PASSED

- `crates/rc-agent/src/app_state.rs` exists: FOUND
- `mod app_state;` in main.rs: FOUND
- `pub struct AppState` in app_state.rs: FOUND
- `pub(crate) pod_id:` in app_state.rs: FOUND
- `pub(crate) game_process:` in app_state.rs: FOUND
- `pub(crate) heartbeat_status:` in app_state.rs: FOUND
- `grep -c "pub(crate)"` = 34 (>30): CONFIRMED
- Commit 4c7a591: FOUND in git log

## Next Phase Readiness

- Plan 74-03 (ws_handler.rs extraction): AppState available as `&mut AppState` parameter
- Plan 74-04 (event_loop.rs extraction): ConnectionState struct for inner-loop locals + AppState for outer state
- PANIC statics remain in main.rs per Pitfall 6 (PANIC_HOOK_ACTIVE, PANIC_LOCK_STATE)
- WS_MAX_CONCURRENT_EXECS, WS_EXEC_SEMAPHORE, handle_ws_exec remain in main.rs (move to ws_handler.rs in 74-03)

---
*Phase: 74-rc-agent-decomposition*
*Completed: 2026-03-21*
