# Technology Stack: Security Hardening

**Project:** Racing Point Operations Security (v12)
**Researched:** 2026-03-20
**Scope:** Adding API auth, admin PIN protection, HTTPS, customer data protection, and PWA hardening to existing Rust/Axum + Next.js system

## Existing Stack (Do Not Change)

These are already in place and must be extended, not replaced:

| Technology | Version | Role |
|------------|---------|------|
| Rust (edition 2024) | 1.93.1 | Backend language |
| Axum | 0.8 | HTTP framework (ws, macros) |
| Tower / tower-http | 0.5 / 0.6 | Middleware (cors, fs, trace) |
| SQLx + SQLite | 0.8 | Database |
| jsonwebtoken | 9 | JWT creation/validation (already in workspace) |
| Next.js | 16.1.6 | PWA + web dashboard |
| React | 19.2.3 | Frontend |
| Tailwind CSS | 4 | Styling |

---

## Recommended Security Stack

### 1. API Authentication Middleware

**Already have:** `jsonwebtoken = "9"` in workspace, `auth/mod.rs` with JWT Claims, token creation, PIN validation.
**Missing:** Axum middleware layer that enforces auth on protected routes.

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `axum::middleware::from_fn` | (built-in to axum 0.8) | Auth middleware layer | Native Axum pattern. No extra crate needed. Use `from_fn` to create a layer that extracts Bearer token from Authorization header, validates JWT via existing `jsonwebtoken` crate, and injects claims into request extensions. Already have the JWT infra -- just need the middleware wiring. |
| `tower-http` (existing) | 0.6 | Sensitive header marking | Already a dependency. Use `SetSensitiveHeadersLayer` to mark Authorization headers as sensitive in traces so tokens don't leak to logs. |

**Confidence:** HIGH -- axum::middleware::from_fn is the documented, idiomatic approach for custom auth in Axum 0.8. No third-party auth crate needed given existing JWT setup.

**What NOT to use:**
- `axum-jwt-auth` crate -- adds complexity for JWKS/remote key support you don't need. Your JWT secret is local in `racecontrol.toml`.
- `tower-http`'s `ValidateRequestHeaderLayer` -- too simplistic for JWT validation (only does basic auth/bearer presence check, no claims parsing).
- Auth0/Keycloak/any external identity provider -- massive overkill for single-venue, single-admin operation.

### 2. Admin PIN / Password Hashing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `argon2` | 0.5 | Admin PIN hashing | RustCrypto's pure-Rust Argon2id implementation. OWASP-recommended algorithm for password hashing. Argon2id is memory-hard, making GPU/ASIC brute-force impractical. Use for hashing Uday's admin PIN at rest in config/DB. The `password-hash` trait it implements gives PHC string format output (algorithm + salt + hash in one string). |

**Confidence:** HIGH -- `argon2` (RustCrypto) is the standard Rust crate. Version 0.5.3 is latest stable.

**What NOT to use:**
- `rust-argon2` (different crate, `sru-systems`) -- less maintained, confusing name overlap.
- `bcrypt` -- Argon2id is strictly better (memory-hard, won PHC competition). bcrypt has a 72-byte password limit.
- Plain SHA-256/SHA-512 -- not a password hash; no salt, no work factor, trivially brute-forced.
- Storing PIN in plaintext in `racecontrol.toml` -- must be hashed.

### 3. HTTPS / TLS

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `axum-server` | 0.8 | TLS termination for Axum | Drop-in replacement for `axum::serve` that adds rustls TLS. Feature flag `tls-rustls` enables `bind_rustls()`. Loads PEM cert+key files. Works with self-signed certs for LAN use. |
| `rcgen` | 0.14 | Self-signed certificate generation | Pure Rust X.509 cert generator. `generate_simple_self_signed(&["192.168.31.23", "localhost"])` produces cert+key for the server IP. Part of the rustls ecosystem (maintained by same org). Use at first startup to auto-generate certs if none exist. |
| `rustls` | (transitive via axum-server) | TLS implementation | Pure Rust, no OpenSSL dependency. Already works with static CRT builds (important -- pods use +crt-static). No vcruntime or OpenSSL DLL needed on pods. |

