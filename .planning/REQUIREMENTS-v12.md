# Requirements: Racing Point Operations Security (v12.0)

**Defined:** 2026-03-20
**Core Value:** No unauthorized actor can manipulate billing, launch sessions without payment, or access customer data.

## v1 Requirements

### Security Audit & Foundations

- [ ] **AUDIT-01**: Complete inventory of all exposed API endpoints with auth status (public/customer/staff/admin)
- [ ] **AUDIT-02**: PII data location audit — identify all places customer data is stored (SQLite, logs, bot messages, cloud sync payloads)
- [ ] **AUDIT-03**: Move JWT signing key and all secrets from racecontrol.toml to environment variables
- [ ] **AUDIT-04**: Generate cryptographically random JWT key on first run if not set
- [ ] **AUDIT-05**: Document current CORS, HTTPS, and auth state across all services

### API Authentication

- [ ] **AUTH-01**: JWT middleware enforcement on all billing endpoints — reject unauthenticated requests
- [ ] **AUTH-02**: JWT middleware enforcement on all session start/stop endpoints
- [ ] **AUTH-03**: Route classification middleware — public, customer, staff, admin tiers with appropriate auth checks
- [ ] **AUTH-04**: Rate limiting on all auth endpoints (PIN validation, OTP, login) — per-IP sliding window
- [ ] **AUTH-05**: Bot command authorization — verify wallet balance before session launch via Discord/WhatsApp
- [ ] **AUTH-06**: Service-to-service auth for pod agents (rc-agent → racecontrol) using HMAC shared secret
- [ ] **AUTH-07**: Cloud sync request signing — HMAC-SHA256 on sync payloads with timestamp + nonce for replay prevention

### Admin Panel Protection

- [ ] **ADMIN-01**: PIN/password gate on admin dashboard — no access without valid credential
- [ ] **ADMIN-02**: Admin PIN hashed with argon2 — no plaintext PIN storage
- [ ] **ADMIN-03**: Session timeout — auto-lock admin panel after 15 minutes of inactivity
- [ ] **ADMIN-04**: Admin action audit trail — log all wallet topups, pricing changes, session overrides, fleet exec, terminal commands
- [ ] **ADMIN-05**: WhatsApp alert on admin login and sensitive actions (wallet topup, fleet exec)
- [ ] **ADMIN-06**: Staff PIN rotation — alert if admin PIN unchanged for >30 days

### Session Integrity

- [ ] **SESS-01**: Session launch requires valid authenticated request — no anonymous session starts
- [ ] **SESS-02**: Auth tokens are single-use and time-bounded — prevent replay attacks
- [ ] **SESS-03**: Database transaction wrapping token consumption + billing creation to prevent TOCTOU races
- [ ] **SESS-04**: Session-scoped kiosk tokens — kiosk locks automatically when billing session ends
- [ ] **SESS-05**: Automated session pause on security anomaly (kiosk escape attempt, unauthorized process) with WhatsApp alert

### Transport Security (HTTPS)

- [ ] **TLS-01**: HTTPS for customer-facing PWA traffic (WiFi browser → server)
- [ ] **TLS-02**: Self-signed TLS certificate generation via rcgen for LAN
- [ ] **TLS-03**: Let's Encrypt TLS for cloud endpoints (racingpoint.cloud on Bono VPS)
- [ ] **TLS-04**: Dual-port support (HTTP 8080 + HTTPS 8443) for phased pod migration

### Kiosk & PWA Hardening

- [ ] **KIOSK-01**: Chrome kiosk flag lockdown — disable dev tools, extensions, file:// protocol
- [ ] **KIOSK-02**: Block keyboard shortcuts (Win+R, Alt+Tab, Ctrl+Alt+Del, Alt+F4) via low-level keyboard hook
- [ ] **KIOSK-03**: Disable USB mass storage on pod machines via Group Policy
- [ ] **KIOSK-04**: Disable Sticky Keys and accessibility escape vectors via registry
- [ ] **KIOSK-05**: PWA route protection — kiosk cannot access admin routes
- [ ] **KIOSK-06**: Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS) via middleware
- [ ] **KIOSK-07**: Network source tagging — different trust levels for wired LAN, WiFi, and WAN requests

