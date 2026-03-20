# Phase 77: Transport Security - Research

**Researched:** 2026-03-20
**Domain:** HTTPS/TLS termination, self-signed cert generation, security response headers for Axum 0.8 on Windows LAN
**Confidence:** HIGH

## Summary

Phase 77 adds TLS termination to the racecontrol server so that customer WiFi browsers connect over HTTPS instead of plaintext HTTP. The server currently binds a single `tokio::net::TcpListener` on port 8080 via `axum::serve()`. This must be extended to also bind an HTTPS listener on port 8443 using `axum-server` with `rustls`. Self-signed certificates are generated at first startup via `rcgen` for the server's LAN IP (192.168.31.23). Security response headers (CSP, HSTS, X-Frame-Options, X-Content-Type-Options) are added via `tower-helmet` middleware. Let's Encrypt for the cloud VPS (TLS-03) is a Bono-side concern -- coordination only, no local code.

The dual-port design (HTTP 8080 + HTTPS 8443) is critical. Pod agents (rc-agent) connect via WebSocket on port 8080 and stay on plain HTTP/WS during this phase. Only customer-facing WiFi browser traffic migrates to HTTPS on 8443. This avoids the "HTTPS breaks WebSocket fleet" pitfall documented in PITFALLS.md. The kiosk PWA's `API_BASE` in `kiosk/src/lib/api.ts` dynamically reads `window.location.hostname` -- when served over HTTPS on 8443, it will automatically target the HTTPS origin.

**Primary recommendation:** Add `axum-server` (0.8, `tls-rustls` feature) + `rcgen` (0.14) to racecontrol crate. Spawn HTTPS listener on 8443 alongside existing HTTP on 8080 via `tokio::spawn`. Generate self-signed cert on first run if no PEM files exist. Add `tower-helmet` (0.3) for security headers on all responses.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TLS-01 | HTTPS for customer-facing PWA traffic (WiFi browser -> server) | axum-server 0.8 with tls-rustls feature provides `bind_rustls()` for TLS termination; kiosk API_BASE auto-detects protocol from window.location |
| TLS-02 | Self-signed TLS certificate generation via rcgen for LAN | rcgen 0.14 `generate_simple_self_signed()` supports SanType::IpAddress for 192.168.31.23; outputs PEM cert+key |
| TLS-03 | Let's Encrypt TLS for cloud endpoints (racingpoint.cloud on Bono VPS) | Already HTTPS on VPS. Bono-side task: verify certbot auto-renewal. James coordinates via comms-link INBOX.md |
| TLS-04 | Dual-port support (HTTP 8080 + HTTPS 8443) for phased pod migration | tokio::spawn second listener; existing HTTP stays on 8080 for pods, HTTPS on 8443 for customer WiFi browsers |
| KIOSK-06 | Security response headers (CSP, X-Frame-Options, X-Content-Type-Options, HSTS) via middleware | tower-helmet 0.3 HelmetLayer provides all four headers; applied as Tower layer on the Axum router |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum-server` | 0.8.0 | TLS termination for Axum | Drop-in HTTPS server for Axum. `bind_rustls(addr, config)` returns `Server<RustlsAcceptor>`. Feature `tls-rustls` pulls in rustls. Verified 0.8.0 on crates.io (released 2025-12-06). |
| `rcgen` | 0.14.7 | Self-signed X.509 cert generation | Pure Rust, maintained by rustls team. `generate_simple_self_signed()` produces cert+key PEM. Supports `SanType::IpAddress` for LAN IPs. Verified 0.14.7 on crates.io. |
| `tower-helmet` | 0.3.0 | Security HTTP response headers | Sets CSP, X-Frame-Options, X-Content-Type-Options, HSTS in one Tower layer. `HelmetLayer::with_defaults()` enables all. Verified 0.3.0 on crates.io. |
| `rustls` | (transitive) | TLS implementation | Pure Rust, no OpenSSL. Works with `+crt-static` build (critical -- pods use static CRT). No vcruntime or OpenSSL DLL needed. |

### Supporting (already in workspace)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tower-http` | 0.6 | Existing middleware stack (CORS, trace) | Security headers layer stacks with existing tower-http layers |
| `tokio` | (existing) | Async runtime | `tokio::spawn` for running dual listeners concurrently |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `axum-server` | `axum-server-dual-protocol` | Accepts HTTP+HTTPS on same port; but we want separate ports for explicit migration control |
| `tower-helmet` | Manual `SetResponseHeaderLayer` from tower-http | Works but verbose (8 separate layers vs 1 HelmetLayer) |
| `rcgen` | `mkcert` CLI tool | External dependency, manual execution; rcgen auto-generates in code at first startup |
| `rcgen` | OpenSSL CLI | Requires OpenSSL installed on server Windows machine; rcgen is pure Rust |
| `rustls` (via axum-server) | `native-tls` / OpenSSL | **Ruled out** -- breaks `+crt-static` build requirement. OpenSSL DLLs needed on all pods. |

