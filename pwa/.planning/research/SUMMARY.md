# Project Research Summary

**Project:** Racing Point Cloud Platform
**Domain:** Cloud-connected venue management (sim racing)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Executive Summary

Racing Point is deploying three existing Next.js frontends (customer PWA, admin panel, live dashboard) to the cloud at `racingpoint.cloud` subdomains, backed by a cloud instance of the existing Rust `racecontrol` binary that already handles bidirectional SQLite sync with the on-premise server. The infrastructure approach is simple and well-validated: Caddy reverse proxy handles automatic TLS for all three subdomains, Docker Compose orchestrates four containers (Caddy + 3 Next.js apps), and the existing `cloud_sync.rs` handles data sync — no new sync technology is needed. The cloud layer is deliberately read-heavy and write-light: most writes happen locally at the venue, with cloud-authoritative writes limited to pricing, driver profiles, and bookings.

The key differentiator is the remote booking + PIN flow: a customer books from their phone, receives a 6-character PIN, and enters it at the kiosk — triggering automatic pod assignment and game launch with zero staff involvement. This flow requires one new database table (`reservations`) added to the sync layer, plus kiosk PIN entry UI. All other admin and dashboard features already exist; the cloud deploy is primarily a configuration and infrastructure problem, not a development problem.

The critical risks are financial (wallet sync conflicts causing double-charges or phantom credits), operational (double-booking of pods between remote and walk-in customers), and security (PIN brute force at kiosk). All three have well-defined mitigations: wallet debits use intent-based sync, pod assignment is deferred until PIN redemption (never at booking time), and PINs are 6-character alphanumeric with rate limiting. The infrastructure work is low-risk; the remote booking flow demands careful design.

## Key Findings

### Recommended Stack

The stack is almost entirely the existing stack reconfigured for cloud. Caddy replaces any need for Nginx + Certbot — a 10-line Caddyfile handles TLS provisioning, renewal, HTTP/2, HTTP/3, and subdomain routing for all three apps. Docker Compose (already in use) defines all four services. The existing `cloud_sync.rs` handles bidirectional SQLite sync and must NOT be replaced with CRDTs or Litestream — it expresses write-authority rules that generic sync libraries cannot.

**Core technologies:**
- **Caddy 2.11.x**: Reverse proxy + automatic TLS — zero-config cert management, replaces Nginx+Certbot entirely
- **Docker Compose v2**: Multi-container orchestration — already used, defines all 4 services in one file
- **node:22-alpine**: Container base image — multi-stage Dockerfile, ~150-200MB per app, already configured
- **cloud_sync.rs (custom)**: Bidirectional SQLite sync — purpose-built with authority rules, HMAC-signed, keep as-is
- **Cloudflare DNS (DNS-only)**: Subdomain routing — grey cloud mode required (orange cloud breaks ACME)
- **GitHub Actions (optional)**: CI/CD — start with manual SSH deploys; add automation only if deploy frequency warrants it

### Expected Features

**Must have (table stakes):**
- Remote booking with date/time selection — key reason customers need a cloud PWA
- Wallet top-up from anywhere — Razorpay integration exists; needs wallet sync
- Session history and leaderboards — endpoints exist; needs cloud data sync
- Admin panel at `admin.racingpoint.cloud` — remote revenue reports, pricing config, customer management
- Live dashboard at `dashboard.racingpoint.cloud` — pod status, today's revenue, active sessions for Uday
- Secure admin authentication — local trust-network auth is NOT sufficient for internet-exposed admin panel

**Should have (differentiators):**
- PIN-based zero-staff game launch — books remotely, enters PIN at kiosk, game auto-launches; no competitor does this
- AI racing coach (POST /customer/ai/chat) — already works; unique in sim racing venues
- Live telemetry in PWA — real-time during session; competitors show post-session only
- Driving passport and badges — gamification; already exists
- WhatsApp booking confirmation and PIN delivery — existing WhatsApp integration; needs booking templates

**Defer to v2+:**
- True real-time WebSocket relay for cloud dashboard — polling every 10-30s is sufficient for v1
- Multi-venue architecture — single venue only; multi-tenant adds massive complexity for zero current benefit
- Native mobile apps — PWA covers mobile; no app store process needed
- Real-time telemetry to cloud — too much bandwidth; cloud shows post-session summaries only

### Architecture Approach

The system is a two-tier architecture: a cloud layer on Bono's VPS (72.60.101.58) serving internet-facing frontends via the cloud `racecontrol` instance, and the existing local layer at `192.168.31.23` which remains the source of truth for all venue operations (pod control, billing, sessions). The two layers sync via `cloud_sync` every 2s (relay) or 30s (HTTP fallback). Cloud is authoritative for drivers, pricing, and the new `reservations` table. Local is authoritative for billing sessions, lap records, and pod state. Pod control never crosses the internet — all game launches happen LAN-side.

