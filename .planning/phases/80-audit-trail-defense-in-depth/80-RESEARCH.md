# Phase 80: Audit Trail & Defense in Depth - Research

**Researched:** 2026-03-21 IST
**Domain:** Rust/Axum security hardening -- audit logging, WhatsApp alerting, PIN rotation, HMAC sync signing
**Confidence:** HIGH

## Summary

Phase 80 adds defense-in-depth layers to the existing racecontrol security stack: an append-only admin action audit trail (ADMIN-04), WhatsApp alerts on admin login and sensitive actions (ADMIN-05), staff PIN rotation alerting (ADMIN-06), and HMAC-SHA256 request signing for cloud sync payloads (AUTH-07).

The existing codebase already has all the building blocks: an `audit_log` table exists with schema for config-change tracking, `whatsapp_alerter.rs` has a working `send_whatsapp()` function and `send_security_alert()` pattern, `hmac 0.12` and `sha2 0.10` are already workspace dependencies used in `crypto/encryption.rs`, and `admin_login()` in `auth/admin.rs` is the entry point for admin authentication. The work is primarily integration -- wiring existing infrastructure into the right handler call sites.

**Primary recommendation:** Extend the existing `audit_log` table with an `action_type` column for admin-action classification (not just CRUD), add `log_admin_action()` to `accounting.rs`, wire it into the 6 handler categories (wallet topup, pricing CRUD, fleet exec, terminal commands, billing overrides, admin login), then make `send_whatsapp` pub(crate) and call it from the high-sensitivity subset.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ADMIN-04 | Admin action audit trail -- log all wallet topups, pricing changes, session overrides, fleet exec, terminal commands to append-only audit_log table | Existing `audit_log` table + `log_audit()` in `accounting.rs` -- extend schema, wire into 6 handler categories |
| ADMIN-05 | WhatsApp alert on admin login and sensitive actions (wallet topup, fleet exec) | Existing `send_whatsapp()` in `whatsapp_alerter.rs` -- make pub(crate), call from admin_login + topup + exec handlers |
| ADMIN-06 | Staff PIN rotation -- alert if admin PIN unchanged for >30 days | No `pin_changed_at` tracking exists -- add to DB or config, check on startup + daily tick |
| AUTH-07 | Cloud sync request signing -- HMAC-SHA256 on sync payloads with timestamp + nonce for replay prevention | `hmac 0.12` + `sha2 0.10` already in workspace, `Hmac<Sha256>` pattern in `crypto/encryption.rs` -- add signing to `push_to_cloud()` and `sync_once_http()`, verification to `/sync/push` and `/sync/changes` handlers |
</phase_requirements>

## Standard Stack

### Core (Already in Dependency Tree)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| `hmac` | 0.12 | HMAC-SHA256 for sync payload signing | Workspace dep, used in `crypto/encryption.rs` |
| `sha2` | 0.10 | SHA-256 digest for HMAC | Workspace dep, used in `crypto/encryption.rs` |
| `sqlx` | (existing) | SQLite audit_log table operations | Already used throughout |
| `chrono` + `chrono-tz` | (existing) | IST timestamps for audit records | Already used in `whatsapp_alerter.rs` |
| `uuid` | (existing) | Audit log record IDs | Already used in `accounting.rs` |
| `serde_json` | (existing) | Serializing action payloads for audit | Already used throughout |
| `hex` | 0.4 | Encoding HMAC signatures | Workspace dep |

### No New Dependencies Required

Zero new crates. Every capability needed is already compiled into racecontrol.

## Architecture Patterns

### Existing Code to Extend (Not Rewrite)

