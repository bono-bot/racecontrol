# Phase 79: Data Protection - Research

**Researched:** 2026-03-21
**Domain:** Column-level encryption, deterministic hashing, log redaction, GDPR-style data rights in Rust/Axum/SQLite
**Confidence:** HIGH

## Summary

Phase 79 implements data-at-rest protection for customer PII in the Racing Point racecontrol server. The drivers table in SQLite stores phone numbers, emails, names, guardian phones, and DOBs in plaintext. The security audit (Phase 75) identified PII in 6 locations: SQLite, application logs, WhatsApp payloads, cloud sync payloads, and bot messages. This phase encrypts PII columns with AES-256-GCM, adds a deterministic HMAC-SHA256 hash column for phone lookups (OTP flow), redacts PII from logs, and adds data export/deletion endpoints.

The critical design challenge is the dual requirement of DATA-02: phone numbers must be searchable (for OTP matching via `WHERE phone = ?`) while also encrypted for display. The solution is a two-column approach: store an HMAC-SHA256 hash of the phone in a `phone_hash` column for lookups, and store the AES-256-GCM encrypted phone in a `phone_enc` column for display. The encryption key and HMAC key must be separate from each other and from the JWT key.

**Primary recommendation:** Use `aes-gcm` 0.10.3 for column-level encryption with per-value random nonces stored alongside ciphertext. Use HMAC-SHA256 (via `hmac` + `sha2` crates) for deterministic phone hash. Add a tracing Layer that regex-scrubs PII patterns from log output. Migrate existing plaintext data in a one-time migration.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DATA-01 | AES-256-GCM encryption on PII columns (phone, email, name, guardian_phone) in SQLite | aes-gcm 0.10.3 column-level encryption; nonce+ciphertext stored as base64 TEXT; encrypt/decrypt module in racecontrol |
| DATA-02 | Deterministic hash for phone lookups + reversible encryption for display | HMAC-SHA256 via hmac+sha2 crates for `phone_hash` column; separate AES-GCM `phone_enc` column; OTP queries use `WHERE phone_hash = ?` |
| DATA-03 | Log redaction -- scrub PII from application logs and bot messages | Custom tracing Layer with regex replacement for phone patterns (\d{10}), email patterns, and known PII field names |
| DATA-04 | Customer data export endpoint (JSON dump) | New `GET /api/v1/customer/data-export` behind customer JWT; decrypts PII and returns full driver record |
| DATA-05 | Customer data deletion endpoint (cascade delete) | New `DELETE /api/v1/customer/data-delete` behind customer JWT; cascade delete from drivers + wallets + sessions + laps |
| DATA-06 | Encryption key management -- separate from JWT key, stored securely, rotatable | Two env vars: `RACECONTROL_ENCRYPTION_KEY` (AES-256) and `RACECONTROL_HMAC_KEY` (HMAC-SHA256); fail-to-start if unset |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `aes-gcm` | 0.10.3 | AES-256-GCM authenticated encryption | RustCrypto official AEAD implementation; audited by NCC Group; pure Rust (no OpenSSL); latest stable release |
| `hmac` | 0.12 | HMAC-SHA256 for deterministic phone hashing | RustCrypto standard; used with sha2 for keyed hashing |
| `sha2` | 0.10 | SHA-256 hash primitive (used by hmac) | RustCrypto standard; dependency of hmac |
| `hex` | 0.4 | Hex encoding for HMAC hash output | Standard hex encoding crate; deterministic string format for SQLite TEXT column |

### Supporting (Already in Stack)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `base64` | 0.22 | Base64 encoding for encrypted ciphertext+nonce | Already in racecontrol Cargo.toml; encode nonce+ciphertext as TEXT for SQLite |
| `rand` | 0.8 | Nonce generation for AES-GCM | Already in workspace; `OsRng` for cryptographic randomness |
| `tracing` | 0.1 | Log infrastructure | Already in workspace; custom Layer for redaction |
| `tracing-subscriber` | 0.3 | Subscriber with Layer support | Already in workspace; compose redaction layer into existing subscriber |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Column-level aes-gcm | SQLCipher (full-DB encryption) | SQLCipher requires rebuilding SQLx with `bundled-sqlcipher`, breaks tooling, high Windows build complexity. Column-level is simpler and sufficient for ~5 PII columns |
| HMAC-SHA256 for phone hash | Argon2 hash of phone | Argon2 is deliberately slow (100ms+); phone lookups happen on every OTP request; HMAC is sub-microsecond and sufficient since phone numbers are not passwords |
| Custom tracing Layer | `redactable` crate | redactable requires wrapping every PII field in a Sensitive<T> type; too invasive for existing codebase. Regex-based Layer is drop-in |

