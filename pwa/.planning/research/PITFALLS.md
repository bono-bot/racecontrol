# Pitfalls: Cloud-Synced Venue Platform

**Project:** Racing Point Cloud Platform
**Researched:** 2026-03-21
**Overall confidence:** HIGH

## Critical Pitfalls

### 1. Double-Booking Pods (Severity: CRITICAL)

**The problem:** Current `pod_reservation.rs` assigns pods locally with no awareness of cloud bookings. If a customer books remotely (cloud) and a walk-in customer books at the kiosk (local) at the same time, both could be assigned the same pod.

**Warning signs:**
- Two customers showing up for the same pod at the same time
- Pod assignment logic that directly picks pod numbers at booking time

**Prevention strategy:**
- Cloud reservations must be **pod-agnostic intents** — customer books an experience, NOT a specific pod
- Pod assignment happens ONLY at PIN redemption time on the local server
- Local server is the single source of truth for pod availability
- Reservation = "I want to play X experience for Y minutes" — pod number is NULL until redeemed

**Phase mapping:** Phase 4 (Remote Booking + PIN Launch) — core design constraint

---

### 2. Wallet Sync Financial Risk (Severity: CRITICAL)

**The problem:** Wallet balances sync bidirectionally. If a customer tops up on cloud (Razorpay) while simultaneously billing locally (session charges), last-write-wins can create phantom credits or double-charges.

**Warning signs:**
- Wallet balance discrepancies between cloud and local
- Customer complaints about being charged twice or having incorrect balance
- Sync conflicts on the `wallets` table

**Prevention strategy:**
- Wallets should be **locally authoritative** — local server is the billing source of truth
- Cloud bookings that debit wallet should use **"debit intents"** — a request sent via sync, not a direct wallet mutation
- Local server processes debit intent, applies charge, syncs updated balance back to cloud
- Alternative: wallet top-ups go through local server (cloud proxies to local Razorpay endpoint)

**Phase mapping:** Phase 3 (Sync Hardening) — must fix before remote booking goes live

---

### 3. Anti-Loop Protection Fragility (Severity: HIGH)

**The problem:** Existing `cloud_sync.rs` uses timestamp-based delta sync to avoid sync loops (A syncs to B, B syncs same data back to A). This breaks under:
- Clock drift between cloud and local servers
- SQLite triggers that auto-update `updated_at` on any write
- Schema changes that alter timestamp columns

**Warning signs:**
- Sync log showing repeated pushes of the same data
- CPU/bandwidth spike from sync loop
- Database growing unexpectedly

**Prevention strategy:**
- Add `origin` tag to sync payloads (e.g., `"origin": "cloud"` or `"origin": "local"`)
- Receiving side skips rows that originated from itself
- Verify no SQLite triggers auto-update `updated_at` on synced tables
- Add sync cycle counter to detect loops early

**Phase mapping:** Phase 3 (Sync Hardening)

---

### 4. PIN Brute Force on Kiosk (Severity: HIGH)

**The problem:** If PINs are 4-digit numeric (10,000 combinations), a malicious user at the kiosk could try all combinations in minutes. Kiosks have no rate limiting today.

**Warning signs:**
- Rapid PIN entry attempts from a single kiosk
- Unauthorized session starts

**Prevention strategy:**
- Use 6-character alphanumeric PINs (2.1 billion combinations)
- Rate limit: max 5 PIN attempts per minute per kiosk
- Lock out kiosk PIN entry for 5 minutes after 10 failed attempts
- PINs expire after configurable TTL (e.g., 24 hours)
- PINs are one-time use — mark as redeemed immediately
- Log all PIN attempts for audit trail

**Phase mapping:** Phase 4 (Remote Booking + PIN Launch) and Phase 6 (Operational Hardening)

---

### 5. VPS Memory Exhaustion (Severity: HIGH)

**The problem:** Three Next.js standalone containers each use 200-400MB RAM. Plus cloud racecontrol (~50MB) and Caddy (~20MB). On a 4GB VPS, this could OOM.

**Warning signs:**
- Container restarts with OOM kill in `dmesg`
- Slow response times from cloud apps
- Docker events showing `oom-kill`

**Prevention strategy:**
- Set memory limits per container in compose.yml: `mem_limit: 512m` for each Next.js app
- Enable swap on VPS: `fallocate -l 2G /swapfile && mkswap /swapfile && swapon /swapfile`
- Monitor with `docker stats` — set up WhatsApp alert if total memory > 80%
- If 4GB is insufficient, upgrade to CX32 (8GB, ~EUR 8/month)
- Consider: could admin and dashboard share a single Next.js app with route-based separation?

