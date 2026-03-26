# Phase 206: Observable State Transitions - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Eliminate all silent failures in the system. Every degraded state emits an observable signal within 30 seconds — operators learn of pod failures, config fallbacks, and empty allowlists at the moment they occur, not after downstream symptoms appear. Covers rc-agent, rc-sentry, and racecontrol modifications.

</domain>

<decisions>
## Implementation Decisions

### Alert Routing
- MAINTENANCE_MODE alert uses WhatsApp via Evolution API, following existing app_health_monitor.rs pattern (same channel as health alerts)
- Sentinel file watching uses `notify 8.2.0` crate with RecommendedWatcher (ReadDirectoryChangesW on Windows) — zero CPU, instant detection
- Sentinel change events surface in fleet health API (/api/v1/fleet/health gains active_sentinels: Vec<String> field) AND via DashboardEvent WS broadcast
- WhatsApp alert rate limiting: 1 alert per sentinel type per pod per 5 minutes — prevents alert storms during crash cascades

### Silent Failure Sweep
- Config fallback logging: collect pre-init errors in Vec<String>, flush to tracing::error! after subscriber init + always eprintln! immediately
- Instrument ALL 6+ unwrap_or sites in rc-agent main.rs + load_or_default() in racecontrol config.rs — exhaustive sweep
- Empty allowlist auto-response: auto-switch to report_only + emit error! + fleet alert — never kill processes on empty allowlist
- rc-sentry FSM logging: log ALL transitions (Healthy→Suspect, Suspect(N)→Suspect(N+1), Suspect→Crashed) to RecoveryLogger, not just Crashed

### WebSocket Protocol
- New AgentMessage::SentinelChange variant — explicit type, clean deserialization
- Rolling deploy strategy: server ignores unknown variants (serde default) — deploy server first, then pods
- Active sentinels: add active_sentinels: Vec<String> to PodFleetStatus — additive, backward compatible

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `racecontrol/src/app_health_monitor.rs` — existing WhatsApp alert channel via Evolution API
- `rc-common/src/protocol.rs` — AgentMessage enum, CoreToAgentMessage enum
- `rc-common/src/recovery.rs` — RecoveryLogger JSONL writing
- `racecontrol/src/fleet_health.rs` — PodFleetStatus struct, /api/v1/fleet/health endpoint
- `rc-agent/src/startup_log.rs` — phased startup log, eprintln! pattern
- `rc-sentry/src/watchdog.rs` — WatchdogState FSM (Healthy/Suspect/Crashed)

### Established Patterns
- eprintln! for pre-tracing-init errors (already in startup_log.rs)
- tracing::warn! with structured fields for post-init state transitions
- DashboardEvent broadcast for real-time fleet UI updates
- AgentMessage variants use serde with #[serde(tag = "type")]

### Integration Points
- rc-agent event_loop.rs writes sentinel files — add emit_transition() calls before fs::write()
- racecontrol pod_monitor.rs reads fleet health — add active_sentinels to PodFleetStatus
- rc-sentry watchdog.rs FSM transitions — add RecoveryLogger writes
- rc-agent main.rs config loading — add pre-init error buffer

</code_context>

<specifics>
## Specific Ideas

- The `notify` watcher should watch `C:\RacingPoint\` directory for any file matching known sentinel names (MAINTENANCE_MODE, GRACEFUL_RELAUNCH, OTA_DEPLOYING, rcagent-restart-sentinel.txt)
- Pre-init error buffer pattern: `static mut PRE_INIT_ERRORS: Vec<String>` is unsafe; prefer a OnceCell<Mutex<Vec<String>>> or just eprintln! without buffering
- WhatsApp alert format should include pod number, sentinel file name, action (created/deleted), and IST timestamp

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
