//! spec §11 compliance — `manifest_signature.rs`.
//!
//! The verification gates (§7) fire in order; bypassing any one yields the
//! expected `VerifyError` variant. Uses a deterministic test signing key
//! (fixed seed) so the vectors are reproducible with no RNG.
//!
//! Data model reconciled to the canonical contract `release.ts` (2026-05-31):
//! ReleaseArtifact = {artifact_id, sha256, size_bytes, target} (no download_url);
//! cut_at = u64 Unix-ms; release_ring ∈ {canary, early, general}.

use ed25519_dalek::{Signer, SigningKey};
use rc_installer::error::VerifyError;
use rc_installer::manifest::{InstallerArtifact, ReleaseArtifact, ReleaseManifest};
use rc_installer::signature_verifier::{verify_artifact_sha, verify_installer_sha, verify_manifest};
use rc_installer::trusted_keys::{TrustedKey, TrustedKeySet};
use sha2::{Digest, Sha256};

const KID: &str = "rc-release-test-001";

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// A fully-valid, signed manifest + a trust set that trusts its key.
fn valid_fixture() -> (ReleaseManifest, TrustedKeySet) {
    let sk = test_signing_key();
    let pubkey_hex = hex::encode(sk.verifying_key().to_bytes());

    let installer_artifact = InstallerArtifact {
        sha256: sha256_hex(b"installer-binary-bytes"),
        size_bytes: 21,
        download_url: "https://console.racecontrol.in/install/bootstrapper".into(),
        signing_key_id: KID.into(),
    };
    let artifacts = vec![ReleaseArtifact {
        artifact_id: "rc-agent-2026Q2-001".into(),
        sha256: sha256_hex(b"rc-agent-payload"),
        size_bytes: 16,
        target: "rc-agent".into(),
    }];

    let mut manifest = ReleaseManifest {
        release_id: "rel-2026Q2-001".into(),
        release_class: "feature".into(),
        release_ring: "general".into(),
        artifacts,
        installer_artifact,
        previous_release_id: None,
        cut_at: 1_748_649_600_000,
        signing_key_id: KID.into(),
        signature: String::new(),
    };

    let bytes = manifest.canonical_signed_bytes().unwrap();
    manifest.signature = hex::encode(sk.sign(&bytes).to_bytes());

    let keys = TrustedKeySet {
        keys: vec![TrustedKey {
            kid: KID.into(),
            public_key: pubkey_hex,
            status: "active".into(),
        }],
    };
    (manifest, keys)
}

#[test]
fn valid_manifest_verifies() {
    let (m, k) = valid_fixture();
    assert_eq!(verify_manifest(&m, &k), Ok(()));
}

#[test]
fn unknown_kid_rejected() {
    let (m, _) = valid_fixture();
    let empty = TrustedKeySet { keys: vec![] };
    assert_eq!(
        verify_manifest(&m, &empty),
        Err(VerifyError::UnknownKid(KID.into()))
    );
}

#[test]
fn placeholder_kid_rejected() {
    let (m, mut k) = valid_fixture();
    k.keys[0].status = "placeholder".into();
    assert_eq!(
        verify_manifest(&m, &k),
        Err(VerifyError::PlaceholderKid(KID.into()))
    );
}

#[test]
fn revoked_kid_rejected() {
    let (m, mut k) = valid_fixture();
    k.keys[0].status = "revoked".into();
    assert_eq!(
        verify_manifest(&m, &k),
        Err(VerifyError::RevokedKid(KID.into()))
    );
}

#[test]
fn tampered_field_breaks_signature() {
    let (mut m, k) = valid_fixture();
    m.release_ring = "canary".into(); // signed field changed after signing
    assert_eq!(verify_manifest(&m, &k), Err(VerifyError::BadSignature));
}

#[test]
fn malformed_signature_rejected() {
    let (mut m, k) = valid_fixture();
    m.signature = "zznothex".into();
    assert_eq!(verify_manifest(&m, &k), Err(VerifyError::MalformedSignature));
}

#[test]
fn signing_key_id_mismatch_rejected() {
    // installer_artifact.signing_key_id diverges; re-sign so the signature
    // itself is valid — proving the equality invariant is an INDEPENDENT gate
    // (defense-in-depth §4.3), not just a side effect of signature failure.
    let sk = test_signing_key();
    let (mut m, k) = valid_fixture();
    m.installer_artifact.signing_key_id = "some-other-kid".into();
    let bytes = m.canonical_signed_bytes().unwrap();
    m.signature = hex::encode(sk.sign(&bytes).to_bytes());
    assert_eq!(verify_manifest(&m, &k), Err(VerifyError::SigningKeyIdMismatch));
}

#[test]
fn installer_sha_mismatch_rejected() {
    let (m, _) = valid_fixture();
    assert_eq!(
        verify_installer_sha(&m, b"WRONG-installer-bytes"),
        Err(VerifyError::InstallerArtifactShaMismatch)
    );
}

#[test]
fn installer_sha_match_ok() {
    let (m, _) = valid_fixture();
    assert_eq!(verify_installer_sha(&m, b"installer-binary-bytes"), Ok(()));
}

#[test]
fn artifact_sha_mismatch_rejected() {
    let (m, _) = valid_fixture();
    assert_eq!(
        verify_artifact_sha(&m.artifacts[0], b"WRONG"),
        Err(VerifyError::ArtifactShaMismatch {
            target: "rc-agent".into()
        })
    );
}

#[test]
fn artifact_sha_match_ok() {
    let (m, _) = valid_fixture();
    assert_eq!(
        verify_artifact_sha(&m.artifacts[0], b"rc-agent-payload"),
        Ok(())
    );
}

#[test]
fn wrong_length_signature_rejected() {
    // Valid hex, but NOT 64 bytes (32 bytes = 64 hex chars). Exercises the
    // Signature::try_from length gate — distinct from the hex-decode gate that
    // `malformed_signature_rejected` covers. (Active kid + consistent ids so
    // verification reaches gate 4.)
    let (mut m, k) = valid_fixture();
    m.signature = "00".repeat(32);
    assert_eq!(verify_manifest(&m, &k), Err(VerifyError::MalformedSignature));
}

#[test]
fn unrecognized_status_rejected_fail_closed() {
    // A status outside {active, placeholder, revoked} MUST fail closed. The
    // catch-all maps to RevokedKid (most conservative reject); this locks the
    // fail-closed behavior of the wildcard arm against future regression.
    let (m, mut k) = valid_fixture();
    k.keys[0].status = "suspended".into();
    assert_eq!(
        verify_manifest(&m, &k),
        Err(VerifyError::RevokedKid(KID.into()))
    );
}
