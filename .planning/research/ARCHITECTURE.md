# Architecture Patterns: Security Layers for Racing Point Operations

**Domain:** Security hardening for distributed eSports cafe operations system
**Researched:** 2026-03-20
**Confidence:** HIGH (based on direct codebase analysis, not external sources)

---

## Current System Topology

```
                    INTERNET
                       |
              [Bono VPS :443]          <-- app.racingpoint.cloud (HTTPS)
              Cloud API + Gateway       <-- PostgreSQL cloud DB
                       |
            ~~~~ WAN / Tailscale ~~~~
                       |
         [Router 192.168.31.1]         <-- Local network boundary
                       |
       +---------------+---------------+
       |               |               |
[James .27]    [Server .23]      [Pods .28-.91]
RTX 4070       racecontrol:8080  rc-agent:8090 (x8)
Ollama:11434   kiosk:3300        rc-sentry:8091
webterm:9999   dashboard:3200    lock-screen (local)
               rc-sentry:8091
               SQLite DB
```

### Communication Channels (Current State)

| Channel | Protocol | Auth | Encrypted |
|---------|----------|------|-----------|
| Pod agent <-> Server | WebSocket :8080 | None (pod_id in first message) | No (HTTP) |
| Pod remote_ops | HTTP :8090 | None | No |
| rc-sentry (all nodes) | HTTP :8091 | None | No |
| Customer PWA <-> Server | HTTP :8080 | JWT (customer_sessions table) | No |
| Staff Dashboard <-> Server | HTTP/WS :8080 | None | No |
| Kiosk <-> Server | HTTP :3300 proxied via :8080 | None | No |
| Server <-> Cloud VPS | HTTPS :443 | terminal_secret header | Yes |
| Cloud sync (relay mode) | HTTP localhost :8765/8766 | None (localhost only) | No |
| Bono relay | HTTP over Tailscale | x-api-key header | Tailscale encrypted |
| Remote terminal (cloud) | HTTPS :443 | terminal_secret | Yes |
| WhatsApp (Evolution API) | HTTPS | evolution_api_key | Yes |

---

## Recommended Security Architecture

### Layered Defense Model

Security layers apply from outside-in. Each layer is independent -- a breach of one does not compromise others.

```
Layer 1: Network Boundary         Router firewall + no port forwarding
Layer 2: Transport Security       HTTPS/TLS for all data in transit
Layer 3: API Authentication       Bearer tokens + HMAC for service-to-service
Layer 4: Authorization             Role-based route guards (admin/staff/customer)
Layer 5: Data Protection           Encryption at rest, PII audit, minimal retention
Layer 6: Kiosk Hardening          OS-level lockdown, PWA escape prevention
```

**What this project covers:** Layers 2-6. Layer 1 (network/firewall/VLANs) is out of scope per PROJECT-v12.md.

---

## Component Boundaries

### Component 1: API Authentication Middleware (racecontrol)

**Responsibility:** Verify identity of every HTTP/WS request before it reaches route handlers.

**Communicates with:** All clients (pods, dashboard, PWA, kiosk, cloud, bots)

**Current state:** JWT infrastructure exists (`jsonwebtoken` crate, `Claims` struct, `auth/mod.rs`) but is only used for customer PWA sessions and terminal auth. Staff dashboard and billing endpoints have zero authentication.

**Target state:**
```
                       Request arrives at :8080
                              |
                    [CORS Layer] (existing)
                              |
                    [Auth Middleware Layer]  <-- NEW
                       /      |      \
                 Public    Authed    Service
                 routes    routes    routes
                   |         |         |
                /health   /billing   /sync/push
                /venue    /pods      /ws (agent)
                /kiosk/*  /admin/*
```

**Implementation pattern:** Axum middleware layer using `tower::Layer` + `axum::middleware::from_fn`. Three tiers:

| Tier | Routes | Auth Method |
|------|--------|-------------|
| Public | `/health`, `/venue`, `/kiosk/*` proxy, `/customer/login`, `/customer/register` | None |
| Customer | `/customer/*` (except login/register) | JWT Bearer token from customer_sessions |
| Staff/Admin | `/billing/*`, `/pods/*`, `/drivers/*`, `/admin/*`, `/wallet/*`, etc. | Admin PIN -> JWT, or daily-rotating employee PIN -> JWT |
| Service | `/ws` (agent), `/sync/*`, `/fleet/*` | HMAC shared secret or pre-shared key |

