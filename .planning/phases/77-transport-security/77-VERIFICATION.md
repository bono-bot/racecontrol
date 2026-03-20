---
phase: 77-transport-security
verified: 2026-03-20T20:45:00+05:30
status: human_needed
score: 8/9 must-haves verified
re_verification: false
human_verification:
  - test: "Enable HTTPS by adding tls_port = 8443 to [server] in racecontrol.toml, restart racecontrol, open https://192.168.31.23:8443 in Chrome"
    expected: "Kiosk PWA loads over HTTPS (accept self-signed cert warning), no mixed content errors in console"
    why_human: "Cannot verify TLS handshake, browser cert acceptance, and page rendering programmatically"
  - test: "Run curl -kI https://192.168.31.23:8443/api/v1/health and inspect response headers"
    expected: "Headers present: content-security-policy (default-src 'self'), x-frame-options: DENY, x-content-type-options: nosniff, strict-transport-security: max-age=300"
    why_human: "Server not running during verification -- need live deployment to confirm headers"
  - test: "Verify HTTP still works: curl http://192.168.31.23:8080/api/v1/health and check pod fleet health dashboard"
    expected: "HTTP returns 200, all 8 pods show connected in fleet health"
    why_human: "Need live server to confirm no regression on HTTP port"
  - test: "Verify Let's Encrypt on app.racingpoint.cloud (TLS-03) -- awaiting Bono confirmation"
    expected: "Bono confirms valid cert and certbot renew --dry-run succeeds"
    why_human: "TLS-03 is on Bono's VPS -- coordination via comms-link, not verifiable from this machine"
---

# Phase 77: Transport Security Verification Report

**Phase Goal:** All browser-to-server traffic (PWA and admin dashboard) is encrypted in transit
**Verified:** 2026-03-20T20:45:00+05:30
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rcgen generates valid self-signed PEM cert with IP SAN 192.168.31.23 and localhost DNS SAN | VERIFIED | `tls.rs:46-47` -- `generate_simple_self_signed(vec![server_ip, "localhost"])`, 4 unit tests covering PEM creation, IP SAN, auto-generate, and reuse |
| 2 | ServerConfig deserializes tls_port, cert_path, key_path from TOML with backward-compatible defaults | VERIFIED | `config.rs:64-72` -- three `Option<T>` fields with `#[serde(default)]`; `config.rs:699-727` -- two unit tests confirm with/without TLS fields |
| 3 | load_or_generate_rustls_config returns RustlsConfig when PEM files exist on disk | VERIFIED | `tls.rs:31-38` -- checks file existence, loads via `RustlsConfig::from_pem_file`; `tls.rs:186-243` -- unit test confirms existing files are not regenerated |
| 4 | load_or_generate_rustls_config auto-generates PEM files when they do not exist | VERIFIED | `tls.rs:31-34` -- generates when missing; `tls.rs:155-183` -- unit test confirms creation |
| 5 | HTTPS listener binds on tls_port (8443) when tls_port is set in config | VERIFIED | `main.rs:631-650` -- `if let Some(tls_port) = state.config.server.tls_port` -> `axum_server::bind_rustls(https_addr, tls_config)` spawned via `tokio::spawn` |
| 6 | HTTP listener on port 8080 continues to work unchanged for pod agents | VERIFIED | `main.rs:622,653` -- HTTP TcpListener bind and `axum::serve` remain unchanged, runs in main thread |
| 7 | Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS) present on all responses | VERIFIED | `main.rs:266-300` -- `security_headers_layer()` enables CSP (use_defaults:false, 8 directives), XFrameOptions::Deny, XContentTypeOptions, HSTS max-age=300s; applied at `main.rs:599` |
| 8 | CORS predicate accepts both http:// and https:// origins from 192.168.31.* subnet | VERIFIED | `main.rs:602-613` -- predicate includes both `http://192.168.31.` and `https://192.168.31.`; `racingpoint.cloud` changed to exact `==` match (security fix) |
| 9 | Kiosk PWA API_BASE derives protocol and port from window.location -- no mixed content | VERIFIED | `api.ts:3-7` -- `${window.location.protocol}//${window.location.host}` replaces hardcoded `http://${hostname}:8080` |

