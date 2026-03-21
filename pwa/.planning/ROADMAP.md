# Roadmap: Racing Point Cloud Platform

## Overview

Deploy three existing web properties (customer PWA, admin panel, live dashboard) to racingpoint.cloud subdomains, add remote booking with PIN-based zero-staff game launch, and harden the cloud-local sync layer for production reliability. The journey starts with infrastructure (DNS, Caddy, Docker Compose), deploys the customer-facing PWA first to validate sync, hardens the sync layer for financial correctness, builds the remote booking + PIN flow (the key differentiator), deploys admin and dashboard for Uday's remote management, then adds CI/CD, monitoring, and operational hardening.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Cloud Infrastructure** - DNS, Caddy reverse proxy, Docker Compose, firewall, and swap on VPS
- [ ] **Phase 2: API + PWA Cloud Deploy** - Customer PWA and cloud API live at racingpoint.cloud with HTTPS
- [x] **Phase 3: Sync Hardening** - Reservations table, wallet authority, anti-loop tags, sync health endpoint
- [ ] **Phase 4: Remote Booking + PIN Generation** - Customer books from phone, receives 6-char PIN via WhatsApp
- [ ] **Phase 5: Kiosk PIN Launch** - Customer enters PIN at venue kiosk, pod assigned, game auto-launches
- [ ] **Phase 6: Admin Panel Cloud Deploy** - Business admin panel live at admin.racingpoint.cloud
- [ ] **Phase 7: Dashboard Cloud Deploy** - Live ops dashboard at dashboard.racingpoint.cloud
- [ ] **Phase 8: CI/CD Pipeline** - Automated build and deploy on push to main
- [ ] **Phase 9: Health Monitoring + Alerts** - Container health checks with WhatsApp alerts on failure
- [ ] **Phase 10: Operational Hardening** - Split-brain handling, rate limiting, production edge cases

## Phase Details

### Phase 1: Cloud Infrastructure
**Goal**: All racingpoint.cloud subdomains resolve, terminate TLS, and route to running containers on the VPS
**Depends on**: Nothing (first phase)
**Requirements**: INFRA-01, INFRA-02, INFRA-03, INFRA-06, INFRA-07
**Success Criteria** (what must be TRUE):
  1. Visiting app.racingpoint.cloud, admin.racingpoint.cloud, dashboard.racingpoint.cloud, and api.racingpoint.cloud in a browser shows HTTPS with valid Let's Encrypt certificates
  2. All four containers (Caddy + 3 frontends) are running via Docker Compose with memory limits and healthchecks
  3. VPS firewall blocks all inbound ports except 80 and 443
  4. VPS has 2GB swap enabled and containers survive under memory pressure
**Plans**: TBD

Plans:
- [ ] 01-01: TBD
- [ ] 01-02: TBD

### Phase 2: API + PWA Cloud Deploy
**Goal**: Customers can access the PWA from any device and use existing features (login, wallet, sessions, leaderboards) via the cloud API
**Depends on**: Phase 1
**Requirements**: PWA-01, PWA-02, PWA-03, PWA-04, PWA-05, API-01, API-02
**Success Criteria** (what must be TRUE):
  1. Customer can open app.racingpoint.cloud on their phone and log in with phone + WhatsApp OTP
  2. Customer can view their profile, wallet balance, session history, and leaderboards from the cloud PWA
  3. Customer can top up wallet via Razorpay from the cloud PWA and see updated balance after sync
  4. PWA is installable to home screen (manifest, service worker, icons all working)
  5. All existing customer API endpoints at api.racingpoint.cloud return correct synced data
**Plans**: TBD

Plans:
- [ ] 02-01: TBD
- [ ] 02-02: TBD

### Phase 3: Sync Hardening
**Goal**: Cloud-local sync is financially correct, loop-free, and exposes health status for all tables needed by admin and dashboard
**Depends on**: Phase 2
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-06, SYNC-07
**Success Criteria** (what must be TRUE):
  1. Reservations table syncs cloud-to-local (cloud-authoritative) and a booking created on cloud appears on local server within one sync cycle
  2. Wallet debit for a booking uses intent pattern — cloud sends debit request, local processes it, balance syncs back correctly without double-charge
  3. Sync payloads include origin tags and the receiving side skips rows that originated from itself (no sync loops)
  4. When sync lag exceeds 60 seconds, cloud UI shows "booking pending confirmation" status
  5. Sync health endpoint at api.racingpoint.cloud/sync/status returns last sync timestamp, lag, and relay status
**Plans**: 3 plans

Plans:
- [x] 03-01-PLAN.md — Schema foundation: reservations + debit_intents tables, origin_id config, SCHEMA_VERSION bump
- [x] 03-02-PLAN.md — Sync integration: sync_changes/sync_push handlers, origin filter, debit intent processing
- [x] 03-03-PLAN.md — Sync health enhancement: lag_seconds, health status tiers, per-table staleness

### Phase 4: Remote Booking + PIN Generation
**Goal**: Customer can book an experience from their phone at home and receive a PIN for venue redemption
**Depends on**: Phase 3
**Requirements**: BOOK-01, BOOK-02, BOOK-03, BOOK-04, BOOK-05, BOOK-06, BOOK-07, API-04
**Success Criteria** (what must be TRUE):
  1. Customer can select a game, car/track, and duration tier from the PWA and complete a booking
  2. Booking creates a pod-agnostic reservation (no pod assigned) and generates a 6-character alphanumeric PIN displayed to the customer
  3. Customer receives PIN via WhatsApp message after booking
  4. Customer can view, cancel, or modify their reservation from the PWA before arriving
  5. Expired reservations (past 24h TTL) are automatically cleaned up and wallet is refunded if debited
