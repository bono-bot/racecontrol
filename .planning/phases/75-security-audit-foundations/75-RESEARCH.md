# Phase 75: Security Audit & Foundations - Research

**Researched:** 2026-03-20
**Domain:** Security audit of Rust/Axum eSports cafe operations system (endpoint inventory, PII trace, secrets migration)
**Confidence:** HIGH

## Summary

Phase 75 is a discovery and foundations phase -- no new features, no new crates, no middleware changes. The deliverables are documentation (endpoint inventory, PII map, CORS/auth state document) and a targeted config migration (secrets from racecontrol.toml to environment variables with auto-generation of JWT key).

The codebase investigation reveals 155+ route definitions in a single `api_routes()` function in `routes.rs` (13,776 lines). Zero routes have auth middleware enforcement -- the only existing middleware is `jwt_error_to_401` which post-processes responses but does NOT require authentication. Customer routes use `extract_driver_id()` inside handlers (28+ call sites) which validates JWT but only when a token is present; missing tokens simply fail the handler, not the middleware layer. Staff/admin/billing/pod routes have no auth whatsoever.

PII is spread across 5 distinct locations: SQLite `drivers` table (name, email, phone, guardian_name, guardian_phone, dob, signature_data), `staff_members` table (name, phone), application logs (phone numbers logged in OTP flow at INFO level including the OTP code itself), WhatsApp message payloads (phone numbers in billing receipts and OTP messages), and cloud sync payloads (full drivers table including all PII fields synced to Bono's VPS via /sync/changes). The cloud sync endpoint explicitly comments "exposes customer PII" in the code.

Secrets currently in racecontrol.toml: `auth.jwt_secret` (with hardcoded default "racingpoint-jwt-change-me-in-production"), `cloud.terminal_secret`, `bono.relay_secret`, `auth.evolution_api_key`, and `gmail.client_secret`/`gmail.refresh_token`. The JWT secret has a default fallback function that returns a known string -- any attacker who reads the source code can forge tokens if the config file omits the key.

**Primary recommendation:** This phase produces 3 audit documents + 1 code change (env var migration for secrets + JWT key auto-generation). No auth middleware, no new crates, no route restructuring -- that is Phase 76.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AUDIT-01 | Complete inventory of all exposed API endpoints with auth status | 155+ routes enumerated from routes.rs; rc-agent remote_ops has 11 routes on :8090; rc-sentry on :8091 has raw TCP handler. All currently unauthenticated. Classification into public/customer/staff/admin/service tiers documented below. |
| AUDIT-02 | PII data location audit | 5 PII locations identified: SQLite drivers table (7 PII columns), staff_members table (2 PII columns), application logs (phone+OTP at INFO level), WhatsApp payloads, cloud sync /sync/changes (full driver records). |
| AUDIT-03 | Move JWT signing key and all secrets to environment variables | 5 secrets identified in config.rs: jwt_secret, terminal_secret, relay_secret, evolution_api_key, gmail credentials. Config already has env var override pattern for anthropic_api_key (config.rs:425). |
| AUDIT-04 | Generate cryptographically random JWT key on first run if not set | Current default_jwt_secret() returns hardcoded string. rand crate already in dependencies. Replace with env var check -> generate 256-bit random key -> persist to env or warn. |
| AUDIT-05 | Document current CORS, HTTPS, and auth state | CORS: predicate-based AllowOrigin matching 192.168.31.*, localhost, racingpoint.cloud; allow_headers: Any; allow_credentials: false. HTTPS: none (all HTTP). Auth: jwt_error_to_401 middleware exists but does not enforce auth. |
</phase_requirements>

## Standard Stack

### Core (Already in Dependencies -- No New Crates Needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rand` | (in Cargo.toml) | Generate cryptographic JWT key | Already used for OTP generation in auth/mod.rs |
| `jsonwebtoken` | (in Cargo.toml) | JWT encode/decode | Already used throughout auth module |
| `serde`/`serde_json` | (in Cargo.toml) | Config deserialization | Already used for config.rs |

### No New Dependencies

This is an audit phase. The only code change is modifying `config.rs` to:
1. Read secrets from environment variables with fallback to config file
2. Generate a random JWT key if neither env var nor config value is set
3. Remove the hardcoded `default_jwt_secret()` function

**No new crates needed.** The `rand` crate is already a dependency.

## Architecture Patterns

### Pattern 1: Endpoint Inventory Structure

The inventory document should classify every route into one of 5 tiers based on who should be allowed to call it:

| Tier | Description | Current Auth | Example Routes |
|------|-------------|-------------|----------------|
| **Public** | No auth needed, read-only public data | None (correct) | `/health`, `/venue`, `/public/*`, `/customer/login`, `/customer/register`, `/customer/verify-otp` |
| **Customer** | Authenticated customer via JWT | `extract_driver_id()` in handler (inconsistent) | `/customer/profile`, `/customer/sessions`, `/customer/wallet`, `/customer/friends/*` |
| **Staff/Admin** | Staff PIN or admin credential required | None (CRITICAL gap) | `/billing/*`, `/pods/*`, `/drivers/*`, `/wallet/*/topup`, `/games/*`, `/kiosk/*`, `/config/*`, `/deploy/*`, `/terminal/*`, `/staff/*`, `/employee/*` |
| **Service** | Machine-to-machine, shared secret | `terminal_secret` header check in some handlers | `/sync/*`, `/actions/*`, `/bot/*`, `/ws` (WebSocket) |
| **Debug** | Should be admin-only or disabled in production | None | `/debug/*`, `/logs`, `/ai/diagnose` |

### Pattern 2: Secrets Migration via Environment Variables

Follow the existing pattern from config.rs line 425:

```rust
// Existing pattern (anthropic_api_key):
if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
    tracing::info!("Overriding anthropic_api_key from ANTHROPIC_API_KEY env var");
    self.ai_debugger.anthropic_api_key = Some(key);
}
```

Apply this to all secrets:

| Secret | Config Path | Env Var Name | Current Default |
|--------|-------------|-------------|-----------------|
| JWT signing key | `auth.jwt_secret` | `RACECONTROL_JWT_SECRET` | `"racingpoint-jwt-change-me-in-production"` (DANGEROUS) |
| Terminal secret | `cloud.terminal_secret` | `RACECONTROL_TERMINAL_SECRET` | `None` |
| Relay secret | `bono.relay_secret` | `RACECONTROL_RELAY_SECRET` | `None` |
| Evolution API key | `auth.evolution_api_key` | `RACECONTROL_EVOLUTION_API_KEY` | `None` |
| Gmail client_secret | `gmail.client_secret` | `RACECONTROL_GMAIL_CLIENT_SECRET` | `None` |
| Gmail refresh_token | `gmail.refresh_token` | `RACECONTROL_GMAIL_REFRESH_TOKEN` | `None` |

### Pattern 3: JWT Key Auto-Generation

```rust
// Replace default_jwt_secret() with:
fn resolve_jwt_secret(config_value: &str) -> String {
    // 1. Environment variable takes priority
    if let Ok(key) = std::env::var("RACECONTROL_JWT_SECRET") {
        if !key.is_empty() {
            return key;
        }
    }
    // 2. Config file value (if not the dangerous default)
    if config_value != "racingpoint-jwt-change-me-in-production" && !config_value.is_empty() {
        return config_value.to_string();
    }
    // 3. Generate random 256-bit key
    use rand::Rng;
    let key: [u8; 32] = rand::rng().random();
    let hex_key = hex::encode(key);
    tracing::warn!(
        "Generated random JWT secret (tokens will be invalidated on restart). \
         Set RACECONTROL_JWT_SECRET env var for persistence."
    );
    hex_key
}
```

### Anti-Patterns to Avoid

- **Do NOT add auth middleware in this phase.** Phase 75 is audit-only. Adding middleware risks breaking the live system.
- **Do NOT restructure routes.rs.** Route grouping happens in Phase 76.
- **Do NOT encrypt PII columns.** Data protection is Phase 79.
- **Do NOT remove the config file secret fields.** Env vars override config, but config remains as fallback for backward compatibility.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Random key generation | Custom entropy collection | `rand::rng().random::<[u8; 32]>()` | rand uses OS entropy (getrandom), cryptographically secure |
| Hex encoding of key | Manual byte-to-hex | `hex::encode()` | hex crate may already be a transitive dep; if not, use format!("{:02x}") loop |

## Common Pitfalls

### Pitfall 1: Missing the OTP Log Leak
**What goes wrong:** Audit finds PII in the database but misses that phone numbers AND OTP codes are logged at INFO level in auth/mod.rs lines 1063, 1066, 1070.
**Why it happens:** Logs are not a "storage location" in the traditional sense.
**How to avoid:** The PII audit MUST grep for phone/email/name in tracing::info/warn/error calls, not just SQL schema.
**Warning signs:** `tracing::warn!("Evolution API returned {}: OTP for {} is {}", resp.status(), phone, otp_str)` -- this logs both the phone number and the OTP code.

### Pitfall 2: Forgetting rc-agent and rc-sentry Endpoints
**What goes wrong:** Audit only inventories racecontrol :8080 routes, missing the 11 rc-agent routes on :8090 and rc-sentry on :8091.
**Why it happens:** They are in different crates.
**How to avoid:** Endpoint inventory MUST cover all 3 binaries: racecontrol (155+ routes on :8080), rc-agent (11 routes on :8090), rc-sentry (1 TCP handler on :8091).

### Pitfall 3: JWT Key Generation Without Persistence Warning
**What goes wrong:** Auto-generated key works on first run, but server restart generates a new key, invalidating all active customer JWTs and causing silent auth failures.
**Why it happens:** Random key is ephemeral if not persisted.
**How to avoid:** Log a WARN-level message on every startup if the key is auto-generated. The warning must include the env var name to set.

### Pitfall 4: Breaking start-racecontrol.bat
**What goes wrong:** Env vars added to code but not to the startup batch file on the server.
**Why it happens:** The server starts via `start-racecontrol.bat` registered in HKLM Run key. Env vars set in a terminal session are not available to services started by the Run key.
**How to avoid:** Env vars must be set as system-level environment variables (via `setx /M` or System Properties), not session-level. The bat file should NOT contain secrets in plaintext -- use system env vars that the bat file inherits.

### Pitfall 5: Cloud Sync PII Exposure Not Documented
**What goes wrong:** PII audit documents local SQLite but misses that /sync/changes sends full driver records (name, email, phone, pin_hash) to the cloud VPS.
**Why it happens:** Cloud sync is in a different code path.
**How to avoid:** The sync_changes handler at routes.rs:6886-6895 explicitly sends name, email, phone, pin_hash via JSON. This must be documented as a PII transit path.

## Code Examples

### Complete Drivers Table PII Columns (from db/mod.rs)

```sql
-- Base table (line 31)
CREATE TABLE drivers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,        -- PII
    email TEXT,                -- PII
    phone TEXT,                -- PII
    ...
);

-- Migration additions:
ALTER TABLE drivers ADD COLUMN dob TEXT;              -- PII
ALTER TABLE drivers ADD COLUMN guardian_name TEXT;     -- PII (minor's guardian)
ALTER TABLE drivers ADD COLUMN guardian_phone TEXT;    -- PII (minor's guardian)
ALTER TABLE drivers ADD COLUMN signature_data TEXT;    -- PII (waiver signature)
ALTER TABLE drivers ADD COLUMN nickname TEXT;          -- Potentially PII
ALTER TABLE drivers ADD COLUMN otp_code TEXT;          -- Sensitive (temporary)
ALTER TABLE drivers ADD COLUMN pin_hash TEXT;          -- Sensitive (hashed PIN)

-- Staff table (line 1419):
CREATE TABLE staff_members (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,         -- PII
    phone TEXT NOT NULL UNIQUE, -- PII
    ...
);
```

### Log Lines That Leak PII (from auth/mod.rs)

```rust
// Line 1060: Logs phone number at INFO level
tracing::info!("OTP sent via WhatsApp to {}", wa_phone);

// Line 1063: Logs phone AND OTP code at WARN level (!)
tracing::warn!("Evolution API returned {}: OTP for {} is {}", resp.status(), phone, otp_str);

// Line 1066: Logs phone AND OTP code at WARN level
tracing::warn!("Failed to send OTP via WhatsApp: {}. OTP for {} is {}", e, phone, otp_str);

// Line 1070: Logs phone AND OTP code at INFO level (fallback path)
tracing::info!("OTP for phone {}: {} (Evolution API not configured)", phone, otp_str);
```

```rust
// billing.rs line 2263: Logs phone in receipt confirmation
tracing::info!("WhatsApp receipt sent to {} for session {}", wa_phone, session_id);

// billing.rs line 2266: Logs phone on receipt failure
tracing::warn!("Evolution API returned {} for receipt to {}", resp.status(), wa_phone);
```

### Current CORS Configuration (from main.rs lines 556-567)

```rust
CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
        let origin = origin.to_str().unwrap_or("");
        origin.starts_with("http://localhost:")
            || origin.starts_with("http://127.0.0.1:")
            || origin.starts_with("http://192.168.31.")  // entire LAN subnet
            || origin.starts_with("http://kiosk.rp")
            || origin.contains("racingpoint.cloud")
    }))
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
    .allow_headers(tower_http::cors::Any)  // NOTE: allows any header
    .allow_credentials(false)
```

**Issues to document:**
1. `allow_headers(Any)` -- should be restricted to known headers (Content-Type, Authorization, X-Terminal-Secret)
2. `http://192.168.31.*` -- allows any device on the LAN subnet (intentional for now, but document it)
3. No HTTPS origins -- all HTTP, no `https://` patterns except racingpoint.cloud
4. `allow_credentials(false)` -- means cookies won't be sent cross-origin (relevant for future admin JWT cookies)

### Current JWT Infrastructure (from auth/mod.rs)

```rust
// Claims struct (line 38-43) -- customer-only, no role field
pub struct Claims {
    pub sub: String,  // driver_id
    pub exp: usize,
    pub iat: usize,
    // NOTE: no "role" claim -- cannot distinguish customer vs staff vs admin
}

// verify_jwt (line 968) -- validates signature + expiry, returns driver_id
pub fn verify_jwt(token: &str, secret: &str) -> Result<String, String> { ... }

// extract_driver_id (routes.rs:4127) -- called inside 28+ handler functions
// NOT middleware -- each handler must remember to call this
fn extract_driver_id(state: &AppState, headers: &HeaderMap) -> Result<String, String> {
    let token = headers.get("authorization")...strip_prefix("Bearer ")...;
    auth::verify_jwt(token, &state.config.auth.jwt_secret)
}
```

### Existing Env Var Override Pattern (from config.rs line 425)

```rust
// This pattern already exists for one key -- extend to all secrets
if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
    tracing::info!("Overriding anthropic_api_key from ANTHROPIC_API_KEY env var");
    self.ai_debugger.anthropic_api_key = Some(key);
}
```

## Complete Route Inventory (Pre-Classified)

### racecontrol :8080 (155+ routes)

**Public (no auth needed):**
- `GET /health`, `GET /venue`, `GET /api/v1/fleet/health`
- `GET /public/*` (14 routes: leaderboard, drivers, events, championships, etc.)
- `POST /customer/login`, `POST /customer/verify-otp`, `POST /customer/register`
- `GET /wallet/bonus-tiers`

**Customer (JWT required):**
- `GET/PUT /customer/profile`, `GET /customer/sessions`, `GET /customer/sessions/{id}`
- `GET /customer/laps`, `GET /customer/stats`, `GET /customer/wallet`
- `GET /customer/wallet/transactions`, `GET /customer/experiences`
- `POST /customer/book`, `GET /customer/active-reservation`
- `POST /customer/end-reservation`, `POST /customer/continue-session`
- `GET/POST /customer/friends/*` (7 routes)
- `POST /customer/book-multiplayer`, `GET /customer/group-session`
- `POST /customer/group-session/{id}/accept`, `POST /customer/group-session/{id}/decline`
- `GET /customer/telemetry`, `GET /customer/tournaments`
- `POST /customer/tournaments/{id}/register`, `GET /customer/compare-laps`
- `GET /customer/sessions/{id}/share`, `GET /customer/referral-code`
- `POST /customer/generate-referral-code`, `POST /customer/redeem-referral`
- `POST /customer/apply-coupon`, `GET /customer/packages`
- `GET /customer/membership`, `POST /customer/membership/subscribe`
- `GET /customer/ac/catalog`, `POST /customer/ai/chat`
- `GET /customer/multiplayer-results/{id}`, `PUT /customer/presence`
- `GET /customer/waiver-status`
- **Total: ~40 routes, currently 28+ use extract_driver_id() in handler**

**Staff/Admin (requires staff credential):**
- `GET/POST /pods`, `POST /pods/{id}/wake|shutdown|lockdown|enable|disable|screen|unrestrict|restart`
- `POST /pods/wake-all|shutdown-all|restart-all|lockdown-all`, `POST /pods/{id}/exec`
- `GET /pods/{id}/self-test`, `GET /pod-status-summary`, `POST /pods/seed`
- `GET/POST /drivers`, `GET /drivers/{id}`, `GET /drivers/{id}/full-profile`
- `GET/POST /sessions`, `GET /sessions/{id}`, `GET /laps`, `GET /sessions/{id}/laps`
- `GET /leaderboard/{track}`
- `GET/POST /events`, `GET/POST /bookings`
- `GET/POST /pricing`, `PUT/DELETE /pricing/{id}`
- `GET/POST /billing/rates`, `PUT/DELETE /billing/rates/{id}`
- `POST /billing/start`, `GET /billing/active|sessions`, `GET /billing/sessions/{id}`
- `GET /billing/sessions/{id}/events|summary`, `POST /billing/{id}/stop|pause|resume|extend|refund`
- `GET /billing/{id}/refunds`, `GET /billing/report/daily`
- `GET /billing/split-options/{min}`, `POST /billing/continue-split`
- `POST /games/launch|stop`, `POST /games/relaunch/{pod_id}`, `GET /games/active|history|pod/{pod_id}`
- `POST /pods/{pod_id}/transmission|ffb|assists`, `GET /pods/{pod_id}/assist-state`
- `GET/POST /ac/presets`, `GET/PUT/DELETE /ac/presets/{id}`
- `POST /ac/session/start|stop|retry-pod|update-config`, `GET /ac/session/active|sessions`
- `POST /ac/session/{id}/continuous`, `GET /ac/sessions/{id}/leaderboard`
- `GET /ac/content/tracks|cars`
- `POST /auth/assign`, `POST /auth/cancel/{id}`, `GET /auth/pending`, `GET /auth/pending/{pod_id}`
- `POST /auth/start-now`, `POST /auth/validate-pin`, `POST /auth/kiosk/validate-pin`
- `POST /auth/validate-qr`
- `GET /wallet/{id}`, `POST /wallet/{id}/topup|debit|refund`, `GET /wallet/{id}/transactions`
- `GET /wallet/transactions`
- `GET/POST /waivers`, `GET /waivers/check`, `GET /waivers/{id}/signature`
- `GET/POST/PUT/DELETE /kiosk/experiences/*`, `GET/PUT /kiosk/settings`
- `POST /kiosk/pod-launch-experience`, `POST /kiosk/book-multiplayer`
- `GET/POST/DELETE /config/kiosk-allowlist/*`
- `GET/POST /pos/lockdown`
- `POST /ai/chat`, `POST /ai/diagnose`
- `GET /ai/suggestions`, `POST /ai/suggestions/{id}/dismiss`
- `GET /ai/training/stats|pairs`, `POST /ai/training/import`
- `GET /ops/stats`
- `GET /deploy/status`, `POST /deploy/rolling`, `POST /deploy/{pod_id}`
- `POST /staff/validate-pin`, `GET/POST /staff`
- `GET /employee/daily-pin`, `POST /employee/debug-unlock`
- `GET/POST/PUT /pricing/rules/*`, `GET/POST/PUT/DELETE /coupons/*`
- `GET /review-nudges/pending`, `POST /review-nudges/{id}/sent`
- `GET/POST/PUT/DELETE /time-trials/*`
- `GET/POST/PUT /tournaments/*`, `POST /tournaments/{id}/generate-bracket`
- `POST /tournaments/{id}/matches/{id}/result`
- `PUT /scheduler/settings`, `GET /scheduler/status|analytics`
- `GET /accounting/*` (5 routes), `GET /audit-log`
- `POST /pods/{pod_id}/watchdog-crash`
- `GET/POST/PUT /staff/events/*`, `GET/POST /staff/championships/*`
- `POST /staff/events/{id}/link-session`, `POST /staff/group-sessions/{id}/complete`
- `GET /activity`, `GET /pods/{pod_id}/activity`
- **Total: ~100+ routes, currently zero auth**

**Service (machine-to-machine):**
- `GET /sync/changes`, `POST /sync/push`, `GET /sync/health` -- terminal_secret check in handler
- `POST /actions`, `GET /actions/pending`, `POST /actions/process`, `POST /actions/{id}/ack`, `GET /actions/history`
- `POST /terminal/auth`, `GET/POST /terminal/commands`, `GET /terminal/commands/pending`
- `POST /terminal/commands/{id}/result`, `POST /terminal/book-multiplayer`, `GET /terminal/group-sessions`
- `GET /bot/*` (8 routes) -- terminal_secret check in handler
- `GET /logs`
- **Total: ~20 routes, partial terminal_secret auth**

**Debug (should be admin-only or disabled):**
- `GET /debug/activity|playbooks`, `GET/POST /debug/incidents`, `PUT /debug/incidents/{id}`
- `POST /debug/diagnose`
- **Total: 5 routes, zero auth**

### rc-agent :8090 (11 routes, zero auth)

- `GET /ping`, `GET /health`, `GET /info`
- `GET /files`, `GET /file`, `POST /exec` (CRITICAL -- arbitrary command execution)
- `POST /mkdir`, `POST /write` (CRITICAL -- arbitrary file write)
- `GET /screenshot`, `GET /cursor`, `POST /input`

### rc-sentry :8091 (1 TCP handler, zero auth)

- Raw TCP: accepts command strings, executes shell commands, returns output
- Comment in source: "binds to 0.0.0.0 on a private subnet with no auth"

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUDIT-01 | Endpoint inventory document is complete and accurate | manual-only | N/A -- document review | N/A |
| AUDIT-02 | PII locations document is complete | manual-only | N/A -- document review | N/A |
| AUDIT-03 | Secrets load from env vars, config file fallback works | unit | `cargo test -p racecontrol -- config::tests --test-threads=1` | Partially (config.rs has tests) |
| AUDIT-04 | Random JWT key generated when no env var or config value set | unit | `cargo test -p racecontrol -- config::tests --test-threads=1` | Wave 0 |
| AUDIT-05 | CORS/HTTPS/auth state document is complete | manual-only | N/A -- document review | N/A |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol -- config --test-threads=1`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/config.rs` -- add test for env var override of jwt_secret
- [ ] `crates/racecontrol/src/config.rs` -- add test for random JWT key generation when no secret configured

## Open Questions

1. **Is the current JWT secret on the production server the default or a custom value?**
   - What we know: Code warns if default is used (main.rs:344). We cannot check the actual racecontrol.toml on the server from this machine.
   - What's unclear: Whether Uday or a previous setup changed the default.
   - Recommendation: The migration should handle both cases. If the current value is the default, generate a new one. If custom, preserve it via env var.

2. **Should hex crate be added for key encoding?**
   - What we know: hex may be a transitive dependency already.
   - What's unclear: Whether it is directly available.
   - Recommendation: Check `cargo tree | grep hex` during implementation. If not available, use `format!("{:02x}")` loop instead of adding a new dependency.

3. **Where should the generated JWT key be persisted?**
   - What we know: The server starts via bat file from HKLM Run key. System env vars persist across reboots.
   - What's unclear: Whether Uday has admin access to set system env vars on the server.
   - Recommendation: On first run with auto-generated key, log the key value at WARN level with instructions to set it as a system env var. Do NOT write it to the config file programmatically (config file is not writable by the running process necessarily).

## Sources

### Primary (HIGH confidence)
- Direct codebase audit: `crates/racecontrol/src/api/routes.rs` -- all 155+ route definitions (lines 31-320)
- Direct codebase audit: `crates/racecontrol/src/config.rs` -- all secret fields, default_jwt_secret(), env var override pattern
- Direct codebase audit: `crates/racecontrol/src/auth/mod.rs` -- JWT Claims struct, verify_jwt, extract_driver_id, OTP logging
- Direct codebase audit: `crates/racecontrol/src/main.rs` -- CORS config, jwt_error_to_401 middleware, JWT default warning
- Direct codebase audit: `crates/racecontrol/src/db/mod.rs` -- all CREATE TABLE and ALTER TABLE statements for PII columns
- Direct codebase audit: `crates/rc-agent/src/remote_ops.rs` -- 11 routes, zero auth, arbitrary exec/write
- Direct codebase audit: `crates/rc-sentry/src/main.rs` -- TCP handler, zero auth, raw shell exec
- Direct codebase audit: `crates/racecontrol/src/billing.rs` -- phone number logging in WhatsApp receipt flow
- Direct codebase audit: `crates/racecontrol/src/cloud_sync.rs` -- terminal_secret usage for cloud auth

### Secondary (MEDIUM confidence)
- `.planning/research/ARCHITECTURE.md` -- security architecture patterns, auth tier model
- `.planning/research/FEATURES.md` -- feature landscape, dependency graph, current state assessment
- `.planning/research/SUMMARY.md` -- phased rollout plan, stack recommendations

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new crates needed, all code changes in existing config.rs
- Architecture: HIGH -- patterns directly observed in codebase (env var override, JWT infrastructure)
- Pitfalls: HIGH -- specific code locations identified (log lines, sync payloads, bat file dependency)

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable -- audit of existing code, no external dependency changes)
