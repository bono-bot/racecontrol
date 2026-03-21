# Architecture: Cloud-Synced Venue Platform

**Project:** Racing Point Cloud Platform
**Researched:** 2026-03-21
**Overall confidence:** HIGH

## System Overview

```
                        INTERNET
                           |
                     [Caddy Reverse Proxy]
                     (auto-TLS, Hetzner VPS)
                    /        |        \
              app.*      admin.*    dashboard.*
                |            |           |
            [PWA]      [Admin]     [Dashboard]
            :3100       :3300       :3200
                \          |          /
                 [Cloud racecontrol]
                      :8080
                    (SQLite)
                       |
              [cloud_sync - HMAC signed]
              (relay 2s / HTTP 30s fallback)
                       |
                 [Local racecontrol]
                      :8080
                    (SQLite)
                    /    \
              [Kiosk]  [8 Pods via rc-agent]
              :3300     :8090 each
```

## Component Boundaries

### Cloud Layer (Bono's VPS - 72.60.101.58)

| Component | Port | Role | Talks To |
|-----------|------|------|----------|
| Caddy | 80, 443 | Reverse proxy, auto-TLS, subdomain routing | All 3 frontends |
| PWA (Next.js) | 3100 | Customer-facing app | Cloud racecontrol API |
| Admin (Next.js) | 3300 | Business admin panel | Cloud racecontrol API |
| Dashboard (Next.js) | 3200 | Live operations | Cloud racecontrol API (WebSocket) |
| Cloud racecontrol (Rust) | 8080 | API server, sync endpoint | Local racecontrol (via cloud_sync) |

### Local Layer (Server 192.168.31.23)

| Component | Port | Role | Talks To |
|-----------|------|------|----------|
| Local racecontrol | 8080 | Source of truth for sessions, billing, pods | Cloud racecontrol, pods, kiosk |
| Kiosk (Next.js) | 3300 | On-site staff/customer UI | Local racecontrol API |
| rc-agent (per pod) | 8090 | Pod lifecycle, game launch | Local racecontrol (WebSocket) |
| Comms-link relay | — | Fast sync relay (2s) | Both racecontrol instances |

## Data Flow

### Sync Authority Model (existing, correct)

| Data | Authority | Direction | Rationale |
|------|-----------|-----------|-----------|
| Drivers/profiles | Cloud | Cloud -> Local | Customers register from anywhere |
| Pricing tiers/rules | Cloud | Cloud -> Local | Uday configures pricing remotely |
| Billing rates | Cloud | Cloud -> Local | Admin sets rates |
| Kiosk experiences | Cloud | Cloud -> Local | Admin configures catalog |
| Kiosk settings | Cloud | Cloud -> Local | Admin controls kiosk behavior |
| Auth tokens | Cloud | Cloud -> Local | Login happens on cloud |
| Wallets | Cloud | Cloud -> Local | Top-up happens on cloud |
| Billing sessions | Local | Local -> Cloud | Sessions created at venue |
| Lap records | Local | Local -> Cloud | Laps recorded at venue |
| Pod state | Local | Local -> Cloud | Pods are LAN-only |
| **Reservations** (NEW) | Cloud | Cloud -> Local | Remote booking creates reservation |

### Remote Booking + PIN Flow (new)

```
Customer (phone)                Cloud                    Local                 Kiosk
     |                            |                        |                    |
     |-- POST /customer/book ---->|                        |                    |
     |                            |-- create reservation --|                    |
     |                            |   (PIN generated)      |                    |
     |                            |-- sync reservation --->|                    |
     |<-- PIN + confirmation -----|                        |                    |
     |                            |                        |                    |
     |   (customer arrives)       |                        |                    |
     |                            |                        |<-- enter PIN ------|
     |                            |                        |-- validate PIN --->|
     |                            |                        |-- assign pod ----->|
     |                            |                        |-- launch game ---->|
     |                            |                        |-- sync session --->|
     |                            |<-- sync session -------|                    |
```

Key design decisions:
- PIN is generated on **cloud** (where booking happens)
- PIN is synced to **local** via reservations table in cloud_sync
- PIN is validated on **local** server (kiosk talks to local server only)
- Pod assignment + game launch happen on **local** (LAN-only operations)
- Session result syncs back to cloud for customer to view in PWA

### Dashboard Data Flow

Two options for cloud dashboard:

**Option A: Polling (recommended for v1)**
```
Cloud dashboard --poll every 10s--> Cloud racecontrol --read from SQLite--> synced pod state
```
- Simple, reliable, no WebSocket relay needed
- Data is 10-30s stale (acceptable for Uday monitoring from phone)
- Pod state already syncs via cloud_sync every 2-30s

