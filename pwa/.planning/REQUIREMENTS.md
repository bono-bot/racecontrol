# Requirements: Racing Point Cloud Platform

**Defined:** 2026-03-21
**Core Value:** Customers book and pay from anywhere, walk in with a PIN, and race — while Uday sees everything live from his phone without being on-site.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Infrastructure

- [ ] **INFRA-01**: DNS A records for app, admin, dashboard, api subdomains pointing to VPS (72.60.101.58)
- [ ] **INFRA-02**: Caddy reverse proxy routes each subdomain to correct container with auto-TLS (Let's Encrypt)
- [ ] **INFRA-03**: Docker Compose orchestrates all services (Caddy + 3 frontends + racecontrol) with memory limits and healthchecks
- [ ] **INFRA-04**: GitHub Actions CI/CD pipeline: push to main triggers build + deploy to VPS via SSH
- [ ] **INFRA-05**: Container health monitoring with WhatsApp alerts on failures or OOM
- [ ] **INFRA-06**: VPS firewall allows ports 80/443 inbound, all other ports blocked externally
- [ ] **INFRA-07**: Swap enabled on VPS (2GB) to prevent OOM with 3 Next.js containers

### PWA Deployment

- [ ] **PWA-01**: Customer PWA serves at app.racingpoint.cloud with HTTPS
- [ ] **PWA-02**: PWA connects to cloud racecontrol API at api.racingpoint.cloud
- [ ] **PWA-03**: Customer can login, view profile, wallet, sessions, leaderboards from cloud
- [ ] **PWA-04**: Customer can top up wallet via Razorpay from cloud PWA
- [ ] **PWA-05**: PWA installable to home screen (manifest, service worker, icons)

### Admin Deployment

- [ ] **ADMIN-01**: Business admin panel serves at admin.racingpoint.cloud with HTTPS
- [ ] **ADMIN-02**: Admin panel requires authentication before any page loads (secure remote access)
- [ ] **ADMIN-03**: Uday can view revenue reports, booking history, customer data remotely
- [ ] **ADMIN-04**: Uday can configure pricing tiers, experiences, and kiosk settings remotely
- [ ] **ADMIN-05**: Admin changes sync to local server via cloud_sync (cloud-authoritative)

### Dashboard Deployment

- [ ] **DASH-01**: Live ops dashboard serves at dashboard.racingpoint.cloud with HTTPS
- [ ] **DASH-02**: Dashboard shows real-time pod status grid (all 8 pods) updated via polling or WebSocket
- [ ] **DASH-03**: Dashboard shows today's revenue, active sessions, billing timers
- [ ] **DASH-04**: Dashboard requires authentication (admin-only access)
- [ ] **DASH-05**: Dashboard shows connection status indicator (cloud-to-local sync health)

### Remote Booking

- [ ] **BOOK-01**: Customer can book an experience from PWA at home (select game, car/track, duration tier)
- [ ] **BOOK-02**: Booking creates a pod-agnostic reservation (no specific pod assigned at booking time)
- [ ] **BOOK-03**: 6-character alphanumeric PIN generated on booking, displayed to customer
- [ ] **BOOK-04**: PIN delivered to customer via WhatsApp message
- [ ] **BOOK-05**: Customer can view, cancel, or modify their reservation from PWA
- [ ] **BOOK-06**: Reservations expire after configurable TTL (default: 24 hours)
- [ ] **BOOK-07**: Expired reservations auto-cleaned up with wallet refund if debited

### Kiosk PIN Launch

- [ ] **KIOSK-01**: Kiosk displays PIN entry screen for walk-in customers with remote bookings
- [ ] **KIOSK-02**: PIN validated against local server's synced reservations
- [ ] **KIOSK-03**: Valid PIN triggers pod assignment (first available) and game launch
- [ ] **KIOSK-04**: Rate limiting on PIN entry: max 5 attempts per minute, lockout after 10 failures
- [ ] **KIOSK-05**: PIN is one-time use — marked as redeemed immediately on successful validation
- [ ] **KIOSK-06**: Customer sees assigned pod number and game loading status after PIN entry

### Sync Hardening

- [x] **SYNC-01**: Reservations table added to cloud_sync (cloud-authoritative)
- [x] **SYNC-02**: Wallet uses debit intent pattern — cloud sends debit request, local processes and syncs balance back
- [x] **SYNC-03**: Origin tags added to sync payloads to prevent sync loops
- [ ] **SYNC-04**: Cloud shows "booking pending confirmation" when sync lag exceeds 60 seconds
- [ ] **SYNC-05**: Split-brain handling: cloud bookings during outage queue as pending_sync, local confirms post-reconnection
- [ ] **SYNC-06**: All admin-managed tables (pricing, experiences, settings) sync correctly cloud-to-local
- [ ] **SYNC-07**: Sync health endpoint exposed at api.racingpoint.cloud/sync/status (last sync timestamp, lag, relay status)

### API

- [ ] **API-01**: Cloud racecontrol API accessible at api.racingpoint.cloud with HTTPS
- [ ] **API-02**: All existing customer API endpoints work on cloud instance with synced data
- [ ] **API-03**: All existing admin API endpoints work on cloud instance
- [ ] **API-04**: New reservation endpoints: create, cancel, modify, redeem (PIN validation)
- [ ] **API-05**: Rate limiting on authentication endpoints (login, OTP verify, PIN entry)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Real-Time Dashboard

- **DASH-V2-01**: WebSocket relay from local to cloud for sub-second dashboard updates
- **DASH-V2-02**: Live telemetry streaming to cloud for spectator viewing

### Notifications

- **NOTF-01**: Push notifications for session end, wallet low balance, friend requests
- **NOTF-02**: Email receipts for wallet top-ups and session completions

### Multi-Venue

- **MULTI-01**: Tenant isolation for multiple venues
- **MULTI-02**: Cross-venue leaderboards

## Out of Scope

| Feature | Reason |
|---------|--------|
| Cloud pod control | Pods are LAN-only, latency-sensitive. Cloud is read-only for pod state. |
| Native mobile apps | PWA covers mobile. App store adds weeks with no value. |
| Cloud payment processing | Razorpay stays on local server. Wallet sync handles balance propagation. |
| Replacing local server | Cloud mirrors, does not replace. Venue runs independently if internet drops. |
| Real-time telemetry to cloud | Bandwidth prohibitive (60fps). Post-session summaries only. |
| Customer chat support | AI coach + WhatsApp handles this. No in-app chat widget. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INFRA-01 | Phase 1 | Pending |
| INFRA-02 | Phase 1 | Pending |
| INFRA-03 | Phase 1 | Pending |
| INFRA-04 | Phase 8 | Pending |
| INFRA-05 | Phase 9 | Pending |
| INFRA-06 | Phase 1 | Pending |
| INFRA-07 | Phase 1 | Pending |
| PWA-01 | Phase 2 | Pending |
| PWA-02 | Phase 2 | Pending |
| PWA-03 | Phase 2 | Pending |
| PWA-04 | Phase 2 | Pending |
| PWA-05 | Phase 2 | Pending |
| ADMIN-01 | Phase 6 | Pending |
| ADMIN-02 | Phase 6 | Pending |
| ADMIN-03 | Phase 6 | Pending |
| ADMIN-04 | Phase 6 | Pending |
| ADMIN-05 | Phase 6 | Pending |
| DASH-01 | Phase 7 | Pending |
| DASH-02 | Phase 7 | Pending |
| DASH-03 | Phase 7 | Pending |
| DASH-04 | Phase 7 | Pending |
| DASH-05 | Phase 7 | Pending |
| BOOK-01 | Phase 4 | Pending |
| BOOK-02 | Phase 4 | Pending |
| BOOK-03 | Phase 4 | Pending |
| BOOK-04 | Phase 4 | Pending |
| BOOK-05 | Phase 4 | Pending |
| BOOK-06 | Phase 4 | Pending |
| BOOK-07 | Phase 4 | Pending |
| KIOSK-01 | Phase 5 | Pending |
| KIOSK-02 | Phase 5 | Pending |
| KIOSK-03 | Phase 5 | Pending |
| KIOSK-04 | Phase 5 | Pending |
| KIOSK-05 | Phase 5 | Pending |
| KIOSK-06 | Phase 5 | Pending |
| SYNC-01 | Phase 3 | Complete |
| SYNC-02 | Phase 3 | Complete |
| SYNC-03 | Phase 3 | Complete |
| SYNC-04 | Phase 3 | Pending |
| SYNC-05 | Phase 10 | Pending |
| SYNC-06 | Phase 3 | Pending |
| SYNC-07 | Phase 3 | Pending |
| API-01 | Phase 2 | Pending |
| API-02 | Phase 2 | Pending |
| API-03 | Phase 6 | Pending |
| API-04 | Phase 4 | Pending |
| API-05 | Phase 10 | Pending |

**Coverage:**
- v1 requirements: 47 total
- Mapped to phases: 47
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation*
