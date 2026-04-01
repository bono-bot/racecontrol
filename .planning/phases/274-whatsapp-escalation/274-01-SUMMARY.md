---
phase: 274-whatsapp-escalation
plan: 01
subsystem: tier-engine
tags: [escalation, whatsapp, websocket, protocol]
dependency_graph:
  requires: []
  provides: [EscalationPayload, AgentMessage::EscalationRequest, tier5-ws-send]
  affects: [rc-common, rc-agent, tier_engine]
tech_stack:
  added: []
  patterns: [ws-message-relay, mpsc-channel-threading]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/src/tier_engine.rs
    - crates/rc-agent/src/main.rs
decisions:
  - Used ws_exec_result_tx (existing AgentMessage channel) instead of creating a new channel
  - Severity derived from DiagnosticTrigger variant (critical for GameMidSessionCrash/WsDisconnect)
  - Static actions_tried list since tier engine does not track per-event action history
metrics:
  completed: "2026-04-01"
  tasks: 2
  files: 3
---

# Phase 274 Plan 01: Pod-Side Tier 5 Escalation via WS Summary

EscalationPayload struct (9 fields) added to rc-common, AgentMessage::EscalationRequest wired through tier engine to send structured escalation data to server via existing WS connection.

## What Changed

### Task 1: EscalationPayload + AgentMessage::EscalationRequest (rc-common)

- Added `EscalationPayload` struct with 9 fields: pod_id, incident_id, severity, trigger, summary, actions_tried, impact, dashboard_url, timestamp
- Added `AgentMessage::EscalationRequest(EscalationPayload)` variant after `GameCrashed`
- Serde rename_all = "snake_case" serializes as `"type": "escalation_request"`
- Commit: `d6301049`

### Task 2: Wire tier5_human_escalation to Send WS Message (rc-agent)

- Changed `tier5_human_escalation` from sync stub to async function with WS sender
- Threaded `ws_msg_tx: mpsc::Sender<AgentMessage>` through: `spawn()` -> `run_supervised()` -> `run_tiers()` -> `tier5_human_escalation()`
- Used existing `ws_exec_result_tx` channel (drains in event_loop.rs select loop)
- Updated all 3 call sites: circuit breaker open, budget exhausted, API unavailable
- Returns `TierResult::FailedToFix` instead of `TierResult::Stub`
- `node_id` (COMPUTERNAME env var, already resolved at startup) used as pod_id
- Commit: `d6301049`

## Decisions Made

1. **Channel reuse**: Used existing `ws_exec_result_tx` (mpsc::Sender<AgentMessage>) rather than creating a dedicated escalation channel. The event_loop already drains this and sends via WS.
2. **Static actions_tried**: Tier engine does not currently track per-event action history, so we use a static list of tier names. Future enhancement: accumulate actual action descriptions as tiers execute.
3. **Severity mapping**: GameMidSessionCrash and WsDisconnect = "critical" (customer-impacting). All others = "high".

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all code is functional (sends real WS messages, no placeholder data).

## Self-Check: PASSED

- All 3 modified files exist
- Commit d6301049 found in git log
- cargo check passes for rc-common, rc-agent-crate, racecontrol-crate
- No TierResult::Stub for tier 5 remains
- No .unwrap() in new code
