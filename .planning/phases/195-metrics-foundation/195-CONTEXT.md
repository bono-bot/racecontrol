# Phase 195: Metrics Foundation - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the data backbone for the self-improving launch engine: SQLite tables for launch events, billing events, and crash recovery events. JSONL fallback for immutable audit trail. REST API endpoints for querying launch stats and billing accuracy. Fix log_game_event() silent error swallowing.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key decisions:
- SQLite table schemas (launch_events, billing_events, recovery_events)
- JSONL file location and rotation strategy
- API endpoint response shapes (matching REQUIREMENTS.md specs)
- How to integrate recording into existing launch/billing/crash flows
- Whether to use a shared metrics module or inline recording at each call site

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/game_launcher.rs` — existing `log_game_event()` function (lines 561-582) that silently swallows DB errors
- `crates/racecontrol/src/billing.rs` — existing billing session lifecycle with start/pause/resume/end events
- `crates/racecontrol/src/api/routes.rs` — existing REST API infrastructure (Axum handlers)
- SQLite database already used for billing_sessions, laps, drivers, etc.

### Established Patterns
- Axum handlers with `State<Arc<AppState>>` extraction
- `sqlx::query()` with `.execute(&state.db)` for DB operations
- JSON serialization via serde for API responses
- `tracing::error!` for error logging

### Integration Points
- `launch_game()` — record launch event after launch command sent
- `handle_game_state_update()` — record PlayableSignal timing for billing accuracy
- `handle_game_status_update()` in billing.rs — record billing start/pause/resume events
- `check_game_health()` — record timeout events
- Race Engineer auto-relaunch — record crash recovery events
- New API routes mounted under `/api/v1/metrics/`

</code_context>

<specifics>
## Specific Ideas

- Dual storage: SQLite for queries + JSONL for immutable audit trail
- JSONL fallback: if SQLite insert fails, write to JSONL with `"db_fallback": true` flag
- Launch events table must include: pod_id, sim_type, car, track, session_type, timestamp, outcome, error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number
- Billing events must track: launch_command_at, playable_signal_at, billing_start_at, delta_ms
- Recovery events must track: failure_mode, recovery_action_tried, recovery_outcome, recovery_duration_ms

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
