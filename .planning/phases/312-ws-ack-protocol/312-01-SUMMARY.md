---
phase: 312-ws-ack-protocol
plan: 01
subsystem: ws-protocol
tags: [ws, ack, game-launch, game-stop, confirmed-delivery]
dependency_graph:
  requires: []
  provides: [command-ack-protocol, confirmed-game-commands]
  affects: [game_launcher, ws_handler, billing, auth, config_push, multiplayer, preset_library, mesh_handler, pod_healer, bot_coordinator, reservation, promotion, ac_server]
tech_stack:
  added: []
  patterns: [oneshot-channel-ack, pre-wrapped-coremessage]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/racecontrol/src/ac_server.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/bot_coordinator.rs
    - crates/racecontrol/src/config_push.rs
    - crates/racecontrol/src/mesh_handler.rs
    - crates/racecontrol/src/multiplayer.rs
    - crates/racecontrol/src/pod_healer.rs
    - crates/racecontrol/src/pod_reservation.rs
    - crates/racecontrol/src/preset_library.rs
    - crates/racecontrol/src/promotion.rs
    - crates/racecontrol/src/reservation.rs
decisions:
  - "Changed agent_senders channel type from Sender<CoreToAgentMessage> to Sender<CoreMessage> so callers control command_id for ACK correlation"
  - "Agent sends CommandAck immediately on receipt (not after process spawned) — ACK means delivery confirmed, not launch success"
  - "Old agents without CommandAck support trigger 5s timeout gracefully — server returns error, no crash"
metrics:
  duration_minutes: 54
  completed: "2026-04-03"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 19
  tests_added: 3
  tests_total_passing: 1042
---

# Phase 312 Plan 01: WS ACK Protocol Summary

Confirmed-delivery ACK protocol for game launch and stop commands using oneshot channels with 5s timeout.

## One-liner

CommandAck variant + oneshot channel pattern: launch/stop wait 5s for agent ACK before returning success to API caller.

## Changes

### Task 1: Protocol + State (CommandAck variant and pending ack map)

- Added `AgentMessage::CommandAck { command_id, success, error }` variant to rc-common protocol
- Added `CommandAckResult` struct and `pending_command_acks: RwLock<HashMap<String, oneshot::Sender<CommandAckResult>>>` to AppState
- Changed `agent_senders` type from `Sender<CoreToAgentMessage>` to `Sender<CoreMessage>` — this is the key architectural change that lets callers control the `command_id` for ACK correlation
- Updated all ~60 send call sites across 18 files to wrap messages with `CoreMessage::wrap()`
- Updated 4 function signatures that accept sender references (`push_full_config_to_pod`, `replay_pending_config_pushes`, `push_presets_to_pod`, `handle_game_status_update`)
- 3 serialization roundtrip tests for CommandAck (success, error, backward compat)

### Task 2: Server ACK wait + Agent ACK send (full flow wiring)

**Server side (game_launcher.rs):**
- `launch_game()`: generates command_id via `CoreMessage::wrap()`, registers oneshot before send, waits 5s for ACK after successful send. Timeout returns error to API caller (WSCMD-01/03).
- `stop_game()`: same pattern — wraps StopGame, registers oneshot, waits 5s. Timeout logged as warning (WSCMD-02/03).
- Cleanup: pending ack removed on send failure, timeout, or disconnect.

**Server side (ws/mod.rs):**
- CommandAck handler resolves pending oneshot channel when agent ACK arrives.
- WS write loop no longer wraps messages (channel carries pre-wrapped CoreMessage).

**Agent side (ws_handler.rs):**
- LaunchGame handler sends CommandAck immediately after dedup check, before any launch processing. ACK means "received and starting to process."
- StopGame handler sends CommandAck immediately on receipt.
- Both use `command_id` from the outer `CoreMessage` wrapper (already parsed by DEPLOY-05 logic).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Channel type change cascade**
- **Found during:** Task 2
- **Issue:** Changing `agent_senders` from `Sender<CoreToAgentMessage>` to `Sender<CoreMessage>` required updating ~60 send call sites across 18 files, plus 4 function signatures
- **Fix:** Systematic replacement of all `.send(CoreToAgentMessage::Foo {...})` to `.send(CoreMessage::wrap(CoreToAgentMessage::Foo {...}))` with import additions
- **Files modified:** 18 server source files
- **Commit:** b7359a02

**2. [Rule 1 - Bug] Test code used old channel type**
- **Found during:** Task 2 verification
- **Issue:** Test code in routes.rs created `mpsc::channel::<CoreToAgentMessage>` and matched on bare `CoreToAgentMessage` — now needs `CoreMessage` channel and `msg.inner` match
- **Fix:** Updated 4 test channel declarations and 1 match expression
- **Files modified:** crates/racecontrol/src/api/routes.rs
- **Commit:** b7359a02

## Verification

- `cargo test -p rc-common -- command_ack`: 3/3 pass (roundtrip success, roundtrip error, backward compat)
- `cargo test -p rc-common`: 235/235 pass
- `cargo test -p racecontrol-crate --lib`: 807/807 pass
- `cargo build --release --bin racecontrol --bin rc-agent`: both compile clean
- 8 pre-existing integration test failures (lap_suspect, notification) — unrelated to this change

## Known Stubs

None. All data paths are wired end-to-end.

## Self-Check: PASSED
