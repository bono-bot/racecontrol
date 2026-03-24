# Phase 178: Agent & Sentry Consumer - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire rc-agent to receive and apply flag updates, config pushes, and OTA messages over existing WebSocket. Add in-memory flag cache with offline persistence, kill switch evaluation, hot-reloadable config fields, and ConfigAck flow. rc-sentry receives flags via local JSON file written by rc-agent. Shared TypeScript types updated for SYNC-03.

</domain>

<decisions>
## Implementation Decisions

### Flag Storage & Hot Path Access
- `Arc<RwLock<HashMap<String, bool>>>` on AppState for in-memory flags — matches existing `guard_whitelist` pattern
- Kill switch (`kill_*`) flags evaluated FIRST in every gated code path — if any kill flag matches, halt the feature immediately regardless of other flags
- Offline cache: `C:\RacingPoint\flags-cache.json` — simple JSON `{"flag_name": true}`, written on every FlagSync receive
- Fresh pod startup with no cache: all flags default to `true` (features enabled) — safe default prevents accidental feature disabling

### Config Hot-Reload & Boundaries
- Hot-reloadable fields: billing_rates, game_limits, process_guard_whitelist, debug_verbosity — update in-memory via `Arc<RwLock>` or existing `watch::Sender`
- Non-reloadable fields: port bindings (8090), WS URL, pod_number, pod_id — documented in code, ConfigPush with these fields logs warning + ignores
- rc-agent sends `AgentMessage::ConfigAck { pod_id, seq_num }` back over WS after applying — server marks queue entry acked
- rc-sentry config delivery: rc-agent writes `C:\RacingPoint\sentry-flags.json` on FlagSync → rc-sentry reads on next watchdog cycle (5s)

### Flag Integration Points
- Flag checks at: game launch gate (ws_handler LaunchGame arm), billing guard poll loop, process guard spawn — 3 integration points
- `state.flag_enabled("flag_name")` helper method — returns `bool`, checks kill switch first, then flag cache
- SYNC-03: add FlagSync, ConfigPush, ConfigAck WS message types to shared TypeScript types + contract test verifying field agreement

### Claude's Discretion
- Internal module organization for flag/config handling code
- Exact ConfigAck payload structure beyond pod_id + seq_num
- Contract test fixture details

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `app_state.rs:48` — `guard_whitelist: Arc<RwLock<MachineWhitelist>>` as pattern for flag storage
- `ws_handler.rs:975-979` — `UpdateProcessWhitelist` handler as pattern for WS → RwLock update
- `billing_guard.rs:66-75` — 5s polling loop with `watch::Receiver` for state reads
- `ws_handler.rs:283-519` — LaunchGame handler with safe mode gate as pattern for flag gate
- `sentry_config.rs:11-109` — OnceLock config with TOML, file-based config loading
- `CoreToAgentMessage::FlagSync`, `::ConfigPush` variants already in protocol.rs (Phase 176)
- `AgentMessage::ConfigAck` variant already in protocol.rs (Phase 176)

### Established Patterns
- WS message handling: `match core_msg { ... }` in `handle_ws_message()`
- State mutation via `&mut AppState` in handler
- Config loaded at startup from TOML, no existing hot-reload
- File I/O at `C:\RacingPoint\` directory for all pod-local state

### Integration Points
- `crates/rc-agent/src/ws_handler.rs` — add FlagSync + ConfigPush match arms
- `crates/rc-agent/src/app_state.rs` — add flags HashMap + flag_enabled() helper
- `crates/rc-agent/src/event_loop.rs` — send FlagCacheSync on WS connect
- `crates/rc-agent/src/billing_guard.rs` — add flag check in poll loop
- `crates/rc-sentry/src/main.rs` or `sentry_config.rs` — read sentry-flags.json
- `packages/shared-types/src/` — add WS message types for SYNC-03

</code_context>

<specifics>
## Specific Ideas

- Standing rule: Config push must NEVER route through fleet exec endpoint — WebSocket typed ConfigPush only
- FlagCacheSync sent on reconnect to get latest flags from server (already handled server-side in Phase 177)
- ConfigAck must include seq_num for deterministic audit log correlation (Phase 177 server expects this)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
