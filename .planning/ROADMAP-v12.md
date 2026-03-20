# Roadmap: Racing Point Operations Security (v12.0)

## Overview

Lock down the Racing Point operations stack in 6 phases, starting with a security audit to understand what's exposed, then closing the two biggest holes (open API + open admin panel), adding transport encryption, hardening the kiosk attack surface, encrypting customer PII at rest, and finishing with audit trails for compliance. Each phase delivers a complete, verifiable security layer. Phases 75-80, continuing from v11.0.

## Phases

**Phase Numbering:**
- Integer phases (75, 76, ...): Planned milestone work
- Decimal phases (76.1, 76.2): Urgent insertions (marked with INSERTED)

- [ ] **Phase 75: Security Audit & Foundations** - Inventory all exposed endpoints, trace PII locations, move secrets to env vars
- [ ] **Phase 76: API Authentication & Admin Protection** - JWT enforcement on all sensitive routes, admin PIN gate, rate limiting, bot auth
- [ ] **Phase 77: Transport Security** - HTTPS for browser traffic, self-signed LAN certs, Let's Encrypt for cloud, security headers
- [ ] **Phase 78: Kiosk & Session Hardening** - Chrome lockdown, hotkey blocking, USB disable, session-scoped tokens, anomaly detection
- [ ] **Phase 79: Data Protection** - PII column encryption, phone hash for lookups, log redaction, data export/deletion
- [ ] **Phase 80: Audit Trail & Defense in Depth** - Admin action logging, WhatsApp alerts, PIN rotation, cloud sync signing

## Phase Details

### Phase 75: Security Audit & Foundations
**Goal**: Complete understanding of the current security posture and secure secret management before any auth work begins
**Depends on**: Nothing (first phase of v12.0)
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04, AUDIT-05
**Success Criteria** (what must be TRUE):
  1. Every API route (80+) has a documented classification: public, customer, staff, admin, or service
  2. Every location where customer PII is stored or logged is identified (SQLite columns, log files, bot messages, cloud sync payloads, localStorage)
  3. JWT signing key and all secrets load from environment variables, not from racecontrol.toml
  4. A cryptographically random JWT key is auto-generated on first run if no key is set
  5. CORS, HTTPS, and auth state is documented for every service (racecontrol, rc-agent, kiosk, dashboard, cloud)
**Plans**: TBD

Plans:
- [ ] 75-01: TBD
- [ ] 75-02: TBD

### Phase 76: API Authentication & Admin Protection
**Goal**: No unauthenticated request can manipulate billing, start sessions, or access the admin panel
**Depends on**: Phase 75
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, AUTH-06, ADMIN-01, ADMIN-02, ADMIN-03, SESS-01, SESS-02, SESS-03
**Success Criteria** (what must be TRUE):
  1. A curl request to any billing or session endpoint without a valid JWT returns 401 Unauthorized
  2. The admin dashboard requires a PIN/password before any page loads -- no content visible without authentication
  3. Admin PIN is stored as an argon2 hash -- no plaintext PIN exists anywhere in config or database
  4. After 5 failed PIN/OTP attempts from an IP, further attempts are rate-limited (429 response)
  5. A Discord/WhatsApp bot command to start a session checks wallet balance before launching -- zero-balance users are rejected
  6. Pod agent endpoints (8090/8091) reject requests without valid HMAC signatures
  7. Session launch atomically deducts balance and creates billing record -- no race condition can produce a free session
**Plans**: TBD

Plans:
- [ ] 76-01: TBD
- [ ] 76-02: TBD
- [ ] 76-03: TBD
- [ ] 76-04: TBD
- [ ] 76-05: TBD

