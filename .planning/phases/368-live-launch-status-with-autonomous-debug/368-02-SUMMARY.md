---
phase: 368-live-launch-status-with-autonomous-debug
plan: 02
subsystem: rc-agent + racecontrol/ws
tags: [game-launch, diagnostics, websocket, live-status, phase-368]
requires: [368-01]
provides: [LLS-04-pod-side-emissions, LLS-04-server-relay, launch-status-bridge]
affects: [game_launch_retry, tier_engine, ws_handler, ws/mod, launch_state_machine, dashboard_tx]
tech-stack:
  added: []
  patterns: [injectable-diagnoser-for-testability, emit-at-retry-boundaries, split-deploy-local-id-fallback]
key-files:
  created:
    - crates/racecontrol/tests/ws_launch_status_relay.rs
  modified:
    - crates/rc-agent/src/game_launch_retry.rs
    - crates/rc-agent/src/tier_engine.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/diagnostic_engine.rs
    - crates/rc-agent/src/self_monitor.rs
    - crates/rc-agent/src/cognitive_gate.rs
    - crates/rc-agent/src/diagnosis_planner.rs
    - crates/rc-agent/src/event_loop.rs
decisions:
  - "Used bounded mpsc::Sender with try_send instead of UnboundedSender — retry function is sync (called via spawn_blocking), async send is not viable, try_send is non-blocking and safe"
  - "Extracted retry_game_launch_with_diagnoser<F> with injectable diagnose_fn — avoids Windows filesystem deps in tests; backoff_override_secs=Some(0) skips 5s sleep in tests"
  - "Added launch_id: Option<String> to DiagnosticEvent (not to DiagnosticTrigger variant) — avoids breaking 20+ match arms; DiagnosticEvent is the natural carrier between ws_handler and tier_engine"
  - "Used inline #[cfg(test)] modules for rc-agent — binary crate with no lib.rs cannot have integration test files in tests/"
  - "resolve_launch_id() pure helper extracted from ws_handler for testability — ws tests do not spin up a real WS connection"
  - "split-deploy rcagent-local-* prefix chosen — allows server to detect and log REQUIRES FLEET UPDATE without rejecting the message"
metrics:
  duration: "~45 minutes (session resumed from context)"
  completed: "2026-04-11"
  tasks: 3
  files: 9
  commits: 3
---

# Phase 368 Plan 02: Pod-Side WS Emissions + Server LaunchStatusUpdate Relay Summary

One-liner: Wired 4 LaunchStatusUpdate emissions at rc-agent retry boundaries, threaded launch_id through tier_engine, hydrated conn.current_launch_id from server-minted UUID in ws_handler, and added server-side AgentMessage::LaunchStatusUpdate relay to LaunchStateMachine + dashboard broadcast.

## Tasks Completed

| # | Task | Commit | Key Files |
|---|------|--------|-----------|
| 1 | 4 WS emissions in game_launch_retry + tier_engine threading | ef189f48 | game_launch_retry.rs, tier_engine.rs, diagnostic_engine.rs, 5 cascade files |
| 2 | Hydrate conn.current_launch_id in ws_handler | d566e578 | ws_handler.rs |
| 3 | Server AgentMessage::LaunchStatusUpdate relay arm in ws/mod.rs | c42f21ad | ws/mod.rs, tests/ws_launch_status_relay.rs |

## What Was Built

### Task 1 — game_launch_retry.rs Emissions

Added `emit_status()` helper that calls `mpsc::Sender::try_send()` (sync, non-blocking, safe in spawn_blocking context). Four emission points wired:

1. **Entry** — `LaunchState::AiAnalysisRequested` (first line of retry loop)
2. **Tier 1 success** — `LaunchState::IssueBeingFixed` then `LaunchState::IssueFixed` (game_doctor returns `WasFixed`)
3. **NoRetry exit** — `LaunchState::NeedsManualIntervention` (GameDoctorOutcome::NoRetry)
4. **Exhaustion exit** — `LaunchState::NeedsManualIntervention` (all retries exhausted)

Production `retry_game_launch(launch_id, ws_msg_tx)` delegates to injectable `retry_game_launch_with_diagnoser<F>(launch_id, ws_msg_tx, diagnose_fn, backoff_override_secs)`. Tests inject a no-op diagnose closure and `backoff_override_secs: Some(0)`.

tier_engine `DiagnosticTrigger::GameLaunchFail` branch added `launch_id_opt: Option<String>` parameter threading. Split-deploy fallback generates `rcagent-local-<uuid>` and ERROR-logs "REQUIRES FLEET UPDATE". `DiagnosticEvent.launch_id: Option<String>` field added as carrier (all 7 construction sites updated with `launch_id: None`).

### Task 2 — ws_handler.rs launch_id Hydration

