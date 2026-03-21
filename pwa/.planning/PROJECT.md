# Racing Point Cloud Platform

## What This Is

A cloud-accessible platform that mirrors Racing Point's local venue operations to the internet. Three web properties — app.racingpoint.cloud (customer PWA), admin.racingpoint.cloud (business admin), and dashboard.racingpoint.cloud (live operations) — all synced bidirectionally with the local racecontrol server via Bono's VPS. Customers can book and pay remotely, arrive at the venue, enter a PIN at the kiosk, and start racing with zero staff interaction. Uday can monitor and manage the entire business from his phone.

## Core Value

Customers book and pay from anywhere, walk in with a PIN, and race — while Uday sees everything live from his phone without being on-site.

## Requirements

### Validated

- Customer login via phone + WhatsApp OTP — existing
- Session booking with duration tiers — existing
- Wallet top-up via Razorpay — existing
- Lap records, telemetry, leaderboards — existing
- Cloud sync (drivers, wallets, pricing, billing, laps) — existing via cloud_sync.rs
- HMAC-signed sync payloads — existing (AUTH-07)
- Relay mode (2s) + HTTP fallback (30s) sync — existing

### Active

- [ ] Deploy PWA at app.racingpoint.cloud — customers access from anywhere
- [ ] Deploy racingpoint-admin at admin.racingpoint.cloud — Uday manages business remotely
- [ ] Deploy web/ dashboard at dashboard.racingpoint.cloud — live pod grid, revenue, telemetry
- [ ] Remote booking flow: customer books from PWA at home, gets PIN
- [ ] PIN-based game launch: customer enters PIN at venue kiosk, pod assigned, game launches
- [ ] Cloud-local sync hardening: ensure all required tables sync, handle conflicts
- [ ] Admin business ops: pricing config, driver management, revenue reports accessible remotely
- [ ] Dashboard live ops: real-time pod status, billing timers, session alerts via WebSocket
- [ ] Sync missing tables: add any tables not yet in SYNC_TABLES that admin/dashboard need
- [ ] Game launch from PWA: select experience + car/track from phone, reservation holds until arrival

### Out of Scope

- Moving pod control to cloud — pods are LAN-only, controlled via rc-agent on local network
- Real-time telemetry streaming to cloud — too much bandwidth, stays venue-only
- Payment processing on cloud — Razorpay stays on local server (webhook delivery)
- Replacing local racecontrol server — cloud mirrors, does not replace
- Mobile native apps — PWA serves mobile users

## Context

**Architecture today:**
- Local server (192.168.31.23:8080) runs racecontrol — the source of truth for billing, sessions, pod state
- Cloud VPS (72.60.101.58, Bono) runs a racecontrol instance at app.racingpoint.cloud
- cloud_sync.rs does bidirectional sync: cloud authoritative for drivers/pricing, local authoritative for billing/laps/game state
- Sync uses HMAC-SHA256 signed payloads with replay prevention (5-min window)
- Relay mode (via comms-link) syncs every 2s; HTTP fallback every 30s

**Three web properties:**
- `pwa/` (port 3100) — customer-facing Next.js 16 app. Phone login, booking, wallet, sessions, leaderboards, telemetry, AI coach, friends, tournaments. Currently served locally only.
- `web/` (port 3200) — live operations dashboard. WebSocket-connected, real-time pod grid, telemetry bars, lap feed, billing timers. Currently served locally at server .23.
- `racingpoint-admin/` (separate repo) — business admin panel. Bookings, customers, finance, pricing, coupons, memberships, tournaments, HR, analytics. 27 route groups. Currently served locally.

**PIN-based game launch flow (new):**
1. Customer opens app.racingpoint.cloud on phone at home
2. Books experience (selects game, car/track, duration)
3. Gets a reservation with a PIN code
4. Arrives at venue, goes to any available kiosk
5. Enters PIN on kiosk screen
6. Kiosk validates PIN with local server, assigns pod, launches game
7. Customer sits down and races — no staff interaction needed

**Cloud sync data flow:**
- Cloud pushes to local: driver registrations, pricing changes, wallet top-ups
- Local pushes to cloud: billing sessions, lap records, game state, pod status
- Current SYNC_TABLES: drivers, wallets, pricing_tiers, pricing_rules, billing_rates, kiosk_experiences, kiosk_settings, auth_tokens

## Constraints

- **Network**: Cloud VPS is on Hetzner (72.60.101.58), venue on local WiFi (192.168.31.x). Sync must handle intermittent connectivity gracefully.
- **Data authority**: Cloud cannot directly control pods — all pod commands go through local server. Cloud is read-only for session/pod state.
- **Existing infra**: Bono manages cloud deploys. James manages local. Coordinate via comms-link.
- **Stack**: Next.js 16 + React 19 + TypeScript for all frontends. Rust/Axum backend. SQLite databases (local + cloud).
- **Security**: All cloud endpoints must use HTTPS. Admin panel requires authentication. Sync payloads are HMAC-signed.
- **Docker**: All frontends deploy as Docker containers (node:22-alpine, standalone output).

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Three separate domains vs single app | Each audience (customer, admin, ops) has different needs and auth | -- Pending |
| Cloud mirrors local (not replaces) | Pods are LAN-only, latency-sensitive. Cloud adds remote access, not replaces local control | -- Pending |
| PIN-based launch (not QR-only) | Simpler UX — customer types 4-6 digit PIN, no camera needed on kiosk | -- Pending |
| Razorpay stays on local server | Webhook delivery, existing integration. Cloud wallet sync handles balance propagation | -- Pending |

---
*Last updated: 2026-03-21 after initialization*
