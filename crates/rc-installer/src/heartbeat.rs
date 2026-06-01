//! License-heartbeat trust gate (INC-7a). PURE verifier — no network/DB/state.
//!
//! A tier-1 console signs a `LicenseHeartbeat` (ed25519) and returns a
//! [`SignedLicenseHeartbeat`]. The installer/agent embeds the trusted B9 public
//! key(s) and calls [`verify_heartbeat`] on every heartbeat. All runtime inputs
//! (the local machine fingerprint, the expected tenant, `now`, the anti-rollback
//! floor, the trusted-key set) arrive via [`VerifyContext`], so this module is
//! pure and fully unit-testable; the platform fingerprint computation, the HTTPS
//! fetch/refresh, anti-rollback persistence, offline grace, rate-limiting, and
//! entitlement policy are INC-7b (gated).
//!
//! Canonical signed bytes mirror the Captain-pinned recipe (`packages/contracts`
//! `license.yaml`/`license.ts`): compact JSON, struct-declaration order,
//! `signature` excluded, `feature_opt_in` an OPEN map with keys sorted, timestamps
//! Unix-ms. Cross-language byte-parity is locked by
//! `tests/canonical_heartbeat_golden_vector.rs`.
//!
//! Design gate: `.planning/specs/v2/rc-installer/RCA-INC7A-heartbeat-verifier-20260601.md`
//! (MMA Step 1 DIAGNOSE, 4/5 models / 4 vendor families). The gate order below is
//! load-bearing: authenticate (kid → status → signature) BEFORE authorizing
//! (fingerprint → tenant → time → rollback), and FAIL CLOSED on every error.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::trusted_keys::TrustedKeySet;

/// Closed license-class enum. An unknown value is a deserialize error (fail-closed)
/// — it never defaults to a privileged class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseClass {
    Production,
    Trial,
    Demo,
}

/// The signed payload (7 fields, declaration order). `signature` is NOT here — it
/// is excluded from the signed bytes and carried by [`SignedLicenseHeartbeat`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseHeartbeat {
    pub tenant_id: String,
    pub machine_fingerprint: String,
    pub issued_at: u64,
    pub valid_until: u64,
    pub signing_key_id: String,
    pub license_class: LicenseClass,
    /// OPEN map; canonicalizes with SORTED keys (BTreeMap), always present.
    pub feature_opt_in: BTreeMap<String, bool>,
}

/// The 200-body envelope. `signature` is base64 (per the pinned wire format);
/// `next_refresh_after` steers the client poll cadence and is NOT signed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedLicenseHeartbeat {
    pub license_heartbeat: LicenseHeartbeat,
    /// ed25519 signature, base64. EXCLUDED from the signed bytes.
    pub signature: String,
    pub next_refresh_after: u64,
}

/// Borrowing view that serializes the signed payload in declaration order — the
/// exact bytes the signature covers. (Same pattern as `manifest::CanonicalManifest`:
/// not `serde_json::Value`, which would alphabetize the top-level keys.)
#[derive(Serialize)]
struct CanonicalHeartbeat<'a> {
    tenant_id: &'a str,
    machine_fingerprint: &'a str,
    issued_at: u64,
    valid_until: u64,
    signing_key_id: &'a str,
    license_class: LicenseClass,
    feature_opt_in: &'a BTreeMap<String, bool>,
}

impl LicenseHeartbeat {
    /// The exact bytes the ed25519 signature covers: the 7 fields, compact JSON,
    /// declaration order. MUST stay byte-identical to the `packages/contracts`
    /// canonicalizer (locked by the golden vectors).
    pub fn canonical_signed_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&CanonicalHeartbeat {
            tenant_id: &self.tenant_id,
            machine_fingerprint: &self.machine_fingerprint,
            issued_at: self.issued_at,
            valid_until: self.valid_until,
            signing_key_id: &self.signing_key_id,
            license_class: self.license_class,
            feature_opt_in: &self.feature_opt_in,
        })
    }
}

/// Every runtime input the verifier needs. Pure: the caller (INC-7b) supplies the
/// locally-computed fingerprint, the expected tenant, the current time, the
/// anti-rollback floor, and the trusted-key set.
pub struct VerifyContext<'a> {
    pub trusted_keys: &'a TrustedKeySet,
    /// The fingerprint computed from THIS machine's hardware (7b).
    pub local_machine_fingerprint: &'a str,
    /// The tenant this installer is provisioned/claiming for.
    pub expected_tenant_id: &'a str,
    /// Current Unix-ms (caller's clock policy).
    pub now_ms: u64,
    /// Max allowed `valid_until - issued_at` (e.g. ~90 min).
    pub max_ttl_ms: u64,
    /// Allowed clock skew both directions (e.g. 5 min).
    pub clock_skew_ms: u64,
    /// Anti-rollback floor: reject a heartbeat whose `valid_until` is older than the
    /// newest previously accepted (7b persists this). `None` on first acceptance.
    pub min_valid_until_ms: Option<u64>,
}

