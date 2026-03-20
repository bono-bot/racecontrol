# Phase 76: API Authentication & Admin Protection - Research

**Researched:** 2026-03-20
**Domain:** Axum middleware auth enforcement, argon2 PIN hashing, HMAC service auth, rate limiting, session integrity
**Confidence:** HIGH

## Summary

Phase 76 closes the most critical security gap in the Racing Point system: 230 of 269 API routes (85.5%) have zero authentication enforcement. The existing JWT infrastructure (jsonwebtoken 9.3.1, Claims struct, verify_jwt(), extract_driver_id()) is fully built but only used opt-in inside individual customer route handlers. Staff/admin routes (172), service routes (16), and debug routes (6) are completely open. Anyone on the LAN can `curl POST /api/v1/billing/start` to create fake sessions or `curl POST /api/v1/wallet/{id}/topup` to add credits.

The implementation requires three distinct auth mechanisms: (1) JWT middleware layers on grouped Axum sub-routers for customer and staff tiers, (2) argon2 PIN hashing for admin authentication, and (3) HMAC-SHA256 shared secret for rc-agent service-to-service auth on port 8090. The critical deployment constraint is the expand-migrate-contract pattern: the server must accept both authenticated and unauthenticated requests during rollout, clients update pod-by-pod with Pod 8 as canary, and unauthenticated requests are rejected only after confirmed zero unauthenticated traffic.