**Installation:**

```toml
# Add to workspace Cargo.toml [workspace.dependencies]
aes-gcm = "0.10"
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"

# Add to crates/racecontrol/Cargo.toml [dependencies]
aes-gcm = { workspace = true }
hmac = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
```

**Note on versions:** `aes-gcm` 0.11 is in release-candidate state (`0.11.0-rc.3` on crates.io). Use stable 0.10.3. Similarly `hmac` 0.13 and `sha2` 0.11 are pre-release; use stable 0.12 and 0.10 respectively.

## Architecture Patterns

### Recommended Project Structure

```
crates/racecontrol/src/
  crypto/
    mod.rs          # pub mod encryption; pub mod redaction;
    encryption.rs   # FieldCipher struct: encrypt_field(), decrypt_field(), hash_phone()
    redaction.rs    # PiiRedactionLayer for tracing
  db/
    mod.rs          # Existing -- add migration for encrypted columns
    migration.rs    # One-time plaintext -> encrypted data migration
  api/
    routes.rs       # Add data export/deletion endpoints
```

### Pattern 1: FieldCipher for Column-Level Encryption

**What:** A single struct that holds the AES-256-GCM key and HMAC key, providing `encrypt_field()`, `decrypt_field()`, and `hash_phone()` methods. Stored in `AppState` for access from any handler.

**When to use:** Every read/write of PII columns in the drivers table.

**Example:**

```rust
// Source: aes-gcm 0.10.3 docs + hmac 0.12 docs
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce, AeadCore,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

type HmacSha256 = Hmac<Sha256>;

pub struct FieldCipher {
    cipher: Aes256Gcm,
    hmac_key: Vec<u8>,
}

impl FieldCipher {
    /// Create from raw key bytes (32 bytes each)
    pub fn new(aes_key: &[u8; 32], hmac_key: &[u8]) -> Self {
        let cipher = Aes256Gcm::new(aes_key.into());
        Self { cipher, hmac_key: hmac_key.to_vec() }
    }

    /// Encrypt a plaintext field. Returns base64(nonce || ciphertext).
    pub fn encrypt_field(&self, plaintext: &str) -> Result<String, String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;
        // Prepend 12-byte nonce to ciphertext, then base64 encode
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(&combined))
    }

    /// Decrypt a base64(nonce || ciphertext) field.
    pub fn decrypt_field(&self, encoded: &str) -> Result<String, String> {
        let combined = BASE64.decode(encoded)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;
        if combined.len() < 12 {
            return Err("Ciphertext too short".to_string());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;
        String::from_utf8(plaintext)
            .map_err(|e| format!("UTF-8 decode failed: {}", e))
    }

    /// Deterministic HMAC-SHA256 hash of a phone number.
    /// Returns hex string. Same phone always produces same hash.
    pub fn hash_phone(&self, phone: &str) -> String {
        // Normalize: strip whitespace, strip +91 prefix
        let normalized = phone.trim().trim_start_matches("+91");
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key)
            .expect("HMAC key length is valid");
        mac.update(normalized.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}
```

### Pattern 2: Phone Number Dual-Column Storage

**What:** Replace the single `phone TEXT` column with `phone_hash TEXT` (HMAC, indexed, for lookups) and `phone_enc TEXT` (AES-GCM, for display).

**When to use:** Every query that currently does `WHERE phone = ?`.

**Current queries that use `WHERE phone = ?` (from codebase grep):**
1. `auth/mod.rs:1000` -- `send_otp`: `SELECT id, name FROM drivers WHERE phone = ?`
2. `auth/mod.rs:1094` -- `verify_otp`: `SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone = ?`
3. `cloud_sync.rs:884` -- wallet sync phone match: `SELECT id FROM drivers WHERE phone = ?`
4. `api/routes.rs:4801` -- waiver lookup: `WHERE phone LIKE '%' || ?`
5. `api/routes.rs:11188` -- registration: `phone = params.phone.trim()`