**Installation:**

```toml
# crates/racecontrol/Cargo.toml [dependencies]
axum-server = { version = "0.8", features = ["tls-rustls"] }
rcgen = "0.14"
tower-helmet = "0.3"
```

No workspace-level changes needed -- these are racecontrol-only dependencies.

## Architecture Patterns

### Recommended Project Structure

```
crates/racecontrol/src/
    main.rs              # Dual listener startup (HTTP + HTTPS)
    config.rs            # Extended ServerConfig with tls_port, cert_path, key_path
    tls.rs               # NEW: cert generation, loading, RustlsConfig builder
    api/
        routes.rs        # No changes (same router serves both ports)
        middleware.rs     # NEW or extend: security_headers layer
```

### Pattern 1: Dual-Port Server Startup

**What:** Run HTTP and HTTPS listeners concurrently on separate ports sharing the same Axum app Router.

**When to use:** Phased migration where pod agents stay on HTTP while customer browsers move to HTTPS.

```rust
// main.rs — simplified startup pattern
use axum_server::tls_rustls::RustlsConfig;

// Build the shared app Router (same for both ports)
let app = build_app(state.clone());

// HTTP listener on 8080 (existing — for pod agents and backward compat)
let http_addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
    .parse()?;
let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
let http_app = app.clone();
tokio::spawn(async move {
    axum::serve(http_listener, http_app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
});

// HTTPS listener on 8443 (new — for customer WiFi browsers)
if let Some(tls_port) = config.server.tls_port {
    let tls_config = load_or_generate_tls_config(&config).await?;
    let https_addr: SocketAddr = format!("{}:{}", config.server.host, tls_port).parse()?;
    axum_server::bind_rustls(https_addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
}
```

**Key detail:** `axum_server::bind_rustls().serve()` accepts `.into_make_service_with_connect_info::<SocketAddr>()` just like `axum::serve()`. The same app Router with the same middleware stack serves both ports.

### Pattern 2: Auto-Generate Cert on First Run

**What:** Check for PEM files on disk. If missing, generate self-signed cert via rcgen and write to disk. Then load into RustlsConfig.

**When to use:** First-time server startup with TLS enabled.

```rust
use rcgen::{generate_simple_self_signed, CertifiedKey, SanType};
use std::net::IpAddr;

fn generate_self_signed_cert(server_ip: &str) -> Result<(String, String)> {
    let ip: IpAddr = server_ip.parse()?;
    let subject_alt_names = vec![
        SanType::IpAddress(ip),
        SanType::DnsName("localhost".try_into()?),
    ];

    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}
```

**File locations:** `C:\RacingPoint\tls\cert.pem` and `C:\RacingPoint\tls\key.pem` on the server. These live alongside `racecontrol.toml` in the RacingPoint config directory.

### Pattern 3: Security Headers via tower-helmet

**What:** Apply HelmetLayer to the router for all responses.

