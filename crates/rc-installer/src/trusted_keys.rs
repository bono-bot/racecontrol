//! Embedded trusted ed25519 public keys (spec §4.4).
//!
//! Shipped inside the bootstrapper at compile time (public keys only). The
//! lookup is fail-closed: a missing kid yields no key, and a non-`active`
//! status is rejected by [`crate::signature_verifier::verify_manifest`].

use serde::{Deserialize, Serialize};

/// One trusted signing key. `status` ∈ {`active`, `placeholder`, `revoked`}.
/// `placeholder` is the pre-launch posture until the B8 key ceremony issues a
/// real key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub kid: String,
    /// ed25519 public key, 32-byte hex.
    pub public_key: String,
    pub status: String,
}

/// The set of trusted keys shipped in the bootstrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKeySet {
    pub keys: Vec<TrustedKey>,
}

impl TrustedKeySet {
    /// Find a key by id. `None` → the caller fails closed with `UnknownKid`.
    pub fn find(&self, kid: &str) -> Option<&TrustedKey> {
        self.keys.iter().find(|k| k.kid == kid)
    }
}