```
crates/racecontrol/src/
  accounting.rs        # log_audit() + snapshot_row() -- EXTEND with log_admin_action()
  whatsapp_alerter.rs  # send_whatsapp() (private) + send_security_alert() (pub) -- make send_whatsapp pub(crate)
  auth/admin.rs        # admin_login() -- add audit log + WA alert on success
  cloud_sync.rs        # push_to_cloud(), sync_once_http() -- add HMAC signing
  api/routes.rs        # topup_wallet(), ws_exec_pod(), pricing CRUD, terminal_submit() -- add audit calls
  config.rs            # Config struct -- add cloud.sync_hmac_key, auth.pin_changed_at
  db/mod.rs            # audit_log table DDL -- add migration for action_type column
  crypto/encryption.rs # Hmac<Sha256> pattern -- reference for sync signing
```

### Pattern 1: Admin Action Audit Log

**What:** Every admin action gets an audit record with actor, action type, payload, IP, and timestamp.

**Current audit_log schema:**
```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    table_name TEXT NOT NULL,
    row_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('create', 'update', 'delete')),
    old_values TEXT,
    new_values TEXT,
    staff_id TEXT,
    ip_address TEXT,
    created_at TEXT DEFAULT (datetime('now'))
)
```

**Problem:** The current schema is designed for config-table CRUD tracking (table_name, row_id, action=create/update/delete). Admin actions like "fleet exec on pod 3" or "admin login" don't fit this model -- there's no table_name or row_id.

**Solution:** Add an `action_type` column for admin-action classification. Use `table_name = 'admin_actions'` as a sentinel for non-CRUD audit entries, with `action_type` providing the category:

```sql
-- Migration: add action_type column
ALTER TABLE audit_log ADD COLUMN action_type TEXT;
CREATE INDEX IF NOT EXISTS idx_audit_log_action_type ON audit_log(action_type);
```

**Action types to track:**
| action_type | Handler | Sensitivity |
|-------------|---------|-------------|
| `admin_login` | `auth/admin.rs::admin_login()` | HIGH -- WA alert |
| `wallet_topup` | `api/routes.rs::topup_wallet()` | HIGH -- WA alert |
| `fleet_exec` | `api/routes.rs::ws_exec_pod()` | HIGH -- WA alert |
| `terminal_command` | `api/routes.rs::terminal_submit()` | MEDIUM |
| `pricing_create` | `api/routes.rs::create_pricing_tier()` | MEDIUM |
| `pricing_update` | `api/routes.rs::update_pricing_tier()` | MEDIUM |
| `pricing_delete` | `api/routes.rs::delete_pricing_tier()` | MEDIUM |
| `pricing_rule_create` | `api/routes.rs::create_pricing_rule()` | MEDIUM |
| `pricing_rule_update` | `api/routes.rs::update_pricing_rule()` | MEDIUM |
| `pricing_rule_delete` | `api/routes.rs::delete_pricing_rule()` | MEDIUM |
| `billing_start` | `api/routes.rs::start_billing()` | LOW |
| `billing_stop` | `api/routes.rs` billing stop | LOW |
| `billing_refund` | `api/routes.rs` billing refund | MEDIUM |
| `billing_override` | `api/routes.rs` billing pause/resume/extend | MEDIUM |
| `session_override` | `api/routes.rs` session stop/force-end | MEDIUM |

**New helper function:**
```rust
// In accounting.rs
pub async fn log_admin_action(
    state: &Arc<AppState>,
    action_type: &str,
    details: &str,       // JSON payload of what happened
    staff_id: Option<&str>,
    ip_address: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, action_type, new_values, staff_id, ip_address)
         VALUES (?, 'admin_actions', ?, 'create', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&id) // row_id = self-referential for admin actions
    .bind(action_type)
    .bind(details)
    .bind(staff_id)
    .bind(ip_address)
    .execute(&state.db)
    .await;
}
```

**Append-only enforcement:** SQLite does not support row-level security. Append-only is enforced by convention:
1. No `DELETE FROM audit_log` or `UPDATE audit_log` in any handler
2. The `query_audit_log()` endpoint is GET-only (already the case)
3. A simple grep verification: `grep -r "DELETE.*audit_log\|UPDATE.*audit_log" crates/` should return zero results

### Pattern 2: WhatsApp Alert on Admin Actions

