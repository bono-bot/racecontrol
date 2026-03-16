# RaceControl

## Current State

**Shipped:** v1.0 RaceControl HUD & Safety (2026-03-13), v2.0 Kiosk URL Reliability (2026-03-14)

The pod management stack is reliable: self-healing, branded screens, stable URLs, staff dashboard controls. v2.0 delivered server IP pinning, lock screen hardening, Edge hardening, pod lockdown UI, and session results display.

## Current Milestone: v5.5 Billing Credits

**Goal:** Replace hardcoded paise billing tiers with DB-driven credits (1 credit = ₹1 = 100 paise), non-retroactive 3-tier per-minute rates configurable from the admin panel without a code deploy.

**Target features:**
- Credits as currency: all UI shows "X cr" instead of "Rs. X" (overlay, kiosk, admin, history)
- DB-driven billing rates: `billing_rates` table with tier_order, tier_name, threshold_minutes, rate_per_min_paise
- Non-retroactive tier calculation: 45 min = (30 × 25) + (15 × 20) = 1050 cr, not 45 × 20
- In-memory rate cache on BillingManager — refreshed at startup and every 60s
- Admin panel rate editor: staff update rates via UI, cache invalidated immediately
- Rates synced to cloud via cloud_sync.rs SYNC_TABLES

## What This Is

The RaceControl platform for Racing Point eSports — 8 sim racing pods managed from a central server (racecontrol, rc-agent, pod-agent), staff kiosk for pod management, and a cloud PWA for customer engagement. Captures lap times, sector splits, and telemetry from Assetto Corsa and F1 25, with leaderboards, competitive events, and driver profiles accessible publicly.

## Core Value

Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.

## Requirements

### Validated (v1.0 + v2.0 — Shipped)

- ✓ WebSocket connection between racecontrol and rc-agent — existing
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

- ✓ Server IP pinning and DHCP reservation (HOST-01 through HOST-04) — v2.0
- ✓ Pod lock screen hardening with startup connecting state (LOCK-01 through LOCK-03) — v2.0
- ✓ Edge browser hardening (EDGE-01 through EDGE-03) — v2.0
- ✓ Staff dashboard lockdown and power controls (KIOSK-01, KIOSK-02, PWR-01 through PWR-06) — v2.0
- ✓ Customer experience branding and session results (BRAND-01 through BRAND-03, SESS-01 through SESS-03) — v2.0

### Completed (v4.0 — shipped 2026-03-16)

- ✓ rc-agent as Windows Service with auto-restart on crash (SVC-01 through SVC-04)
- ✓ WebSocket-based remote exec for pod management when HTTP blocked (WSEX-01 through WSEX-04)
- ✓ Firewall auto-configuration in Rust on startup (FW-01 through FW-03)
- ✓ Startup error capture and reporting to racecontrol (HEAL-01 through HEAL-04)
- ✓ Self-healing config (detect and repair missing toml/bat/registry) (HEAL-01 through HEAL-04)
- ✓ Deploy resilience (verify, rollback, handle partial failures) (DEPL-01 through DEPL-05)
- ✓ Fleet health dashboard for Uday (real-time pod status) (FLEET-01 through FLEET-03)

### Completed (v5.0 — shipped 2026-03-17)

- ✓ Bot handles pod crash/hang — detect + auto-kill/restart game or rc-agent without staff (Phase 24)
- ✓ Bot handles billing edge cases — stuck sessions, idle drift, cloud sync failures (Phase 25)
- ✓ Bot handles network/connection drops — WS loss, server unreachable, IP drift (Phase 23)
- ✓ Bot handles USB hardware failures — wheelbase disconnect/reconnect, FFB fault (Phase 24)
- ✓ Bot handles game launch failures — CM hang, AC timeout, launch auto-retry (Phase 24)
- ✓ Bot handles telemetry gaps — detect missing UDP data, alert on persistent drop (Phase 26)
- ✓ Bot handles multiplayer issues — desync detection, safe teardown or auto-rejoin (Phase 26)
- ✓ Bot handles kiosk PIN failures — validation errors, staff unlock, session recovery (Phase 26)
- ✓ Bot handles lap filtering — auto-flag invalid laps, separate hotlap vs practice (Phase 26)

### Active (v5.5)

- [ ] Credits replace INR in all user-facing UI — overlay, kiosk, billing history, admin
- [ ] `billing_rates` DB table with 3 configurable tiers (non-retroactive)
- [ ] BillingManager holds in-memory rate cache refreshed at startup and every 60s
- [ ] `compute_session_cost()` rewritten with non-retroactive additive algorithm, accepts tiers param
- [ ] Admin panel Per-Minute Rates table with inline editing
- [ ] billing_rates added to SYNC_TABLES for cloud replication

### Paused (v3.0 — resume after v5.0)

- [ ] Hotlap events with staff creation and car class rankings
- [ ] Group event results with F1-style auto-scoring
- [ ] Multi-round championship system
- [ ] Driver profiles with class rating and lap history
- [ ] Telemetry visualization with speed trace, lap comparison, and track map
- [ ] Driver skill rating system
- [ ] Public access to all competitive data (no login required)

### Out of Scope

- HUD overlay features — deferred (archived in .planning/archive/hud-safety/)
- FFB safety — deferred (archived research available)
- New game integrations — current sims only (AC, F1 25)
- Real-time chat or messaging between drivers
- Mobile native app — PWA only
- External payment/wallet top-up changes — only billing rate calculation changes, not payment provider integration
- Venue kiosk changes — v3.0 targets cloud PWA only

## Context

- **Venue:** 8 gaming pods (192.168.31.x subnet), 1 server (.23), 1 James workstation (.27)
- **Stack:** Rust/Axum (racecontrol port 8080, rc-agent per-pod), Node.js (pod-agent port 8090), Next.js (kiosk + PWA)
- **Crates:** rc-common (shared types/protocol), racecontrol (server), rc-agent (pod client)
- **Cloud:** app.racingpoint.cloud (72.60.101.58, Bono's VPS) — existing cloud_sync pushes laps, track records, driver stats
- **Existing data foundations:** laps table (sector1/2/3_ms, valid flag), personal_bests, track_records, telemetry_samples, group_sessions, friendships, drivers (total_laps, total_time_ms)
- **Existing API endpoints:** /leaderboard/{track}, /public/leaderboard, /public/laps/{id}/telemetry, /sessions, /laps
- **PWA scaffolds exist:** leaderboard, telemetry, coaching, tournaments pages (mostly empty)
- **Inspiration:** rps.racecentres.com — Track of the Month, Group Events, Championships, Circuit/Vehicle Records, Driver Data

## Constraints

- **Rust/Axum:** racecontrol and rc-agent must stay Rust — no language change
- **Pod-agent:** MERGED into rc-agent (v3.0 Phase 13.1) — remote_ops.rs module on port 8090
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

## Future Milestone Candidates

- HUD overlay with live sector times and telemetry
- FFB safety (zero wheelbase torque on session boundary)
- Cloud dashboard for remote monitoring
- On-site deployment automation improvements
- Kiosk spectator leaderboard display (venue TV screens)

| Merge pod-agent into rc-agent | Eliminates 2-process dependency, simplifies deploy | ✓ Good — deployed all 8 pods |
| HKLM Run key for Session 1 GUI | Ensures rc-agent starts in user session at login | ⚠️ Revisit — no crash restart, needs Windows Service |
| Batch file firewall rules | netsh in .bat scripts for port 8090 | ⚠️ Revisit — CRLF bug silently breaks rules, move to Rust |

---
*Last updated: 2026-03-17 after milestone v5.5 Billing Credits started*