**Key design decision:** Use Axum's nested Router with different middleware stacks rather than a single middleware that checks every route. This is idiomatic Axum 0.8 and avoids a giant allowlist.

```rust
// Pseudostructure (not literal code)
let public_routes = Router::new()
    .route("/health", get(health))
    .route("/venue", get(venue_info));

let customer_routes = Router::new()
    .route("/customer/profile", get(profile))
    .layer(from_fn(require_customer_jwt));

let staff_routes = Router::new()
    .route("/billing/start", post(start_billing))
    .layer(from_fn(require_staff_jwt));

let service_routes = Router::new()
    .route("/sync/push", post(sync_push))
    .layer(from_fn(require_service_hmac));

let app = Router::new()
    .merge(public_routes)
    .merge(customer_routes)
    .merge(staff_routes)
    .merge(service_routes);
```

### Component 2: Admin Authentication (racecontrol auth module)

**Responsibility:** Gate all admin/staff operations behind PIN-based authentication. Uday enters PIN once, gets JWT valid for shift duration.

**Communicates with:** Staff dashboard (web), terminal auth, bot commands

**Current state:** `terminal_auth` endpoint exists with daily-rotating PIN. `staff_validate_pin` exists. No middleware enforcement -- these are optional entry points that return JWTs, but downstream routes do not check for them.

**Target state:** Staff JWT required on all non-public, non-customer routes. Admin PIN entered once per browser session (stored in httpOnly cookie or localStorage). JWT expiry = 12 hours (one shift).

**Design:**
- Reuse existing `jsonwebtoken` infrastructure
- Add `role` claim to JWT: `{ sub: "admin", role: "staff", exp, iat }`
- Staff login: `POST /auth/admin-login` with `{ pin: "1234" }` -> returns JWT
- Middleware extracts `Authorization: Bearer <jwt>` and validates role

### Component 3: Service-to-Service Auth (rc-agent <-> racecontrol)

**Responsibility:** Prevent unauthorized devices from connecting as pods or issuing commands.

**Communicates with:** rc-agent instances on pods, rc-sentry on all nodes

**Current state:** WebSocket connections from pods are unauthenticated. Any device on the LAN can connect to `:8080/ws` and impersonate a pod. rc-agent `:8090` and rc-sentry `:8091` accept commands from any LAN device with zero auth.

**Target state:** Pre-shared key (PSK) authentication for all service-to-service communication.

**Design:**
- Shared secret in `racecontrol.toml` (`[auth].service_secret`) and `rc-agent.toml` (`[core].service_secret`)
- WebSocket: Agent sends HMAC(pod_id + timestamp, secret) in first message. Server validates within 30s window.
- HTTP (remote_ops :8090): `X-Service-Key: <secret>` header on all requests. Middleware rejects without it.
- rc-sentry: Same `X-Service-Key` header. This is the highest-risk endpoint (raw shell exec with no auth).

**Why PSK over mTLS:** PSK is simple to deploy, simple to rotate, and adequate for a trusted LAN. mTLS adds certificate management complexity that is not justified for 8 pods on a private subnet. If the network grows beyond the venue, revisit.

### Component 4: HTTPS / TLS (transport layer)

**Responsibility:** Encrypt all data in transit between PWA browsers and the server.

**Communicates with:** All browser-based clients (customer PWA, staff dashboard, kiosk)

**Current state:** All local traffic is plain HTTP. Cloud traffic to VPS is HTTPS. Customer PII (phone, name, payment info) travels in plaintext on the LAN.

**Target state:** TLS termination at racecontrol `:8080` for all browser traffic.

**Design options (choose one):**

| Option | Pros | Cons | Recommendation |
|--------|------|------|----------------|
| Self-signed cert on server | Simple, no external deps | Browser warnings, PWA install issues | No |
| Caddy/nginx reverse proxy with Let's Encrypt | Automatic TLS, well-tested | Adds another service, needs domain pointed at LAN IP | Maybe (if external access needed) |
| `axum-server` with `rustls` + self-signed CA | In-process TLS, no extra services | Must distribute CA cert to pod browsers, more Rust code | **Yes** |
| mkcert (local CA) | Trusted certs for LAN | Manual CA distribution | Good complement to option 3 |