**Migration approach:**
```sql
-- Step 1: Add new columns
ALTER TABLE drivers ADD COLUMN phone_hash TEXT;
ALTER TABLE drivers ADD COLUMN phone_enc TEXT;
ALTER TABLE drivers ADD COLUMN email_enc TEXT;
ALTER TABLE drivers ADD COLUMN name_enc TEXT;
ALTER TABLE drivers ADD COLUMN guardian_phone_hash TEXT;
ALTER TABLE drivers ADD COLUMN guardian_phone_enc TEXT;

-- Step 2: Create index on phone_hash (replaces idx_drivers_phone)
CREATE INDEX IF NOT EXISTS idx_drivers_phone_hash ON drivers(phone_hash);

-- Step 3: Application-level migration populates _hash and _enc from plaintext
-- Step 4: After migration verified, clear plaintext columns:
-- UPDATE drivers SET phone = NULL, email = NULL, guardian_phone = NULL;
-- (Keep name in plaintext for leaderboard display unless privacy requires it)
```

**Critical decision: name column.**
The `name` column is used in leaderboard display (`/public/leaderboard`), kiosk display, and admin dashboard. Encrypting it means every leaderboard query must decrypt N names. For a cafe with ~500 drivers, this is manageable but adds latency. The `nickname` column (already exists, low sensitivity) can be the public display name while `name` is encrypted. However, many existing queries SELECT name for display purposes.

**Recommendation:** Encrypt `name` into `name_enc` but keep `nickname` plaintext for public-facing display. Admin views decrypt `name_enc` on demand.

### Pattern 3: PII Redaction Layer for Tracing

**What:** A custom `tracing_subscriber::Layer` that intercepts log events and replaces phone number patterns and PII field values before they reach the output.

**When to use:** Added to the existing tracing subscriber stack in main.rs.

**Example:**

```rust
use tracing_subscriber::Layer;
use tracing::Subscriber;
use regex::Regex;
use std::sync::LazyLock;

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{10}\b").unwrap()
});

static OTP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bOTP[^:]*:\s*\d{4,6}\b").unwrap()
});

// Option A: Modify log messages at the source (simpler, preferred)
// Replace tracing::info!("OTP for phone {}: {}", phone, otp)
// with   tracing::info!("OTP sent for phone ***{}", &phone[phone.len()-4..])

// Option B: Custom fmt::FormatEvent wrapper
// Wraps the existing fmt layer's event formatter to regex-replace PII
```

**Recommendation:** Option A (source-level redaction) is simpler and more reliable than regex interception. Change the ~4 log statements identified in the security audit (auth/mod.rs lines 1076, 1079, 1082, 1086 and billing.rs lines 2263, 2266) to use redacted values. This is a targeted fix for known PII leaks rather than a generic regex that might miss edge cases or false-positive on legitimate data.

### Pattern 4: Cloud Sync Compatibility

**What:** Encrypted PII columns must work correctly with cloud sync (Bono VPS pull/push).

**Current sync behavior (from codebase analysis):**
- **Push direction (venue -> cloud):** Only sends non-PII fields (has_used_trial, total_laps, registration_completed, waiver_signed, etc.). Does NOT send phone, email, or name. **No changes needed for push.**
- **Pull direction (cloud -> venue):** Cloud sends full driver records including name, phone, email, guardian_name, guardian_phone. The `upsert_driver()` function writes these directly to the drivers table.

**Impact:** The pull direction must be updated. When cloud sends plaintext PII, the `upsert_driver()` function must:
1. Encrypt name, email, phone, guardian_name, guardian_phone before storing
2. Compute phone_hash and guardian_phone_hash from the plaintext values
3. Store encrypted values in the _enc columns, hashes in _hash columns
4. NOT store plaintext in the original columns

### Anti-Patterns to Avoid

- **Reusing the JWT key for encryption:** The JWT signing key (HMAC-HS256) must be separate from the AES encryption key and the HMAC phone hash key. Three separate keys for three separate purposes. If one is compromised, the others remain safe.
- **Using the same nonce twice:** AES-GCM with a repeated nonce is catastrophically broken (reveals the XOR of two plaintexts). Always generate a random 96-bit nonce per encryption. Storing nonce alongside ciphertext is the standard approach.
- **Encrypting the entire drivers row as a blob:** Loses the ability to query individual columns, breaks partial updates, makes cloud sync impossible.
- **Using HMAC for email/name lookup:** Only phone needs deterministic lookup (OTP flow). Email and name lookups happen via ID or admin search -- admin can decrypt and filter in application code.
- **Storing the encryption key in racecontrol.toml:** Keys must be in environment variables, not config files. Config files are on every pod, the server, the pendrive, and git.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AES-256-GCM encryption | Custom XOR cipher or CBC mode | `aes-gcm` crate | AES-GCM provides authenticated encryption (integrity + confidentiality). CBC requires separate MAC. GCM is the industry standard. |
| Deterministic phone hash | SHA-256(phone) without key | HMAC-SHA256(phone, key) | Unkeyed SHA-256 is vulnerable to rainbow tables. With ~1 billion possible 10-digit Indian phone numbers, a rainbow table is trivially constructable. HMAC with a secret key prevents this. |
| Nonce generation | Counter-based nonce | `Aes256Gcm::generate_nonce(&mut OsRng)` | Counter nonces require persistent state and are fragile across restarts. Random 96-bit nonces have negligible collision probability for the data volumes here. |
| Log redaction regex | Manual string.replace() calls | `regex::Regex` with lazy static | Phone patterns vary (10 digits, +91 prefix, spaces). A single regex handles all variants. |

