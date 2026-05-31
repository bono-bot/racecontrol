//! INC-3a — fetch the release manifest and VERIFY it before returning.
//!
//! The fetch logic is pure: it takes a [`Fetcher`] transport seam, so the
//! "verify the signature before the caller ever sees the manifest" invariant is
//! unit-tested on Linux with a mock — no network. The concrete HTTP transport
//! ([`crate::http_fetch::UreqFetcher`]) is feature-gated; this module is not.
//!
//! Invariant (spec §7): [`fetch_manifest`] returns `Ok` ONLY after
//! [`verify_manifest`] passes. A placeholder/unknown/revoked kid, a tampered
//! field, or a bad signature all fail closed here — nothing downstream runs.

use thiserror::Error;

use crate::error::VerifyError;
use crate::manifest::ReleaseManifest;
use crate::signature_verifier::verify_manifest;
use crate::trusted_keys::TrustedKeySet;

/// Transport seam: GET a URL → `(http_status, body_bytes)`. `Err` is reserved
/// for failures that never produced an HTTP status (DNS/connect/TLS/timeout);
/// a non-2xx response is `Ok((status, body))` so the caller maps it uniformly.
pub trait Fetcher {
    fn get(&self, url: &str) -> Result<(u16, Vec<u8>), String>;
}

/// A fetch/stage failure. `PartialEq` (not `Eq`) so tests can `assert_eq!`;
/// `Eq` is omitted because the wrapped [`VerifyError`] is only `PartialEq`.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum FetchError {
    /// No HTTP status (DNS / connect / TLS / timeout).
    #[error("fetch transport error: {0}")]
    Transport(String),

    /// A non-2xx HTTP status from the control plane.
    #[error("fetch failed with HTTP status {status}")]
    Http { status: u16 },

    /// The body was not valid UTF-8 / not valid manifest JSON.
    #[error("manifest parse error: {0}")]
    Parse(String),

    /// A trust gate rejected the manifest (fail-closed; nothing is returned).
    #[error("manifest verification failed: {0}")]
    Verify(VerifyError),

    /// A URL is not a child of the locked `…/install/` prefix (R-129).
    #[error("url '{0}' is not under the locked install prefix")]
    UrlPrefix(String),

    /// A fetched artifact's bytes did not hash to its signed manifest entry.
    #[error("artifact verification failed: {0}")]
    Artifact(VerifyError),

    /// Disk staging failed (write / rename / mkdir).
    #[error("artifact staging io error: {0}")]
    Io(String),
}

/// The release-manifest URL under a control-plane base.
pub fn manifest_url(base: &str) -> String {
    format!("{}/install/release-manifest.json", base.trim_end_matches('/'))
}

/// GET `{base}/install/release-manifest.json`, parse it, and **verify it before
/// returning**. The returned manifest is trust-checked; an unverified manifest
/// is never handed to the caller.
pub fn fetch_manifest(
    fetcher: &dyn Fetcher,
    base: &str,
    trusted: &TrustedKeySet,
) -> Result<ReleaseManifest, FetchError> {
    let url = manifest_url(base);
    let (status, body) = fetcher.get(&url).map_err(FetchError::Transport)?;
    if !(200..300).contains(&status) {
        return Err(FetchError::Http { status });
    }
    let text = String::from_utf8(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let manifest = ReleaseManifest::from_json(&text).map_err(|e| FetchError::Parse(e.to_string()))?;

    // The gate. Ok only if every signature/kid/sha check passes.
    verify_manifest(&manifest, trusted).map_err(FetchError::Verify)?;
    Ok(manifest)
}

/// Defense-in-depth (R-129): the manifest's `installer_artifact.download_url`
/// MUST be a child of the locked `…/install/` prefix. Callable independently of
/// the signature gate.
pub fn check_installer_download_url(manifest: &ReleaseManifest) -> Result<(), FetchError> {
    let url = &manifest.installer_artifact.download_url;
    if url.starts_with(crate::config::INSTALLER_DOWNLOAD_URL_PREFIX) {
        Ok(())
    } else {
        Err(FetchError::UrlPrefix(url.clone()))
    }
}