**Major components:**
1. **Caddy (VPS host)** — terminates TLS, routes `app.*` → :3100, `admin.*` → :3300, `dashboard.*` → :3200
2. **Cloud racecontrol (Rust, :8080)** — serves API to all 3 frontends, runs cloud_sync, holds cloud SQLite
3. **PWA (Next.js, :3100)** — customer-facing: booking, wallet, leaderboards, telemetry, passport
4. **Admin (Next.js, :3300)** — business management: revenue, pricing, bookings, customers
5. **Dashboard (Next.js, :3200)** — live ops: pod status (polled every 10-30s), revenue ticker
6. **Reservations table (NEW)** — cloud-authoritative sync table for remote bookings + PINs

### Critical Pitfalls

1. **Double-booking pods** — Remote and walk-in customers assigned the same pod. Prevention: cloud reservations are experience intents only (pod number = NULL until PIN redemption); pod assignment happens exclusively on the local server at redemption time.
2. **Wallet sync financial corruption** — Last-write-wins on wallet table can create phantom credits or double-charges during concurrent top-up (cloud) and billing (local). Prevention: wallets must be locally authoritative; cloud bookings submit debit intents, never mutate wallet directly.
3. **Anti-loop sync fragility** — Timestamp-based delta sync breaks under clock drift or SQLite triggers that auto-update `updated_at`. Prevention: add `origin` tag to sync payloads so receiving side skips rows that originated from itself.
4. **PIN brute force at kiosk** — 4-digit numeric PIN = 10,000 combinations, trivially exhausted. Prevention: 6-character alphanumeric PINs (2.1B combinations), 5 attempts/minute rate limit, 5-minute lockout after 10 failures, one-time use, 24h TTL.
5. **VPS memory exhaustion** — Three Next.js containers at 200-400MB each plus racecontrol plus Caddy may OOM a 4GB VPS. Prevention: set `mem_limit: 512m` per container in compose.yml, enable 2GB swap, alert if memory exceeds 80%.

## Implications for Roadmap

Based on research, the 6-phase build order from ARCHITECTURE.md is sound. Each phase unblocks the next and delivers standalone value.

### Phase 1: Cloud Infrastructure
**Rationale:** Nothing else can happen without DNS, TLS, and running containers. This is the hard dependency for all 5 subsequent phases.
**Delivers:** `app.racingpoint.cloud`, `admin.racingpoint.cloud`, `dashboard.racingpoint.cloud` live with HTTPS. Uday gets immediate remote access to admin and dashboard even before booking is built.
**Addresses:** DNS setup, Caddy config, compose.yml, environment wiring, WebSocket URL update (ws:// → wss://)
**Avoids:** Pitfall #5 (VPS memory — set container limits at deploy time), #8 (NEXT_PUBLIC env baked at build time — must be correct in compose.yml build args), #10 (DNS propagation — use Cloudflare for near-instant propagation)

### Phase 2: PWA Cloud Deploy
**Rationale:** PWA is the customer-facing product. Deploying it first validates that cloud_sync is working for the read-heavy flows (sessions, leaderboards, wallet balance) before adding the write-heavy booking flow.
**Delivers:** Customer app accessible from any device. Existing features (wallet, sessions, leaderboards, AI coach, telemetry, passport) work from cloud.
**Addresses:** All table-stakes PWA features (remote access to existing features)
**Avoids:** Low risk phase — mostly environment config; validates sync before booking complexity is added

### Phase 3: Sync Hardening
**Rationale:** Must happen before remote booking goes live. Wallet sync integrity and anti-loop protection are financial and operational correctness requirements, not nice-to-haves.
**Delivers:** Reliable bidirectional sync, wallet authority rules corrected, `reservations` table added to sync layer, anti-loop origin tags implemented.
**Addresses:** All admin and dashboard data availability (billing, pricing, pod state)
**Avoids:** Pitfall #2 (wallet corruption — fix authority rules here), #3 (anti-loop fragility — add origin tags here)

### Phase 4: Remote Booking + PIN Launch
**Rationale:** The primary differentiator. Requires Phase 3's reservations table in sync. The PIN flow is the most complex new development in the entire project.
**Delivers:** Customer books from phone, receives PIN via WhatsApp, enters PIN at kiosk, game auto-launches. Zero staff involvement.
**Addresses:** PIN-based zero-staff game launch, WhatsApp booking confirmation, reservation API on cloud, kiosk PIN entry UI
**Avoids:** Pitfall #1 (double-booking — pod-agnostic reservations, assignment at redemption only), #4 (PIN security — 6-char alphanumeric, rate limiting, one-time use)

### Phase 5: Admin + Dashboard Deploy
**Rationale:** Can run parallel to Phase 4. Admin and dashboard are read-heavy with no new data models. Admin auth hardening is required before internet exposure.
**Delivers:** Uday manages pricing, views reports, monitors pods from any device. Dashboard at cloud URL with polling-based pod status.
**Addresses:** All admin table-stakes features, dashboard pod status (polling v1), admin authentication hardening
**Avoids:** Pitfall #7 (WebSocket drops — Caddy handles natively, client-side reconnect), #9 (admin auth — JWT required before internet exposure)

### Phase 6: Operational Hardening
**Rationale:** Addresses edge cases that only manifest in production: extended outages, stale reservations, reservation conflicts between remote and walk-in bookings.
**Delivers:** Production-grade resilience. Split-brain handling, stale PIN cleanup, double-booking prevention policy, WhatsApp health alerts for cloud containers.
**Addresses:** Reservation conflict resolution, expired PIN cleanup, sync lag indicators
**Avoids:** Pitfall #6 (split-brain — pending_sync queue, "booking pending confirmation" state), #11 (stale reservations — background cleanup job every 5 minutes)

### Phase Ordering Rationale

- Phase 1 is the absolute prerequisite — no containers, no cloud access, no testing of anything else.
- Phases 2 and 3 together validate sync correctness with low-risk read flows before introducing financial writes.
- Phase 4 is sequenced after sync hardening because wallet debit intents and the reservations table must exist first.
- Phase 5 can overlap Phase 4 — admin and dashboard have no dependency on the booking flow.
- Phase 6 is post-launch hardening, not pre-launch blocking — the system works without it, but has known edge case exposure.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 4 (Remote Booking + PIN):** WhatsApp Business API template approval process is an external dependency with uncertain timeline. PIN delivery mechanism needs validated before building the booking confirmation flow.
- **Phase 3 (Sync Hardening):** Wallet debit intent schema needs design review against existing `cloud_sync.rs` merge logic. The exact wire format for intent-based debits is not yet defined.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Cloud Infrastructure):** Caddy + Docker Compose on Hetzner VPS is exhaustively documented. Config is provided in STACK.md verbatim.
- **Phase 2 (PWA Deploy):** Straightforward config — existing Dockerfile + updated NEXT_PUBLIC_API_URL. No new development.
- **Phase 5 (Admin + Dashboard):** Same pattern as Phase 2 with admin auth already existing (JWT middleware from Phase 76).

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Caddy + Docker Compose + existing sync — all proven in production for this scale. No new technology choices. |
| Features | MEDIUM-HIGH | Existing features well-understood. Remote booking flow is new but clearly scoped. WhatsApp template approval is an unknown variable. |
| Architecture | HIGH | Two-tier local+cloud with existing sync is the correct pattern. Reservations table design is clear. PIN flow sequence is fully specified. |
| Pitfalls | HIGH | All critical pitfalls have documented prevention strategies. Financial risk (wallet sync) and operational risk (double-booking) are well-defined with concrete mitigations. |