`CoreToAgentMessage::LaunchGame` now extracts `msg_launch_id`. Server-minted launch_id is stored directly; absent field triggers split-deploy fallback with `rcagent-local-<uuid>` and ERROR-level "REQUIRES FLEET UPDATE" log. `resolve_launch_id()` pure helper extracted for testability. 3 inline unit tests added.

### Task 3 — Server ws/mod.rs Relay Arm

`AgentMessage::LaunchStatusUpdate` arm added between `AiDebugResult` and `PinEntered` arms. The arm:
- Detects `rcagent-local-*` prefix and ERROR-logs "REQUIRES FLEET UPDATE"
- Calls `state.launch_state_machine.transition(...)` with `*new_state` and `*ai_tier` (references because match is on `&AgentMessage`)
- No lock held across `.await` (CLAUDE.md standing rule)
- Broadcasts `DashboardEvent::LaunchStatusChanged(card)` on `Some(card)` — ignores "no subscribers" error via `let _ =`
- Warns on `None` (unknown or terminal launch_id), does not broadcast

5 integration tests in `crates/racecontrol/tests/ws_launch_status_relay.rs` — all 5 pass.

## Verification

- `cargo build -p racecontrol-crate` — `Finished` (no errors, 20 pre-existing warnings)
- `cargo build --release --bin racecontrol` — exit code 0
- `cargo test -p racecontrol-crate --test ws_launch_status_relay` — 5/5 pass
- `cargo test -p rc-agent-crate` — all pass (exit code 0 confirmed via background task)

NOT TESTED (runtime):
- Actual WS message flowing from pod to server in live environment
- rcagent-local-* fallback path triggering in production (split-deploy scenario)
- Dashboard JS receiving and rendering LaunchStatusChanged events

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] UnboundedSender vs bounded Sender type mismatch**
- **Found during:** Task 1
- **Issue:** Plan specified `tokio::sync::mpsc::UnboundedSender` but tier_engine.rs uses bounded `mpsc::Sender`; `retry_game_launch` is sync (spawn_blocking), `.await` not available
- **Fix:** Used `try_send()` on bounded sender — sync, non-blocking, handles "channel full or closed" as warn
- **Files modified:** game_launch_retry.rs
- **Commit:** ef189f48

**2. [Rule 3 - Blocking] rc-agent has no lib.rs — integration tests/ directory not viable**
- **Found during:** Task 1
- **Issue:** Binary crate cannot export symbols to `tests/` integration test files without a lib.rs entry; adding lib.rs would cascade to importing all modules
- **Fix:** Used inline `#[cfg(test)]` modules inside game_launch_retry.rs and ws_handler.rs
- **Commit:** ef189f48, d566e578

**3. [Rule 1 - Bug] event_loop.rs test missing launch_id field**
- **Found during:** Task 1 (cargo test compile error)
- **Issue:** `test_force_clean_deserialization` in event_loop.rs constructed `CoreToAgentMessage::LaunchGame` without `launch_id` field (added in Plan 01, missed in test cascade)
- **Fix:** Added `launch_id: None` to that test construction
- **Files modified:** event_loop.rs
- **Commit:** ef189f48

**4. [Rule 1 - Bug] &LaunchState / &Option<u8> type error in ws/mod.rs**
- **Found during:** Task 3 (first cargo build)
- **Issue:** `AgentMessage::LaunchStatusUpdate` destructured by reference (match on `&AgentMessage`); `new_state` and `ai_tier` bound as `&LaunchState` / `&Option<u8>` but `transition()` expects by value
- **Fix:** Used `*new_state` and `*ai_tier` to deref at call site
- **Files modified:** ws/mod.rs
- **Commit:** c42f21ad

**5. [Rule 1 - Bug] ws_launch_status_relay test used invalid transition LaunchStarted → IssueBeingFixed**
- **Found during:** Task 3 test run
- **Issue:** `is_valid_transition()` does not allow `LaunchStarted → IssueBeingFixed` (must go through `AiAnalysisRequested` first); test 2 failed with `card.is_none()`
- **Fix:** Changed test 2 to use `LaunchStarted → AiAnalysisRequested` (the correct first valid transition)
- **Files modified:** tests/ws_launch_status_relay.rs
- **Commit:** c42f21ad

## Known Stubs

None. All emissions are wired to production types (LaunchState, AgentMessage::LaunchStatusUpdate, DashboardEvent::LaunchStatusChanged). No placeholder values or TODO-marked code.

## Self-Check

Checking key files exist and commits are present:

- crates/rc-agent/src/game_launch_retry.rs — FOUND
- crates/rc-agent/src/tier_engine.rs — FOUND
- crates/rc-agent/src/ws_handler.rs — FOUND
- crates/racecontrol/src/ws/mod.rs — FOUND
- crates/racecontrol/tests/ws_launch_status_relay.rs — FOUND
- commit ef189f48 — FOUND
- commit d566e578 — FOUND
- commit c42f21ad — FOUND

## Self-Check: PASSED