**Primary recommendation:** Split the monolithic `api_routes()` into 4 grouped sub-routers (public, customer, staff, service) with per-group `axum::middleware::from_fn` auth layers. Add argon2 for admin PIN hashing. Add HMAC validation to rc-agent :8090. Use tower_governor 0.8 for rate limiting. Wrap auth token consumption + billing creation in a single SQLx transaction for SESS-03.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AUTH-01 | JWT middleware on all billing endpoints | Axum from_fn layer on staff sub-router; existing verify_jwt() + Claims reusable |
| AUTH-02 | JWT middleware on all session start/stop endpoints | Same staff sub-router layer covers /auth/assign, /auth/start-now, /billing/start |
| AUTH-03 | Route classification middleware (public/customer/staff/admin tiers) | Split api_routes() into 4 sub-routers with per-group middleware stacks |
| AUTH-04 | Rate limiting on auth endpoints (PIN, OTP, login) | tower_governor 0.8 with per-IP GCRA on /auth/* and /customer/login |
| AUTH-05 | Bot command authorization (wallet balance check before session) | Bot routes already use terminal_secret; add wallet balance pre-check in handler |
| AUTH-06 | Service-to-service HMAC auth for rc-agent | HMAC-SHA256 via ring (already transitive dep); X-Service-Auth header on :8090 |
| ADMIN-01 | PIN/password gate on admin dashboard | POST /auth/admin-login with PIN -> staff JWT (role: "staff"); dashboard sends JWT on all requests |
| ADMIN-02 | Admin PIN hashed with argon2 | argon2 0.5.3 (latest stable); hash stored in racecontrol.toml or DB; RACECONTROL_ADMIN_PIN_HASH env var |
| ADMIN-03 | Session timeout (auto-lock after 15 min inactivity) | JWT exp claim = 12h shift; frontend heartbeat + server-side exp check; idle timeout via frontend timer |
| SESS-01 | Session launch requires valid authenticated request | Staff JWT required on /auth/assign and /billing/start via middleware |
| SESS-02 | Auth tokens single-use and time-bounded | Already implemented: auth_tokens table has status='consuming'/'consumed', expires_at; verify in middleware |
| SESS-03 | DB transaction wrapping token consumption + billing creation | Wrap token UPDATE + billing INSERT in sqlx::Transaction to prevent TOCTOU races |
</phase_requirements>

## Standard Stack

### Core (New Dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `argon2` | 0.5.3 | Admin PIN hashing | RustCrypto pure-Rust Argon2id. OWASP-recommended. Latest stable (0.6 is RC only). |
| `tower_governor` | 0.8.0 | Rate limiting middleware | Tower-native GCRA rate limiter backed by `governor`. Per-IP sliding window. |
| `hex` | 0.4.3 | HMAC signature encoding | Hex encode/decode for HMAC signatures in headers |

### Already Present (No Changes)

| Library | Version | Purpose |
|---------|---------|---------|
| `jsonwebtoken` | 9.3.1 | JWT encode/decode (HS256) |
| `axum` | 0.8.8 | HTTP framework with `middleware::from_fn` |
| `tower` | 0.5.3 | Middleware composition |
| `tower-http` | 0.6 | CORS, trace, sensitive headers |
| `ring` | 0.17.14 | HMAC-SHA256 (transitive via jsonwebtoken -- use directly for service auth) |
| `rand` | 0.8.5 | Random generation (workspace dep) |
| `sqlx` | 0.8 | Database with transaction support |

### Not Adding (Deliberate Omissions)

| Library | Why Not |
|---------|---------|
| `tower-helmet` 0.3 | Phase 77 scope (security headers), not Phase 76 |
| `axum-server` / `rcgen` / `rustls` | Phase 77 scope (HTTPS/TLS) |
| `aes-gcm` | Phase 79 scope (data protection) |
| `axum-jwt-auth` | Unnecessary -- existing jsonwebtoken + from_fn is sufficient |
| Auth0 / Keycloak / NextAuth | Overkill for single-venue PIN-to-JWT flow |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `ring` for HMAC | `hmac` + `sha2` crates | ring is already in the dep tree via jsonwebtoken; adding hmac+sha2 would duplicate crypto. Use ring directly. |
| `tower_governor` | Custom rate limiter | GCRA is non-trivial; governor is battle-tested |
| `argon2` 0.5 | `argon2` 0.6-rc | 0.6 is release candidate only; 0.5.3 is proven stable |
| JWT in httpOnly cookie | JWT in Authorization header | Both work; header is simpler for API calls, cookie is better for browser auto-send. Use header for API, cookie for dashboard session. |

**Installation (add to workspace Cargo.toml):**
```toml
[workspace.dependencies]
argon2 = "0.5"
```

**Installation (add to crates/racecontrol/Cargo.toml):**
```toml
argon2 = { workspace = true }
tower_governor = "0.8"
hex = "0.4"
```

**Installation (add to crates/rc-agent/Cargo.toml):**
```toml
hex = "0.4"
# ring is NOT needed as a direct dep -- use hmac via the approach below
```

Note: For rc-agent HMAC, since rc-agent does not depend on jsonwebtoken (and thus does not have ring), the simplest approach is to add `hmac = "0.12"` and `sha2 = "0.10"` (latest stable, not the 0.13/0.11 RCs) to rc-agent, OR use a simple constant-time comparison of a shared secret header. Given that this is LAN-only traffic behind a private subnet, a PSK header with constant-time comparison is adequate for Phase 76 MVP; HMAC with timestamp can be added in Phase 80 for replay prevention.

## Architecture Patterns

### Recommended Router Structure

```
Router::new()
    .merge(public_routes())      // No auth layer
    .merge(customer_routes())    // from_fn(require_customer_jwt)
    .merge(staff_routes())       // from_fn(require_staff_jwt)
    .merge(service_routes())     // from_fn(require_service_key)
    .merge(debug_routes())       // from_fn(require_staff_jwt) -- admin only
    .route("/ws/agent", ...)     // Service auth in first WS message
    .route("/ws/dashboard", ...) // Staff auth via query param
    .route("/ws/ai", ...)        // Staff auth via query param
    .route("/register", ...)     // Public
    .route("/", ...)             // Public health
    .fallback(kiosk_proxy)       // Public (kiosk proxy)
    .layer(from_fn(jwt_error_to_401))  // Keep existing error reformatter
    .layer(CorsLayer::new()...)        // Keep existing CORS
    .layer(TraceLayer::new_for_http()) // Keep existing trace
```

### Pattern 1: Staff JWT Middleware (from_fn)

**What:** Axum `middleware::from_fn` that validates Bearer JWT with role claim before passing to handler.
**When:** All 172 staff/admin routes + 6 debug routes.

```rust
use axum::{extract::State, middleware::Next, http::StatusCode};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StaffClaims {
    pub sub: String,       // "admin" or staff_id
    pub role: String,      // "staff" or "admin"
    pub exp: usize,
    pub iat: usize,
}

async fn require_staff_jwt(
    State(state): State<Arc<AppState>>,
    mut req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = jsonwebtoken::decode::<StaffClaims>(
        token,
        &DecodingKey::from_secret(state.config.auth.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Inject claims for downstream handlers
    req.extensions_mut().insert(claims.claims);
    Ok(next.run(req).await)
}
```

**Confidence:** HIGH -- this is the documented Axum 0.8 pattern. The project already uses `axum_mw::from_fn(jwt_error_to_401)` in main.rs line 554.

### Pattern 2: Expand-Migrate-Contract for Safe Rollout

**What:** Three-phase deployment pattern that prevents big-bang auth bricking the fleet.
**When:** Every auth enforcement change.

**Phase A (Expand):** Server accepts both authenticated AND unauthenticated requests. Auth middleware logs warnings for unauthenticated requests but does not reject.
```rust
async fn require_staff_jwt_permissive(
    State(state): State<Arc<AppState>>,
    mut req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    match extract_staff_claims(&state, &req) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
        }
        Err(_) => {
            tracing::warn!(
                path = %req.uri().path(),
                "Unauthenticated staff request (permissive mode)"
            );
        }
    }
    next.run(req).await
}
```

**Phase B (Migrate):** Update clients to send auth tokens. Dashboard sends staff JWT. rc-agent sends service key. Deploy pod-by-pod (Pod 8 canary first).

**Phase C (Contract):** After 24h of zero unauthenticated warnings in logs, switch to strict rejection middleware.

### Pattern 3: Admin PIN-to-JWT Flow

**What:** POST /auth/admin-login with PIN -> validate against argon2 hash -> return staff JWT.
**When:** Admin dashboard first access; refreshed every 12 hours.

```rust
// In config.rs AuthConfig:
pub admin_pin_hash: Option<String>,  // argon2 PHC string

// Login endpoint:
async fn admin_login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AdminLoginRequest>,
) -> Result<Json<Value>, StatusCode> {
    let pin_hash = state.config.auth.admin_pin_hash.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let parsed_hash = argon2::PasswordHash::new(pin_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    argon2::Argon2::default()
        .verify_password(body.pin.as_bytes(), &parsed_hash)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let claims = StaffClaims {
        sub: "admin".to_string(),
        role: "staff".to_string(),
        exp: (Utc::now() + Duration::hours(12)).timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
    };

    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.auth.jwt_secret.as_bytes()),
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "token": token, "expires_in": 43200 })))
}
```

### Pattern 4: Service Key Auth for rc-agent :8090

**What:** Shared secret validated via constant-time comparison on rc-agent HTTP endpoints.
**When:** All rc-agent remote_ops routes except /ping and /health.

```rust
// rc-agent side:
async fn require_service_key(
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected = std::env::var("RCAGENT_SERVICE_KEY")
        .unwrap_or_default();
    if expected.is_empty() {
        // No key configured = permissive mode (Phase A)
        return Ok(next.run(req).await);
    }

    let provided = req.headers()
        .get("x-service-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Constant-time comparison to prevent timing attacks
    if ring::constant_time::verify_slices_are_equal(
        expected.as_bytes(),
        provided.as_bytes(),
    ).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}
```

Note: rc-agent does not have ring as a dependency. For constant-time comparison without ring, use `subtle` crate (already in the dep tree via ring's transitive deps) or implement a simple byte-by-byte XOR comparison. Alternatively, since rc-agent needs minimal crypto, add a lightweight approach.

### Pattern 5: Session Integrity via SQLx Transaction (SESS-03)

**What:** Wrap auth token consumption + billing session creation in a single database transaction.
**When:** The moment a customer PIN is validated and billing starts.

```rust
// In auth/mod.rs where token is consumed:
let mut tx = state.db.begin().await
    .map_err(|e| format!("Transaction start failed: {}", e))?;

// Mark token as consuming (optimistic lock)
let rows = sqlx::query(
    "UPDATE auth_tokens SET status = 'consuming' WHERE id = ? AND status = 'pending'"
)
.bind(&token_id)
.execute(&mut *tx)
.await
.map_err(|e| format!("Token consume failed: {}", e))?;

if rows.rows_affected() == 0 {
    tx.rollback().await.ok();
    return Err("Token already consumed or expired".to_string());
}

// Create billing session inside same transaction
let billing_id = create_billing_in_tx(&mut tx, &pod_id, &driver_id, &pricing_tier_id).await?;

// Finalize token
sqlx::query(
    "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?"
)
.bind(&billing_id)
.bind(&token_id)
.execute(&mut *tx)
.await
.map_err(|e| format!("Token finalize failed: {}", e))?;

tx.commit().await
    .map_err(|e| format!("Transaction commit failed: {}", e))?;
```

**Why:** The current code has a TOCTOU window between `UPDATE auth_tokens SET status = 'consumed'` and the billing session creation. Two simultaneous requests could both read status='pending', both update to 'consumed', and create duplicate billing sessions. The 'consuming' intermediate state + transaction prevents this.

### Anti-Patterns to Avoid

- **Auth checks inside route handlers:** The existing `extract_driver_id()` pattern (called at top of each handler) must NOT be replicated for staff auth. Use middleware so new routes get auth automatically.
- **Single auth middleware for all tiers:** Do not use one middleware with a route allowlist. Use separate sub-routers with separate layers -- idiomatic Axum, impossible to forget auth on a new route.
- **Argon2 on every request:** Argon2 is deliberately slow (100-500ms). Hash the PIN once at login, issue a JWT, validate the JWT on subsequent requests. Never re-hash the PIN per request.
- **Storing admin PIN hash in racecontrol.toml committed to git:** Use RACECONTROL_ADMIN_PIN_HASH env var. First-run setup: CLI tool hashes the PIN and prints the hash for the operator to set.
- **WebSocket auth via Bearer header:** WS upgrade requests cannot carry custom headers in all browsers. Auth WS via query parameter (`?token=X`) or first message after connection. rc-agent already sends pod_id in the first WS message -- add HMAC signature to this message.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rate limiting | Custom token bucket | `tower_governor` 0.8 | GCRA is non-trivial; `governor` handles clock drift, bursting, per-key limits |
| Password hashing | SHA-256 + salt | `argon2` 0.5 | Argon2id is memory-hard, prevents GPU brute-force, won PHC competition |
| JWT validation | Custom token parser | `jsonwebtoken` 9.3 (existing) | Already in workspace; handles HS256, exp validation, clock skew |
| Constant-time comparison | `==` on secrets | `ring::constant_time` or `subtle::ConstantTimeEq` | Prevents timing side-channel on secret comparison |
| HMAC-SHA256 | Custom hash construction | `ring::hmac` (already transitive dep) | Correct HMAC construction is subtle; ring is battle-tested |

## Common Pitfalls

### Pitfall 1: Big-Bang Auth Bricks the Fleet
**What goes wrong:** Enabling auth middleware on racecontrol breaks all 8 pods, dashboard, bots, and cloud sync simultaneously.
**Why it happens:** Server and clients deploy independently; atomic updates are impossible.
**How to avoid:** Expand-migrate-contract pattern (see Pattern 2 above). Server accepts both modes first. Deploy clients pod-by-pod. Reject unauthenticated only after 24h clean logs.
**Warning signs:** Plan has "add auth middleware" as a single task; no dual-mode or grace period.

### Pitfall 2: Pod Agent Bypass via localhost:8090
**What goes wrong:** Auth on racecontrol :8080 is useless if rc-agent :8090 still accepts unauthenticated `/exec` commands from any device on LAN (or localhost on the pod itself).
**Why it happens:** Security focus on the "front door" ignores the per-pod backdoor.
**How to avoid:** Add service key auth to rc-agent :8090 in the SAME phase as racecontrol auth. Not a later phase.
**Warning signs:** Phase plan addresses racecontrol auth but defers rc-agent auth.

### Pitfall 3: Auth Latency on Billing Endpoints
**What goes wrong:** JWT validation on every billing poll adds latency. Billing is timing-sensitive (10s idle threshold, per-second charges).
**Why it happens:** Global middleware treats all routes equally.
**How to avoid:** JWT validation is sub-millisecond (HMAC check, no DB lookup). Verified: `jsonwebtoken::decode` with HS256 is ~2 microseconds. No concern at 8 concurrent pods polling every 5s. Do NOT add DB-backed session validation per request.
**Warning signs:** Auth middleware does a database query per request.

### Pitfall 4: Argon2 Blocks the Tokio Runtime
**What goes wrong:** Argon2 is CPU-intensive (100-500ms). Running it on the async runtime blocks other tasks.
**Why it happens:** Called directly in an async handler without spawning to a blocking thread.
**How to avoid:** Use `tokio::task::spawn_blocking` for argon2 verification. Only called on admin login (rare), not per request.

### Pitfall 5: Cloud Sync and Bot Routes Break
**What goes wrong:** Cloud sync (/sync/changes, /sync/push) and bot routes already use terminal_secret in handlers. Moving to middleware-level auth could double-check or conflict.
**Why it happens:** Two auth systems running simultaneously.
**How to avoid:** Service routes that already check terminal_secret should move the check to middleware and remove the in-handler check. Ensure Bono's VPS sends the correct header before the server starts rejecting.
**Warning signs:** Routes return 401 to cloud sync after auth rollout.

### Pitfall 6: WebSocket Auth Breaks Agent Connections
**What goes wrong:** Adding auth to WebSocket upgrade paths blocks rc-agent connections.
**Why it happens:** WS upgrades are HTTP GET requests; agents do not send Authorization headers on the upgrade.
**How to avoid:** Auth for WebSocket connections happens AFTER the upgrade, in the first message. The agent already sends `pod_id` in its first message. Add an HMAC signature field to this message. Server validates before processing further messages.
**Warning signs:** All 8 pods disconnect after auth deployment.

## Code Examples

### Existing JWT Infrastructure (reuse, do not rebuild)

```rust
// auth/mod.rs line 39 -- existing Claims struct
pub struct Claims {
    pub sub: String, // driver_id
    pub exp: usize,
    pub iat: usize,
}

// auth/mod.rs line 968 -- existing verify function
pub fn verify_jwt(token: &str, secret: &str) -> Result<String, String> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("JWT decode error: {}", e))?;
    Ok(data.claims.sub)
}

// routes.rs line 4127 -- existing extraction pattern
fn extract_driver_id(state: &AppState, headers: &HeaderMap) -> Result<String, String> {
    let auth_header = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;
    let token = auth_header.strip_prefix("Bearer ")
        .ok_or_else(|| "Invalid Authorization format".to_string())?;
    auth::verify_jwt(token, &state.config.auth.jwt_secret)
}
```

### New StaffClaims (extend existing Claims)

```rust
// New struct alongside existing Claims:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffClaims {
    pub sub: String,   // "admin" or staff_id
    pub role: String,  // "staff"
    pub exp: usize,
    pub iat: usize,
}

// Create staff JWT:
pub fn create_staff_jwt(secret: &str, staff_id: &str) -> Result<String, String> {
    let claims = StaffClaims {
        sub: staff_id.to_string(),
        role: "staff".to_string(),
        exp: (Utc::now() + Duration::hours(12)).timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}
```

### Argon2 PIN Hashing

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;

// One-time: hash PIN for storage
fn hash_admin_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(pin.as_bytes(), &salt)
        .map_err(|e| format!("Hash error: {}", e))?;
    Ok(hash.to_string())  // PHC string format: $argon2id$v=19$m=...
}

// Per login: verify PIN against stored hash
fn verify_admin_pin(pin: &str, hash_str: &str) -> bool {
    let parsed = match PasswordHash::new(hash_str) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok()
}
```

### tower_governor Rate Limiting

```rust
use tower_governor::{GovernorConfig, GovernorLayer};
use std::time::Duration;

// 5 requests per minute per IP on auth endpoints
let governor_config = GovernorConfig::default()
    .per_second(5)  // Actually means 5 requests per period
    .burst_size(5);

let auth_routes = Router::new()
    .route("/auth/admin-login", post(admin_login))
    .route("/customer/login", post(customer_login))
    .route("/customer/verify-otp", post(verify_otp))
    .layer(GovernorLayer::new(&governor_config));
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tower_governor 0.4 (research assumed) | tower_governor 0.8 | Recent | API may differ; verify GovernorConfig builder |
| argon2 0.5 (latest stable) | argon2 0.6.0-rc.7 (pre-release) | In progress | Stay on 0.5.3 stable; 0.6 RC has breaking changes |
| hmac 0.12 + sha2 0.10 (latest stable) | hmac 0.13-rc / sha2 0.11-rc (pre-release) | In progress | Stay on 0.12/0.10 stable if needed; prefer ring for HMAC |
| rand 0.8 (workspace) | rand 0.9.2 (transitive) | Recent | Workspace still on 0.8; both versions in dep tree; argon2 0.5 uses rand_core which bridges both |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | None (standard Cargo.toml test config) |
| Quick run command | `cargo test -p racecontrol-crate --lib -- auth` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | Staff JWT required on billing endpoints | integration | `cargo test -p racecontrol-crate -- test_billing_requires_auth` | No - Wave 0 |
| AUTH-02 | Staff JWT required on session endpoints | integration | `cargo test -p racecontrol-crate -- test_session_requires_auth` | No - Wave 0 |
| AUTH-03 | Route tier classification works | unit | `cargo test -p racecontrol-crate -- test_route_tiers` | No - Wave 0 |
| AUTH-04 | Rate limiting rejects after threshold | integration | `cargo test -p racecontrol-crate -- test_rate_limit` | No - Wave 0 |
| AUTH-05 | Bot checks wallet before launch | unit | `cargo test -p racecontrol-crate -- test_bot_wallet_check` | No - Wave 0 |
| AUTH-06 | rc-agent rejects without service key | unit | `cargo test -p rc-agent-crate -- test_service_key_required` | No - Wave 0 |
| ADMIN-01 | Admin login returns JWT | unit | `cargo test -p racecontrol-crate -- test_admin_login` | No - Wave 0 |
| ADMIN-02 | PIN hashed with argon2 | unit | `cargo test -p racecontrol-crate -- test_argon2_hash_verify` | No - Wave 0 |
| ADMIN-03 | Expired JWT rejected | unit | `cargo test -p racecontrol-crate -- test_expired_jwt_rejected` | No - Wave 0 |
| SESS-01 | Unauthenticated session launch rejected | integration | `cargo test -p racecontrol-crate -- test_session_no_auth` | No - Wave 0 |
| SESS-02 | Consumed token rejected on reuse | unit | `cargo test -p racecontrol-crate -- test_token_single_use` | No - Wave 0 |
| SESS-03 | TOCTOU race prevented by transaction | unit | `cargo test -p racecontrol-crate -- test_billing_transaction_atomic` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate --lib -- auth`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/auth/middleware.rs` -- new module for auth middleware functions + unit tests
- [ ] `crates/racecontrol/tests/auth_integration.rs` -- integration tests for route auth enforcement
- [ ] `crates/rc-agent/src/remote_ops.rs` -- add service key tests to existing test module
- [ ] argon2 dependency install: `cargo add argon2@0.5 --manifest-path crates/racecontrol/Cargo.toml`
- [ ] tower_governor dependency install: `cargo add tower_governor@0.8 --manifest-path crates/racecontrol/Cargo.toml`

## Open Questions

1. **Dashboard authentication UX**
   - What we know: Staff dashboard is a Next.js app at :3200. It calls racecontrol API.
   - What's unclear: How does the dashboard currently send requests? Does it use fetch with credentials? What UI does the admin PIN prompt look like?
   - Recommendation: Plan should include a dashboard login page task. JWT stored in localStorage (dashboard is staff-only, not customer-facing -- XSS risk is lower).

2. **WebSocket auth for dashboard and AI WS**
   - What we know: `/ws/dashboard` and `/ws/ai` are unauthenticated. Agent WS sends pod_id in first message.
   - What's unclear: Does the dashboard WS need auth? It shows real-time billing and pod status.
   - Recommendation: Yes, auth is needed. Send staff JWT as query param on WS upgrade: `/ws/dashboard?token=eyJ...`. Validate in the upgrade handler.

3. **tower_governor 0.8 API surface**
   - What we know: Version jumped from 0.4 (research assumed) to 0.8.
   - What's unclear: Builder API may have changed significantly.
   - Recommendation: Verify API at implementation time. The core concept (per-IP rate limit layer) is stable.

4. **Kiosk proxy auth bypass**
   - What we know: The kiosk proxy (`.fallback(kiosk_proxy)`) forwards requests to localhost:3300. It is before the CORS layer.
   - What's unclear: Should kiosk-proxied requests bypass staff auth? The kiosk serves the customer PWA.
   - Recommendation: Yes, kiosk proxy stays public. The PWA behind it handles its own customer JWT auth.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis: `auth/mod.rs` (Claims, verify_jwt, PIN validation), `routes.rs` (extract_driver_id, 269 routes), `main.rs` (middleware stack, router construction), `config.rs` (AuthConfig, resolve_jwt_secret)
- Phase 75 SECURITY-AUDIT.md: Complete endpoint inventory with auth status for all 269+11+1 routes
- `cargo tree` output: Verified jsonwebtoken 9.3.1, ring 0.17.14, axum 0.8.8, rand 0.8.5/0.9.2 in dep tree
- `cargo search`: argon2 0.5.3 (stable), tower_governor 0.8.0, hex 0.4.3

### Secondary (MEDIUM confidence)
- ARCHITECTURE.md: Route grouping pattern, middleware ordering, PSK design
- PITFALLS.md: Expand-migrate-contract pattern, pod agent bypass, auth latency
- STACK.md: Technology recommendations (verified versions differ from originals)

### Tertiary (LOW confidence)
- tower_governor 0.8 API: Major version jump from research's assumed 0.4; builder API needs verification at implementation time

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All crate versions verified via cargo search/cargo tree
- Architecture: HIGH - Based on direct codebase analysis of existing auth infra, router structure, and middleware stack
- Pitfalls: HIGH - Operational pitfalls specific to Racing Point fleet topology, verified against SECURITY-AUDIT.md

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (30 days -- stable domain, no fast-moving external deps)
