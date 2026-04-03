# Phase 318: Launch Intelligence - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via autonomous mode)

<domain>
## Phase Boundary

Every game launch has a timeout watchdog that prevents permanent pod lockout and records step-level timeline spans so launch failures can be debugged at the exact checkpoint where they stalled. Adds 90s default timeout (dynamic per-combo from historical data) that auto-transitions GameTracker to Error on expiry. Launch timeline events stored in launch_timeline_spans table.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion. Key constraints:
- Timeout watchdog: server-side tokio::time::timeout on GameTracker state — if no GameStateUpdate within timeout, auto-transition to Error + emit DiagnosticTrigger::GameLaunchTimeout
- Dynamic timeout: configurable via AgentConfig.game_launch_timeout_secs pushed from server, based on historical p95 launch time per combo
- Timeline spans: launch_timeline_spans SQLite table (server-side), populated from LaunchTimelineReport WS message from agent
- Timeline events: ws_sent (server records), agent_received (agent reports), process_spawned (agent reports), playable_signal (agent reports)
- GET /api/v1/launch-timeline/{launch_id} endpoint returns timeline data
- v40.0 Phase 312 WS ACK is deployed (b7359a02) — can rely on delivery confirmation

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/game_launcher.rs` — GameTracker, launch state FSM
- `crates/racecontrol/src/ws/mod.rs` — WS handlers including GameStateUpdate
- `crates/racecontrol/src/db/mod.rs` — SQLite migrations
- `crates/rc-agent/src/game_launch_retry.rs` — retry orchestrator
- `crates/rc-agent/src/tier_engine.rs` — DiagnosticTrigger::GameLaunchTimeout (Phase 315)
- `crates/rc-common/src/types.rs` — LaunchTimelineReport, LaunchTimeline structs (Phase 315)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — refer to ROADMAP success criteria.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