```rust
use tower_helmet::HelmetLayer;
use tower_helmet::header::ContentSecurityPolicy;
use std::collections::HashMap;

// Custom CSP for the kiosk PWA
let mut csp_directives = HashMap::new();
csp_directives.insert("default-src", vec!["'self'"]);
csp_directives.insert("script-src", vec!["'self'"]);
csp_directives.insert("style-src", vec!["'self'", "'unsafe-inline'"]);  // Tailwind needs this
csp_directives.insert("img-src", vec!["'self'", "data:"]);
csp_directives.insert("connect-src", vec!["'self'", "wss:", "ws:"]);    // WebSocket connections
csp_directives.insert("frame-ancestors", vec!["'none'"]);

let csp = ContentSecurityPolicy {
    directives: csp_directives,
    ..Default::default()
};

let helmet = HelmetLayer::with_defaults().enable(csp);

let app = Router::new()
    .merge(routes)
    .layer(helmet)              // Security headers
    .layer(cors_layer)          // Existing CORS
    .layer(TraceLayer::new_for_http());  // Existing tracing
```

**Middleware ordering:** HelmetLayer should be applied AFTER CORS (so CORS preflight responses also get security headers) but BEFORE route handlers. In Axum, layers applied later run first (outermost), so: `.layer(helmet)` before `.layer(cors)` in code means helmet wraps cors.

### Pattern 4: Config Extension

**What:** Extend `ServerConfig` with optional TLS fields.

```rust
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// HTTPS port. When set, enables TLS listener alongside HTTP.
    #[serde(default)]
    pub tls_port: Option<u16>,
    /// Path to TLS certificate PEM file. Auto-generated if missing.
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to TLS private key PEM file. Auto-generated if missing.
    #[serde(default)]
    pub key_path: Option<String>,
}
```

**racecontrol.toml changes:**
```toml
[server]
host = "0.0.0.0"
port = 8080
tls_port = 8443
# cert_path and key_path are optional — auto-generated to C:\RacingPoint\tls\ if missing
```

### Anti-Patterns to Avoid

- **Replacing HTTP with HTTPS on port 8080:** This breaks all 8 pod agents instantly. Pods use `ws://192.168.31.23:8080/ws/agent`. Changing 8080 to TLS requires updating every pod's `rc-agent.toml` simultaneously. Use a NEW port (8443) instead.
- **TLS on pod-to-server WebSocket (now):** rc-agent uses `tokio-tungstenite` with `native-tls` feature. Switching pods to WSS requires distributing the self-signed CA cert to every pod and changing their tungstenite TLS config. This is out of scope for Phase 77 -- pods stay on HTTP/WS.
- **Generating certs at every startup:** Generate once, save to disk, reuse. Only regenerate if PEM files are deleted or expired.
- **HSTS with long max-age on first deploy:** Start with a short max-age (e.g., 300 seconds / 5 minutes) during testing. Only increase after verifying HTTPS works reliably. HSTS tells browsers to NEVER connect via HTTP -- if HTTPS breaks, browsers are locked out.
- **CSP that blocks the PWA's own resources:** The kiosk PWA uses Tailwind (inline styles via `'unsafe-inline'`), WebSocket connections (`ws:` / `wss:` in connect-src), and data: URIs for images. A strict CSP that blocks these will silently break the UI.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TLS termination | Custom TCP+TLS acceptor loop | `axum-server` 0.8 with `tls-rustls` | TLS handshake, ALPN, connection pooling are complex. axum-server handles it. |
| X.509 certificate generation | OpenSSL CLI scripts or manual DER construction | `rcgen` 0.14 | X.509 is a complex ASN.1 format. rcgen abstracts it to a single function call. |
| Security response headers | Manual `SetResponseHeaderLayer` for each header | `tower-helmet` 0.3 | 8 individual header layers vs 1 HelmetLayer. Easy to miss one or misformat a value. |
| CSP header value formatting | String concatenation of CSP directives | `tower-helmet::header::ContentSecurityPolicy` | CSP syntax is semicolon+space delimited with specific quoting rules. Easy to produce invalid headers. |

