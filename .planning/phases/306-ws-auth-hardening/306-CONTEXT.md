# Phase 306: WS Auth Hardening - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Security phase — discuss skipped

<domain>
## Phase Boundary

Replace static PSK WebSocket auth with per-pod JWT tokens (24h expiry), auto-rotation 1h before expiry, disconnect + WhatsApp alert on invalid token. PSK remains as bootstrap fallback for initial connection.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key constraints: `jsonwebtoken` crate already in use (HS256), existing `StaffClaims` pattern in auth/middleware.rs can be extended for pod claims, WhatsApp alerter exists at `whatsapp_alerter.rs`.

</decisions>

<code_context>
## Existing Code Insights

### Current WS Auth (PSK-only)
- `crates/racecontrol/src/ws/mod.rs` — `verify_ws_token()` (line ~74) checks query param `?token=<terminal_secret>`
- Three WS endpoints: `agent_ws()` (line 83), `dashboard_ws()` (line 96), `ai_ws()` (line 2004)
- All use `verify_ws_token()` before WebSocket upgrade
- rc-agent connects via `ws://server:8080/ws/agent?token=<terminal_secret>` — NO auth headers

### Existing JWT Infrastructure (REST-only, NOT on WS)
- `crates/racecontrol/src/auth/middleware.rs` — `StaffClaims { sub, role, exp, iat }`, HS256
- `create_staff_jwt_with_role()` (line 222) — token generation
- JWT secret rotation already supported: `jwt_secret` + `jwt_secret_previous` (grace period)
- `jsonwebtoken` crate already a dependency

### Alert Infrastructure
- `crates/racecontrol/src/whatsapp_alerter.rs` — `send_admin_alert()` (line 106) for WhatsApp alerts
- Evolution API integration, Uday's phone in config
- Pattern: fire event → format message → POST to Evolution API

### Agent-side Connection
- `crates/rc-agent/src/main.rs` — `connect_with_tls_config()` (line 275)
- Plain `connect_async(url)` for ws://, TLS for wss://
- No auth headers sent, relies on URL query param
- `ws_connected` atomic flag on connect/disconnect

### Config
- Server: `config.cloud.terminal_secret` for PSK
- Server: `config.auth.jwt_secret` for JWT
- Agent: `CoreConfig` in `crates/rc-agent/src/config.rs`

### Integration Points
- Server: `ws/mod.rs` `verify_ws_token()` → needs JWT alternative path
- Server: Need `PodClaims` struct (pod_id, pod_number, exp, iat) parallel to `StaffClaims`
- Agent: `main.rs` connection setup → needs to store JWT, send in subsequent messages
- Agent: Need JWT refresh handler in `ws_handler.rs`
- Server: Need token rotation timer + refresh message on WS channel

</code_context>

<specifics>
## Requirements
- WSAUTH-01: Per-pod JWT with 24h expiry after initial PSK-authenticated connection
- WSAUTH-02: Auto-rotate 1h before expiry via refresh message, no reconnection needed
- WSAUTH-03: Expired/invalid JWT → immediate disconnect + WhatsApp alert
- WSAUTH-04: PSK remains as bootstrap — server issues JWT in first authenticated response
</specifics>

<deferred>
None.
</deferred>