/// Verification failure. Every variant denies access (fail-closed).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HeartbeatError {
    #[error("unknown signing_key_id: {0}")]
    UnknownKid(String),
    #[error("placeholder signing_key_id (unprovisioned): {0}")]
    PlaceholderKid(String),
    #[error("revoked/inactive signing_key_id: {0}")]
    RevokedKid(String),
    #[error("malformed signature")]
    MalformedSignature,
    #[error("bad signature")]
    BadSignature,
    #[error("machine_fingerprint does not match this machine")]
    FingerprintMismatch,
    #[error("tenant_id mismatch")]
    TenantMismatch,
    #[error("invalid validity window (valid_until <= issued_at)")]
    InvalidWindow,
    #[error("TTL exceeds the maximum allowed")]
    TtlTooLong,
    #[error("issued_at is in the future beyond allowed skew")]
    NotYetValid,
    #[error("heartbeat expired")]
    Expired,
    #[error("rollback: valid_until older than the last accepted heartbeat")]
    Rollback,
}

/// Verify a signed heartbeat against the trusted keys + runtime context.
/// `Ok(())` only if EVERY ordered gate passes. Authenticate first, then authorize.
pub fn verify_heartbeat(
    signed: &SignedLicenseHeartbeat,
    ctx: &VerifyContext,
) -> Result<(), HeartbeatError> {
    let hb = &signed.license_heartbeat;

    // Gate 1 — kid must be present in the pinned trust set (no try-all-keys).
    let key = ctx
        .trusted_keys
        .find(&hb.signing_key_id)
        .ok_or_else(|| HeartbeatError::UnknownKid(hb.signing_key_id.clone()))?;

    // Gate 2 — only an `active` key proceeds (revocation via trust-store status).
    match key.status.as_str() {
        "active" => {}
        "placeholder" => return Err(HeartbeatError::PlaceholderKid(hb.signing_key_id.clone())),
        _ => return Err(HeartbeatError::RevokedKid(hb.signing_key_id.clone())),
    }

    // Gate 3 — ed25519 (only) over the canonical bytes, verified with THE kid's key.
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signed.signature.as_bytes())
        .map_err(|_| HeartbeatError::MalformedSignature)?;
    let signature =
        Signature::try_from(sig_bytes.as_slice()).map_err(|_| HeartbeatError::MalformedSignature)?;
    let pk_bytes = hex::decode(&key.public_key).map_err(|_| HeartbeatError::BadSignature)?;
    let pk_arr: [u8; 32] = pk_bytes
        .as_slice()
        .try_into()
        .map_err(|_| HeartbeatError::BadSignature)?;
    let verifying_key =
        VerifyingKey::from_bytes(&pk_arr).map_err(|_| HeartbeatError::BadSignature)?;
    let message = hb
        .canonical_signed_bytes()
        .map_err(|_| HeartbeatError::BadSignature)?;
    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| HeartbeatError::BadSignature)?;

    // --- payload is now authentic; authorize it ---

    // Gate 4 — machine binding: a valid signature is NOT enough (MMA #1). The
    // heartbeat must be for THIS machine, else it is a transferable token.
    if hb.machine_fingerprint != ctx.local_machine_fingerprint {
        return Err(HeartbeatError::FingerprintMismatch);
    }

    // Gate 5 — tenant binding.
    if hb.tenant_id != ctx.expected_tenant_id {
        return Err(HeartbeatError::TenantMismatch);
    }

    // Gate 6 — temporal. Gate on `valid_until` only; never trust issued_at for
    // validity. Reject inverted/over-long windows + future issuance + expiry.
    if hb.valid_until <= hb.issued_at {
        return Err(HeartbeatError::InvalidWindow);
    }
    if hb.valid_until - hb.issued_at > ctx.max_ttl_ms {
        return Err(HeartbeatError::TtlTooLong);
    }
    if hb.issued_at > ctx.now_ms.saturating_add(ctx.clock_skew_ms) {
        return Err(HeartbeatError::NotYetValid);
    }
    if ctx.now_ms > hb.valid_until.saturating_add(ctx.clock_skew_ms) {
        return Err(HeartbeatError::Expired);
    }

    // Gate 7 — anti-rollback: reject a heartbeat older than the newest accepted.
    if ctx.min_valid_until_ms.is_some_and(|min| hb.valid_until < min) {
        return Err(HeartbeatError::Rollback);
    }

    Ok(())
}