**Phase mapping:** Phase 1 (Cloud Infrastructure) — configure limits at deploy time

---

### 6. Split-Brain During Extended Outages (Severity: HIGH)

**The problem:** If internet goes down for hours, both cloud and local continue accepting writes independently. When connectivity returns, conflicting data must be reconciled.

**Warning signs:**
- Sync backlog growing during outage
- Conflicting timestamps on same records post-reconnection
- Cloud bookings made during outage referencing stale availability

**Prevention strategy:**
- Cloud bookings during outage go to `pending_sync` queue with explicit status
- Customer sees "booking pending confirmation" (not "booking confirmed") when sync is down
- Cloud checks sync lag: if last sync > 60s, show "venue status unavailable" on dashboard
- Post-reconnection: local server reviews pending_sync queue, confirms or rejects each
- Existing hysteresis (3 failures before relay-down, 2 successes before relay-up) is good — keep it

**Phase mapping:** Phase 6 (Operational Hardening)

---

## Medium Pitfalls

### 7. WebSocket Connection Drops

**The problem:** If Caddy or nginx proxies WebSocket for the dashboard, default timeouts kill idle connections after 60 seconds.

**Prevention:** Caddy handles WebSocket natively (no special config needed). If using nginx, add `proxy_read_timeout 3600s` and `proxy_set_header Upgrade/Connection` headers. Client-side: reconnect with exponential backoff.

**Phase mapping:** Phase 5 (Dashboard Deploy)

### 8. NEXT_PUBLIC_API_URL Baked at Build Time

**The problem:** Next.js `NEXT_PUBLIC_*` env vars are embedded at `next build` time, not runtime. If the API URL changes, you must rebuild the Docker image.

**Prevention:** This is already handled — existing Dockerfile passes `NEXT_PUBLIC_API_URL` as a build ARG. Just make sure the compose.yml build args point to the correct cloud URLs.

**Phase mapping:** Phase 1 (Cloud Infrastructure)

### 9. Admin Panel Without Proper Auth

**The problem:** racingpoint-admin may have minimal auth locally (trusted network). Exposing it at admin.racingpoint.cloud without proper auth exposes business data to the internet.

**Prevention:**
- Admin panel MUST require authentication before any page loads
- Use existing JWT admin auth or add PIN-based admin login
- Consider IP allowlisting via Caddy (Tailscale IPs only)
- Rate limit login attempts

**Phase mapping:** Phase 5 (Admin Deploy)

### 10. DNS Propagation Delays

**The problem:** After adding A records for subdomains, DNS propagation can take up to 48 hours. Caddy's ACME challenges fail if DNS hasn't propagated.

**Prevention:**
- Use Cloudflare DNS (near-instant propagation)
- Set low TTL (300s) initially
- Verify DNS resolution before starting Caddy: `dig app.racingpoint.cloud`
- Caddy retries ACME challenges automatically

**Phase mapping:** Phase 1 (Cloud Infrastructure)

### 11. Stale Reservation Cleanup

**The problem:** Expired PINs/reservations accumulate in the database if not cleaned up. Cloud shows outdated availability.

**Prevention:**
- Background job: mark reservations as expired if `expires_at < now()` and status = pending
- Run every 5 minutes on both cloud and local
- Sync the status change to the other side
- Free up any held resources (wallet debits reversed for expired bookings)

**Phase mapping:** Phase 6 (Operational Hardening)

## Low Pitfalls

### 12. Docker Image Size Bloat

**Prevention:** Already handled — multi-stage Dockerfile with standalone output. Keep `node_modules` out of final image.

### 13. Caddy Certificate Rate Limits

**Prevention:** Let's Encrypt allows 50 certificates per domain per week. With 3-4 subdomains, this is never an issue. Caddy's `caddy_data` volume persists certs across restarts.

### 14. Time Zone Inconsistencies

**Prevention:** Store all timestamps as UTC in SQLite. Display in IST on frontends. Existing pattern — already followed.

## Summary: Pitfalls by Phase

| Phase | Pitfalls to Address |
|-------|-------------------|
| 1. Cloud Infrastructure | #5 (VPS memory), #8 (build-time env), #10 (DNS) |
| 2. PWA Deploy | — (mostly config, low risk) |
| 3. Sync Hardening | #2 (wallet sync), #3 (anti-loop) |
| 4. Remote Booking + PIN | #1 (double-booking), #4 (PIN security) |
| 5. Admin + Dashboard Deploy | #7 (WebSocket), #9 (admin auth) |
| 6. Operational Hardening | #6 (split-brain), #11 (stale reservations) |

---

*Pitfalls research: 2026-03-21*