**Recommended approach:** Use `mkcert` to generate a local CA + server cert for `192.168.31.23` and `racingpoint.local`. Install the CA cert on all pod browsers (one-time setup via deploy script). Configure Axum with `axum-server` + `rustls` to serve HTTPS. This gives trusted TLS without external dependencies or browser warnings.

**For the cloud path:** Already HTTPS via VPS. No changes needed.

**WebSocket upgrade:** Once HTTPS is enabled, WebSocket connections automatically upgrade to WSS. No code changes needed in rc-agent beyond changing `ws://` to `wss://` in the connection URL.

### Component 5: Data Protection (SQLite + cloud)

**Responsibility:** Protect customer PII at rest and limit exposure.

**Communicates with:** SQLite on server, PostgreSQL on cloud VPS

**Current state:** SQLite file on server `.23` stores phone numbers, names, emails, OTP codes, wallet balances, session history in plaintext. Cloud sync replicates drivers table (with PII) to cloud PostgreSQL.

**Target state:**
- PII fields encrypted at application level before SQLite storage
- OTP codes cleared after verification (already done in `verify_otp`)
- Minimal PII retention policy
- SQLite file permissions locked to service account

**Design:**
- **Application-level encryption** for `drivers.phone`, `drivers.email` using AES-256-GCM with a key from `racecontrol.toml`
- **NOT SQLite-level encryption** (SQLCipher) -- adds build complexity, breaks sqlx compatibility, and does not protect against application-level leaks
- Encryption key stored in config file (which should have restricted file permissions)
- Phone number stored as encrypted blob + last-4-digits hash for lookup
- Cloud sync: encrypted fields stay encrypted in transit and at rest on cloud DB

### Component 6: Kiosk Hardening (rc-agent + OS)

**Responsibility:** Prevent customers from escaping the kiosk PWA to access the underlying Windows OS or network.

**Communicates with:** rc-agent kiosk module, Windows Group Policy, browser configuration

**Current state:** `kiosk.rs` module exists with process allowlisting, keyboard hook stubs (unused), and debug mode. Kiosk browser runs in kiosk/fullscreen mode but escape vectors exist (Alt+Tab, Ctrl+Alt+Del, task manager).

**Target state:** Defense-in-depth kiosk lockdown:
1. Windows Shell replacement (browser as shell instead of explorer.exe)
2. Group Policy: disable Task Manager, disable Ctrl+Alt+Del options, disable USB mass storage
3. rc-agent process monitor kills unauthorized processes
4. Browser configured with `--kiosk` flag + disabled dev tools + disabled right-click

---

## Data Flow Diagrams

### Flow 1: Customer Session (Current -- No Auth on Admin)

```
Staff Dashboard                racecontrol:8080              Pod rc-agent
     |                              |                            |
     |--POST /auth/assign--------->|                            |
     |  {pod, driver, tier}        |--WS: ShowPinLockScreen--->|
     |                              |                            |
     |                              |<--WS: PinEntered----------|
     |                              |   (customer types PIN)     |
     |                              |                            |
     |                              |--validate_pin()            |
     |                              |--start_billing()           |
     |                              |--WS: LaunchGame---------->|
     |<--WS: DashboardEvent--------|                            |
     |  (session_started)          |                            |
```

### Flow 2: Customer Session (Target -- Auth on Admin)

```
Staff Dashboard                racecontrol:8080              Pod rc-agent
     |                              |                            |
     |--POST /auth/admin-login---->|                            |
     |  {pin: "1234"}             |                            |
     |<--{jwt: "eyJ..."}----------|                            |
     |                              |                            |
     |--POST /auth/assign--------->|                            |
     |  Authorization: Bearer jwt  |                            |
     |  {pod, driver, tier}        |--WSS: ShowPinLockScreen-->|
     |                              |  (HMAC-authed WS)         |
     |                              |                            |
     |                              |<--WSS: PinEntered---------|
     |                              |                            |
     |                              |--validate_pin()            |
     |                              |--start_billing()           |
     |<--WSS: DashboardEvent-------|                            |
```

### Flow 3: Cloud Sync (Current -- terminal_secret only)

```
racecontrol (venue)          Cloud VPS
     |                          |
     |--GET /sync/changes------>|
     |  X-Terminal-Secret: xxx  |
     |<--{drivers, pricing}-----|
     |                          |
     |--POST /sync/push-------->|
     |  X-Terminal-Secret: xxx  |
     |  {laps, billing, pods}   |
     |                          |
```