### Data Protection

- [ ] **DATA-01**: AES-256-GCM encryption on PII columns (phone, email, name, guardian_phone) in SQLite
- [ ] **DATA-02**: Deterministic hash for phone number lookups (OTP matching) with separate reversible encryption for display
- [ ] **DATA-03**: Log redaction — scrub PII from application logs and bot messages
- [ ] **DATA-04**: Customer data export endpoint (JSON dump of customer's own data)
- [ ] **DATA-05**: Customer data deletion endpoint (cascade delete from drivers table)
- [ ] **DATA-06**: Encryption key management — separate from JWT key, stored securely, rotatable

## v2 Requirements

### Advanced Compliance

- **COMP-01**: Full DPDP Act compliance documentation and breach notification workflow
- **COMP-02**: Data retention policy — auto-purge customer data after configurable period
- **COMP-03**: Consent management — explicit opt-in for data collection at registration

### Network Security

- **NET-01**: VLAN segmentation — separate pod, staff, and customer WiFi networks
- **NET-02**: Rogue device detection on the LAN
- **NET-03**: Pod-to-server WebSocket encryption (if LAN threat model changes)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full PCI-DSS certification | Payments via third-party (UPI/Razorpay), no card numbers stored locally |
| Role-based access control (RBAC) | Single owner (Uday), binary admin/staff/customer model sufficient |
| External penetration testing | Premature before basic hardening — findings would be "you have no auth" |
| Biometric authentication | No biometric hardware, PIN + OTP sufficient for cafe scale |
| E2E encryption for all LAN traffic | Pods on private wired LAN, HTTPS for WiFi sufficient, LAN stays HTTP+HMAC |
| OAuth2/OIDC external identity provider | Phone OTP for customers exists, single-user admin PIN sufficient |
| Web Application Firewall (WAF) | Server not internet-exposed, CORS + rate limiting sufficient |
| Client-side encryption in PWA | Server-side encryption at rest + HTTPS in transit covers actual threats |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUDIT-01 | Phase 75 | Pending |
| AUDIT-02 | Phase 75 | Pending |
| AUDIT-03 | Phase 75 | Pending |
| AUDIT-04 | Phase 75 | Pending |
| AUDIT-05 | Phase 75 | Pending |
| AUTH-01 | Phase 76 | Pending |
| AUTH-02 | Phase 76 | Pending |
| AUTH-03 | Phase 76 | Pending |
| AUTH-04 | Phase 76 | Pending |
| AUTH-05 | Phase 76 | Pending |
| AUTH-06 | Phase 76 | Pending |
| AUTH-07 | Phase 80 | Pending |
| ADMIN-01 | Phase 76 | Pending |
| ADMIN-02 | Phase 76 | Pending |
| ADMIN-03 | Phase 76 | Pending |
| ADMIN-04 | Phase 80 | Pending |
| ADMIN-05 | Phase 80 | Pending |
| ADMIN-06 | Phase 80 | Pending |
| SESS-01 | Phase 76 | Pending |
| SESS-02 | Phase 76 | Pending |
| SESS-03 | Phase 76 | Pending |
| SESS-04 | Phase 78 | Pending |
| SESS-05 | Phase 78 | Pending |
| TLS-01 | Phase 77 | Pending |
| TLS-02 | Phase 77 | Pending |
| TLS-03 | Phase 77 | Pending |
| TLS-04 | Phase 77 | Pending |
| KIOSK-01 | Phase 78 | Pending |
| KIOSK-02 | Phase 78 | Pending |
| KIOSK-03 | Phase 78 | Pending |
| KIOSK-04 | Phase 78 | Pending |
| KIOSK-05 | Phase 78 | Pending |
| KIOSK-06 | Phase 77 | Pending |
| KIOSK-07 | Phase 78 | Pending |
| DATA-01 | Phase 79 | Pending |
| DATA-02 | Phase 79 | Pending |
| DATA-03 | Phase 79 | Pending |
| DATA-04 | Phase 79 | Pending |
| DATA-05 | Phase 79 | Pending |
| DATA-06 | Phase 79 | Pending |

**Coverage:**
- v1 requirements: 40 total
- Mapped to phases: 40
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after roadmap creation*