## Common Pitfalls

### Pitfall 1: Migration Corrupts Existing Data

**What goes wrong:** The one-time migration encrypts all existing plaintext data, but a bug or crash midway leaves some rows encrypted and others plaintext. Subsequent reads fail because the code tries to decrypt plaintext or parse ciphertext as text.

**Why it happens:** Migration is treated as a simple UPDATE loop without transaction safety or progress tracking.

**How to avoid:**
- Use a migration flag column or separate migration tracking table
- Process in batches of 100 rows within transactions
- Each row gets a `pii_migrated` boolean flag (or use the presence of `phone_enc` as the flag)
- On read: if `phone_enc` is NULL but `phone` is not, treat as unmigrated and encrypt on access (lazy migration)
- Run migration on startup with idempotent logic (safe to re-run)

**Warning signs:** Some drivers display "[DECRYPTION FAILED]" after migration.

### Pitfall 2: Phone LIKE Queries Break

**What goes wrong:** The waiver lookup uses `WHERE phone LIKE '%' || ?` for partial phone matching. This cannot work with encrypted or hashed values.

**Why it happens:** Deterministic hash is exact-match only. There is no way to do LIKE/partial matching on HMAC output.

**How to avoid:**
- For exact phone lookups (OTP): use `WHERE phone_hash = ?` with the HMAC hash
- For partial phone search (waiver lookup at `routes.rs:4801`): change to search by driver ID or name instead, or require the full phone number
- For admin search (`routes.rs:1208`): decrypt in application code and filter. At 500 drivers, decrypting all phones is ~5ms (negligible)
- Document that partial phone search is no longer supported

### Pitfall 3: Cloud Sync Sends Encrypted Data That Cloud Cannot Read

**What goes wrong:** If encrypted values leak into the sync push payload, Bono's VPS receives ciphertext it cannot decrypt (it does not have the encryption key).

**Why it happens:** Sync push queries SELECT from drivers table. If column names change or the wrong columns are selected, encrypted data is sent.

