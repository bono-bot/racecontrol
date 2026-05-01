//! V2-skeleton AMEND-1 §1 connection_heart_admin contract — substrate.
//!
//! @cite_pact AMEND-1-V2-SKELETON-bundle-of-8 (ratify 815142d 2026-05-01 ~06:05 IST)
//! @satisfies AMEND-1 §1 connection_heart_admin (every Heart write routes through Admin)
//!
//! Phase 1 (this commit): substrate ship; middleware NOT wired into Heart router.
//!   Activation gate: AuthConfig.admin_origin_mode flip ("disabled" → "log-only" → "enforce")
//!   in racecontrol.toml + Phase 2 router-wiring commit.
//! Phase 2 (post-FP-window): wire into router as `.route_layer` on write endpoints; mode
//!   stays "log-only" while Admin instances are updated to send signed requests.
//! Phase 3 (active=enforce flip): missing/invalid Admin-origin signature → 403 origin-not-admin
//!   + audit row to data/admin_origin_audit.jsonl.
//!
//! Per AMEND-1 §1 contract spec:
//!   "Middleware on Heart's Axum router (Rust) that rejects any request lacking valid
//!    Admin-origin signature; falsification path = signed-nonce verification fails →
//!    403 + audit-row in HALO findings"
//!
//! Bypass: BYPASS_ADMIN_ORIGIN=1 env var (logged WARN; for disaster recovery only).
//!
//! Cryptographic shape (HMAC-SHA256):
//!   canonical_string = "{admin_instance_id}\n{method}\n{path}\n{nonce}\n{timestamp}\n{body_sha256_hex}"
//!   signature = base64(HMAC-SHA256(secret, canonical_string))
//!
//! Required headers on every Admin-originated write:
//!   X-Origin-Admin       — admin_instance_id (e.g. "venue-admin", "cloud-admin")
//!   X-Origin-Nonce       — 16 bytes base64 (per-request, replay defense)
//!   X-Origin-Timestamp   — RFC3339 UTC (skew bound by max_skew_secs)
//!   X-Origin-Signature   — base64 HMAC-SHA256 over canonical string
//!
//! Replay defense: in-memory LRU cache keyed by (admin_id, nonce) with TTL = 2× max_skew.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Mode for Admin-origin verification — controls fail-mode at the middleware boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminOriginMode {
    /// Middleware no-op — no verification, no logging. Fast-path.
    Disabled,
    /// Verify signatures; log WARN on missing/invalid; allow all requests through.
    LogOnly,
    /// Verify signatures; return 403 origin-not-admin + audit row on missing/invalid.
    Enforce,
}

impl AdminOriginMode {
    pub fn from_config_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "log-only" | "log_only" | "logonly" => Self::LogOnly,
            "enforce" => Self::Enforce,
            _ => Self::Disabled,
        }
    }
}

/// Outcome of signature verification — granular for both LogOnly observability and
/// Enforce mode 403 reasoning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// All four headers present; signature valid; nonce fresh; timestamp within skew.
    Valid {
        admin_id: String,
    },
    /// One or more required headers absent or unparseable.
    MissingHeaders { missing: Vec<&'static str> },
    /// Header X-Origin-Admin names an admin_instance_id not present in config secrets map.
    UnknownAdmin { admin_id: String },
    /// Timestamp outside [-max_skew, +max_skew] window.
    TimestampSkew { skew_secs: i64 },
    /// Nonce already seen within the replay window.
    NonceReplay { admin_id: String },
    /// HMAC verification failed (signature mismatch).
    SignatureMismatch { admin_id: String },
    /// Signature/nonce/body bytes failed base64 decode.
    MalformedSignature,
}

impl VerifyOutcome {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid { .. })
    }
    pub fn fail_reason(&self) -> &'static str {
        match self {
            Self::Valid { .. } => "valid",
            Self::MissingHeaders { .. } => "missing-headers",
            Self::UnknownAdmin { .. } => "unknown-admin",
            Self::TimestampSkew { .. } => "timestamp-skew",
            Self::NonceReplay { .. } => "nonce-replay",
            Self::SignatureMismatch { .. } => "signature-mismatch",
            Self::MalformedSignature => "malformed-signature",
        }
    }
}

/// Headers extracted from a request — caller (middleware shim) is responsible for
/// pulling these from the http::HeaderMap. Decoupled from axum so the verification
/// logic is unit-testable without a full Axum runtime.
#[derive(Debug, Clone)]
pub struct AdminOriginHeaders<'a> {
    pub admin_id: Option<&'a str>,
    pub nonce_b64: Option<&'a str>,
    pub timestamp_rfc3339: Option<&'a str>,
    pub signature_b64: Option<&'a str>,
}

impl<'a> AdminOriginHeaders<'a> {
    pub fn missing(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self.admin_id.is_none() { m.push("X-Origin-Admin"); }
        if self.nonce_b64.is_none() { m.push("X-Origin-Nonce"); }
        if self.timestamp_rfc3339.is_none() { m.push("X-Origin-Timestamp"); }
        if self.signature_b64.is_none() { m.push("X-Origin-Signature"); }
        m
    }
}

/// Replay defense — small in-memory LRU keyed by (admin_id, nonce_b64), entries
/// expire after window_secs. `prune` is O(n) but n is small (max ~1k for venue
/// throughput at one Heart write/sec × 2× skew window). NOT thread-safe internally;
/// caller wraps in Mutex.
#[derive(Debug, Default)]
pub struct NonceCache {
    seen: HashMap<(String, String), SystemTime>,
}

