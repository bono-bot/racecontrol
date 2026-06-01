//! INC-7a — `verify_heartbeat` gate coverage (per the MMA Step 1 DIAGNOSE findings,
//! RCA-INC7A-heartbeat-verifier-20260601.md). Drives the verifier with the committed
//! ceremony-signed production vector (`license-heartbeat-golden.json`, signed by the
//! test key `rc-hb-test-001`) and a context, then exercises each gate's reject path.
//!
//! The positive test also proves enum<->string canonicalization parity: the verifier
//! serializes `license_class` as a `LicenseClass` enum; the signature was made over the
//! generator's string form — if they produced different bytes, verify would fail.

use base64::Engine;
use rc_installer::heartbeat::{verify_heartbeat, HeartbeatError, LicenseHeartbeat, SignedLicenseHeartbeat, VerifyContext};
use rc_installer::trusted_keys::{TrustedKey, TrustedKeySet};

const GOLDEN: &str = include_str!("golden/license-heartbeat-golden.json");

struct Fix {
    signed: SignedLicenseHeartbeat,
    pubkey: String,
}

/// Build a `SignedLicenseHeartbeat` from the committed production vector: the
/// fixture stores the signature as hex, the wire/envelope uses base64, so convert.
fn load() -> Fix {
    let f: serde_json::Value = serde_json::from_str(GOLDEN).expect("parse fixture");
    let v = f["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["license_heartbeat"]["license_class"] == "production")
        .expect("production vector");
    let hb: LicenseHeartbeat = serde_json::from_value(v["license_heartbeat"].clone()).expect("hb deserializes");
    let sig_bytes = hex::decode(v["signature_hex"].as_str().unwrap()).unwrap();
    let signature = base64::engine::general_purpose::STANDARD.encode(sig_bytes);
    Fix {
        signed: SignedLicenseHeartbeat {
            license_heartbeat: hb,
            signature,
            next_refresh_after: f["next_refresh_after"].as_u64().unwrap(),
        },
        pubkey: f["vector_signing_public_key_hex"].as_str().unwrap().to_string(),
    }
}

fn keyset(pubkey: &str, status: &str) -> TrustedKeySet {
    TrustedKeySet { keys: vec![TrustedKey { kid: "rc-hb-test-001".into(), public_key: pubkey.into(), status: status.into() }] }
}

#[test]
fn valid_production_heartbeat_verifies() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000, // 90 min
        clock_skew_ms: 300_000, // 5 min
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Ok(()));
}

#[test]
fn wrong_machine_fingerprint_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::FingerprintMismatch));
}

#[test]
fn wrong_tenant_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: "rp-someone-else",
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::TenantMismatch));
}

#[test]
fn expired_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.valid_until + 300_000 + 1, // past valid_until + skew
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::Expired));
}

#[test]
fn future_issued_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at - 300_000 - 1, // before issued_at - skew
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::NotYetValid));
}

#[test]
fn ttl_too_long_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 1_000, // real TTL (3_600_000) exceeds this
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::TtlTooLong));
}

#[test]
fn rollback_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: Some(hb.valid_until + 1), // newer one already accepted
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::Rollback));
}

#[test]
fn unknown_kid_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = TrustedKeySet { keys: vec![] }; // kid not present
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::UnknownKid("rc-hb-test-001".into())));
}

#[test]
fn revoked_kid_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "revoked");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::RevokedKid("rc-hb-test-001".into())));
}

#[test]
fn placeholder_kid_rejected() {
    let fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    let ks = keyset(&fx.pubkey, "placeholder");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::PlaceholderKid("rc-hb-test-001".into())));
}

#[test]
fn tampered_payload_rejected() {
    let mut fx = load();
    let real_fp = fx.signed.license_heartbeat.machine_fingerprint.clone();
    let real_tenant = fx.signed.license_heartbeat.tenant_id.clone();
    let issued = fx.signed.license_heartbeat.issued_at;
    fx.signed.license_heartbeat.valid_until += 1; // mutate a signed field
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &real_fp,
        expected_tenant_id: &real_tenant,
        now_ms: issued + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    // signature gate (Gate 3) fires before the temporal gates.
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::BadSignature));
}

#[test]
fn malformed_signature_rejected() {
    let mut fx = load();
    let hb = fx.signed.license_heartbeat.clone();
    fx.signed.signature = "@@@ not base64 @@@".into();
    let ks = keyset(&fx.pubkey, "active");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &hb.machine_fingerprint,
        expected_tenant_id: &hb.tenant_id,
        now_ms: hb.issued_at + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&fx.signed, &ctx), Err(HeartbeatError::MalformedSignature));
}
