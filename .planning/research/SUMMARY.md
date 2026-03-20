# Project Research Summary

**Project:** Racing Point Operations Security (v12.0)
**Domain:** Security hardening for a live, distributed eSports cafe operations system (Rust/Axum server, Next.js PWA, 8-pod fleet, Linux VPS)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

Racing Point's operations stack (racecontrol server, 8 rc-agent pods, kiosk PWA, staff dashboard, cloud sync) currently has zero API authentication on 80+ routes, no admin panel protection, plaintext HTTP on all local traffic, and customer PII stored unencrypted in SQLite. The system is a single-venue eSports cafe on a private LAN, but the attack surface is real: tech-savvy customers sit at pod machines with kiosk escape potential, anyone on the WiFi can sniff credentials, and India's DPDP Act (compliance deadline May 2027) mandates encryption and access logging for personal data. The existing JWT infrastructure (jsonwebtoken crate, Claims struct, OTP login flow) is built but not enforced -- the middleware layer is missing.

The recommended approach is a phased security rollout using the existing Rust/Axum + tower middleware stack. No new frameworks or external identity providers are needed. The critical addition is Axum middleware layers (`from_fn`) that enforce JWT and HMAC-based authentication on route groups, plus argon2 for PIN hashing, aes-gcm for PII column encryption, and axum-server with rustls for HTTPS. The most important architectural decision is splitting the monolithic 80+ route Router into grouped sub-routers (public, customer, staff/admin, service) with separate auth layers -- this is idiomatic Axum 0.8 and naturally decomposes the 9,500-line routes.rs monolith.

The single biggest risk is a big-bang auth rollout that bricks the pod fleet. The server, 8 agents, and the PWA deploy independently -- enabling auth on the server while clients still send unauthenticated requests will take the cafe offline. Every auth phase must follow the expand-migrate-contract pattern: server accepts both authenticated and unauthenticated requests first, clients are updated pod-by-pod, then unauthenticated requests are rejected only after 24 hours of zero unauthenticated traffic. Pod agent endpoints (:8090, :8091) must be included in auth scope -- they are the backdoor if only the central API is locked.

## Key Findings

### Recommended Stack

The existing Rust/Axum/tower/SQLx stack requires only 4 new crates for full security coverage. No npm packages needed on the frontend side. See [STACK.md](STACK.md) for full details.

**Core additions:**
- `argon2` 0.5: Admin PIN hashing -- OWASP-recommended, pure Rust, memory-hard (prevents GPU brute-force)
- `aes-gcm` 0.10: PII column encryption -- AES-256-GCM for phone/email fields, avoids SQLCipher build complexity
- `axum-server` 0.8 + `rcgen` 0.14: HTTPS via rustls -- in-process TLS, no OpenSSL dependency, compatible with static CRT builds
- `tower-helmet` 0.2 + `tower_governor` 0.4: Security headers and rate limiting -- drop-in tower middleware layers

**What NOT to add:** No external identity providers (Auth0, Keycloak), no SQLCipher (Windows build pain for only 5 PII columns), no NextAuth (overkill for PIN-to-JWT), no OpenSSL (breaks static CRT).

### Expected Features

See [FEATURES.md](FEATURES.md) for full analysis including dependency graph and current state assessment.

