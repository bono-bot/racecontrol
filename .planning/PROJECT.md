# RaceControl Reliability & Connection Hardening

## Current State

**Shipped:** v1.0 RaceControl HUD & Safety (2026-03-13)

The pod management stack (rc-core, rc-agent, kiosk) now self-heals, deploys reliably, and shows clean branded screens to customers at all times. All 22 v1.0 requirements are code-complete. On-site deployment of Phase 5 (blanking screen protocol) is pending manual execution at the venue.

### What v1.0 Delivered

- **Watchdog hardening**: Escalating backoff (30s→2m→10m→30m), post-restart verification (process + WS + lock screen), email alerts on persistent failures
- **WebSocket resilience**: WS-level ping/pong keepalive (15s), app-level Ping/Pong (30s), fast-then-backoff reconnect (1s×3 then exponential to 30s), kiosk 15s disconnect debounce
- **Deployment pipeline**: DeployState FSM (9 states), HEAD-before-kill URL validation, canary-first (Pod 8), session-aware rolling deploy with WaitingSession + pending_deploys + session-end hook
- **Blanking screen protocol**: Lock-screen-before-kill ordering, LaunchSplash branded screen, extended dialog suppression (5 processes), PIN auth unification, pod lockdown (taskbar hidden, Win key blocked)
- **Config hardening**: rc-agent fails fast on bad config with branded error screen, deploy template matches AgentConfig struct

## What This Is

A reliability overhaul of the RaceControl pod management stack (rc-core, rc-agent, pod-agent) to eliminate fragile connections, cascading debug cycles, and deployment pain. Targets the Racing Point eSports venue's 8 sim racing pods managed from a central server.

## Core Value

Deploying updates and launching games should work reliably on all 8 pods without manual debugging — the system recovers from failures automatically and reports problems instead of silently breaking. The customer never sees system internals.

## Requirements

### Validated (v1.0 — Shipped)

- ✓ WebSocket connection between rc-core and rc-agent — existing
- ✓ Pod-agent HTTP exec endpoint for remote commands — existing
- ✓ Game launch from staff kiosk — existing
- ✓ UDP heartbeat for pod liveness detection (6s timeout) — existing
- ✓ Pod monitoring and healing (pod_monitor.rs, pod_healer.rs) — existing
- ✓ Lock screen with PIN auth — existing
- ✓ Billing lifecycle (start/stop/idle) — existing
- ✓ Escalating watchdog backoff (WD-01)
- ✓ Shared backoff state in AppState (WD-02)
- ✓ Post-restart verification (WD-03)
- ✓ Backoff reset on recovery (WD-04)
- ✓ WebSocket keepalive ping/pong (CONN-01)
- ✓ Kiosk disconnect debounce (CONN-02)
- ✓ Auto-reconnect with backoff (CONN-03)
- ✓ Config validation at startup (DEPLOY-01)
- ✓ Safe deploy sequence (DEPLOY-02)
- ✓ Honest exec status codes (DEPLOY-03)
- ✓ Stale config cleanup (DEPLOY-04)
- ✓ Rolling deploy without session disruption (DEPLOY-05)
- ✓ Email alerts on failure (ALERT-01)
- ✓ Rate-limited alerts (ALERT-02)
- ✓ Clean branded screens (SCREEN-01, SCREEN-02, SCREEN-03)
- ✓ PIN auth unification (AUTH-01)
- ✓ Performance targets met (PERF-01 through PERF-04)

### Out of Scope

- HUD overlay features — deferred to next project (archived in .planning/archive/hud-safety/)
- FFB safety — deferred (archived research available)
- New game integrations — current games only
- Cloud sync changes — cloud_sync.rs is stable
- Customer-facing PWA changes

## Context

- **Venue:** 8 gaming pods (192.168.31.x subnet), 1 server (.23), 1 James workstation (.27)
- **Stack:** Rust/Axum (rc-core port 8080, rc-agent per-pod), Node.js (pod-agent port 8090), Next.js (kiosk)
- **Crates:** rc-common (shared types/protocol), rc-core (server), rc-agent (pod client)

## Constraints

- **Rust/Axum:** rc-core and rc-agent must stay Rust — no language change
- **Pod-agent:** Node.js, runs on each pod alongside rc-agent
- **No new dependencies:** Use existing crate deps where possible (tokio, reqwest, serde, chrono, tracing)
- **Email via send_email.js:** Reuse existing Gmail auth, don't add SMTP crate
- **Windows:** All pods run Windows 11, Session 1 requirement for GUI processes
- **Backward compat:** Changes must not break existing billing, game launch, or lock screen

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Archive HUD project, start reliability-first | Can't add features on fragile base | Shipped v1.0 |
| Reuse watchdog hardening research | Research already done, high confidence | Shipped v1.0 |
| EscalatingBackoff in rc-common | Shared between core and agent | Shipped v1.0 |
| Email alerts via send_email.js shell-out | Reuses existing Gmail OAuth, no new deps | Shipped v1.0 |
| Pod 8 canary-first deployment | Catch issues on one pod before rolling to all | Shipped v1.0 |
| Lock screen before game kill | Prevents desktop flash during session end | Shipped v1.0 |
| Registry-based pod lockdown | Survives rc-agent restarts, one-time apply | Shipped v1.0 |

## Next Milestone Goals

To be defined via `/gsd:new-milestone`. Candidates:
- HUD overlay with live sector times and telemetry
- FFB safety (zero wheelbase torque on session boundary)
- Cloud dashboard for remote monitoring
- On-site deployment automation improvements

---
*Last updated: 2026-03-13 — v1.0 shipped*
