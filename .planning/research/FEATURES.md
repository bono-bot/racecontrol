# Feature Landscape: eSports Cafe Operations Security

**Domain:** Security hardening for eSports cafe operations (billing, kiosk, admin, customer data)
**Researched:** 2026-03-20
**Confidence:** HIGH (based on codebase audit + industry research + India DPDP Act requirements)

---

## Table Stakes

Features that are non-negotiable. Without these, the system is actively vulnerable to exploitation by anyone on the LAN.

| # | Feature | Why Expected | Complexity | Notes |
|---|---------|--------------|------------|-------|
| TS-1 | **API authentication on billing/session endpoints** | Anyone on the LAN can `curl` billing/start, add credits, launch sessions. Direct financial loss vector. All 80+ API routes are completely open. | Medium | Shared secret or API key for server-to-server (agent, cloud sync); JWT for customer-facing. Current state: zero middleware protection on any route. |
| TS-2 | **Admin panel authentication** | Admin panel has zero auth. Any device on the network can access wallet topups, pricing changes, pod control, fleet exec, terminal commands. | Low | Simple PIN/password gate. Uday-only access is sufficient. Session cookie or short-lived JWT after PIN entry. Staff already have `/staff/validate-pin` and `/employee/daily-pin` -- just not enforced as middleware. |
| TS-3 | **HTTPS for PWA and admin traffic** | Customer PII (phone, name, email), PINs, and JWTs transit in plaintext over HTTP. Trivially sniffable on shared WiFi. | Medium | Self-signed cert for LAN is acceptable. Let's Encrypt for cloud endpoints (racingpoint.cloud). Kiosk-to-server on wired LAN is lower priority than PWA traffic over WiFi. |
| TS-4 | **Customer JWT enforcement on all /customer/* routes** | JWT infrastructure exists (jsonwebtoken crate, Claims struct, `extract_driver_id` helper) but routes do not uniformly enforce it via middleware. Customer endpoints may be accessible without a valid token. | Low | Axum middleware layer that rejects requests without valid JWT on all `/customer/*` routes. The infrastructure is already built -- just needs consistent enforcement via `axum_mw::from_fn`. |
| TS-5 | **Rate limiting on auth endpoints** | PIN validation and OTP endpoints have no rate limiting. Brute-force a 4-6 digit PIN in minutes. Customer PIN lockout exists (5 attempts per CUSTOMER_PIN_LOCKOUT_THRESHOLD) but no IP/device-level throttle exists on staff or admin endpoints. | Low | Per-IP rate limit on `/auth/validate-pin`, `/customer/login`, `/customer/verify-otp`, `/staff/validate-pin`. tower-governor crate or simple in-memory counter with sliding window. |
| TS-6 | **Kiosk escape prevention hardening** | Current kiosk.rs has process allowlisting (good), but known escape vectors are unaddressed: keyboard shortcuts (Win+R, Ctrl+Alt+Del, Alt+Tab, Alt+F4), USB mass storage attacks, file dialog escapes, Sticky Keys accessibility exploit, file:// protocol in browser, barcode/QR scanner emulation. | Medium | Disable hotkeys via low-level keyboard hook (Windows SetWindowsHookEx API). Disable USB mass storage via Group Policy (already noted as pending in CLAUDE.md). Block file:// protocol in Chrome kiosk flags. Disable Sticky Keys via registry. |
| TS-7 | **PII storage audit and encryption** | Phone numbers, emails, names, guardian phone stored as plaintext in SQLite (drivers table). India's DPDP Act 2023 (Rules published Nov 2025, main compliance deadline May 2027) mandates encryption, access logging, and breach notification for personal data. Penalties up to 250 crore INR. | Medium | Audit all PII columns: drivers.phone, drivers.email, drivers.name, drivers.guardian_phone. At minimum: encrypt phone/email at application level before SQLite write. Hash phone for OTP lookups (deterministic), encrypt for display (reversible with key). |
| TS-8 | **Bot command payment verification** | Discord/WhatsApp bots can trigger session launches via bot_coordinator.rs. Must verify wallet balance or pending payment before session launch -- not just accept the command. | Low | Partially exists. Audit: every bot-initiated path must check `wallet.balance >= session_cost` before calling `billing::start`. No bypass for "staff override" via bot -- staff override only through admin panel. |
| TS-9 | **Session launch integrity** | Prevent session start without valid payment. Multiple attack vectors: direct API call to `/billing/start`, kiosk manipulation, bot command, replay of expired auth token, race condition between token validation and billing start. | Medium | Auth tokens must be single-use (mark consumed in DB), time-bounded (expiry already exists), validated server-side before billing starts. Database transaction wrapping token consumption + billing creation to prevent TOCTOU races. |
| TS-10 | **Secrets management** | JWT signing key, database credentials, API keys likely in plaintext in racecontrol.toml. If config file is readable (it lives at C:\RacingPoint\racecontrol.toml on the server), all JWT tokens are forgeable. | Low | Move JWT secret to environment variable. Generate a cryptographically random key on first run if not set. Never commit secrets to git. Rotate key if compromise suspected (invalidates all active JWTs -- acceptable for single-location). |

## Differentiators

Extra protection that goes beyond plugging obvious holes. Valuable but not urgent.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Audit trail for admin actions** | Know who did what, when. "Who topped up that wallet at 2am?" Essential for incident investigation and DPDP Act compliance (access logging). | Medium | Log all admin actions (wallet topup, pricing change, session override, fleet exec, terminal command) to an append-only audit_log table with timestamp, source IP, actor identifier, action, and payload. |
| D-2 | **Session-scoped kiosk tokens** | Instead of relying on process-level lockdown alone, issue a session token that rc-agent validates. When session ends, token expires, kiosk locks automatically. Prevents "session ended but kiosk still unlocked" window. | Medium | Ties kiosk unlock state to billing session lifecycle. Server issues token on billing start, agent validates on each kiosk action, agent locks on token expiry or billing end event. |
| D-3 | **Network source tagging** | Tag API requests by source: wired LAN (pod/server), WiFi (customer PWA), WAN (cloud sync). Apply different trust levels per source. | Medium | Not full VLAN (out of scope) but application-level IP range checks. Pods (192.168.31.x wired) get agent-level trust. WiFi devices get customer-only access. Cloud (Bono VPS IP) gets sync-level access. |
| D-4 | **Automated session cleanup on security anomaly** | If rc-agent detects kiosk escape attempt, unauthorized process surviving kills, or hardware tamper, auto-pause billing and alert staff via WhatsApp. | Low | Extend kiosk.rs to emit security events over WebSocket. Server-side: pause billing on security alert. WhatsApp alert to Uday (already have whatsapp_alerter.rs). |
| D-5 | **Customer data export and deletion** | DPDP Act grants data principals the right to erasure and data portability. Provide mechanism to export and delete a customer's data on request. | Low | SQL cascade delete from drivers table. Export as JSON dump. Not urgent at current scale but required by May 2027 deadline. |
| D-6 | **Staff PIN rotation** | Employee daily PIN already exists (`/employee/daily-pin`). Extend to auto-rotate admin PIN periodically or after suspected compromise. | Low | Ensure admin PIN is not static forever. Add last_changed timestamp. Alert if PIN unchanged for >30 days. |
| D-7 | **Cloud sync request signing** | Cloud sync (pull/push every 30s via cloud_sync.rs) between local server and Bono's VPS. Intercepted or replayed sync payloads could inject fake billing data. | Medium | HMAC-SHA256 signature on sync payloads. Timestamp + nonce to prevent replay. Shared secret between server and VPS (separate from JWT key). |
| D-8 | **Security response headers** | Prevent XSS, clickjacking, MIME sniffing in the kiosk PWA and admin dashboard. | Low | Add CSP, X-Frame-Options, X-Content-Type-Options, Strict-Transport-Security headers via Axum middleware layer. Blocks script injection and framing attacks. |

## Anti-Features

Things to deliberately NOT build. Either overkill for current scale, introduce complexity that hurts operations, or create false sense of security.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Full PCI-DSS certification** | Overkill for a single-location cafe. Payments processed via third-party (UPI/Razorpay). No card numbers stored locally. The wallet stores credits, not payment instruments. | Follow PCI best practices (no card storage, HTTPS for payment flows) without formal certification. |
| **Role-based access control (RBAC)** | Single owner (Uday). Adding roles, permissions, and access matrices adds complexity with zero users to benefit. Staff already have a limited daily PIN. | Binary access model: admin (Uday PIN) or staff (daily PIN) or customer (JWT). No need for permission matrices. |
| **External penetration testing** | Premature before basic hardening is done. Findings will be "you have no auth on your APIs" -- which is already known. Money wasted. | Internal hardening first across all table stakes. Consider pen test only after all TS features are deployed and stable. |
| **Biometric authentication** | No biometric hardware on pods or kiosks. Adds cost and hardware dependency for a walk-in cafe. | PIN + OTP is sufficient for customer auth. Staff use admin PIN. Physical presence at the venue is itself a factor. |
| **End-to-end encryption for all internal traffic** | Pods are on a private wired LAN behind a router. Encrypting pod-to-server WebSocket and HTTP adds latency to real-time telemetry (UDP ports 9996/20777/5300/6789/5555) and kiosk control. | HTTPS for customer-facing traffic (PWA over WiFi). Plain HTTP acceptable for wired pod-to-server on private LAN. Monitor for rogue devices instead. |
| **OAuth2/OIDC with external identity provider** | No Google/Facebook login needed. Customers auth via phone OTP (already implemented). Adding OAuth2 complexity for staff auth serving 1 user is engineering for engineering's sake. | Phone OTP for customers (exists). Static/rotating PIN for admin (simple). |
| **Web Application Firewall (WAF)** | Local server (192.168.31.23) has no public internet exposure. Cloud endpoints are behind Bono's VPS (72.60.101.58). No inbound traffic from the internet to the cafe server. | Existing CORS policy + rate limiting + input validation is sufficient for the threat model. |
| **Immutable audit log (blockchain/append-only DB)** | Buzzword solution. At this scale, a regular SQL table with an auto-increment ID and created_at timestamp is tamper-evident enough. The threat actor is a curious customer, not a sophisticated attacker. | Simple audit_log table. If tamper-evidence is later needed, add SHA-256 chain hash (hash of previous row included in current row). |
| **Client-side encryption in PWA** | Encrypting data in the browser before sending to server adds complexity. The server must decrypt to process. Does not protect against a compromised server. Gives false sense of security. | Server-side encryption at rest (TS-7) and HTTPS in transit (TS-3) cover the actual threat vectors. |

---

## Feature Dependencies

```
TS-10 (Secrets management) ──> TS-1 (API auth)
    API auth tokens derive from JWT key. Key must be securely stored first.

TS-1 (API auth) ──> TS-4 (JWT enforcement on customer routes)
    JWT middleware depends on auth infrastructure and key management.

TS-5 (Rate limiting) ──> TS-2 (Admin auth)
    Rate limiting protects the admin PIN from brute-force.
    Implement together or rate limiting first.

TS-3 (HTTPS) ──> TS-7 (PII encryption at rest)
    Encrypting data at rest is undermined if it transits in plaintext.
    HTTPS first, then at-rest encryption.

TS-1 (API auth) ──> TS-9 (Session launch integrity)
    Cannot verify session launch integrity without authenticated requests.

TS-6 (Kiosk hardening) ──> D-2 (Session-scoped tokens)
    Kiosk process lockdown is the foundation; session tokens refine it.

D-1 (Audit trail) ──> D-4 (Automated cleanup on anomaly)
    Need event logging before you can trigger automated responses.

TS-1 (API auth) ──> D-7 (Cloud sync signing)
    Sync signing builds on the auth key infrastructure.

TS-2 (Admin auth) ──> D-1 (Audit trail)
    Must know WHO performed an action (admin identity) to log it meaningfully.
```

---

## MVP Recommendation

**Phase 1 -- Plug the Biggest Holes (immediate financial risk):**
1. TS-10: Secrets management -- foundation for all auth
2. TS-1: API authentication on billing/session endpoints
3. TS-2: Admin panel PIN protection
4. TS-5: Rate limiting on auth endpoints

**Rationale:** These four features close the "anyone on the LAN can steal money" gap. TS-10 first because API auth tokens need a secure signing key. TS-1 is the highest-impact single change. TS-2 locks the admin door. TS-5 prevents brute-forcing the new locks.

**Phase 2 -- Protect Customer Data (legal compliance + trust):**
5. TS-3: HTTPS for PWA and admin traffic
6. TS-4: JWT enforcement on all customer routes
7. TS-7: PII storage audit and encryption

**Rationale:** India's DPDP Act compliance deadline is May 2027. These features address the legal requirement. HTTPS first (transit), then JWT enforcement (access control), then at-rest encryption (storage).

**Phase 3 -- Harden the Kiosk (physical exploitation prevention):**
8. TS-6: Kiosk escape prevention hardening
9. TS-8: Bot command payment verification
10. TS-9: Session launch integrity

**Rationale:** Physical exploitation requires being at the venue -- lower blast radius than remote API abuse. But tech-savvy customers WILL try Win+R, USB sticks, and Sticky Keys. This phase hardens the physical attack surface.

**Phase 4 -- Defense in Depth (differentiators):**
11. D-1: Audit trail for admin actions
12. D-8: Security response headers (CSP, etc.)
13. D-2: Session-scoped kiosk tokens
14. D-4: Automated session cleanup on anomaly

**Defer indefinitely:** D-3 (network source tagging), D-5 (data export/deletion -- implement closer to May 2027), D-6 (staff PIN rotation -- low risk), D-7 (cloud sync signing -- implement if cloud sync volume grows).

---

## Current State Assessment

Based on direct codebase audit of routes.rs, auth/mod.rs, main.rs, kiosk.rs, db/mod.rs:

| Area | Current State | Gap Severity |
|------|--------------|--------------|
| API Auth | Zero middleware protection. All 80+ routes open to any LAN device. Single `jwt_error_to_401` middleware exists but only converts JWT errors -- does NOT require JWT. | CRITICAL |
| Admin Auth | No authentication gate. Dashboard, terminal, fleet exec, wallet operations all accessible without credentials. | CRITICAL |
| Customer Auth | JWT infrastructure exists (jsonwebtoken crate, Claims struct, extract_driver_id helper). OTP login flow exists. But no Axum middleware enforcing JWT on /customer/* routes. | HIGH |
| Kiosk Security | Process allowlisting via kiosk.rs (allowlist + sightings + learned list). Good foundation. Hotkey, USB, file dialog, accessibility escape vectors unaddressed. USB lockdown noted as pending in CLAUDE.md. | MEDIUM |
| HTTPS | All traffic is HTTP. No TLS configured on any service. CORS allows 192.168.31.* origins over HTTP. | HIGH |
| PII Storage | Phone, email, name, guardian_phone as plaintext TEXT columns in SQLite. No encryption, no access logging. | HIGH |
| Rate Limiting | Customer PIN lockout at 5 failures (CUSTOMER_PIN_LOCKOUT_THRESHOLD). No IP-level throttle. No protection on /staff/validate-pin or admin endpoints. | MEDIUM |
| Secrets | JWT key in racecontrol.toml (C:\RacingPoint\racecontrol.toml on server). Plaintext on disk. | MEDIUM |
| Audit Trail | None. No record of who performed admin actions, when, or from where. | LOW (no current incidents reported, but complete blind spot) |
| Cloud Sync | Plain HTTP between server and VPS. No request signing. No replay protection. | LOW (sync is bidirectional with conflict resolution, not directly exploitable without LAN access) |

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `crates/racecontrol/src/api/routes.rs` -- 80+ routes, zero auth middleware, JWT helper exists but not enforced
  - `crates/racecontrol/src/auth/mod.rs` -- JWT Claims, PIN validation, lockout threshold
  - `crates/racecontrol/src/main.rs` -- CORS config, jwt_error_to_401 middleware, no TLS
  - `crates/rc-agent/src/kiosk.rs` -- process allowlisting, no hotkey/USB protection
  - `crates/racecontrol/src/db/mod.rs` -- plaintext PII columns in drivers table
- **Kiosk escape research (HIGH confidence):**
  - [Kiosk escape techniques - InternalAllTheThings](https://swisskyrepo.github.io/InternalAllTheThings/cheatsheets/escape-breakout/)
  - [Kiosk mode breakout repository](https://github.com/ikarus23/kiosk-mode-breakout)
  - [Kiosk bypass prevention - Payatu](https://payatu.com/blog/how-to-prevent-hacking-out-of-kiosk/)
- **India data protection (MEDIUM confidence -- law is enacted, rules published, enforcement timeline confirmed):**
  - [India DPDP Act overview - DLA Piper](https://www.dlapiperdataprotection.com/?t=law&c=IN)
  - [DPDP Rules 2025 guide](https://www.dpdpa.com/dpdparules.html)
  - [India DPDP Rules 2025 - Deloitte](https://www.deloitte.com/in/en/services/consulting/about/indias-dpdp-rules-2025-leading-digital-privacy-compliance.html)
  - [India data protection report 2025-2026 - ICLG](https://iclg.com/practice-areas/data-protection-laws-and-regulations/india)
- **API security best practices (MEDIUM confidence):**
  - [API Security Best Practices - StackHawk](https://www.stackhawk.com/blog/api-security-best-practices-ultimate-guide/)
  - [API Security Best Practices - Axway](https://blog.axway.com/learning-center/digital-security/keys-oauth/api-security-best-practices)

---
*Feature research for: v12.0 Racing Point Operations Security*
*Researched: 2026-03-20*