Cloud sync already uses a shared secret. This is adequate -- just ensure the secret is strong and rotated periodically.

### Flow 4: Bot Command (Discord/WhatsApp -> Cloud -> Venue)

```
WhatsApp/Discord              Cloud VPS              racecontrol (venue)
     |                          |                          |
     |--"start pod 3"--------->|                          |
     |                          |--POST /actions---------->|
     |                          |  X-Terminal-Secret: xxx  |
     |                          |                          |
     |                          |<--POST /actions/ack------|
     |<--"Session started"------|                          |
```

Bot commands already flow through cloud with terminal_secret auth. The gap is that the cloud side needs to verify the bot command came from an authorized user (Uday). This is a cloud/bot-side concern, not venue-side.

---

## Build Order (Dependency Graph)

Security components have a specific dependency order. Building out of order causes rework.

```
Phase 1: Security Audit
    |     (discover current state -- what's actually exposed)
    |
Phase 2: API Auth Middleware + Admin PIN
    |     (depends on: nothing. Biggest hole, biggest impact)
    |     Enables: staff JWT required for billing/pod control
    |
Phase 3: Service-to-Service Auth (PSK for pods)
    |     (depends on: Phase 2 pattern for middleware)
    |     Enables: pods authenticate to server, remote_ops locked
    |
Phase 4: HTTPS / TLS
    |     (depends on: nothing technically, but Phase 2-3 first
    |      because auth tokens in plaintext HTTP are still better
    |      than no auth at all)
    |     Enables: encrypted transport for all browser traffic
    |
Phase 5: Data Protection
    |     (depends on: Phase 4 for transit security)
    |     Enables: PII encrypted at rest
    |
Phase 6: Kiosk Hardening
          (independent of other phases, can run in parallel)
          Enables: customer escape prevention
```

### Why This Order

1. **Audit first** because the PROJECT-v12.md itself says "Security audit -- discover current auth state, data storage locations, HTTPS coverage." You cannot fix what you have not measured. The audit also validates/invalidates assumptions in this architecture doc.

2. **API auth before HTTPS** because unauthenticated endpoints are a bigger risk than unencrypted transport on a private LAN. An attacker on the LAN can `curl POST /billing/start` right now -- that is worse than sniffing encrypted but unauthenticated traffic.

3. **Service auth after API auth** because the middleware pattern established in Phase 2 (Axum layer-based auth) directly applies to Phase 3. The pod PSK middleware follows the same `from_fn` pattern.

4. **HTTPS after auth** because TLS without auth still allows unauthorized access (just encrypted unauthorized access). Auth without TLS at least prevents unauthorized access (tokens can be sniffed but this requires active LAN presence).

5. **Data protection last** (of the main phases) because encrypted PII at rest is lower priority than preventing unauthorized API access. If someone can call `/billing/start` without auth, encrypted phone numbers do not matter.

6. **Kiosk hardening is parallel** because it is an OS/browser concern independent of the API security stack. It can proceed alongside any other phase.

---

## Patterns to Follow

### Pattern 1: Axum Middleware Tower for Auth

**What:** Use Axum's `middleware::from_fn` with state injection to compose auth layers.

**When:** Every route group that needs authentication.

```rust
use axum::{middleware, extract::Request, http::StatusCode, response::Response};

async fn require_staff_jwt(
    req: Request,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header.strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate JWT using existing jsonwebtoken crate
    let claims = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Inject claims into request extensions for downstream use
    req.extensions_mut().insert(claims.claims);
    Ok(next.run(req).await)
}
```

**Why:** This is idiomatic Axum. The project already uses `tower` and `tower-http` layers (CORS, trace). Adding auth as another layer is consistent with existing patterns.

### Pattern 2: PSK with HMAC for Service Auth

**What:** Pre-shared key validated via HMAC signature rather than raw key comparison.

**When:** Pod agent WebSocket connection, rc-sentry commands, remote_ops HTTP.

```rust
// Agent sends: HMAC-SHA256(pod_id + timestamp, shared_secret)
// Server validates: recompute HMAC, check timestamp within 30s window
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn validate_service_auth(
    pod_id: &str, timestamp: u64, signature: &str, secret: &str
) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_secs();
    if now.abs_diff(timestamp) > 30 { return false; }

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{}{}", pod_id, timestamp).as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    expected == signature
}
```