**Plans**: 3 plans

Plans:
- [ ] 04-01-PLAN.md — Backend reservation module + API endpoints (create/get/modify/cancel) + PIN generation + WhatsApp delivery
- [ ] 04-02-PLAN.md — Scheduler expiry cleanup + automatic wallet refund for expired reservations
- [ ] 04-03-PLAN.md — PWA remote booking flow + /reservations management page

### Phase 5: Kiosk PIN Launch
**Goal**: Customer enters PIN at venue kiosk and the game auto-launches on an assigned pod with zero staff interaction
**Depends on**: Phase 4
**Requirements**: KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04, KIOSK-05, KIOSK-06
**Success Criteria** (what must be TRUE):
  1. Kiosk displays a PIN entry screen where walk-in customers with remote bookings can type their PIN
  2. Valid PIN triggers automatic pod assignment (first available) and game launch — customer sees assigned pod number and loading status
  3. PIN is one-time use and marked as redeemed immediately on successful validation
  4. PIN entry is rate-limited: max 5 attempts per minute, lockout after 10 failures
**Plans**: TBD

Plans:
- [ ] 05-01: TBD
- [ ] 05-02: TBD

### Phase 6: Admin Panel Cloud Deploy
**Goal**: Uday can manage all business operations remotely from admin.racingpoint.cloud
**Depends on**: Phase 3
**Requirements**: ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04, ADMIN-05, API-03
**Success Criteria** (what must be TRUE):
  1. Admin panel at admin.racingpoint.cloud requires authentication before any page loads
  2. Uday can view revenue reports, booking history, and customer data from his phone
  3. Uday can configure pricing tiers, experiences, and kiosk settings remotely and changes sync to local server
  4. All existing admin API endpoints work correctly on the cloud instance with synced data
**Plans**: TBD

Plans:
- [ ] 06-01: TBD
- [ ] 06-02: TBD

### Phase 7: Dashboard Cloud Deploy
**Goal**: Uday can monitor live venue operations from dashboard.racingpoint.cloud
**Depends on**: Phase 3
**Requirements**: DASH-01, DASH-02, DASH-03, DASH-04, DASH-05
**Success Criteria** (what must be TRUE):
  1. Dashboard at dashboard.racingpoint.cloud requires authentication (admin-only)
  2. Dashboard shows real-time pod status grid for all 8 pods, updated via polling
  3. Dashboard shows today's revenue, active sessions, and billing timers
  4. Dashboard shows connection status indicator reflecting cloud-to-local sync health
**Plans**: TBD

Plans:
- [ ] 07-01: TBD
- [ ] 07-02: TBD

### Phase 8: CI/CD Pipeline
**Goal**: Pushing to main automatically builds and deploys all services to the VPS
**Depends on**: Phase 1
**Requirements**: INFRA-04
**Success Criteria** (what must be TRUE):
  1. Pushing a commit to main in GitHub triggers a GitHub Actions workflow that builds Docker images and deploys them to the VPS via SSH
  2. Failed builds do not deploy — only successful builds reach production
**Plans**: TBD

Plans:
- [ ] 08-01: TBD

### Phase 9: Health Monitoring + Alerts
**Goal**: Container failures and resource exhaustion are detected and reported automatically via WhatsApp
**Depends on**: Phase 1
**Requirements**: INFRA-05
**Success Criteria** (what must be TRUE):
  1. When a container crashes, restarts, or goes OOM, a WhatsApp alert is sent to Uday within 2 minutes
  2. Container healthchecks detect unresponsive services and trigger automatic restart
**Plans**: TBD

Plans:
- [ ] 09-01: TBD

### Phase 10: Operational Hardening
**Goal**: Production edge cases (extended outages, brute force, sync conflicts) are handled gracefully
**Depends on**: Phase 5, Phase 6
**Requirements**: SYNC-05, API-05
**Success Criteria** (what must be TRUE):
  1. During an extended internet outage, cloud bookings queue as pending_sync and local server confirms them post-reconnection without data loss
  2. Authentication endpoints (login, OTP verify, PIN entry) are rate-limited to prevent brute force attacks
  3. After connectivity is restored, pending bookings resolve within two sync cycles
**Plans**: TBD

Plans:
- [ ] 10-01: TBD
- [ ] 10-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order. Phases 6 and 7 can run in parallel (both depend on Phase 3, not on each other). Phases 8 and 9 can run anytime after Phase 1.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Cloud Infrastructure | 0/2 | Not started | - |
| 2. API + PWA Cloud Deploy | 0/2 | Not started | - |
| 3. Sync Hardening | 3/3 | Complete | 2026-03-21 |
| 4. Remote Booking + PIN Generation | 0/3 | Not started | - |
| 5. Kiosk PIN Launch | 0/2 | Not started | - |
| 6. Admin Panel Cloud Deploy | 0/2 | Not started | - |
| 7. Dashboard Cloud Deploy | 0/2 | Not started | - |
| 8. CI/CD Pipeline | 0/1 | Not started | - |
| 9. Health Monitoring + Alerts | 0/1 | Not started | - |
| 10. Operational Hardening | 0/2 | Not started | - |
