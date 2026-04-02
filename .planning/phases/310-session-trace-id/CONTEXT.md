# Phase 310: Session Trace ID Propagation

## Origin
MI-5 finding from Mermaid AI billing flow analysis (2026-04-02).
No single trace_id links Launch → Billing → Refund actions through logs,
making cross-subsystem debugging difficult.

## Problem
`billing_session.id` exists and is used in billing operations, but it's NOT
consistently threaded through:
- Game launch events (GameTracker uses pod_id, not session_id)
- Agent-side logs (ws_handler logs pod_id + game state, not billing session)
- Refund journal entries (wallet journal has session_id but launch/crash events don't)
- Activity log entries (log_pod_activity has no session_id parameter)
- Metrics/LaunchEvent (has pod_id but not billing_session_id)

When debugging "customer says they were overcharged", you need to manually
correlate: billing_sessions.id → pod_activity_log timestamps → game_launcher
events → agent logs → wallet journal. No single query can trace the full path.

## Goal
Every log entry, metric, and event generated during a customer session includes
`session_id` (the billing_session.id), enabling single-query trace from
Launch → Playable → Billing → Crash → Resume → End → Refund.

## Scope

### Must Have
1. Add `session_id: Option<String>` to `log_pod_activity()` — propagate through all callers
2. Add `billing_session_id` to `GameTracker` — set when launch is tied to a billing session
3. Add `session_id` to `metrics::LaunchEvent` — link launch metrics to billing
4. Add `session_id` to `DashboardEvent::GameStateChanged` — kiosk can correlate game + billing

### Nice to Have
5. Add `session_id` to agent-side WS log entries (requires protocol change — `LaunchGame` message already has it?)
6. Query endpoint: `GET /api/v1/sessions/{id}/trace` — returns all events for a session in chronological order

### Out of Scope
- Distributed tracing (OpenTelemetry/Jaeger) — overkill for single-server SQLite
- Agent-side trace storage — agent is stateless, server is source of truth

## Dependencies
- None — additive change, no breaking modifications

## Risk
- Low — adding an optional field to existing structs
- Must NOT break existing log queries or dashboard WS consumers (serde default)

## Files to Modify
- `crates/racecontrol/src/activity_log.rs` — add session_id parameter
- `crates/racecontrol/src/game_launcher.rs` — add session_id to GameTracker + LaunchEvent
- `crates/racecontrol/src/billing.rs` — pass session_id to log_pod_activity calls
- `crates/racecontrol/src/ws/mod.rs` — pass session_id in game state events
- `crates/rc-common/src/types.rs` — add session_id to GameLaunchInfo
- `crates/racecontrol/src/api/routes.rs` — optional trace query endpoint

## Estimated Effort
- 2 plans, ~100 lines of Rust changes
- Plan 1: Core propagation (activity_log + GameTracker + LaunchEvent)
- Plan 2: Dashboard events + optional trace query endpoint