**Confidence:** HIGH -- axum-server 0.8 with tls-rustls is the documented approach for HTTPS with Axum. rcgen is maintained by the rustls team.

**What NOT to use:**
- `openssl` / `native-tls` -- requires OpenSSL DLLs on Windows. Breaks your static CRT requirement. Pain to cross-compile.
- Let's Encrypt / ACME -- requires public DNS and port 80/443 accessible from internet. Your server is on a private LAN (192.168.31.x). Self-signed is correct for LAN-only services.
- Reverse proxy (nginx/caddy) -- adds operational complexity for a Windows-based setup. Axum-native TLS is simpler.
- No TLS at all (relying on "it's a LAN") -- kiosk pods are customer-accessible machines. Anyone on WiFi can sniff HTTP traffic. HTTPS prevents session hijacking and credential theft on the local network.

### 4. Security Headers

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `tower-helmet` | 0.2 | Security HTTP headers | Sets CSP, X-Frame-Options, X-Content-Type-Options, Strict-Transport-Security, etc. in one middleware layer. Tower-native, works directly with your existing tower-http stack. Default config is sane -- enable and customize CSP for your PWA's needs. |

**Confidence:** MEDIUM -- tower-helmet is small but well-maintained. Alternative is manually setting headers via tower-http's `SetResponseHeaderLayer`, which works but is verbose.

### 5. Rate Limiting

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `tower_governor` | 0.4 | API rate limiting | Wraps the `governor` crate (GCRA algorithm) as a Tower layer. Prevents brute-force attacks on PIN/auth endpoints. Configure per-IP limits: e.g., 5 requests/minute on `/api/v1/auth/*` endpoints. Already Tower-native, slots into your existing middleware stack. |

**Confidence:** MEDIUM -- tower_governor is the most popular rate limiting layer for Tower. Version needs verification at build time.

**What NOT to use:**
- Rolling your own rate limiter -- GCRA is non-trivial to implement correctly. governor is battle-tested.
- Global rate limiting only -- need per-IP limits to prevent one actor from locking everyone out.

### 6. Data-at-Rest Protection (SQLite)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Column-level encryption via `aes-gcm` | 0.10 | Encrypt PII columns in SQLite | Encrypt phone numbers, emails, payment details before storing in SQLite. AES-256-GCM provides authenticated encryption (tamper detection). Key stored in `racecontrol.toml` (separate from DB file). Simpler than SQLCipher for your use case -- only PII columns need encryption, not the entire DB. |

**Confidence:** MEDIUM -- Column-level encryption is pragmatic for your scale. Full-DB encryption (SQLCipher) requires rebuilding SQLx with `bundled-sqlcipher` feature and adds significant build complexity on Windows.

**What NOT to use:**
- SQLCipher (`sqlx-sqlite-cipher`) -- requires linking against SQLCipher C library. Build complexity on Windows is high (need pre-built SQLCipher DLL or compile from source). Overkill when only ~5 columns contain PII. If you later need full-DB encryption, revisit.
- No encryption at all -- customer phone numbers and payment details are in plaintext SQLite files on a multi-user Windows machine. Anyone with file access can read them.
- Filesystem-level encryption (BitLocker) -- helps but doesn't protect against other processes or logged-in users reading the DB while the system is running.