**Overall confidence:** HIGH

### Gaps to Address

- **racingpoint.cloud domain status:** Is the domain registered? Are DNS records pointed to 72.60.101.58? This is Phase 1 Day 1. If not registered, register immediately — propagation adds latency.
- **VPS firewall:** Hetzner VPS firewall must allow inbound 80/443. Verify before deploying Caddy.
- **Admin repo location:** `racingpoint-admin` is a separate repo. Compose.yml references it as `./racingpoint-admin` — needs to be cloned into the VPS alongside the main repo, or build images locally and push to a registry.
- **WhatsApp Business API templates:** Booking confirmation and PIN delivery messages require pre-approved templates. Submit templates early (Phase 3) so approval arrives before Phase 4 goes live.
- **Wallet authority rules:** Current `cloud_sync.rs` treats wallets as cloud-authoritative (top-ups happen on cloud). This conflicts with the correct model where local is authoritative for billing. Need to audit actual sync config before Phase 3.
- **Reservation conflict policy:** What happens when a customer books the last available slot remotely, and a walk-in attempts to book the same slot simultaneously? Local server wins (LAN operations take precedence), but the customer who booked remotely needs a resolution path (refund trigger, alternative slot offer).

## Sources

### Primary (HIGH confidence)
- `STACK.md` (2026-03-21) — Caddy, Docker Compose, cloud_sync, DNS, monitoring
- `ARCHITECTURE.md` (2026-03-21) — system overview, component boundaries, data flow, build order
- `PITFALLS.md` (2026-03-21) — 14 pitfalls with severity ratings and phase mappings
- `FEATURES.md` (2026-03-21) — table stakes, differentiators, anti-features, competitive comparison

### Secondary (MEDIUM confidence)
- Existing codebase knowledge — feature existence flags in FEATURES.md are based on known routes in `racingpoint-admin` and `web/`; actual implementation completeness should be verified during Phase 2 and 5 development

### Tertiary (LOW confidence — needs validation)
- WhatsApp Business API template approval timeline — assumed standard 24-48h; actual timing varies by region and message category
- VPS memory usage at runtime — estimates based on Next.js standalone build patterns; actual usage depends on traffic and Node.js heap behavior

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