**What:** `send_whatsapp()` in `whatsapp_alerter.rs` is currently private. Make it `pub(crate)` and call from high-sensitivity handlers.

**Current state:**
- `send_whatsapp()` is `async fn` (private) -- takes `&Config` and `&str` message
- `send_security_alert()` is `pub(crate)` -- adds per-pod debounce, calls `send_whatsapp()`
- Evolution API URL, key, instance, and uday_phone are all in `Config`

**Admin alert pattern:**
```rust
// New function in whatsapp_alerter.rs
pub(crate) async fn send_admin_alert(config: &Config, action: &str, details: &str) {
    let msg = format!(
        "[ADMIN] {} -- {} | {}",
        action, details, ist_now_string()
    );
    send_whatsapp(config, &msg).await;
}
```

**Where to call:**
1. `admin_login()` -- on successful login: "Admin login from [IP]"
2. `topup_wallet()` -- "Wallet topup: [amount] for driver [id] via [method]"
3. `ws_exec_pod()` -- "Fleet exec on pod [id]: [cmd_preview]"

No debounce needed for admin alerts -- these are low-frequency, high-value events. Each one should send.

### Pattern 3: PIN Rotation Tracking

**What:** Track when the admin PIN was last changed. Alert if unchanged for >30 days.

**Current state:**
- `admin_pin_hash` is stored in `Config.auth.admin_pin_hash` (from config file or env var)
- There is NO mechanism to change the PIN at runtime -- it's set in the config file or env var
- There is NO `pin_changed_at` timestamp anywhere

**Design decision:** Since the PIN is set in config (not via API), the simplest approach is to store `pin_changed_at` in the database rather than the config file:

```sql
CREATE TABLE IF NOT EXISTS system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT DEFAULT (datetime('now'))
);
```

On startup, if `admin_pin_hash` is configured:
1. Query `system_settings` for key `admin_pin_hash_sha256` (a SHA-256 of the hash itself, not the PIN)
2. If the stored SHA-256 matches the current config hash -- the PIN hasn't changed, use existing `updated_at`
3. If it differs or doesn't exist -- the PIN was changed, update/insert with current timestamp
4. If `updated_at` is >30 days ago -- log a warning and send WhatsApp alert

**Daily check:** Add a check in the `whatsapp_alerter_task` loop (already runs every 5s for pod monitoring). Once per day (tracked by a `last_pin_check` Instant), check the `pin_changed_at` age.

**Alert message:** "Staff PIN has not been changed in [N] days. Please update your admin PIN for security."

### Pattern 4: HMAC-SHA256 Sync Payload Signing

**What:** Sign cloud sync payloads with HMAC-SHA256 + timestamp + nonce to prevent tampering and replay attacks.

**Current auth:** Cloud sync uses `x-terminal-secret` header (simple shared secret comparison). This is vulnerable to replay attacks if network traffic is captured.

**HMAC signing approach:**

Outbound (venue -> cloud):
```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn sign_sync_payload(payload_bytes: &[u8], key: &[u8], timestamp: i64, nonce: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key");
    mac.update(&timestamp.to_be_bytes());
    mac.update(nonce.as_bytes());
    mac.update(payload_bytes);
    hex::encode(mac.finalize().into_bytes())
}
```

Headers added to sync requests:
- `x-sync-timestamp`: Unix timestamp (seconds)
- `x-sync-nonce`: UUID v4 (prevents replay)
- `x-sync-signature`: HMAC-SHA256 hex digest

Inbound verification (on `/sync/push` and `/sync/changes` endpoints):
1. Extract timestamp, nonce, signature from headers
2. Reject if timestamp is >300 seconds old (5-minute window)
3. Check nonce against in-memory set (HashSet with TTL cleanup)
4. Recompute HMAC over timestamp + nonce + body bytes
5. Constant-time compare signature (use `subtle` crate pattern or `hmac::Mac::verify()` which is already constant-time)