### 7. PWA Route Protection (Next.js 16)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Next.js `proxy.ts` (was `middleware.ts`) | Built-in (Next.js 16) | Route-level auth checks | Next.js 16 renamed middleware.ts to proxy.ts. Use it to check for session cookie existence and redirect unauthenticated users. Lightweight -- only checks cookie presence, no DB calls. |
| Server-side session validation | Custom (fetch to racecontrol API) | Validate session on data access | Never trust proxy.ts alone (CVE-2025-29927 showed middleware bypass). Validate JWT on every API call from the PWA to racecontrol. The Rust backend is the source of truth. |
| `HttpOnly` + `Secure` + `SameSite=Strict` cookies | Built-in | Session cookie security | Store JWT in HttpOnly cookie (not localStorage). Prevents XSS from stealing tokens. SameSite=Strict prevents CSRF. Secure flag requires HTTPS (see TLS section). |

**Confidence:** HIGH -- This is standard Next.js auth pattern. The CVE-2025-29927 middleware bypass reinforces that backend validation is mandatory.

**What NOT to use:**
- Auth.js / NextAuth -- designed for OAuth/social login flows. You have a simple PIN-to-JWT flow. Adding NextAuth adds complexity without value.
- Clerk / Auth0 / any SaaS auth -- external dependency for a LAN-only app. Breaks when internet is down.
- localStorage for JWT storage -- XSS-vulnerable. HttpOnly cookies are strictly safer.

### 8. Kiosk Escape Prevention

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Chrome `--kiosk` flags | N/A (Chrome CLI) | Browser lockdown | `--kiosk --disable-extensions --disable-dev-tools --disable-pinch --noerrdialogs --disable-translate --no-first-run --autoplay-policy=no-user-gesture-required`. Already partially in use via rc-agent kiosk module. |
| Windows Group Policy | N/A (OS) | OS-level lockdown | Disable Ctrl+Alt+Del task manager access, Alt+Tab, Windows key via Group Policy on pod machines. Filter keyboard shortcuts. Already noted as pending in CLAUDE.md (USB mass storage lockdown). |
| CSP headers on kiosk PWA | Via tower-helmet | Prevent script injection | Strict Content-Security-Policy: only allow scripts from self, no inline scripts, no eval. Prevents any injected content from executing. |

**Confidence:** HIGH -- Chrome kiosk flags + Group Policy is the standard approach for Windows kiosk lockdown. rc-agent already has kiosk module infrastructure.

---

## Supporting Libraries (Already in Stack)

These existing dependencies serve security purposes -- no changes needed:

| Library | Version | Security Role |
|---------|---------|--------------|
| `jsonwebtoken` | 9 | JWT encode/decode -- already handles HS256 signing |
| `uuid` | 1 (v4) | Cryptographically random token IDs |
| `rand` | 0.8 | Random PIN generation, JWT secret generation |
| `tower-http` (cors) | 0.6 | CORS policy enforcement -- restrict origins to known kiosk/PWA domains |
| `reqwest` | 0.12 | HTTPS client for cloud sync (already uses rustls by default) |

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Auth middleware | `axum::middleware::from_fn` | `axum-jwt-auth` crate | Extra dep for JWKS features we don't need |
| Password hashing | `argon2` (RustCrypto) | `bcrypt`, `rust-argon2` | bcrypt has 72-byte limit; rust-argon2 is less maintained |
| TLS | `axum-server` + `rustls` | `openssl` / `native-tls` | OpenSSL DLLs break static CRT; Windows build pain |
| Cert generation | `rcgen` | Manual openssl CLI | rcgen is pure Rust, can auto-generate at startup |
| Rate limiting | `tower_governor` | Custom implementation | GCRA is complex; governor is proven |
| DB encryption | Column-level `aes-gcm` | SQLCipher | SQLCipher Windows build complexity; only ~5 PII columns |
| Security headers | `tower-helmet` | Manual `SetResponseHeader` | tower-helmet is one line vs 8 manual layers |
| PWA auth | Cookie-based JWT + backend validation | NextAuth / Auth.js | Overkill for PIN-to-JWT flow |
| Frontend storage | HttpOnly cookies | localStorage | localStorage is XSS-vulnerable |

---

