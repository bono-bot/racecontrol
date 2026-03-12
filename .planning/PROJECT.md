# RaceControl Reliability & Connection Hardening

## Current Milestone: v1.0 Reliability & Clean UX

**Goal:** Make pods self-healing, deployments repeatable, and customer-facing screens always clean — no error popups, no system messages, no visible failures.

**Target features:**
- Escalating watchdog with post-restart verification
- WebSocket resilience during game launch
- Clean deployment lifecycle across all 8 pods
- Config validation (fail-fast on bad config)
- Email alerts for persistent failures
- Blanking screen protocol (clean branded UI always, zero error leakage)

## What This Is

A reliability overhaul of the RaceControl pod management stack (rc-core, rc-agent, pod-agent) to eliminate fragile connections, cascading debug cycles, and deployment pain. Targets the Racing Point eSports venue's 8 sim racing pods managed from a central server.

## Core Value

Deploying updates and launching games should work reliably on all 8 pods without manual debugging — the system recovers from failures automatically and reports problems instead of silently breaking. The customer never sees system internals.

## Requirements

### Validated

- ✓ WebSocket connection between rc-core and rc-agent — existing
- ✓ Pod-agent HTTP exec endpoint for remote commands — existing
- ✓ Game launch from staff kiosk — existing
- ✓ UDP heartbeat for pod liveness detection (6s timeout) — existing
- ✓ Pod monitoring and healing (pod_monitor.rs, pod_healer.rs) — existing
- ✓ Lock screen with PIN auth — existing
- ✓ Billing lifecycle (start/stop/idle) — existing

### Active

- [ ] Connection resilience: WebSocket doesn't drop during game launch or CPU spikes
- [ ] Escalating watchdog: backoff 30s→2m→10m→30m instead of fixed cooldown
- [ ] Post-restart verification: confirm pod is actually healthy after restart
- [ ] Email alerts for persistent pod failures requiring manual intervention
- [ ] Clean process lifecycle: old binaries fully die before new ones start
- [ ] Config validation: rc-agent fails fast on missing/invalid config fields
- [ ] Consistent deployment: same binary works identically on all 8 pods
- [ ] Instruction handling: pod-agent commands are idempotent and return clear success/failure
- [ ] Kiosk stability: no "disconnected" flash during game launch operations
- [ ] Blanking screen protocol: clean branded screen before/after sessions, suppress all error popups and system dialogs

### Out of Scope

- HUD overlay features — deferred to next project (archived in .planning/archive/hud-safety/)
- FFB safety — deferred (archived research available)
- New game integrations — current games only
- Cloud sync changes — cloud_sync.rs is stable
- Customer-facing PWA changes

## Context

- **Venue:** 8 gaming pods (192.168.31.x subnet), 1 server (.23), 1 James workstation (.27)
- **Stack:** Rust/Axum (rc-core port 8080, rc-agent per-pod), Node.js (pod-agent port 8090), Next.js (kiosk)
- **Existing pain:** Game launch momentarily shows "disconnected" in kiosk. Deploying new rc-agent binaries fails in multiple ways: binary doesn't start, old process lingers, config mismatch, works on 1 pod but fails on others. Customers see error popups ("Cannot find rc agent", "Conspit Link is running", "No C:\\" paths) leaking through the lock screen.
- **Watchdog research:** Phase 05 research from HUD project covers escalating backoff, post-restart health verification, email notifications — ready to implement (see .planning/archive/hud-safety/phases/05-watchdog-hardening/05-RESEARCH.md)
- **3-tier supervision:** watchdog.bat/pod-agent → pod_monitor.rs → pod_healer.rs
- **Deploy method:** HTTP download from James (.27:9998) via pod-agent /exec, or pendrive install.bat

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
| Archive HUD project, start reliability-first | Can't add features on fragile base | — Pending |
| Reuse watchdog hardening research | Research already done, high confidence | — Pending |
| EscalatingBackoff in rc-common | Shared between core and agent | — Pending |
| Email alerts via send_email.js shell-out | Reuses existing Gmail OAuth, no new deps | — Pending |

---
*Last updated: 2026-03-13 after milestone v1.0 scoping*