## Common Pitfalls

### Pitfall 1: HTTPS Breaks Pod WebSocket Fleet

**What goes wrong:** Enabling TLS on port 8080 disconnects all 8 rc-agent pods that connect via `ws://192.168.31.23:8080/ws/agent`. Pods enter reconnect loops. Fleet management is blind.
**Why it happens:** TLS on the HTTP port means ALL connections must speak TLS. WebSocket upgrades happen over the same TCP connection.
**How to avoid:** NEW port 8443 for HTTPS. Existing 8080 stays plain HTTP for pods. Pods are NOT migrated in this phase.
**Warning signs:** Plan mentions "change port 8080 to HTTPS" or "replace axum::serve with bind_rustls".

### Pitfall 2: Self-Signed Cert Not Trusted by Browsers

**What goes wrong:** Customer's phone browser shows "Your connection is not private" warning when accessing `https://192.168.31.23:8443`. Customers cannot access the PWA.
**Why it happens:** Self-signed certs are not in the browser's trust store by default. Chrome on Android is especially strict.
**How to avoid:** For pod kiosk browsers (Chrome), install the generated CA cert in the Windows certificate store via the deploy script. For customer WiFi phones, accept that there will be a warning OR use a hostname-based approach with DNS (e.g., `racingpoint.local` with mDNS). The self-signed cert is primarily for the kiosk pods and staff dashboard.
**Warning signs:** No cert trust distribution plan; assuming self-signed "just works" on customer phones.

### Pitfall 3: CORS Blocks HTTPS Origins

**What goes wrong:** The existing CORS predicate checks `origin.starts_with("http://192.168.31.")`. HTTPS requests from port 8443 have origin `https://192.168.31.23:8443` which does NOT match the `http://` prefix. All cross-origin API calls fail with CORS errors.
**How to avoid:** Update the CORS predicate to also allow `https://` origins from the LAN subnet. Add `origin.starts_with("https://192.168.31.")` to the predicate.
**Warning signs:** Plan does not mention CORS update alongside HTTPS enablement.

### Pitfall 4: HSTS Locks Out HTTP During Testing

**What goes wrong:** HSTS header with long max-age (e.g., 1 year) is set. If HTTPS breaks, browsers remember HSTS and refuse HTTP connections. Staff dashboard becomes inaccessible.
**How to avoid:** Start with `max-age=300` (5 minutes). Only increase to production values (e.g., 31536000) after 1 week of stable HTTPS operation.
**Warning signs:** `Strict-Transport-Security: max-age=31536000` set on first deploy without testing period.

### Pitfall 5: axum-server vs axum::serve API Mismatch

**What goes wrong:** `axum_server::bind_rustls().serve()` requires `.into_make_service()` (not `.into_make_service_with_connect_info::<SocketAddr>()`). If `ConnectInfo<SocketAddr>` is used in handlers, HTTPS requests fail with extraction errors.
**How to avoid:** Verify that `axum_server` 0.8 supports `into_make_service_with_connect_info`. The current codebase uses `ConnectInfo<SocketAddr>` (for `tower_governor` PeerIpKeyExtractor in Phase 76). If axum-server does not support it, use `IntoMakeServiceWithConnectInfo` from axum directly.
**Warning signs:** HTTPS listener starts but rate limiting or IP extraction fails on HTTPS port only.

### Pitfall 6: Kiosk PWA Mixed Content

**What goes wrong:** Kiosk PWA served over HTTPS on port 8443 makes API calls to `http://hostname:8080`. Browsers block mixed content (HTTPS page calling HTTP API). API calls silently fail.
**How to avoid:** The kiosk `API_BASE` in `kiosk/src/lib/api.ts` uses `window.location.hostname` with hardcoded port 8080. When served over HTTPS, it must target the HTTPS port. Solution: derive both protocol and port from `window.location` so HTTPS pages call HTTPS API.
**Warning signs:** Kiosk works on HTTP but breaks when accessed via HTTPS bookmark.

