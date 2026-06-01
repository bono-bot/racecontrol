//! Verification errors (spec §8 operator-visible codes), mapped 1:1 to the
//! gates in [`crate::signature_verifier`]. The whole chain fails closed.
//!
//! `PartialEq` + `Debug` are required by the integration tests' `assert_eq!`;
//! `thiserror::Error` does not derive them, so they are listed explicitly.

use thiserror::Error;

/// A release-trust verification failure.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum VerifyError {
    /// `signing_key_id` is not present in the embedded trusted-key set.
    #[error("signing key id '{0}' is not in the trusted key set")]
    UnknownKid(String),

    /// The key exists but is a placeholder (the B8 key ceremony has not run).
    #[error("signing key id '{0}' is a placeholder (B8 key ceremony not run)")]
    PlaceholderKid(String),

    /// The key exists but has been revoked (or carries an unrecognized status).
    #[error("signing key id '{0}' is revoked")]
    RevokedKid(String),

    /// The signature field is not 64 bytes of valid ed25519 hex.
    #[error("manifest signature is malformed (expected 64-byte ed25519 hex)")]
    MalformedSignature,

    /// The signature did not verify against the trusted public key.
    #[error("manifest signature verification failed")]
    BadSignature,

    /// `installer_artifact.signing_key_id` != parent `signing_key_id`
    /// (spec §4.3 defense-in-depth — one key signs both per release).
    #[error("installer_artifact.signing_key_id does not match manifest signing_key_id")]
    SigningKeyIdMismatch,

    /// The bootstrapper's own bytes do not hash to `installer_artifact.sha256`.
    #[error("installer binary sha256 does not match installer_artifact.sha256")]
    InstallerArtifactShaMismatch,

    /// A fetched payload's bytes do not hash to its manifest entry.
    #[error("artifact '{target}' sha256 does not match the manifest")]
    ArtifactShaMismatch { target: String },
}

/// Error type for the INC-2H hardening cores (`tls_pin`, `host_allowlist`,
/// `journal`). Kept separate from [`VerifyError`] (signed-release trust) and the
/// transport/redeem enums — this is the installer-runtime hardening layer.
/// Fail-closed; `PartialEq`/`Eq` for `assert_eq!`/`matches!` in tests.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstallError {
    /// Network/transport failure (feature-gated callers).
    #[error("http error: {0}")]
    Http(String),
    /// JSON/TOML parse failure.
    #[error("parse error: {0}")]
    Parse(String),
    /// Filesystem error.
    #[error("io error: {0}")]
    Io(String),
    /// Malformed input (e.g. a pin that is not 32-byte base64).
    #[error("malformed: {0}")]
    Malformed(String),
    /// TLS SPKI pin mismatch — no presented cert matched a configured pin (INC-2H).
    #[error("tls pin mismatch: no presented cert matched a configured SPKI pin")]
    PinMismatch,
    /// Redeemed `server_address`/`bono_address` host not in the allowlist (INC-2H, G5).
    #[error("host not allowed: {0}")]
    HostNotAllowed(String),
    /// Install journal read/write/parse failure (INC-2H, G7).
    #[error("journal error: {0}")]
    Journal(String),
}

/// Convenience alias for the INC-2H hardening cores.
pub type Result<T> = std::result::Result<T, InstallError>;

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for InstallError {
    fn from(e: serde_json::Error) -> Self {
        InstallError::Parse(e.to_string())
    }
}
