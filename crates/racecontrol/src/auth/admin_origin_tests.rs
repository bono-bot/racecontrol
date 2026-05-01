//! Tests for admin_origin middleware substrate (AMEND-1 §1).
//!
//! @cite_pact AMEND-1-V2-SKELETON-bundle-of-8

use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

const TEST_ADMIN: &str = "venue-admin";
const TEST_SECRET: &str = "test-secret-do-not-use-in-prod";
const TEST_NONCE: &str = "AAECAwQFBgcICQoLDA0ODw=="; // 16 bytes base64

fn fresh_cache() -> Mutex<NonceCache> {
    Mutex::new(NonceCache::new())
}

fn fresh_secrets() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(TEST_ADMIN.to_string(), TEST_SECRET.to_string());
    m
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn make_signed_headers(
    admin_id: &str,
    secret: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
    nonce: &str,
    timestamp: &str,
) -> (String, String, String, String) {
    let canonical = canonical_string(admin_id, method, path, nonce, timestamp, body);
    let sig = sign(secret, &canonical);
    (admin_id.to_string(), nonce.to_string(), timestamp.to_string(), sig)
}

#[test]
fn mode_parses_disabled_default() {
    assert_eq!(AdminOriginMode::from_config_str(""), AdminOriginMode::Disabled);
    assert_eq!(AdminOriginMode::from_config_str("disabled"), AdminOriginMode::Disabled);
    assert_eq!(AdminOriginMode::from_config_str("garbage"), AdminOriginMode::Disabled);
}

#[test]
fn mode_parses_log_only_variants() {
    assert_eq!(AdminOriginMode::from_config_str("log-only"), AdminOriginMode::LogOnly);
    assert_eq!(AdminOriginMode::from_config_str("log_only"), AdminOriginMode::LogOnly);
    assert_eq!(AdminOriginMode::from_config_str("LogOnly"), AdminOriginMode::LogOnly);
}

#[test]
fn mode_parses_enforce() {
    assert_eq!(AdminOriginMode::from_config_str("enforce"), AdminOriginMode::Enforce);
    assert_eq!(AdminOriginMode::from_config_str("ENFORCE"), AdminOriginMode::Enforce);
}

#[test]
fn canonical_string_is_deterministic() {
    let body = b"{\"foo\":\"bar\"}";
    let a = canonical_string(TEST_ADMIN, "POST", "/api/v1/x", TEST_NONCE, "2026-05-01T00:00:00Z", body);
    let b = canonical_string(TEST_ADMIN, "POST", "/api/v1/x", TEST_NONCE, "2026-05-01T00:00:00Z", body);
    assert_eq!(a, b);
    // Different body → different canonical
    let c = canonical_string(TEST_ADMIN, "POST", "/api/v1/x", TEST_NONCE, "2026-05-01T00:00:00Z", b"other");
    assert_ne!(a, c);
}

#[test]
fn sign_then_verify_roundtrip_passes() {
    let body = b"{\"action\":\"write\"}";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/api/v1/billing/start", body, TEST_NONCE, &ts,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin),
        nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str),
        signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/api/v1/billing/start", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert!(outcome.is_valid(), "expected Valid, got {:?}", outcome);
}

#[test]
fn missing_all_headers_returns_missing_outcome() {
    let headers = AdminOriginHeaders {
        admin_id: None, nonce_b64: None, timestamp_rfc3339: None, signature_b64: None,
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/api/v1/x", b"", &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    match outcome {
        VerifyOutcome::MissingHeaders { missing } => {
            assert_eq!(missing.len(), 4);
            assert!(missing.contains(&"X-Origin-Admin"));
            assert!(missing.contains(&"X-Origin-Nonce"));
            assert!(missing.contains(&"X-Origin-Timestamp"));
            assert!(missing.contains(&"X-Origin-Signature"));
        }
        _ => panic!("expected MissingHeaders, got {:?}", outcome),
    }
}

#[test]
fn unknown_admin_id_rejected() {
    let body = b"";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        "unknown-admin", TEST_SECRET.as_bytes(), "POST", "/x", body, TEST_NONCE, &ts,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "unknown-admin");
}

