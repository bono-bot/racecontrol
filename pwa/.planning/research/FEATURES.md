# Features: Cloud-Connected Venue Management

**Project:** Racing Point Cloud Platform
**Researched:** 2026-03-21
**Overall confidence:** MEDIUM-HIGH

## Table Stakes (must have or users leave)

### Customer PWA (app.racingpoint.cloud)

| Feature | Complexity | Exists? | Notes |
|---------|-----------|---------|-------|
| Remote booking with date/time selection | Medium | Partial | Booking exists but is venue-only. Need remote access + time slot reservation. |
| Wallet top-up from anywhere | Low | Yes | Razorpay integration exists. Cloud needs wallet sync. |
| Session history & receipts | Low | Yes | Sessions page exists. Needs cloud data sync. |
| Leaderboards & lap records | Low | Yes | Public leaderboard endpoints exist. |
| Profile management | Low | Yes | Profile CRUD exists. |
| Push notifications (booking confirmation, PIN) | Medium | No | WhatsApp OTP exists. Need booking confirmation + PIN delivery. |
| Mobile-responsive design | Low | Yes | PWA is mobile-first. |

**Dependencies:** All require cloud_sync to have driver, wallet, and session data available on cloud instance.

### Admin Panel (admin.racingpoint.cloud)

| Feature | Complexity | Exists? | Notes |
|---------|-----------|---------|-------|
| Revenue reports & daily summaries | Low | Yes | Finance routes exist in racingpoint-admin. |
| Customer/driver management | Low | Yes | Customers routes exist. |
| Pricing configuration | Low | Yes | Pricing routes exist. |
| Booking management & calendar | Low | Yes | Bookings + calendar routes exist. |
| Coupon & discount management | Low | Yes | Coupons routes exist. |
| Membership/package management | Low | Yes | Memberships + packages routes exist. |
| Authentication (admin login) | Medium | Partial | Auth exists locally. Cloud admin needs secure remote auth. |

**Dependencies:** All require cloud_sync to have billing, pricing, and customer data available.

### Live Dashboard (dashboard.racingpoint.cloud)

| Feature | Complexity | Exists? | Notes |
|---------|-----------|---------|-------|
| Real-time pod status grid | Medium | Yes | WebSocket-based, exists in web/. Needs cloud WebSocket relay. |
| Today's revenue ticker | Low | Yes | Billing timers exist. |
| Active session monitoring | Low | Yes | Pod cards show active sessions. |
| Connection status indicator | Low | Yes | WebSocket connected/disconnected indicator exists. |

**Dependencies:** WebSocket relay from local to cloud is the key technical challenge. Dashboard is read-only so sync direction is one-way (local -> cloud).

## Differentiators (competitive advantage)

| Feature | Complexity | Why It Matters |
|---------|-----------|----------------|
| **PIN-based zero-staff game launch** | High | Customer books remotely, enters PIN at kiosk, game auto-launches. No staff interaction. Competitors (ROLLER, ShiftOS) require staff dockets or manual assignment. |
| **AI racing coach** | Low (exists) | POST /customer/ai/chat already works. Unique in sim racing venues. |
| **Live telemetry in PWA** | Low (exists) | Real-time speed/throttle/brake during session. Competitors show only post-session stats. |
| **Driving passport & badges** | Low (exists) | Gamification with tracks visited, cars driven, achievements. |
| **Friend system & multiplayer** | Low (exists) | Social features with friend requests, group sessions. |
| **Local-first with cloud sync** | Medium (exists) | Venue keeps running if internet drops. Competitors are either cloud-only (fragile) or local-only (no remote access). |

## Anti-Features (deliberately NOT building)

| Feature | Why Not |
|---------|---------|
| **Cloud pod control** | Pods are LAN-only. Latency over internet would make game launches unreliable. Cloud is read-only for pod state. |
| **Real-time telemetry to cloud** | Too much bandwidth (60 frames/sec). Telemetry stays venue-local. Cloud shows post-session summaries only. |
| **Cloud payment processing** | Razorpay webhooks are configured for local server. Moving payments to cloud adds complexity with no benefit. Wallet balance syncs via cloud_sync. |
| **Native mobile apps** | PWA covers mobile use case. App store approval process adds weeks. PWA can be installed to home screen. |
| **Multi-venue architecture** | Single venue. Multi-tenant adds massive complexity (tenant isolation, per-venue config, cross-venue leaderboards). Build only if/when second venue opens. |
| **Customer live chat** | AI coach handles racing questions. General support via WhatsApp (existing channel). No need for in-app chat widget. |
| **Real-time cloud dashboard** | True real-time (sub-second) requires WebSocket relay with low latency. Polling every 10-30s is sufficient for Uday monitoring from phone. Much simpler to implement. |

## Feature Dependencies

```
Cloud Deploy (DNS + Caddy + Docker)
  |
  +-- PWA at app.racingpoint.cloud
  |     |
  |     +-- Remote Booking (needs sync: experiences, pricing)
  |     |     |
  |     |     +-- PIN Generation (new: reservation table in sync)
  |     |           |
  |     |           +-- Kiosk PIN Entry (new: kiosk UI + validation)
  |     |                 |
  |     |                 +-- Auto Game Launch (existing: pod assignment)
  |     |
  |     +-- Wallet Top-up (needs sync: wallets)
  |     +-- Sessions/Leaderboards (needs sync: sessions, laps)
  |
  +-- Admin at admin.racingpoint.cloud
  |     |
  |     +-- Admin Auth (secure remote login)
  |     +-- Business Reports (needs sync: billing, sessions)
  |     +-- Pricing Config (cloud-authoritative, syncs to local)
  |
  +-- Dashboard at dashboard.racingpoint.cloud
        |
        +-- Pod Status (needs: polling/WebSocket relay from local)
        +-- Revenue Feed (needs sync: billing data)
```

## MVP Recommendation

**Three phases that each deliver standalone value:**

1. **Cloud Deploy** — Get existing apps serving at cloud domains with HTTPS. This alone gives Uday remote access to admin + dashboard.
2. **Remote Booking + PIN** — The key differentiator. Customer books from phone, gets PIN, enters at kiosk. Requires new reservation sync + kiosk PIN UI.
3. **Sync Hardening + Notifications** — Add missing sync tables, handle edge cases (double-booking, stale PINs, network partitions), add WhatsApp booking confirmations.

## Competitive Positioning

| Competitor | Remote Booking | Zero-Staff Launch | Cloud Dashboard | Live Telemetry |
|-----------|---------------|-------------------|-----------------|----------------|
| ShiftOS | Yes (web) | No (staff assigns) | Yes | No |
| VMS V5 | No (walk-in only) | No | Basic | No |
| ROLLER | Yes (web) | No (QR docket to staff) | Yes | No |
| Parafait | Yes (web) | No (POS terminal) | Yes | No |
| **Racing Point** | **Yes (PWA)** | **Yes (PIN)** | **Yes (real-time)** | **Yes (in-session)** |

## Open Questions

- **Reservation conflict resolution:** What happens if customer books Pod 3 remotely but someone walks in and takes it? Need conflict policy.
- **PIN security:** 6-digit alphanumeric = 2.1B combinations (sufficient), but need rate limiting on kiosk input.
- **WebSocket vs polling for cloud dashboard:** True WebSocket relay or 10-30s polling?
- **WhatsApp Business API:** Transactional messages (booking confirmations, PINs) may require message template approval.

---

*Features research: 2026-03-21*