**Config addition:**
```toml
[cloud]
sync_hmac_key = "base64-encoded-32-byte-key"  # or via RACECONTROL_SYNC_HMAC_KEY env var
```

**Backward compatibility:** If `sync_hmac_key` is not set, fall back to existing `x-terminal-secret` behavior. Log a warning on startup recommending HMAC key configuration.

**Where to modify:**
1. `cloud_sync.rs::push_to_cloud()` -- add signing headers to outbound POST
2. `cloud_sync.rs::sync_once_http()` -- add signing headers to outbound GET
3. `cloud_sync.rs::push_via_relay()` -- relay is localhost, HMAC optional (skip for relay)
4. `api/routes.rs::sync_push()` -- verify inbound HMAC on POST
5. `api/routes.rs::sync_changes()` -- verify inbound HMAC on GET

### Anti-Patterns to Avoid

- **Don't create a separate audit database.** SQLite is single-file. A separate audit.db adds complexity for zero benefit at this scale. Use the existing `racing_point.db`.
- **Don't use SQLite triggers for audit logging.** Triggers cannot capture the staff_id or IP address -- they only see the row data. Application-level logging is required.
- **Don't wrap every handler in audit middleware.** Only the 15-20 admin/sensitive handlers need audit. A blanket middleware would log hundreds of read-only GET requests.
- **Don't store the HMAC key in the database.** Keep it in config/env like the JWT secret and terminal_secret.
- **Don't add nonce storage to SQLite.** Use an in-memory HashSet with periodic cleanup. Nonces only need to survive 5 minutes (the replay window). A server restart naturally clears them.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HMAC signing | Custom signature scheme | `hmac 0.12` + `sha2 0.10` (already in deps) | Constant-time verification, standard algorithm |
| Nonce generation | Custom random strings | `uuid::Uuid::new_v4()` (already in deps) | Cryptographic randomness, no collisions |
| Timestamp comparison | String-based time math | `chrono::Utc::now().timestamp()` (already in deps) | Timezone-safe, no parsing errors |
| WhatsApp messaging | New HTTP client code | Existing `send_whatsapp()` in `whatsapp_alerter.rs` | Already handles Evolution API auth, error logging |
| Audit record IDs | Sequential integers | `uuid::Uuid::new_v4()` | Prevents ID prediction, no conflicts |

## Common Pitfalls

### Pitfall 1: Blocking Tokio with HMAC on Large Payloads
**What goes wrong:** HMAC computation on large sync payloads (100KB+) blocks the async executor.
**Why it happens:** Crypto operations are CPU-bound.
**How to avoid:** At the payload sizes in cloud sync (typically <50KB JSON), HMAC is fast enough (~microseconds). Only use `spawn_blocking` if payloads exceed 1MB. Do NOT spawn_blocking for normal sync.
**Warning signs:** Latency spikes on sync endpoints.

### Pitfall 2: Audit Log Missing IP Address
**What goes wrong:** Audit records have null IP because extractors don't capture it.
**Why it happens:** Axum requires `ConnectInfo<SocketAddr>` from `.into_make_service_with_connect_info()` which is already configured (used by rate limiter).
**How to avoid:** Extract IP in each handler using `ConnectInfo<SocketAddr>` or the existing header-based extraction.
**Warning signs:** All audit records show null IP.

### Pitfall 3: Replay Window Too Tight
**What goes wrong:** Clock skew between venue and cloud causes legitimate sync requests to be rejected.
**Why it happens:** Venue server and cloud VPS may have 1-2 seconds of clock drift.
**How to avoid:** Use a 300-second (5-minute) replay window. This is standard for HMAC-based API auth.
**Warning signs:** Intermittent sync failures with "timestamp expired" errors.

### Pitfall 4: PIN Rotation Check Spamming WhatsApp
**What goes wrong:** The 30-day PIN alert fires every 5 seconds (on every alerter loop tick).
**Why it happens:** Missing daily-check guard.
**How to avoid:** Track `last_pin_rotation_check: Option<Instant>` in P0State. Only check once per 24 hours.
**Warning signs:** Uday's phone flooded with identical PIN rotation alerts.