**Why raw PSK is insufficient:** If the secret is sent as a header, any LAN sniffer captures it. HMAC with timestamp prevents replay attacks even on plaintext HTTP (which is the state before Phase 4 TLS).

### Pattern 3: Route Grouping by Auth Level

**What:** Split the current monolithic `api_routes()` function into grouped routers with different auth layers.

**When:** Phase 2 implementation.

**Current:** Single `api_routes()` function returns one Router with 100+ routes, no auth middleware.

**Target:** Multiple router functions, each with appropriate auth layer, merged at the top level.

This also addresses the CONCERNS.md issue of the 9,515-line `routes.rs` monolith -- security refactoring naturally forces route decomposition.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Auth Checks Inside Route Handlers

**What:** Calling `verify_jwt()` at the top of each handler function.

**Why bad:** Easy to forget on new routes. One missed check = security hole. The existing codebase already has `jwt_error_to_401` middleware that post-processes responses -- this is a symptom of handler-level auth where the JWT check happens inside the handler and returns a 200 with an error message.

**Instead:** Middleware layer that rejects before the handler runs. Handlers never see unauthenticated requests.

### Anti-Pattern 2: Global CORS Allow-All

**What:** Setting `CorsLayer::permissive()` or `Access-Control-Allow-Origin: *`.

**Why bad:** Allows any website to make API calls to the racecontrol server if a browser on the LAN visits a malicious page.

**Instead:** Restrict CORS origins to known frontends: `http://192.168.31.23:3300` (kiosk), `http://192.168.31.23:3200` (dashboard), `https://app.racingpoint.cloud`.

### Anti-Pattern 3: Storing Secrets in Code or Default Config Values

**What:** The existing `default_jwt_secret()` function returns a hardcoded string.

**Why bad:** If config omits `jwt_secret`, anyone who reads the source code can forge tokens.

**Instead:** Fail to start if `jwt_secret` is not set. Panic on startup with clear error message.

### Anti-Pattern 4: Encrypting the Entire Database

**What:** Using SQLCipher or full-disk encryption as the primary data protection strategy.

**Why bad:** Protects against disk theft but not against application-level access. If the app can read the DB, so can any process running as the same user. Adds build complexity (SQLCipher requires native compilation).

**Instead:** Application-level field encryption for specific PII columns. Simpler, targeted, and protects against SQL injection or DB file copying.

---

## Scalability Considerations

| Concern | Current (8 pods) | At 16 pods | At 32+ pods (multi-venue) |
|---------|-------------------|------------|---------------------------|
| Auth overhead | Negligible (JWT validation is CPU-cheap) | Negligible | Negligible |
| PSK management | 1 shared secret in config | Same | Per-venue secrets, key rotation needed |
| TLS certificates | 1 self-signed CA, install on 8 browsers | Same CA, 16 browsers | Per-venue CAs or proper PKI |
| PII encryption | AES-256-GCM per field, ~1ms/field | Same | Same, but key management needs centralization |
| CORS origins | 3 origins | Same | Per-venue origin lists |

The recommended architecture scales to 2-3 venues without rework. Beyond that, centralized key management and proper PKI would be needed.

---

## Sources

- Direct codebase analysis of `crates/racecontrol/src/auth/mod.rs` (JWT, PIN, OTP implementation)
- Direct codebase analysis of `crates/racecontrol/src/api/routes.rs` (route structure, no auth middleware)
- Direct codebase analysis of `crates/rc-agent/src/remote_ops.rs` (unauthenticated remote exec)
- Direct codebase analysis of `crates/rc-sentry/src/main.rs` (unauthenticated shell exec)
- Direct codebase analysis of `crates/racecontrol/src/main.rs` (middleware stack, proxy setup)
- `.planning/codebase/CONCERNS.md` (known security debt, hardcoded JWT secret)
- `.planning/codebase/ARCHITECTURE.md` (system structure, sync patterns)
- `racecontrol.toml` (jwt_secret configured, terminal_secret, evolution_api_key)
- Axum 0.8 middleware patterns (HIGH confidence -- `tower` and `axum::middleware` already in dependencies)
- `jsonwebtoken` crate (HIGH confidence -- already in workspace dependencies, actively used)