**How to avoid:**
- Current sync push already does NOT send PII (only sends has_used_trial, total_laps, etc.). Verify this remains true after schema changes.
- Sync pull (cloud -> venue) sends plaintext PII. The `upsert_driver()` function must encrypt before storing.
- Never put the venue encryption key on the cloud VPS. PII on the cloud side is in PostgreSQL and is a separate concern (Bono's responsibility).

### Pitfall 4: Encryption Key Lost = All PII Unrecoverable

**What goes wrong:** The AES-256 encryption key is stored only in an environment variable. Server reinstall, env wipe, or forgetting the key means all encrypted PII is permanently lost.

**Why it happens:** No key backup procedure.

**How to avoid:**
- Document the key in Uday's password manager (1Password, Bitwarden, etc.)
- Store a backup copy in a sealed envelope at the venue (physical security)
- Include key in the deployment checklist: "Before first run, set RACECONTROL_ENCRYPTION_KEY"
- On startup, if key is missing, refuse to start with a clear error message

### Pitfall 5: Cascade Delete Misses Related Tables

**What goes wrong:** Customer deletion deletes from `drivers` but misses `wallets`, `billing_sessions`, `laps`, `customer_sessions`, `friends`, `group_session_participants`, etc.

**Why it happens:** No foreign key cascade in SQLite (SQLite supports FK cascade but it is OFF by default and must be enabled with `PRAGMA foreign_keys = ON`).

**How to avoid:**
- Enumerate ALL tables with a driver_id/customer_id foreign key reference
- Delete from child tables first, then parent (drivers)
- Wrap in a transaction
- Tables to delete from (from schema analysis): `wallets`, `wallet_transactions`, `billing_sessions`, `billing_session_events`, `laps`, `customer_sessions`, `friends`, `friend_requests`, `group_session_participants`, `tournament_registrations`, `reservations`, `waivers`, `auth_tokens`

## Code Examples

### Encryption Key Loading from Environment

```rust
// Source: standard Rust env var pattern
pub fn load_encryption_keys() -> Result<FieldCipher, String> {
    let enc_key_hex = std::env::var("RACECONTROL_ENCRYPTION_KEY")
        .map_err(|_| "RACECONTROL_ENCRYPTION_KEY env var not set. Cannot start without encryption key.")?;
    let hmac_key_hex = std::env::var("RACECONTROL_HMAC_KEY")
        .map_err(|_| "RACECONTROL_HMAC_KEY env var not set. Cannot start without HMAC key.")?;

    let enc_key_bytes = hex::decode(&enc_key_hex)
        .map_err(|e| format!("RACECONTROL_ENCRYPTION_KEY is not valid hex: {}", e))?;
    if enc_key_bytes.len() != 32 {
        return Err(format!("RACECONTROL_ENCRYPTION_KEY must be 64 hex chars (32 bytes), got {}", enc_key_bytes.len()));
    }

    let hmac_key_bytes = hex::decode(&hmac_key_hex)
        .map_err(|e| format!("RACECONTROL_HMAC_KEY is not valid hex: {}", e))?;

    let mut aes_key = [0u8; 32];
    aes_key.copy_from_slice(&enc_key_bytes);

    Ok(FieldCipher::new(&aes_key, &hmac_key_bytes))
}
```

### Key Generation (One-Time Setup)

```rust
// Generate keys for first-time setup
use rand::RngCore;
let mut enc_key = [0u8; 32];
rand::rngs::OsRng.fill_bytes(&mut enc_key);
println!("RACECONTROL_ENCRYPTION_KEY={}", hex::encode(enc_key));

let mut hmac_key = [0u8; 32];
rand::rngs::OsRng.fill_bytes(&mut hmac_key);
println!("RACECONTROL_HMAC_KEY={}", hex::encode(hmac_key));
```

### Data Export Endpoint

```rust
// GET /api/v1/customer/data-export
// Behind customer JWT middleware (extract_driver_id)
async fn customer_data_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let driver_id = extract_driver_id(&state, &headers)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name_enc, email_enc, phone_enc, nickname, total_laps, total_time_ms FROM drivers WHERE id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let cipher = &state.field_cipher;
    Ok(Json(json!({
        "driver_id": row.0,
        "name": row.1.as_deref().map(|v| cipher.decrypt_field(v).ok()).flatten(),
        "email": row.2.as_deref().map(|v| cipher.decrypt_field(v).ok()).flatten(),
        "phone": row.3.as_deref().map(|v| cipher.decrypt_field(v).ok()).flatten(),
        "nickname": row.4,
        "total_laps": row.5,
        "total_time_ms": row.6,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    })))
}
```

### Data Deletion Endpoint

```rust
// DELETE /api/v1/customer/data-delete
async fn customer_data_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let driver_id = extract_driver_id(&state, &headers)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let mut tx = state.db.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete from child tables first
    for table in &[
        "wallet_transactions", "wallets", "billing_session_events",
        "billing_sessions", "laps", "customer_sessions",
        "friend_requests", "friends", "group_session_participants",
        "tournament_registrations", "reservations", "waivers", "auth_tokens",
    ] {
        let query = format!("DELETE FROM {} WHERE driver_id = ?", table);
        sqlx::query(&query).bind(&driver_id).execute(&mut *tx).await.ok();
    }

    // Delete the driver record itself
    sqlx::query("DELETE FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("Customer {} deleted their data (DPDP compliance)", driver_id);
    Ok(StatusCode::NO_CONTENT)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SQLCipher full-DB encryption | Column-level aes-gcm | N/A (design choice) | Avoids build complexity, keeps tooling working, targets only PII |
| SHA-256 for phone hash | HMAC-SHA256 with secret key | Standard practice | Prevents rainbow table attacks on 10-digit phone space |
| aes-gcm 0.9 | aes-gcm 0.10.3 | 2023 | New `AeadCore` trait, `generate_nonce` API |
| Manual log scrubbing | Source-level redaction of known PII log statements | N/A | More reliable than regex-based interception |

**Deprecated/outdated:**
- `aes-gcm` 0.11 is pre-release (rc.3) -- do not use in production
- `hmac` 0.13 is pre-release (rc.6) -- use stable 0.12
- `sha2` 0.11 is pre-release (rc.5) -- use stable 0.10

## Open Questions

1. **Name encryption scope**
   - What we know: `name` is used in leaderboard display, admin views, billing receipts, and cloud sync
   - What's unclear: Whether encrypting `name` is required or if `nickname` (already exists) is sufficient for public display
   - Recommendation: Encrypt `name` into `name_enc` for DATA-01 compliance. Use `nickname` for leaderboards. Admin views decrypt on demand.

2. **staff_members table PII**
   - What we know: The `staff_members` table also has `name` and `phone` columns (identified in security audit)
   - What's unclear: Whether DATA-01 scope includes staff_members or only drivers (customer data)
   - Recommendation: Include staff_members in encryption for consistency. Only 2-3 staff records, negligible overhead.

3. **Existing data migration timing**
   - What we know: Migration must convert plaintext to encrypted without downtime
   - What's unclear: Whether to do lazy migration (encrypt on access) or eager migration (encrypt all at startup)
   - Recommendation: Eager migration at startup in a transaction. With ~500 drivers, encrypting 5 fields each takes < 1 second. Simpler than lazy migration codepaths.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | None -- uses default Cargo test runner |
| Quick run command | `cargo test -p racecontrol -- --test-threads=1 -q` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DATA-01 | Encrypt/decrypt PII fields roundtrip | unit | `cargo test -p racecontrol crypto::encryption::tests -q` | Wave 0 |
| DATA-02 | HMAC phone hash determinism + encrypt/decrypt separation | unit | `cargo test -p racecontrol crypto::encryption::tests::test_phone_hash -q` | Wave 0 |
| DATA-03 | Log output does not contain phone/OTP patterns | unit | `cargo test -p racecontrol crypto::redaction::tests -q` | Wave 0 |
| DATA-04 | Data export returns decrypted customer record | integration | `cargo test -p racecontrol api::tests::test_data_export -q` | Wave 0 |
| DATA-05 | Data deletion cascades to all child tables | integration | `cargo test -p racecontrol api::tests::test_data_delete -q` | Wave 0 |
| DATA-06 | Missing encryption key prevents startup | unit | `cargo test -p racecontrol crypto::encryption::tests::test_key_loading -q` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p racecontrol -- --test-threads=1 -q`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before verify

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/crypto/mod.rs` -- new module
- [ ] `crates/racecontrol/src/crypto/encryption.rs` -- FieldCipher + tests
- [ ] `crates/racecontrol/src/crypto/redaction.rs` -- redaction helpers + tests
- [ ] Workspace Cargo.toml additions: aes-gcm, hmac, sha2, hex

## Sources

### Primary (HIGH confidence)

- [aes-gcm 0.10.3 docs](https://docs.rs/aes-gcm/0.10.3/aes_gcm/) -- encryption API, Aes256Gcm, Nonce, KeyInit, Aead traits
- Direct codebase analysis of `crates/racecontrol/src/auth/mod.rs` -- OTP flow, phone lookups at lines 1000, 1094
- Direct codebase analysis of `crates/racecontrol/src/cloud_sync.rs` -- sync push (non-PII only, line 342-355), sync pull (full PII, upsert_driver at line 752)
- Direct codebase analysis of `crates/racecontrol/src/db/mod.rs` -- drivers schema, phone index at line 483
- Phase 75 Security Audit (`SECURITY-AUDIT.md`) -- PII location inventory, 6 confirmed PII locations
- `cargo search` results for current stable versions (aes-gcm 0.10.x, hmac 0.12, sha2 0.10)

### Secondary (MEDIUM confidence)

- [RustCrypto AEADs repository](https://github.com/RustCrypto/AEADs) -- aes-gcm project home, NCC Group audit reference
- [Custom tracing layers guide](https://burgers.io/custom-logging-in-rust-using-tracing) -- Layer trait implementation patterns

### Tertiary (LOW confidence)

- [redactable crate](https://github.com/sformisano/redactable) -- alternative PII redaction approach (not recommended for this project)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- aes-gcm 0.10.3 is verified stable, APIs confirmed via docs.rs
- Architecture: HIGH -- based on direct codebase analysis of 5+ source files, all phone query locations mapped
- Pitfalls: HIGH -- migration risks and sync compatibility verified against actual code paths

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable cryptographic crates, slow-moving domain)