## Installation

### Rust (add to workspace Cargo.toml)

```toml
[workspace.dependencies]
# Security additions
argon2 = "0.5"
aes-gcm = "0.10"

# Existing (no changes)
jsonwebtoken = "9"
rand = "0.8"
```

### Rust (add to crates/racecontrol/Cargo.toml)

```toml
[dependencies]
# TLS
axum-server = { version = "0.8", features = ["tls-rustls"] }
rcgen = "0.14"

# Security middleware
tower-helmet = "0.2"
tower_governor = "0.4"

# Password hashing
argon2 = { workspace = true }

# PII encryption
aes-gcm = { workspace = true }
```

### Next.js PWA (no new dependencies)

No new npm packages needed. Security is achieved through:
- Renaming `middleware.ts` to `proxy.ts` (Next.js 16 convention)
- Setting `HttpOnly` / `Secure` / `SameSite` cookie attributes on JWT
- CSP headers served by the Rust backend via tower-helmet

---

## Architecture Integration Notes

1. **Middleware ordering matters:** Rate limiting (tower_governor) -> Security headers (tower-helmet) -> CORS (existing) -> Auth (from_fn) -> Route handlers. Rate limiting must be outermost to prevent brute-force before any processing.

2. **Two auth tiers:**
   - **Customer auth:** PIN -> JWT (existing flow, needs middleware enforcement)
   - **Admin auth:** Uday's PIN -> separate admin JWT with `role: "admin"` claim. Same `jsonwebtoken` crate, different claim type.

3. **HTTPS rollout:** Start with self-signed certs via rcgen. Pods trust the self-signed CA by installing it to Windows cert store (rc-installer can do this). No browser warnings on kiosk Chrome if cert is trusted at OS level.

4. **Backward compatibility:** During rollout, support both HTTP and HTTPS on different ports (8080 HTTP, 8443 HTTPS). Migrate pods one at a time. Kill HTTP listener only after all pods are on HTTPS.

---

## Version Verification Status

| Crate | Claimed Version | Verified | Source |
|-------|----------------|----------|--------|
| axum-server | 0.8 | YES | docs.rs shows 0.8.0, released 2025-12-06 |
| rcgen | 0.14 | YES | docs.rs shows 0.14.6, released 2025-12-13 |
| argon2 | 0.5 | YES | docs.rs shows 0.5.3 |
| aes-gcm | 0.10 | MEDIUM | Training data; verify at build time |
| tower-helmet | 0.2 | MEDIUM | Training data; verify at build time |
| tower_governor | 0.4 | MEDIUM | Training data; verify at build time |

---

## Sources

- [Axum middleware documentation](https://docs.rs/axum/latest/axum/middleware/index.html) -- from_fn pattern
- [axum-server TLS rustls docs](https://docs.rs/axum-server/latest/axum_server/tls_rustls/index.html) -- bind_rustls API
- [axum TLS example](https://github.com/tokio-rs/axum/blob/main/examples/tls-rustls/src/main.rs) -- official example
- [rcgen docs](https://docs.rs/rcgen/latest/rcgen/) -- self-signed cert generation
- [tower-helmet](https://github.com/Atrox/tower-helmet) -- security headers middleware
- [tower_governor](https://github.com/benwis/tower-governor) -- rate limiting for Tower
- [argon2 (RustCrypto)](https://crates.io/crates/argon2) -- password hashing
- [CVE-2025-29927 analysis](https://projectdiscovery.io/blog/nextjs-middleware-authorization-bypass) -- Next.js middleware bypass
- [Next.js 16 auth changes](https://auth0.com/blog/whats-new-nextjs-16/) -- proxy.ts rename
- [Kiosk escape techniques](https://github.com/ikarus23/kiosk-mode-breakout) -- what to defend against
- [Chrome kiosk hardening](https://smartupworld.com/chromium-kiosk-mode/) -- enterprise display security