#[test]
fn timestamp_skew_rejected() {
    let body = b"";
    let stale_ts = (chrono::Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/x", body, TEST_NONCE, &stale_ts,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "timestamp-skew");
}

#[test]
fn signature_mismatch_rejected() {
    let body = b"";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, _sig) = make_signed_headers(
        TEST_ADMIN, b"WRONG-SECRET", "POST", "/x", body, TEST_NONCE, &ts,
    );
    let bad_sig = sign(b"WRONG-SECRET", &canonical_string(&admin, "POST", "/x", &nonce, &ts_str, body));
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&bad_sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "signature-mismatch");
}

#[test]
fn body_tampering_breaks_signature() {
    let original_body = b"{\"amount\":100}";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/x", original_body, TEST_NONCE, &ts,
    );
    let tampered_body = b"{\"amount\":9999}";
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", tampered_body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "signature-mismatch");
}

#[test]
fn replay_attack_rejected_on_second_use() {
    let body = b"";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/x", body, TEST_NONCE, &ts,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let secrets = fresh_secrets();
    // First request: passes
    let first = verify(&headers, "POST", "/x", body, &secrets, DEFAULT_MAX_SKEW_SECS, &cache);
    assert!(first.is_valid(), "first request should pass: {:?}", first);
    // Second request with same nonce: rejected
    let second = verify(&headers, "POST", "/x", body, &secrets, DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(second.fail_reason(), "nonce-replay");
}

#[test]
fn malformed_signature_base64_rejected() {
    let ts = now_rfc3339();
    let headers = AdminOriginHeaders {
        admin_id: Some(TEST_ADMIN), nonce_b64: Some(TEST_NONCE),
        timestamp_rfc3339: Some(&ts), signature_b64: Some("NOT@VALID@BASE64!!!"),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", b"", &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "malformed-signature");
}

#[test]
fn nonce_cache_prunes_expired_entries() {
    let mut cache = NonceCache::new();
    let short_window = Duration::from_millis(50);
    assert!(!cache.check_and_insert("a", "n1", short_window));
    assert_eq!(cache.len(), 1);
    std::thread::sleep(Duration::from_millis(80));
    // After window elapses, prune triggers on next call → n1 evicted
    assert!(!cache.check_and_insert("a", "n2", short_window));
    assert_eq!(cache.len(), 1, "n1 should have been pruned");
}

#[test]
fn different_paths_with_same_nonce_both_rejected_after_first() {
    // Same admin+nonce on two different paths — replay defense is per (admin,nonce)
    // not per (admin,nonce,path). Second call must reject regardless of path.
    let body = b"";
    let ts = now_rfc3339();
    let (admin, nonce, ts_str, _sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/a", body, TEST_NONCE, &ts,
    );
    let cache = fresh_cache();
    let secrets = fresh_secrets();

    // First request on /a — sign for /a
    let canonical_a = canonical_string(&admin, "POST", "/a", &nonce, &ts_str, body);
    let sig_a = sign(TEST_SECRET.as_bytes(), &canonical_a);
    let headers_a = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig_a),
    };
    let first = verify(&headers_a, "POST", "/a", body, &secrets, DEFAULT_MAX_SKEW_SECS, &cache);
    assert!(first.is_valid());

    // Second request — same nonce, different path, properly signed for /b
    let canonical_b = canonical_string(&admin, "POST", "/b", &nonce, &ts_str, body);
    let sig_b = sign(TEST_SECRET.as_bytes(), &canonical_b);
    let headers_b = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig_b),
    };
    let second = verify(&headers_b, "POST", "/b", body, &secrets, DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(second.fail_reason(), "nonce-replay");
}

#[test]
fn future_timestamp_within_skew_accepted() {
    let body = b"";
    let near_future = (chrono::Utc::now() + chrono::Duration::seconds(5)).to_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/x", body, TEST_NONCE, &near_future,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert!(outcome.is_valid(), "5s future should pass with default skew, got {:?}", outcome);
}

#[test]
fn far_future_timestamp_rejected() {
    let body = b"";
    let far_future = (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc3339();
    let (admin, nonce, ts_str, sig) = make_signed_headers(
        TEST_ADMIN, TEST_SECRET.as_bytes(), "POST", "/x", body, TEST_NONCE, &far_future,
    );
    let headers = AdminOriginHeaders {
        admin_id: Some(&admin), nonce_b64: Some(&nonce),
        timestamp_rfc3339: Some(&ts_str), signature_b64: Some(&sig),
    };
    let cache = fresh_cache();
    let outcome = verify(&headers, "POST", "/x", body, &fresh_secrets(), DEFAULT_MAX_SKEW_SECS, &cache);
    assert_eq!(outcome.fail_reason(), "timestamp-skew");
}