### Pitfall 5: Audit Log Table Bloat
**What goes wrong:** audit_log grows unbounded over months.
**Why it happens:** Append-only with no retention policy.
**How to avoid:** Add a periodic cleanup that deletes records older than 90 days. Append-only means no UPDATE/DELETE of recent records -- old records can be pruned for storage. Log the pruning itself.
**Warning signs:** SQLite database file growing by >10MB/month.

### Pitfall 6: HMAC Key Not Deployed to Both Sides
**What goes wrong:** Venue signs but cloud doesn't verify (or vice versa), causing silent security gap.
**Why it happens:** Deploying sync_hmac_key to venue config but not to Bono's cloud config.
**How to avoid:** Permissive mode first -- log signature mismatch but don't reject. Then coordinate deployment with Bono.
**Warning signs:** sync signature warnings in logs on one side only.

## Code Examples

### HMAC Signing (from existing codebase pattern)
```rust
// Source: crates/racecontrol/src/crypto/encryption.rs (existing pattern)
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn sign_sync_request(body: &[u8], key: &[u8]) -> (String, i64, String) {
    let timestamp = chrono::Utc::now().timestamp();
    let nonce = uuid::Uuid::new_v4().to_string();

    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(&timestamp.to_be_bytes());
    mac.update(nonce.as_bytes());
    mac.update(body);

    let signature = hex::encode(mac.finalize().into_bytes());
    (signature, timestamp, nonce)
}

fn verify_sync_signature(
    body: &[u8], key: &[u8],
    timestamp: i64, nonce: &str, signature: &str,
) -> bool {
    // Replay check: reject if >5 minutes old
    let now = chrono::Utc::now().timestamp();
    if (now - timestamp).abs() > 300 {
        return false;
    }

    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC accepts any key length");
    mac.update(&timestamp.to_be_bytes());
    mac.update(nonce.as_bytes());
    mac.update(body);

    // Mac::verify is constant-time
    mac.verify_slice(&hex::decode(signature).unwrap_or_default()).is_ok()
}
```

### Admin Action Audit (new helper)
```rust
// In accounting.rs -- extends existing log_audit()
pub async fn log_admin_action(
    state: &Arc<AppState>,
    action_type: &str,
    details: &str,
    staff_id: Option<&str>,
    ip_address: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, action_type, new_values, staff_id, ip_address)
         VALUES (?, 'admin_actions', ?, 'create', ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&id)
    .bind(action_type)
    .bind(details)
    .bind(staff_id)
    .bind(ip_address)
    .execute(&state.db)
    .await;
}
```

### WhatsApp Admin Alert (extending existing module)
```rust
// In whatsapp_alerter.rs
pub(crate) async fn send_admin_alert(config: &Config, action: &str, details: &str) {
    let msg = format!("[ADMIN] {} -- {} | {}", action, details, ist_now_string());
    send_whatsapp(config, &msg).await;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No audit logging | Config-change CRUD logging (Phase 75) | 2026-03-20 | `audit_log` table exists but only tracks pricing/config changes |
| No admin auth | Argon2id PIN + JWT (Phase 76) | 2026-03-20 | admin_login() exists with 12h JWT |
| `x-terminal-secret` header | HMAC-SHA256 signing (this phase) | Phase 80 | Replay prevention, tamper detection |
| No PIN rotation tracking | `pin_changed_at` + 30-day alert (this phase) | Phase 80 | Security hygiene enforcement |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADMIN-04 | log_admin_action inserts audit record | unit | `cargo test -p racecontrol audit -- --test-threads=1` | No -- Wave 0 |
| ADMIN-04 | No DELETE/UPDATE on audit_log in codebase | grep | `grep -r "DELETE.*audit_log\|UPDATE.*audit_log" crates/` | N/A (manual) |
| ADMIN-05 | send_admin_alert calls send_whatsapp | unit | `cargo test -p racecontrol whatsapp -- --test-threads=1` | No -- Wave 0 |
| ADMIN-06 | PIN age detection triggers alert after 30 days | unit | `cargo test -p racecontrol pin_rotation -- --test-threads=1` | No -- Wave 0 |
| AUTH-07 | sign_sync_request produces valid HMAC | unit | `cargo test -p racecontrol hmac -- --test-threads=1` | No -- Wave 0 |
| AUTH-07 | verify rejects expired timestamp | unit | `cargo test -p racecontrol hmac -- --test-threads=1` | No -- Wave 0 |
| AUTH-07 | verify rejects tampered payload | unit | `cargo test -p racecontrol hmac -- --test-threads=1` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- --test-threads=1`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] HMAC sign/verify unit tests in `cloud_sync.rs` or new `crypto/sync_signing.rs` -- covers AUTH-07
- [ ] `log_admin_action()` test with in-memory SQLite -- covers ADMIN-04
- [ ] PIN rotation age calculation test -- covers ADMIN-06

