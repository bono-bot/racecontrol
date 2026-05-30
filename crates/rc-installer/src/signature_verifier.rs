//! The trust gate (spec §7). Every check fails closed.
//!
//! [`verify_manifest`] runs five ordered gates; the order is load-bearing and
//! pinned by `tests/manifest_signature.rs`:
//!   1. kid lookup            → `UnknownKid`
//!   2. key status            → `PlaceholderKid` / `RevokedKid` (only `active` proceeds)
//!   3. signing_key_id equality → `SigningKeyIdMismatch` (independent of the signature)
//!   4. signature hex/length  → `MalformedSignature`
//!   5. ed25519 `verify_strict` → `BadSignature`
//!
//! Gate 3 precedes signature verification deliberately: the mismatch test
//! re-signs the manifest so the signature is *valid*, so the equality invariant
//! can only be caught upstream of the signature check.

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::error::VerifyError;
use crate::manifest::{ReleaseArtifact, ReleaseManifest};
use crate::trusted_keys::TrustedKeySet;

/// Verify a release manifest against the embedded trusted-key set
/// (spec §7 steps 4–5). `Ok(())` only if every gate passes.
pub fn verify_manifest(
    manifest: &ReleaseManifest,
    trusted: &TrustedKeySet,
) -> Result<(), VerifyError> {
    // Gate 1 — kid must be present in the trust set.
    let key = trusted
        .find(&manifest.signing_key_id)
        .ok_or_else(|| VerifyError::UnknownKid(manifest.signing_key_id.clone()))?;

    // Gate 2 — only an `active` key proceeds. Any non-active status fails closed;
    // an unrecognized status is treated as the most conservative reject.
    match key.status.as_str() {
        "active" => {}
        "placeholder" => return Err(VerifyError::PlaceholderKid(manifest.signing_key_id.clone())),
        _ => return Err(VerifyError::RevokedKid(manifest.signing_key_id.clone())),
    }

    // Gate 3 — the installer artifact and the manifest must name the same key
    // (spec §4.3). Independent of the signature: one ed25519 key signs both.
    if manifest.signing_key_id != manifest.installer_artifact.signing_key_id {
        return Err(VerifyError::SigningKeyIdMismatch);
    }

    // Gate 4 — the signature must be 64 bytes of valid ed25519 hex.
    let sig_bytes = hex::decode(&manifest.signature).map_err(|_| VerifyError::MalformedSignature)?;
    let signature =
        Signature::try_from(sig_bytes.as_slice()).map_err(|_| VerifyError::MalformedSignature)?;

    // The trusted public key is 32-byte hex. A malformed/weak stored key fails
    // closed (the trusted-key set is ours; a bad key is a build error).
    let pk_bytes = hex::decode(&key.public_key).map_err(|_| VerifyError::BadSignature)?;
    let pk_arr: [u8; 32] = pk_bytes
        .as_slice()
        .try_into()
        .map_err(|_| VerifyError::BadSignature)?;
    let verifying_key = VerifyingKey::from_bytes(&pk_arr).map_err(|_| VerifyError::BadSignature)?;

    // Gate 5 — ed25519 over the canonical signed bytes. `verify_strict` rejects
    // small-order / torsion public keys (malleability-safe), the correct choice
    // for a trust boundary; do not relax to plain `verify`.
    let message = manifest
        .canonical_signed_bytes()
        .map_err(|_| VerifyError::BadSignature)?;
    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| VerifyError::BadSignature)?;

    Ok(())
}

/// Spec §7 step 6 — the bootstrapper self-verifies its own binary bytes against
/// the signed `installer_artifact.sha256` (the second factor beyond Authenticode).
pub fn verify_installer_sha(
    manifest: &ReleaseManifest,
    installer_bytes: &[u8],
) -> Result<(), VerifyError> {
    if sha256_hex(installer_bytes) == manifest.installer_artifact.sha256 {
        Ok(())
    } else {
        Err(VerifyError::InstallerArtifactShaMismatch)
    }
}

/// Spec §7 step 7 — a fetched payload must hash to its signed manifest entry
/// before it is staged.
pub fn verify_artifact_sha(
    artifact: &ReleaseArtifact,
    artifact_bytes: &[u8],
) -> Result<(), VerifyError> {
    if sha256_hex(artifact_bytes) == artifact.sha256 {
        Ok(())
    } else {
        Err(VerifyError::ArtifactShaMismatch {
            target: artifact.target.clone(),
        })
    }
}

/// Lowercase hex sha256 (matches the contract's `[0-9a-f]{64}` form).
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