**Must have (table stakes -- 10 features):**
- TS-1: API auth on all billing/session endpoints (CRITICAL gap -- direct financial loss vector)
- TS-2: Admin panel PIN protection (CRITICAL gap -- dashboard is wide open)
- TS-3: HTTPS for PWA and admin traffic over WiFi
- TS-4: JWT enforcement on all /customer/* routes
- TS-5: Rate limiting on auth endpoints (prevent PIN brute-force)
- TS-6: Kiosk escape prevention (hotkeys, USB, file dialogs, Sticky Keys)
- TS-7: PII encryption at rest (DPDP Act compliance, May 2027 deadline)
- TS-8: Bot command payment verification
- TS-9: Session launch integrity (prevent billing bypass via race conditions)
- TS-10: Secrets management (JWT key out of config file, into env vars)

**Should have (differentiators -- 4 features for v12):**
- D-1: Audit trail for admin actions (DPDP Act access logging)
- D-2: Session-scoped kiosk tokens
- D-4: Automated session cleanup on security anomaly
- D-8: Security response headers (CSP, X-Frame-Options)

**Defer to v13+:**
- D-3: Network source tagging (application-level IP trust tiers)
- D-5: Customer data export/deletion (DPDP right to erasure -- implement closer to May 2027)
- D-6: Staff PIN rotation
- D-7: Cloud sync request signing (HMAC on sync payloads)

### Architecture Approach

The architecture follows a 6-layer defense model (network boundary, transport, API auth, authorization, data protection, kiosk hardening) with this project covering layers 2-6. The key pattern is Axum nested Routers with per-group middleware stacks rather than a single global auth check. Three auth tiers emerge: customer JWT, staff/admin JWT (with role claim), and service-to-service HMAC (for pod agents and cloud sync). See [ARCHITECTURE.md](ARCHITECTURE.md) for data flow diagrams and component boundaries.

**Major components:**
1. **API Auth Middleware** -- Axum `from_fn` layers on grouped routers (public/customer/staff/service tiers)
2. **Admin Auth Module** -- PIN-to-JWT flow, argon2 hashing, 12-hour session cookies, rate limiting
3. **Service-to-Service Auth** -- HMAC-SHA256 with timestamp for pod WebSocket and remote_ops, PSK for rc-sentry
4. **HTTPS/TLS Layer** -- axum-server + rustls for browser traffic; LAN pod-to-server stays HTTP with HMAC auth
5. **Data Protection** -- AES-256-GCM column encryption for PII fields, phone hash for lookups
6. **Kiosk Hardening** -- Chrome kiosk flags, Group Policy lockdown, rc-agent process monitor, CSP headers

### Critical Pitfalls

See [PITFALLS.md](PITFALLS.md) for all 10 pitfalls with recovery strategies.

1. **Big-bang auth rollout bricks the fleet** -- Use expand-migrate-contract: server accepts both modes, update clients pod-by-pod (Pod 8 canary first), reject unauthenticated only after 24h clean logs
2. **Pod agent bypass via localhost:8090** -- rc-agent remote_ops and rc-sentry accept commands without auth; locking the central API while pods are open is security theater. Auth must cover pod agents in the same phase as the central server
3. **HTTPS breaks WebSocket connections** -- Enabling TLS on :8080 requires all pods to switch from ws:// to wss:// simultaneously. Decision: keep LAN traffic as HTTP + HMAC auth; HTTPS only for WiFi/external browser traffic
4. **Admin PIN stored plaintext** -- Hash with argon2 on first setup; rate-limit PIN attempts; server-side validation is non-negotiable (client-side React check alone is bypassed with curl)
5. **PII scattered beyond the database** -- Phone numbers in log files, bot messages, cloud sync payloads, browser localStorage. Full-system PII trace must happen before any encryption work

## Implications for Roadmap

Based on combined research, the dependency graph, and pitfall analysis, I recommend 6 phases.

### Phase 1: Security Audit and Foundations
**Rationale:** You cannot protect what you have not measured. PITFALLS.md Pitfall 6 warns PII is in unexpected locations. FEATURES.md dependency graph shows TS-10 (secrets) must precede all auth work.
**Delivers:** Complete inventory of exposed endpoints, PII locations, and secret storage. Secure key management via environment variables. Multi-token acceptance pattern in middleware skeleton.
**Addresses:** TS-10 (secrets management), foundation for TS-1
**Avoids:** Pitfall 2 (hardcoded secrets), Pitfall 6 (PII in unexpected places)

### Phase 2: API Authentication + Admin Protection
**Rationale:** The two CRITICAL gaps (open API + open admin panel) represent immediate financial loss. ARCHITECTURE.md build order and FEATURES.md MVP recommendation agree: auth first, everything else second. Must use expand-migrate-contract rollout.
**Delivers:** Auth middleware on all billing/session/admin routes. Admin PIN-to-JWT with argon2 hashing. Rate limiting on auth endpoints. Pod agent auth on :8090 and :8091.
**Addresses:** TS-1 (API auth), TS-2 (admin auth), TS-4 (customer JWT enforcement), TS-5 (rate limiting), TS-8 (bot payment verification)
**Avoids:** Pitfall 1 (big-bang rollout), Pitfall 3 (plaintext PIN), Pitfall 7 (no token rotation), Pitfall 8 (pod agent bypass), Pitfall 10 (auth latency)
**Uses:** axum::middleware::from_fn, argon2, tower_governor, existing jsonwebtoken

### Phase 3: HTTPS and Transport Security
**Rationale:** With auth in place, transit encryption prevents token sniffing on WiFi. ARCHITECTURE.md places this after auth because "unauthenticated endpoints are a bigger risk than unencrypted transport on a private LAN."
**Delivers:** TLS on racecontrol for browser traffic (PWA, dashboard). Self-signed CA via rcgen, distributed to pod browsers. HttpOnly/Secure/SameSite cookies. WSS for browser WebSocket connections.
**Addresses:** TS-3 (HTTPS), D-8 (security headers)
**Avoids:** Pitfall 4 (HTTPS breaks WebSocket -- scoped to browser traffic only, pod agents stay HTTP+HMAC)
**Uses:** axum-server, rcgen, rustls, tower-helmet

### Phase 4: Kiosk Hardening
**Rationale:** Independent of the API security stack (can run in parallel with Phase 3). Physical exploitation requires venue presence -- lower blast radius than remote API abuse, but tech-savvy gamer customers will try escape vectors.
**Delivers:** Chrome kiosk flag hardening, Group Policy lockdown (Task Manager, USB, hotkeys), rc-agent process allowlist updates, session-scoped kiosk tokens.
**Addresses:** TS-6 (kiosk escape prevention), TS-9 (session launch integrity), D-2 (session-scoped tokens), D-4 (automated cleanup on anomaly)
**Avoids:** Pitfall 5 (kiosk escape via hotkeys/DevTools), Pitfall 8 (pod agent bypass -- already closed in Phase 2)

### Phase 5: Data Protection (PII Encryption)
**Rationale:** Depends on Phase 3 (transit security) being complete -- encrypting at rest is undermined if PII transits in plaintext. DPDP Act deadline is May 2027, so this is not emergency-urgent but must ship well before that.
**Delivers:** AES-256-GCM encryption on drivers.phone, drivers.email, drivers.name, drivers.guardian_phone. Phone hash for OTP lookups. Log redaction middleware. PII boundary policy.
**Addresses:** TS-7 (PII encryption at rest)
**Avoids:** Pitfall 6 (PII in unexpected locations -- traced in Phase 1), Pitfall 9 (encryption breaks tooling -- field-level, not full-DB)
**Uses:** aes-gcm

### Phase 6: Audit Trail and Defense in Depth
**Rationale:** Requires admin identity (Phase 2) and session infrastructure to be stable. Low urgency but important for DPDP compliance (access logging) and incident investigation.
**Delivers:** Append-only audit_log table for admin actions. WhatsApp alerting on security anomalies. Cloud sync auth hardening if needed.
**Addresses:** D-1 (audit trail), remaining differentiators

### Phase Ordering Rationale

- **Secrets before auth** because JWT signing keys must be securely stored before the first token is issued
- **Auth before HTTPS** because unauthorized API access (curl billing/start) is a bigger immediate threat than network sniffing
- **HTTPS before data-at-rest** because encrypting stored PII while it transits in plaintext is inconsistent protection
- **Kiosk hardening is parallel** to Phases 3-5 because it is an OS/browser concern, not an API concern
- **Audit trail last** because it requires identity infrastructure (who did what) to already exist

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2 (API Auth):** The expand-migrate-contract rollout across 8 pods is operationally complex. Needs detailed deploy sequencing. Also needs research on the exact route classification (which of the 80+ routes are public vs. customer vs. staff vs. service).
- **Phase 4 (Kiosk Hardening):** Windows Group Policy settings, Chrome flag combinations, and game overlay compatibility need hands-on testing. Not well-served by desk research -- requires physical testing on a pod.
- **Phase 5 (Data Protection):** Cloud sync payload contents need auditing to determine if encrypted fields break the sync protocol. Field-level encryption impact on query patterns needs validation.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Audit + Foundations):** Straightforward grep/audit work plus env var migration. Well-understood.
- **Phase 3 (HTTPS):** axum-server + rustls is well-documented with official examples. Self-signed CA distribution is a one-time setup script.
- **Phase 6 (Audit Trail):** Simple append-only SQL table. No complex patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommendations verified against docs.rs. 3 of 6 new crates version-verified. Existing JWT/tower/SQLx stack is well-understood from codebase. |
| Features | HIGH | Based on direct codebase audit of routes.rs, auth/mod.rs, kiosk.rs, db/mod.rs. DPDP Act requirements sourced from DLA Piper and Deloitte. |
| Architecture | HIGH | Based on direct codebase analysis, not external sources. Axum middleware patterns already in use (CORS, trace). Route grouping is idiomatic Axum 0.8. |
| Pitfalls | HIGH | Operational pitfalls (big-bang rollout, pod bypass) are specific to Racing Point's fleet topology. Kiosk escape vectors sourced from multiple security research repos. |

**Overall confidence:** HIGH

### Gaps to Address

- **tower-helmet and tower_governor version verification:** Claimed versions (0.2 and 0.4) need validation at build time. May have newer versions or API changes.
- **aes-gcm version verification:** Version 0.10 from training data, needs build-time check.
- **HTTPS scope decision:** PITFALLS.md and ARCHITECTURE.md present slightly different recommendations on LAN HTTPS. PITFALLS.md says "keep LAN as HTTP" while ARCHITECTURE.md recommends axum-server with rustls for browser traffic. Recommendation: HTTPS for WiFi browser traffic (PWA, dashboard), HTTP+HMAC for wired pod-to-server. This must be explicitly decided before Phase 3 implementation.
- **Route classification:** The exact split of 80+ routes into public/customer/staff/service tiers has not been enumerated. Phase 2 planning must include a route audit.
- **Cloud sync PII exposure:** What specific fields does cloud sync send to Bono's VPS? If encrypted PII fields are synced as ciphertext, does the cloud side need to decrypt? This affects Phase 5 design.
- **Game overlay compatibility:** Kiosk process allowlist hardening (Phase 4) risks killing game-required overlay processes (Steam, Discord). Each game title needs testing.

## Sources

### Primary (HIGH confidence)
- Direct codebase audit: routes.rs, auth/mod.rs, main.rs, kiosk.rs, db/mod.rs, remote_ops.rs, rc-sentry main.rs
- [Axum middleware documentation](https://docs.rs/axum/latest/axum/middleware/index.html)
- [axum-server TLS rustls docs](https://docs.rs/axum-server/latest/axum_server/tls_rustls/index.html)
- [rcgen docs](https://docs.rs/rcgen/latest/rcgen/)
- [argon2 (RustCrypto)](https://crates.io/crates/argon2)
- [CVE-2025-29927](https://projectdiscovery.io/blog/nextjs-middleware-authorization-bypass) -- Next.js middleware bypass

### Secondary (MEDIUM confidence)
- [India DPDP Act overview - DLA Piper](https://www.dlapiperdataprotection.com/?t=law&c=IN)
- [DPDP Rules 2025 - Deloitte](https://www.deloitte.com/in/en/services/consulting/about/indias-dpdp-rules-2025-leading-digital-privacy-compliance.html)
- [tower-helmet](https://github.com/Atrox/tower-helmet), [tower_governor](https://github.com/benwis/tower-governor)
- [Kiosk escape techniques](https://github.com/ikarus23/kiosk-mode-breakout), [InternalAllTheThings](https://swisskyrepo.github.io/InternalAllTheThings/cheatsheets/escape-breakout/)
- [Kiosk hardening guides](https://www.hexnode.com/blogs/hardening-windows-kiosk-mode-security-best-practices-for-enterprise-protection/)

### Tertiary (LOW confidence)
- tower-helmet 0.2, tower_governor 0.4, aes-gcm 0.10 version claims -- need build-time verification

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