### Phase 77: Transport Security
**Goal**: All browser-to-server traffic (PWA and admin dashboard) is encrypted in transit
**Depends on**: Phase 76
**Requirements**: TLS-01, TLS-02, TLS-03, TLS-04, KIOSK-06
**Success Criteria** (what must be TRUE):
  1. Customer PWA loads over HTTPS -- browser shows secure connection indicator
  2. Admin dashboard loads over HTTPS on the LAN
  3. Cloud endpoints (racingpoint.cloud) serve valid Let's Encrypt TLS certificates
  4. Pods can be migrated one-by-one from HTTP to HTTPS via dual-port support (8080 HTTP + 8443 HTTPS)
  5. Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS) are present on all HTML responses
**Plans**: TBD

Plans:
- [ ] 77-01: TBD
- [ ] 77-02: TBD
- [ ] 77-03: TBD

### Phase 78: Kiosk & Session Hardening
**Goal**: A customer sitting at a pod cannot escape the kiosk, access other users' data, or keep a session running after payment expires
**Depends on**: Phase 76
**Requirements**: KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04, KIOSK-05, KIOSK-07, SESS-04, SESS-05
**Success Criteria** (what must be TRUE):
  1. Chrome DevTools, extensions, file:// protocol, and address bar are inaccessible on pod kiosk browsers
  2. Win+R, Alt+Tab, Ctrl+Alt+Del, Alt+F4, and Sticky Keys shortcuts are blocked on pod machines
  3. USB mass storage devices are rejected when plugged into pod machines
  4. Kiosk PWA cannot navigate to /admin or /staff routes -- server rejects with 403
  5. When a billing session ends, the kiosk locks automatically within 10 seconds -- no continued access
  6. A kiosk escape attempt (unauthorized process detected, DevTools open) triggers automatic session pause and WhatsApp alert
**Plans**: TBD

Plans:
- [ ] 78-01: TBD
- [ ] 78-02: TBD
- [ ] 78-03: TBD

### Phase 79: Data Protection
**Goal**: Customer PII is encrypted at rest and scrubbed from logs, with self-service data export and deletion
**Depends on**: Phase 77
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, DATA-05, DATA-06
**Success Criteria** (what must be TRUE):
  1. Opening the SQLite database directly shows encrypted (unreadable) values for phone, email, name, and guardian_phone columns
  2. OTP login still works -- phone number lookup uses a deterministic hash, display uses reversible decryption
  3. Application logs and bot messages contain no raw phone numbers, emails, or names -- all PII is redacted
  4. A customer can request a JSON export of their own data via the PWA
  5. A customer can request deletion of their account and all associated data
**Plans**: TBD

Plans:
- [ ] 79-01: TBD
- [ ] 79-02: TBD
- [ ] 79-03: TBD

### Phase 80: Audit Trail & Defense in Depth
**Goal**: Every sensitive admin action is logged and alertable, with remaining security gaps closed
**Depends on**: Phase 76
**Requirements**: ADMIN-04, ADMIN-05, ADMIN-06, AUTH-07
**Success Criteria** (what must be TRUE):
  1. Every wallet topup, pricing change, session override, fleet exec, and terminal command is recorded in an append-only audit_log table with timestamp, actor, and action details
  2. Admin login and sensitive actions (wallet topup, fleet exec) trigger a WhatsApp notification to Uday
  3. If the admin PIN has not been changed in 30+ days, Uday receives an alert prompting rotation
  4. Cloud sync payloads are signed with HMAC-SHA256 including timestamp and nonce -- replayed or tampered payloads are rejected
**Plans**: TBD

Plans:
- [ ] 80-01: TBD
- [ ] 80-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 75 → 76 → 77 → 78 → 79 → 80
Note: Phase 78 (Kiosk) depends only on Phase 76, not 77 -- it can run in parallel with Phase 77 if desired.

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 75. Security Audit & Foundations | 0/2 | Not started | - |
| 76. API Authentication & Admin Protection | 0/5 | Not started | - |
| 77. Transport Security | 0/3 | Not started | - |
| 78. Kiosk & Session Hardening | 0/3 | Not started | - |
| 79. Data Protection | 0/3 | Not started | - |
| 80. Audit Trail & Defense in Depth | 0/2 | Not started | - |

---
*Roadmap created: 2026-03-20*
*Last updated: 2026-03-20*
