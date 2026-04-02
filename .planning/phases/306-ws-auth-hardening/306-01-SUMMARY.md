---
phase: 306
plan: "01"
subsystem: ws-auth
tags: [security, jwt, websocket, psk, pod-auth, rotation]
dependency_graph:
  requires: [TLS-01, TLS-02]
  provides: [WSAUTH-01, WSAUTH-02, WSAUTH-03, WSAUTH-04]
  affects: [racecontrol, rc-agent, rc-common]
tech_stack:
  added: []
  patterns: [PodClaims JWT, PSK bootstrap → JWT steady-state, 22h rotation timer, dual-secret grace, 401 auto-recovery]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/auth/middleware.rs (PodClaims, create_pod_jwt, decode_pod_jwt)
    - crates/racecontrol/src/ws/mod.rs (WsAuthParams, authenticate_agent_ws, issue_pod_jwt, rotation task)
    - crates/rc-common/src/protocol.rs (IssueJwt, RefreshJwt, JwtAck)
    - crates/rc-agent/src/app_state.rs (current_jwt, jwt_expires_at fields)
    - crates/rc-agent/src/main.rs (JWT-aware reconnect URL, 401 JWT clearing)
    - crates/rc-agent/src/ws_handler.rs (IssueJwt, RefreshJwt handlers)
    - crates/rc-agent/src/remote_ops.rs (fix Phase 305 TLS serve() on Result)
decisions:
  - "JWT-first auth: authenticate_agent_ws tries JWT, falls back to PSK — backward compatible"
  - "WhatsApp alert only on JWT rejection, not PSK — PSK failures are too noisy (every agent restart)"
  - "22h rotation timer (not 23h) — gives 2h buffer before 24h expiry"
  - "Agent clears JWT on 401 — auto-recovery to PSK bootstrap without manual intervention"
  - "JwtAck is observability-only — server doesn't block on it"
metrics:
  duration: "~40 minutes (code review + fix + commit)"
  completed: "2026-04-02"
  tasks_completed: 5
  tasks_total: 5
  files_modified: 7
  files_created: 0
---

# Phase 306 Plan 01: WS Auth Hardening Summary

Per-pod JWT WebSocket authentication with automatic rotation, PSK bootstrap fallback, and WhatsApp alerts on invalid tokens.

## What Was Built

### WSAUTH-01: Per-Pod JWT with 24h Expiry
- `PodClaims { pod_id, pod_number, exp, iat }` in `auth/middleware.rs`
- `create_pod_jwt()` generates HS256 JWT with configurable duration (default 24h)
- `decode_pod_jwt()` with dual-secret rotation grace (current + previous secret)
- Server issues JWT via `IssueJwt` WS message after PSK bootstrap authentication

### WSAUTH-02: Auto-Rotation Before Expiry
- Per-connection rotation task in `handle_agent()`: sleeps 22h, then sends `RefreshJwt`
- Agent handles `RefreshJwt` in `ws_handler.rs` — swaps stored token in place
- Zero-downtime: no reconnection needed, just token swap on existing connection

### WSAUTH-03: Disconnect + Alert on Invalid JWT
- `authenticate_agent_ws()` returns `Err` on invalid/expired JWT → 401 Unauthorized
- WhatsApp alert fired via `send_admin_alert("ws_jwt_rejected", ...)` (only for JWT failures, not PSK)
- Agent detects 401 in reconnect loop → clears JWT → falls back to PSK automatically

### WSAUTH-04: PSK Bootstrap Fallback
- `authenticate_agent_ws()` tries JWT first (if `?jwt=` param present), falls back to PSK (`?token=`)
- Agent starts with no JWT, connects with PSK URL → server issues JWT after registration
- PSK is always valid — never removed, ensures backward compatibility

## Flow

```
1. Agent connects: ws://server:8080/ws/agent?token=<psk>
2. Server validates PSK → upgrades WebSocket
3. Agent sends Register message
4. Server issues IssueJwt { token, expires_at } → agent stores
5. On disconnect/reconnect: agent uses ?jwt=<token> instead of ?token=<psk>
6. At 22h: server sends RefreshJwt → agent swaps token
7. If JWT expired/invalid: server rejects → agent clears JWT → retries with PSK
```

## Tests

710 passed, 4 pre-existing failures (unchanged from Phase 307 baseline).

## Commits

| Hash | Message |
|------|---------|
| `b33e388e` | feat(306-01): per-pod JWT WS auth with auto-rotation + PSK bootstrap |