## Open Questions

1. **Cloud-side HMAC verification**
   - What we know: Venue side adds HMAC headers. Cloud side (Bono's VPS) needs to verify them.
   - What's unclear: Bono's sync endpoint implementation -- is it Node.js? Does it need parallel changes?
   - Recommendation: Ship venue-side signing first with permissive mode. Notify Bono via INBOX.md with the signing spec. Cloud verification can be added independently.

2. **PIN change mechanism**
   - What we know: PIN is in config file or env var. No runtime API to change it.
   - What's unclear: Should Phase 80 add a PIN change endpoint, or just track+alert?
   - Recommendation: Track and alert only (per ADMIN-06 scope). A PIN change API is a separate feature.

3. **Audit log retention policy**
   - What we know: Append-only is needed for integrity. Unbounded growth is a risk.
   - What's unclear: How long to retain audit records.
   - Recommendation: 90-day default retention with configurable override. Pruning runs daily in the whatsapp_alerter_task loop.

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/accounting.rs` -- existing `log_audit()` function and `snapshot_row()` helper
- `crates/racecontrol/src/db/mod.rs` lines 1575-1603 -- existing `audit_log` table DDL with 4 indexes
- `crates/racecontrol/src/whatsapp_alerter.rs` -- `send_whatsapp()` private fn, `send_security_alert()` pub(crate) fn, Evolution API integration
- `crates/racecontrol/src/crypto/encryption.rs` lines 2-3, 62-68 -- existing `Hmac<Sha256>` pattern with `hmac 0.12` + `sha2 0.10`
- `crates/racecontrol/src/auth/admin.rs` -- `admin_login()` handler, argon2id verification
- `crates/racecontrol/src/cloud_sync.rs` -- `push_to_cloud()`, `sync_once_http()`, `push_via_relay()`, `x-terminal-secret` header auth
- `crates/racecontrol/src/api/routes.rs` -- `topup_wallet()` line 5435, `ws_exec_pod()` line 868, pricing CRUD lines 1727-1843, `terminal_submit()` line 7816, `sync_push()` line 7206
- `crates/racecontrol/src/config.rs` line 238 -- `admin_pin_hash: Option<String>` in AuthConfig
- `crates/racecontrol/Cargo.toml` lines 37-38 -- `hmac = "0.12"`, `sha2 = "0.10"` workspace deps

### Secondary (MEDIUM confidence)
- HMAC replay prevention with 5-minute window is standard practice per RFC 2104 and AWS Signature V4 patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace dependencies, zero new crates
- Architecture: HIGH -- all handler locations identified, existing audit_log table schema confirmed, existing HMAC pattern in encryption.rs
- Pitfalls: HIGH -- based on direct codebase audit (send_whatsapp visibility, audit_log schema gaps, clock skew)

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-20 (30 days -- stable domain, no external API changes expected)
