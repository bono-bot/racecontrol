# Phase 138: Idle Health Monitor - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add periodic idle-state health checks (Edge alive + window rect + lock screen HTTP) every 60s when no billing session is active. Self-heal on failure (close+relaunch browser). Send IdleHealthFailed to server after 3 consecutive failures (hysteresis). Skip during active billing sessions.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Phase 137 (Browser Watchdog) already added browser_watchdog_interval (30s), is_browser_alive(), is_browser_expected(), count_edge_processes() to lock_screen.rs and event_loop.rs
- Idle health is a SEPARATE concern from browser watchdog — browser watchdog checks Edge liveness, idle health checks the full display stack (HTTP server + window rect + browser)
- Standing rule #10: recovery systems must not fight each other
- IDLE-04: must not interfere with active billing sessions or running games
- IdleHealthFailed is a new AgentMessage variant in rc-common protocol.rs

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pre_flight.rs` check_lock_screen_http() (line 299) and check_window_rect() (line 384) — can be called directly for idle health
- `event_loop.rs` — already has browser_watchdog_interval (30s) and maintenance_retry_interval (30s)
- `protocol.rs` — PreFlightFailed/PreFlightPassed message patterns to follow for IdleHealthFailed
- `ws/mod.rs` — server WS handler patterns for new message types

### Established Patterns
- Hysteresis: rc-sentry uses HYSTERESIS_THRESHOLD=3 consecutive failures before declaring crash
- tokio::time::interval for periodic tasks in event_loop.rs
- AgentMessage variants with pod_id + failure details

### Integration Points
- event_loop.rs — add idle_health_interval (60s)
- protocol.rs — add AgentMessage::IdleHealthFailed variant
- ws/mod.rs — handle IdleHealthFailed on server side
- fleet_health.rs — expose idle health status in fleet API

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
