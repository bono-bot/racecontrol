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
    let (admin_id, nonce, ts_str, sig_b64) = match (
        headers.admin_id,
        headers.nonce_b64,
        headers.timestamp_rfc3339,
        headers.signature_b64,
    ) {
        (Some(a), Some(n), Some(t), Some(s)) => (a, n, t, s),
        _ => return VerifyOutcome::MissingHeaders { missing: headers.missing() },
    };

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

/// Body capture cap for admin_origin verification (4 MiB).
/// Most admin writes are small JSON payloads (<10 KiB); 4 MiB is generous headroom
/// without risking memory pressure on the Heart binary under request burst.
pub const BODY_CAPTURE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

// ─── Phase 2: Axum middleware shim + audit-row sink ───────────────────────

use axum::{
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::state::AppState;

/// Pull the four required Admin-origin headers from a HeaderMap.
fn extract_headers(h: &HeaderMap) -> AdminOriginHeaders<'_> {
    fn h_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
        h.get(name).and_then(|v| v.to_str().ok())
    }
    AdminOriginHeaders {
        admin_id: h_str(h, "x-origin-admin"),
        nonce_b64: h_str(h, "x-origin-nonce"),
        timestamp_rfc3339: h_str(h, "x-origin-timestamp"),
        signature_b64: h_str(h, "x-origin-signature"),
    }
}

/// Append a single audit row to data/admin_origin_audit.jsonl. Best-effort —
/// if the write fails, log and proceed (do not block the request).
///
/// Phase 3 deliverable: migrate this sink to HALO findings format per AMEND-1 §1
/// spec ("audit-row in HALO findings"). Current dedicated file simplifies
/// observability during Phase 2 soak.
fn write_audit_row(
    mode: AdminOriginMode,
    method: &str,
    path: &str,
    outcome: &VerifyOutcome,
    body_len: usize,
) {
    use std::io::Write;
    let now = chrono::Utc::now().to_rfc3339();
    let row = serde_json::json!({
        "ts_utc": now,
        "mode": match mode {
            AdminOriginMode::Disabled => "disabled",
            AdminOriginMode::LogOnly => "log-only",
            AdminOriginMode::Enforce => "enforce",
        },
        "method": method,
        "path": path,
        "outcome": outcome.fail_reason(),
        "body_len": body_len,
        "detail": match outcome {
            VerifyOutcome::Valid { admin_id } => serde_json::json!({"admin_id": admin_id}),
            VerifyOutcome::MissingHeaders { missing } => serde_json::json!({"missing": missing}),
            VerifyOutcome::UnknownAdmin { admin_id } => serde_json::json!({"admin_id": admin_id}),
            VerifyOutcome::TimestampSkew { skew_secs } => serde_json::json!({"skew_secs": skew_secs}),
            VerifyOutcome::NonceReplay { admin_id } => serde_json::json!({"admin_id": admin_id}),
            VerifyOutcome::SignatureMismatch { admin_id } => serde_json::json!({"admin_id": admin_id}),
            VerifyOutcome::MalformedSignature => serde_json::json!({}),
        },
        "cite_pact": "AMEND-1-V2-SKELETON-bundle-of-8",
    });
    let line = format!("{}\n", row);
    let path = "data/admin_origin_audit.jsonl";
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => { let _ = f.write_all(line.as_bytes()); }
        Err(e) => tracing::warn!(target: "admin_origin", error=%e, "audit-row write failed"),
    }
}

/// Build the JSON 403 body emitted in enforce mode on non-Valid outcomes.
fn enforce_403_response(outcome: &VerifyOutcome) -> Response {
    let body = serde_json::json!({
        "error": "origin-not-admin",
        "reason": outcome.fail_reason(),
        "cite_pact": "AMEND-1-V2-SKELETON-bundle-of-8",
    });
    (StatusCode::FORBIDDEN, axum::Json(body)).into_response()
}

/// Axum middleware: verify Admin-origin signature on incoming write requests.
///
/// Mode-driven via AppState.config.auth.admin_origin_mode (read per-request to
/// allow runtime config reload without restart). Disabled mode = fast no-op
/// (no body capture, no header lookup, no audit row). LogOnly = verify + warn +
/// audit-row + allow through. Enforce = verify + 403 on non-Valid.
///
/// Bypass: BYPASS_ADMIN_ORIGIN=1 env var (logs WARN; for disaster recovery).
///
/// Phase 2 default: mode=disabled per AuthConfig default. Phase 2 wiring attaches
/// this layer to staff_routes(); Admin-app signing rollout precedes mode flip.
pub async fn admin_origin_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let mode = AdminOriginMode::from_config_str(&state.config.auth.admin_origin_mode);

    // Disabled fast-path — no body capture, no work.
    if mode == AdminOriginMode::Disabled {
        return Ok(next.run(req).await);
    }

    // Bypass check (Disaster recovery).
    if std::env::var("BYPASS_ADMIN_ORIGIN").as_deref() == Ok("1") {
        tracing::warn!(target: "admin_origin", "BYPASS_ADMIN_ORIGIN=1 — skipping verification");
        return Ok(next.run(req).await);
    }

    let (parts, body) = req.into_parts();
    let method = parts.method.as_str().to_string();
    let path = parts.uri.path().to_string();
    let headers_extracted = extract_headers(&parts.headers);
    let admin_id_opt = headers_extracted.admin_id.map(String::from);
    let nonce_opt = headers_extracted.nonce_b64.map(String::from);
    let ts_opt = headers_extracted.timestamp_rfc3339.map(String::from);
    let sig_opt = headers_extracted.signature_b64.map(String::from);

    let bytes = match axum::body::to_bytes(body, BODY_CAPTURE_LIMIT_BYTES).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: "admin_origin", error=%e, "body capture failed (>limit?)");
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "body too large").into_response());
        }
    };
    let body_len = bytes.len();

    let headers = AdminOriginHeaders {
        admin_id: admin_id_opt.as_deref(),
        nonce_b64: nonce_opt.as_deref(),
        timestamp_rfc3339: ts_opt.as_deref(),
        signature_b64: sig_opt.as_deref(),
    };

    let outcome = verify_with_global_cache(
        &headers,
        &method,
        &path,
        &bytes,
        &state.config.auth.admin_origin_secrets,
        state.config.auth.admin_origin_max_skew_secs,
    );

    if !outcome.is_valid() {
        tracing::warn!(
            target: "admin_origin",
            mode = ?mode,
            method = %method,
            path = %path,
            outcome = %outcome.fail_reason(),
            "Admin-origin verification failed"
        );
        write_audit_row(mode, &method, &path, &outcome, body_len);
        if mode == AdminOriginMode::Enforce {
            return Err(enforce_403_response(&outcome));
        }
    }

    // Reconstruct request with captured body and pass through.
    let req = Request::from_parts(parts, axum::body::Body::from(bytes));
    Ok(next.run(req).await)
}

#[cfg(test)]
#[path = "admin_origin_tests.rs"]
mod admin_origin_tests;