## Code Examples

### Example 1: TLS Module (tls.rs)

```rust
// crates/racecontrol/src/tls.rs
use axum_server::tls_rustls::RustlsConfig;
use rcgen::{generate_simple_self_signed, CertifiedKey, SanType};
use std::net::IpAddr;
use std::path::Path;

const DEFAULT_CERT_DIR: &str = "C:\\RacingPoint\\tls";
const CERT_FILENAME: &str = "cert.pem";
const KEY_FILENAME: &str = "key.pem";

/// Load existing PEM files or generate self-signed cert for the server IP.
pub async fn load_or_generate_rustls_config(
    server_ip: &str,
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> anyhow::Result<RustlsConfig> {
    let cert_file = cert_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}\\{}", DEFAULT_CERT_DIR, CERT_FILENAME));
    let key_file = key_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}\\{}", DEFAULT_CERT_DIR, KEY_FILENAME));

    if !Path::new(&cert_file).exists() || !Path::new(&key_file).exists() {
        tracing::info!("TLS certificates not found, generating self-signed for {}", server_ip);
        generate_and_save(server_ip, &cert_file, &key_file)?;
    }

    let config = RustlsConfig::from_pem_file(&cert_file, &key_file).await?;
    tracing::info!("TLS configured from {} and {}", cert_file, key_file);
    Ok(config)
}

fn generate_and_save(server_ip: &str, cert_path: &str, key_path: &str) -> anyhow::Result<()> {
    let ip: IpAddr = server_ip.parse()?;
    let san = vec![
        SanType::IpAddress(ip),
        SanType::DnsName("localhost".try_into()?),
    ];
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(san)?;

    // Ensure directory exists
    if let Some(parent) = Path::new(cert_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cert_path, cert.pem())?;
    std::fs::write(key_path, key_pair.serialize_pem())?;
    tracing::info!("Self-signed TLS certificate generated for IP {}", server_ip);
    Ok(())
}
```

### Example 2: Dual-Port Startup in main.rs

```rust
// In main() after building the app Router:

// HTTP listener (existing behavior, unchanged)
let http_listener = tokio::net::TcpListener::bind(&bind_addr).await?;
tracing::info!("RaceControl HTTP on http://{}", bind_addr);

// HTTPS listener (new, optional)
let https_handle = if let Some(tls_port) = state.config.server.tls_port {
    let tls_config = tls::load_or_generate_rustls_config(
        &state.config.server.host,
        state.config.server.cert_path.as_deref(),
        state.config.server.key_path.as_deref(),
    ).await?;
    let https_addr: SocketAddr = format!("{}:{}", state.config.server.host, tls_port).parse()?;
    let https_app = app.clone();
    tracing::info!("RaceControl HTTPS on https://{}", https_addr);
    Some(tokio::spawn(async move {
        axum_server::bind_rustls(https_addr, tls_config)
            .serve(https_app.into_make_service())
            .await
    }))
} else {
    None
};

// Run HTTP (blocking) — if HTTPS task panics, it logs but HTTP continues
axum::serve(http_listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
```

### Example 3: Security Headers Middleware

```rust
use tower_helmet::HelmetLayer;
use tower_helmet::header::ContentSecurityPolicy;

fn security_headers_layer() -> HelmetLayer {
    let mut directives = std::collections::HashMap::new();
    directives.insert("default-src", vec!["'self'"]);
    directives.insert("script-src", vec!["'self'"]);
    directives.insert("style-src", vec!["'self'", "'unsafe-inline'"]);
    directives.insert("img-src", vec!["'self'", "data:"]);
    directives.insert("connect-src", vec!["'self'", "ws:", "wss:"]);
    directives.insert("frame-ancestors", vec!["'none'"]);
    directives.insert("base-uri", vec!["'self'"]);
    directives.insert("form-action", vec!["'self'"]);

    let csp = ContentSecurityPolicy {
        directives,
        ..Default::default()
    };

    HelmetLayer::with_defaults().enable(csp)
}
```