**Option B: WebSocket relay (future)**
```
Cloud dashboard --WebSocket--> Cloud racecontrol --broadcast synced state--> connected clients
```
- Cloud racecontrol broadcasts to its own WS clients when sync data arrives
- Does NOT tunnel WebSocket through to venue (would be fragile)
- Better UX but more complexity

### Admin Data Flow

```
Admin panel --> Cloud racecontrol API --> SQLite (cloud)
                                           |
                                     cloud_sync (2-30s)
                                           |
                                      SQLite (local)
                                           |
                                    Local racecontrol applies changes
```

Admin writes are cloud-authoritative: pricing, experiences, kiosk settings. Changes sync to local automatically. No special handling needed — existing cloud_sync already handles this.

## Container Orchestration

### Docker Compose on VPS

```yaml
# compose.yml on Bono's VPS
services:
  racecontrol:
    # Rust binary, already deployed
    # Runs cloud_sync, serves API at :8080

  pwa:
    build: ./pwa
    expose: ["3100"]
    # NEXT_PUBLIC_API_URL=https://app.racingpoint.cloud/api/v1

  admin:
    build: ./racingpoint-admin
    expose: ["3300"]
    # NEXT_PUBLIC_API_URL=https://admin.racingpoint.cloud/api/v1

  dashboard:
    build: ./web
    expose: ["3200"]
    # NEXT_PUBLIC_API_URL=https://dashboard.racingpoint.cloud/api/v1
```

- `expose` (not `ports`) — containers only reachable through Caddy
- Caddy runs on host (not Docker) — simpler cert management, avoids Docker network complexity
- Each frontend is a standalone Next.js container (~150-200MB each)

### Caddy on Host

```caddyfile
app.racingpoint.cloud {
    reverse_proxy localhost:3100
}

admin.racingpoint.cloud {
    reverse_proxy localhost:3300
}

dashboard.racingpoint.cloud {
    reverse_proxy localhost:3200
}

api.racingpoint.cloud {
    reverse_proxy localhost:8080
}
```

If Caddy runs on host, frontends need `ports` (not `expose`) mapped to localhost.
If Caddy runs in Docker, use Docker service names and `expose`.

## Critical Gap: Reservations Table

Current SYNC_TABLES: `drivers, wallets, pricing_tiers, pricing_rules, billing_rates, kiosk_experiences, kiosk_settings, auth_tokens`

**Missing for remote booking:**
- `reservations` — cloud-authoritative. Fields: id, driver_id, experience_id, pin, status (pending/redeemed/expired/cancelled), created_at, expires_at, pod_number (null until redeemed)
- PIN: 6-character alphanumeric, expires after configurable TTL (e.g., 24h)
- Status transitions: pending -> redeemed (PIN entered at kiosk) | expired (TTL passed) | cancelled (customer cancels)

## Suggested Build Order

```
Phase 1: Cloud Infrastructure
  DNS A records + Caddy + compose.yml + HTTPS
  (unblocks everything)
     |
Phase 2: PWA Cloud Deploy
  Deploy pwa/ at app.racingpoint.cloud
  Verify customer flows work with synced data
     |
Phase 3: Sync Hardening
  Add reservations table to cloud_sync
  Verify all admin/dashboard data syncs correctly
  Handle edge cases (stale data, conflicts)
     |
Phase 4: Remote Booking + PIN Launch  ──┐
  Reservation API on cloud              |
  PIN generation + WhatsApp delivery    |
  Kiosk PIN entry UI                    |
  Local PIN validation + pod assignment |
                                        |
Phase 5: Admin + Dashboard Deploy  ─────┘ (parallel with Phase 4)
  Deploy racingpoint-admin at admin.racingpoint.cloud
  Deploy web/ at dashboard.racingpoint.cloud
  Admin auth for remote access
     |
Phase 6: Operational Hardening
  Rate limiting on PIN entry
  Stale reservation cleanup
  Health monitoring + WhatsApp alerts
  Double-booking prevention
```

## Open Questions

- Caddy on host vs in Docker? Host recommended for simpler cert management.
- Cloudflare proxy mode (orange cloud) vs DNS-only? DNS-only recommended for Let's Encrypt ACME.
- Admin repo is separate (racingpoint-admin). Use git submodule, or build images from separate clone?
- Dashboard WebSocket: polling v1, WebSocket relay v2?

---

*Architecture research: 2026-03-21*