impl NonceCache {
    pub fn new() -> Self { Self::default() }

    /// Returns true if (admin_id, nonce) is already in cache (= replay).
    /// Otherwise inserts and returns false.
    pub fn check_and_insert(&mut self, admin_id: &str, nonce: &str, window: Duration) -> bool {
        let now = SystemTime::now();
        self.prune(now, window);
        let key = (admin_id.to_string(), nonce.to_string());
        if self.seen.contains_key(&key) {
            return true;
        }
        self.seen.insert(key, now);
        false
    }

    fn prune(&mut self, now: SystemTime, window: Duration) {
        self.seen.retain(|_, ts| now.duration_since(*ts).map(|d| d < window).unwrap_or(false));
    }

    pub fn len(&self) -> usize { self.seen.len() }
}

/// Process-global nonce cache. Initialised lazily.
static GLOBAL_NONCE_CACHE: OnceLock<Mutex<NonceCache>> = OnceLock::new();

fn global_nonce_cache() -> &'static Mutex<NonceCache> {
    GLOBAL_NONCE_CACHE.get_or_init(|| Mutex::new(NonceCache::new()))
}

/// Compute the canonical string an Admin instance must HMAC-sign.
pub fn canonical_string(
    admin_id: &str,
    method: &str,
    path: &str,
    nonce_b64: &str,
    timestamp: &str,
    body: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash_hex = hex_encode(&hasher.finalize());
    format!(
        "{admin_id}\n{method}\n{path}\n{nonce_b64}\n{timestamp}\n{body_hash_hex}"
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Sign — produce the base64 HMAC-SHA256 signature an Admin instance would emit.
/// Used by Admin instances (venue/cloud) in their request-emission path AND by
/// the middleware test harness to produce known-good signatures.
pub fn sign(secret: &[u8], canonical: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC-SHA256 accepts any key length");
    mac.update(canonical.as_bytes());
    let bytes = mac.finalize().into_bytes();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Verify — pure function over (headers, secrets-map, request shape). Caller is
/// responsible for body capture (axum body extraction) before calling.
///
/// `secrets` = map admin_instance_id → HMAC secret bytes
/// `max_skew_secs` = absolute timestamp skew tolerance (typical: 60s)
/// `nonce_cache` = caller-provided cache (use `global_nonce_cache()` in production)
pub fn verify(
    headers: &AdminOriginHeaders<'_>,
    method: &str,
    path: &str,
    body: &[u8],
    secrets: &HashMap<String, String>,
    max_skew_secs: i64,
    nonce_cache: &Mutex<NonceCache>,
) -> VerifyOutcome {
    let missing = headers.missing();
    if !missing.is_empty() {
        return VerifyOutcome::MissingHeaders { missing };
    }
    let admin_id = headers.admin_id.unwrap();
    let nonce = headers.nonce_b64.unwrap();
    let ts_str = headers.timestamp_rfc3339.unwrap();
    let sig_b64 = headers.signature_b64.unwrap();

    let secret = match secrets.get(admin_id) {
        Some(s) => s.as_bytes(),
        None => return VerifyOutcome::UnknownAdmin { admin_id: admin_id.to_string() },
    };

    // Timestamp skew check
    let req_ts = match chrono::DateTime::parse_from_rfc3339(ts_str) {
        Ok(t) => t.with_timezone(&chrono::Utc),
        Err(_) => return VerifyOutcome::TimestampSkew { skew_secs: i64::MAX },
    };
    let now = chrono::Utc::now();
    let skew = (now - req_ts).num_seconds();
    if skew.abs() > max_skew_secs {
        return VerifyOutcome::TimestampSkew { skew_secs: skew };
    }

    // Signature decode + verify
    let provided_sig = match base64::engine::general_purpose::STANDARD.decode(sig_b64) {
        Ok(b) => b,
        Err(_) => return VerifyOutcome::MalformedSignature,
    };
    let canonical = canonical_string(admin_id, method, path, nonce, ts_str, body);
    let expected_sig_b64 = sign(secret, &canonical);
    let expected_sig = base64::engine::general_purpose::STANDARD
        .decode(&expected_sig_b64)
        .expect("sign() always emits valid base64");
    if !constant_time_eq(&provided_sig, &expected_sig) {
        return VerifyOutcome::SignatureMismatch { admin_id: admin_id.to_string() };
    }

    // Replay check (only after signature passes — prevents cache pollution by attacker)
    let window = Duration::from_secs((max_skew_secs as u64).saturating_mul(2));
    let mut cache = nonce_cache.lock().expect("nonce cache mutex poisoned");
    if cache.check_and_insert(admin_id, nonce, window) {
        return VerifyOutcome::NonceReplay { admin_id: admin_id.to_string() };
    }

    VerifyOutcome::Valid { admin_id: admin_id.to_string() }
}

/// Convenience wrapper using the process-global nonce cache. Production middleware
/// path calls this; tests use `verify()` with their own cache.
pub fn verify_with_global_cache(
    headers: &AdminOriginHeaders<'_>,
    method: &str,
    path: &str,
    body: &[u8],
    secrets: &HashMap<String, String>,
    max_skew_secs: i64,
) -> VerifyOutcome {
    verify(headers, method, path, body, secrets, max_skew_secs, global_nonce_cache())
}

/// Default skew tolerance for Admin-origin timestamps.
pub const DEFAULT_MAX_SKEW_SECS: i64 = 60;

#[cfg(test)]
#[path = "admin_origin_tests.rs"]
mod admin_origin_tests;