### Example 4: Updated CORS Predicate

```rust
// Add https:// origins alongside existing http:// origins
CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
        let origin = origin.to_str().unwrap_or("");
        origin.starts_with("http://localhost:")
            || origin.starts_with("https://localhost:")
            || origin.starts_with("http://127.0.0.1:")
            || origin.starts_with("https://127.0.0.1:")
            || origin.starts_with("http://192.168.31.")
            || origin.starts_with("https://192.168.31.")
            || origin.starts_with("http://kiosk.rp")
            || origin.starts_with("https://kiosk.rp")
            || origin == "https://app.racingpoint.cloud"  // Exact match, not .contains()
    }))
```

### Example 5: Kiosk API_BASE Fix for HTTPS

```typescript
// kiosk/src/lib/api.ts — protocol-aware API_BASE
const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ||
  (typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.host}`
    : "http://localhost:8080");
```

When the kiosk is served from `https://192.168.31.23:8443`, this produces `https://192.168.31.23:8443` as the API base -- no mixed content.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `axum::serve` only | `axum::serve` + `axum_server::bind_rustls` | Phase 77 | HTTP stays for pods; HTTPS added for browsers |
| No security headers | `tower-helmet` 0.3 HelmetLayer | Phase 77 | CSP, HSTS, X-Frame-Options, X-Content-Type-Options on all responses |
| CORS http:// only | CORS http:// + https:// | Phase 77 | HTTPS origins allowed for LAN subnet |
| API_BASE hardcoded http:// | API_BASE from window.location | Phase 77 | PWA works on both HTTP and HTTPS |
| tower-helmet 0.2 (in earlier research) | tower-helmet 0.3 | 2025 | Version 0.3.0 is current on crates.io |

**Deprecated/outdated:**
- `tower-helmet` 0.2 was referenced in STACK.md but current version is 0.3.0
- `rcgen` 0.14 referenced in STACK.md; current is 0.14.7 (minor patch, same API)

## Open Questions

1. **ConnectInfo<SocketAddr> with axum-server**
   - What we know: Current HTTP server uses `.into_make_service_with_connect_info::<SocketAddr>()` for IP extraction (needed by tower_governor PeerIpKeyExtractor from Phase 76)
   - What's unclear: Whether axum-server 0.8's `.serve()` supports `into_make_service_with_connect_info` -- the docs show only `into_make_service()`
   - Recommendation: Test at build time. If not supported, use `axum::extract::ConnectInfo` via `IntoMakeServiceWithConnectInfo` wrapper, or fall back to `X-Forwarded-For` header extraction for the HTTPS port

2. **Customer phone browser cert trust**
   - What we know: Self-signed certs trigger browser warnings on customer phones connecting to WiFi PWA
   - What's unclear: Whether a splash page / cert acceptance flow is acceptable UX for the venue
   - Recommendation: For Phase 77, accept that customer WiFi phones see a warning. Phase 77 primarily secures kiosk pod browsers (where the CA cert can be installed). Customer phone HTTPS is a stretch goal.

3. **tower-helmet 0.3 API compatibility with tower-http 0.6**
   - What we know: Both are Tower middleware layers; should compose fine
   - What's unclear: Exact generic type compatibility at compile time
   - Recommendation: Verify at build time. If type mismatch, fall back to manual `SetResponseHeaderLayer` for the 4 required headers.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + integration test harness |
