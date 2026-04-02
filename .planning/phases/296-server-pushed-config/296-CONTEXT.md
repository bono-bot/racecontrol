# Phase 296: Server-Pushed Config - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

The server is the authoritative source of pod config. Pods receive full AgentConfig over WebSocket on connect and on change. Config is persisted in SQLite (pod_configs table) on the server and locally on each pod. Hot-reload fields apply immediately; cold fields require restart.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase.

Key observations from codebase scout:
- Existing config_push.rs in racecontrol (v22.0 Phase 177) already has: validation, queuing, seq_num, audit log, WS delivery via ConfigPush message
- Agent ws_handler.rs already handles CoreToAgentMessage::ConfigPush with field-level hot-reload
- Phase 295 added AgentConfig to rc-common with Serialize/Deserialize + schema_version
- Extend existing config_push.rs — don't create new module
- SQLite pod_configs table: store full JSON blob per pod_id with last_modified timestamp and config_hash
- On WS connect (handle_agent_connected), push stored config for that pod
- Agent side: receive full AgentConfig JSON, compute sha256 hash, skip if unchanged
- Local persistence: agent writes received config to rc-agent-server-config.json alongside rc-agent.toml
- Hot fields (process_guard.enabled, mma.daily_budget_*, etc.): apply via setter methods
- Cold fields (pod.number, core.url, telemetry_ports.ports): log "requires restart" warning

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/config_push.rs` — full push infrastructure (v22.0)
- `crates/rc-common/src/config_schema.rs` — AgentConfig with Serialize (Phase 295)
- `crates/rc-common/src/protocol.rs` — CoreToAgentMessage::ConfigPush
- `crates/rc-agent/src/ws_handler.rs` — handles ConfigPush messages
- `crates/rc-agent/src/config.rs` — lenient_deserialize, config_search_paths

### Established Patterns
- SQLite tables created via ensure_tables() in db/mod.rs
- WS messages use CoreToAgentMessage enum in rc-common/protocol.rs
- Audit logging via insert_audit_log() in config_push.rs
- Boot resilience: spawn_periodic_refetch() in rc-common

### Integration Points
- Server: ws/mod.rs handle_agent_connected() — push config on connect
- Server: db/mod.rs — new pod_configs table
- Agent: ws_handler.rs — handle full config push
- Agent: config.rs — local persistence + fallback load

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