**Score:** 9/9 truths verified (code-level)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/tls.rs` | TLS cert generation and RustlsConfig loader | VERIFIED | 61 lines production code + 194 lines tests; exports `load_or_generate_rustls_config`; no `.unwrap()` in production code |
| `crates/racecontrol/src/config.rs` | Extended ServerConfig with tls_port, cert_path, key_path | VERIFIED | Lines 64-72: three `Option<T>` fields; `default_config()` at line 437 sets all to None; 2 deserialization tests |
| `crates/racecontrol/src/main.rs` | Dual-port HTTP+HTTPS server startup with security headers | VERIFIED | `security_headers_layer()` at line 266; HTTPS spawn at line 631; CORS update at line 601; `bind_rustls` at line 643 |
| `kiosk/src/lib/api.ts` | Protocol-aware API_BASE | VERIFIED | Line 5: `${window.location.protocol}//${window.location.host}` -- uses `.host` (includes port), not `.hostname` |
| `crates/racecontrol/Cargo.toml` | axum-server, rcgen, tower-helmet dependencies | VERIFIED | `axum-server 0.8 [tls-rustls]`, `rcgen 0.14`, `tower-helmet 0.3` all present |
| `crates/racecontrol/src/lib.rs` | `pub mod tls` declaration | VERIFIED | Line 39: `pub mod tls;` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `tls.rs` | `rcgen` | `generate_simple_self_signed` | WIRED | Line 47: `generate_simple_self_signed(subject_alt_names)?` |
| `tls.rs` | `axum_server::tls_rustls::RustlsConfig` | `from_pem_file` | WIRED | Line 36: `RustlsConfig::from_pem_file(&cert_file, &key_file).await?` |
| `main.rs` | `tls::load_or_generate_rustls_config` | function call | WIRED | Line 632: `tls::load_or_generate_rustls_config(...)` with config fields passed through |
| `main.rs` | `tower-helmet` | `HelmetLayer` in middleware stack | WIRED | Line 12: `use tower_helmet::HelmetLayer`; Line 293: `HelmetLayer::blank()`; Line 599: `.layer(security_headers_layer())` |
| `main.rs` | `axum_server::bind_rustls` | HTTPS listener | WIRED | Line 643: `axum_server::bind_rustls(https_addr, tls_config)` |
| `api.ts` | racecontrol HTTPS listener | `window.location.protocol + host` | WIRED | Line 5: `${window.location.protocol}//${window.location.host}` -- dynamically matches server protocol |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| TLS-01 | 77-02 | HTTPS for customer-facing PWA traffic | VERIFIED | HTTPS listener on tls_port via bind_rustls + kiosk API_BASE protocol-aware |
| TLS-02 | 77-01 | Self-signed TLS certificate generation via rcgen for LAN | VERIFIED | `tls.rs` generates self-signed cert with IP SAN and localhost DNS SAN |
| TLS-03 | 77-02 | Let's Encrypt TLS for cloud endpoints | NEEDS HUMAN | Bono coordination message sent via comms-link; awaiting confirmation |
| TLS-04 | 77-01, 77-02 | Dual-port support (HTTP 8080 + HTTPS 8443) | VERIFIED | main.rs spawns HTTPS alongside HTTP; HTTP remains on main thread |
| KIOSK-06 | 77-02 | Security response headers via middleware | VERIFIED | CSP, X-Frame-Options DENY, X-Content-Type-Options nosniff, HSTS max-age=300 via tower-helmet |

**Orphaned requirements:** None -- all 5 phase requirements (TLS-01 through TLS-04, KIOSK-06) are claimed and covered by Plans 01 and 02.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none found) | - | - | - | - |

No TODO/FIXME/PLACEHOLDER comments, no `.unwrap()` in production code, no empty implementations, no stub returns.

### Human Verification Required

### 1. HTTPS Listener End-to-End

**Test:** Add `tls_port = 8443` to `[server]` in `C:\RacingPoint\racecontrol.toml`, restart racecontrol. Open `https://192.168.31.23:8443` in Chrome.
**Expected:** Kiosk PWA loads (after accepting self-signed cert warning). No mixed-content errors in browser console. Log line: "RaceControl HTTPS on https://0.0.0.0:8443"
**Why human:** Requires live server deployment, browser TLS handshake, and visual confirmation.

### 2. Security Response Headers

**Test:** `curl -kI https://192.168.31.23:8443/api/v1/health`
**Expected:** Response includes: `content-security-policy: default-src 'self'; ...`, `x-frame-options: DENY`, `x-content-type-options: nosniff`, `strict-transport-security: max-age=300; includeSubDomains`
**Why human:** Requires running server to inspect actual HTTP response headers.

### 3. HTTP Port Not Broken

**Test:** `curl http://192.168.31.23:8080/api/v1/health` and check fleet health dashboard for 8 pods.
**Expected:** HTTP returns 200 JSON, all pods connected via WebSocket.
**Why human:** Need live server to confirm no regression from dual-port changes.

### 4. Let's Encrypt on Cloud (TLS-03)

**Test:** Bono runs `sudo certbot certificates` and `sudo certbot renew --dry-run` on app.racingpoint.cloud VPS.
**Expected:** Valid cert with future expiry date; dry-run succeeds.
**Why human:** TLS-03 is on Bono's VPS -- requires cross-team coordination via comms-link.

### Gaps Summary

No code-level gaps found. All 9 observable truths are verified against the actual codebase. All artifacts exist, are substantive (not stubs), and are properly wired together. All 5 phase requirements are accounted for across the two plans.

The only item requiring human follow-up is TLS-03 (Let's Encrypt on cloud), which depends on Bono's VPS confirmation and is inherently outside this codebase. The remaining 3 human verification items confirm that the correctly-wired code actually works end-to-end on the live server (HTTPS handshake, security headers in responses, HTTP port regression).

**Notable implementation quality:**
- HSTS max-age deliberately set to 300s (5 min) for safe initial deploy -- prevents browser lockout
- HelmetLayer::blank() used instead of with_defaults() to avoid COEP/COOP breaking kiosk proxy
- racingpoint.cloud CORS changed from `.contains()` to exact `==` match (security improvement)
- HTTPS uses `into_make_service()` (no ConnectInfo) -- documented trade-off for rate limiting scope

---

_Verified: 2026-03-20T20:45:00+05:30_
_Verifier: Claude (gsd-verifier)_