| Config file | `crates/racecontrol/tests/integration.rs` (existing) |
| Quick run command | `cargo test -p racecontrol --lib` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TLS-01 | HTTPS listener accepts connections on tls_port | integration | `cargo test -p racecontrol tls_listener -- --nocapture` | No -- Wave 0 |
| TLS-02 | rcgen generates valid self-signed cert with IP SAN | unit | `cargo test -p racecontrol tls::test -- --nocapture` | No -- Wave 0 |
| TLS-03 | Let's Encrypt on Bono VPS | manual-only | `curl -v https://app.racingpoint.cloud/api/v1/health` | N/A (Bono-side) |
| TLS-04 | HTTP 8080 and HTTPS 8443 run simultaneously | integration | `cargo test -p racecontrol dual_port -- --nocapture` | No -- Wave 0 |
| KIOSK-06 | Security headers present in HTTP responses | unit | `cargo test -p racecontrol security_headers -- --nocapture` | No -- Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol --lib`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/tls.rs` -- TLS module with cert generation + loading (TLS-01, TLS-02)
- [ ] Unit test for `generate_and_save()` -- verify PEM output format and IP SAN presence
- [ ] Unit test for `security_headers_layer()` -- verify CSP, HSTS, X-Frame-Options, X-Content-Type-Options in response
- [ ] Integration test for dual-port startup -- bind HTTP + HTTPS on ephemeral ports, verify both accept connections

## Sources

### Primary (HIGH confidence)

- [axum-server 0.8 docs -- bind_rustls](https://docs.rs/axum-server/latest/axum_server/fn.bind_rustls.html) -- function signature, RustlsConfig loading
- [axum-server tls_rustls module](https://docs.rs/axum-server/latest/axum_server/tls_rustls/index.html) -- RustlsConfig::from_pem_file API
- [rcgen docs](https://docs.rs/rcgen/latest/rcgen/) -- generate_simple_self_signed, CertifiedKey, SanType::IpAddress
- [tower-helmet crate](https://docs.rs/tower-helmet) -- HelmetLayer::with_defaults, ContentSecurityPolicy directives
- [tower-helmet GitHub](https://github.com/Atrox/tower-helmet) -- source and examples
- cargo search verified versions: axum-server 0.8.0, rcgen 0.14.7, tower-helmet 0.3.0

### Secondary (MEDIUM confidence)

- [axum TLS example](https://github.com/tokio-rs/axum/blob/main/examples/tls-rustls/src/main.rs) -- official axum TLS example pattern
- [axum-server-dual-protocol docs](https://docs.rs/axum-server-dual-protocol/latest/axum_server_dual_protocol/) -- dual protocol approach (evaluated, not chosen)
- [Rust users forum -- multiple listeners](https://users.rust-lang.org/t/axum-multiple-listeners/126151) -- tokio::spawn dual listener pattern

### Codebase (direct analysis)

- `crates/racecontrol/src/main.rs:574-582` -- current TcpListener::bind + axum::serve startup
- `crates/racecontrol/src/config.rs:58-64` -- current ServerConfig (host, port)
- `kiosk/src/lib/api.ts:3-7` -- API_BASE uses window.location.hostname with hardcoded http:// + port 8080
- `crates/rc-agent/Cargo.toml:30` -- tokio-tungstenite uses native-tls (NOT rustls)
- `crates/rc-agent/src/main.rs:213` -- default core URL is `ws://127.0.0.1:8080/ws/agent`
- `crates/rc-agent/src/main.rs:804-805` -- URL parsing already handles both ws:// and wss://
- `.planning/phases/75-security-audit-foundations/SECURITY-AUDIT.md:499-510` -- HTTPS state: all local services are plain HTTP
- `.planning/research/PITFALLS.md` -- Pitfall 4 (HTTPS breaks WebSocket) directly addressed by dual-port design

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- axum-server 0.8 and rcgen 0.14 verified on crates.io; tower-helmet 0.3 verified; all pure Rust, compatible with +crt-static
- Architecture: HIGH -- dual-port pattern is well-documented; codebase startup code is straightforward to extend
- Pitfalls: HIGH -- PITFALLS.md already identified the WebSocket fleet breakage risk; CORS and mixed content are well-known issues
- Security headers: MEDIUM -- tower-helmet 0.3 API verified but compile-time compatibility with tower-http 0.6 needs build verification

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, crate versions unlikely to change in 30 days)
