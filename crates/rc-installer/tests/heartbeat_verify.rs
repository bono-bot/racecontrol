//! INC-7a — `verify_heartbeat` gate coverage (per the MMA Step 1 DIAGNOSE findings,
//! RCA-INC7A-heartbeat-verifier-20260601.md). Drives the verifier with the committed
//! ceremony-signed production vector (`license-heartbeat-golden.json`, signed by the
//! test key `rc-hb-test-001`) and a context, then exercises each gate's reject path.
//!
//! The positive test also proves enum<->string canonicalization parity: the verifier
//! serializes `license_class` as a `LicenseClass` enum; the signature was made over the
//! generator's string form — if they produced different bytes, verify would fail.

use base64::Engine;
use rc_installer::fingerprint::machine_fingerprint_composite;
use rc_installer::heartbeat::{verify_heartbeat, HeartbeatError, LicenseClass, LicenseHeartbeat, SignedLicenseHeartbeat, VerifyContext};
use rc_installer::trusted_keys::{TrustedKey, TrustedKeySet};
use std::collections::BTreeMap;

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

/// End-to-end against Replit's PRODUCTION-signed sample (INC-7b contract, 2026-06-01):
/// real ed25519 signature by the production key `lic-sign-2026-06-1` over the canonical
/// bytes, with `machine_fingerprint` = the GV1 composite. The envelope is the agreed
/// schema (signature is a SIBLING of `license_heartbeat`) — NOT the mis-rendered sample
/// that nested `signature` inside the payload. Proves the shipped verifier + the composite
/// recipe accept a real production heartbeat with Gate-4 byte-match.
#[test]
fn production_signed_sample_verifies_with_prod_key() {
    let mut feature_opt_in = BTreeMap::new();
    feature_opt_in.insert("multiplayer".to_string(), true);
    feature_opt_in.insert("telemetry".to_string(), false);
    let signed = SignedLicenseHeartbeat {
        license_heartbeat: LicenseHeartbeat {
            tenant_id: "rp-hyd".into(),
            machine_fingerprint: "4d81ea8f4ab8be575a087222ffc13cc2cedb8a8f521981758d64e4a3c1b69312".into(),
            issued_at: 1748649600000,
            valid_until: 1748653200000,
            signing_key_id: "lic-sign-2026-06-1".into(),
            license_class: LicenseClass::Production,
            feature_opt_in,
        },
        signature: "OqNmmWIFe74W5wWsIE12eEvgoxlxVN5oBiB81YZpo1OdzDhBZ9tWX/4Z6ESYiwpRRNW5822dxP41u5R2UlyxBA==".into(),
        next_refresh_after: 1748652300000,
    };
    let ks = TrustedKeySet {
        keys: vec![TrustedKey {
            kid: "lic-sign-2026-06-1".into(),
            public_key: "51f0bf799b31aa7e80d7b2714450a33fcb59219a69a746c25c55bb7b21da9340".into(),
            status: "active".into(),
        }],
    };
    // The client recomputes the composite from the GV1 hardware components — Gate-4 input.
    let local_fp = machine_fingerprint_composite(
        "d3b07384-d9a4-4f1e-8c2a-1b6f5e0a7c91",
        Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        "S3PWNF0M812345",
    );
    assert_eq!(local_fp, "4d81ea8f4ab8be575a087222ffc13cc2cedb8a8f521981758d64e4a3c1b69312", "composite must equal GV1 (Gate-4 pre-image)");
    let ctx = VerifyContext {
        trusted_keys: &ks,
        local_machine_fingerprint: &local_fp,
        expected_tenant_id: "rp-hyd",
        now_ms: 1748649600000 + 60_000,
        max_ttl_ms: 5_400_000,
        clock_skew_ms: 300_000,
        min_valid_until_ms: None,
    };
    assert_eq!(verify_heartbeat(&signed, &ctx), Ok(()));
}
