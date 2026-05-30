//! Release-manifest types.
//!
//! Canonical source of truth: `rp-v2-apps/packages/contracts/src/release.ts`
//! (reconciled 2026-05-31, Captain "Reconcile now"). Field DECLARATION ORDER
//! here mirrors the contract and IS the canonical signed-bytes order; the
//! `signature` field is excluded from the signed bytes.
//!
//! Scope: this is structural (shape + types). Value-constraints the TS contract
//! enforces via zod (`release_ring` enum membership, `artifacts.min(1)`,
//! `sha256` hex regex, `download_url` prefix) are a higher validation layer and
//! are NOT enforced here. Byte-parity with the TS signer is gated on the
//! canonicalization-format SEAM (see the RCA) and is not exercised this
//! increment (placeholder keys fail closed before any real signature).

use serde::{Deserialize, Serialize};

/// One signed payload in a release (contract: `ReleaseArtifact`).
///
/// Carries `artifact_id`, NOT a per-artifact `download_url` — the CDN URL is
/// derived from the tier-1 base + `artifact_id`. (The bootstrapper's own URL
/// lives on [`InstallerArtifact`] instead.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifact {
    pub artifact_id: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub target: String,
}

/// The installer bootstrapper's own artifact (contract: `InstallerArtifact`).
///
/// `download_url` MUST be a child of `https://console.racecontrol.app/install/`
/// (R-129; enforced at the validation layer, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallerArtifact {
    pub sha256: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub signing_key_id: String,
}

/// A signed release manifest (contract: `ReleaseManifest`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub release_id: String,
    pub release_class: String,
    pub release_ring: String,
    pub artifacts: Vec<ReleaseArtifact>,
    pub installer_artifact: InstallerArtifact,
    pub previous_release_id: Option<String>,
    /// Unix milliseconds (contract `cut_at: number().int().nonnegative()`).
    pub cut_at: u64,
    pub signing_key_id: String,
    /// ed25519 signature, hex. EXCLUDED from [`ReleaseManifest::canonical_signed_bytes`].
    pub signature: String,
}

/// Borrowing view that serializes every [`ReleaseManifest`] field EXCEPT
/// `signature`, in declaration order. Used only to produce the bytes the
/// ed25519 signature covers.
///
/// Why a dedicated struct (not `#[serde(skip)]` on `signature`, not a
/// `serde_json::Value`): `skip` would also drop `signature` on *deserialize*,
/// breaking `from_json` round-trips; a `Value` is backed by a `BTreeMap` and
/// would alphabetize keys, silently breaking the canonical field order. A
/// borrowing struct preserves declaration order and leaves `from_json` intact.
#[derive(Serialize)]
struct CanonicalManifest<'a> {
    release_id: &'a str,
    release_class: &'a str,
    release_ring: &'a str,
    artifacts: &'a [ReleaseArtifact],
    installer_artifact: &'a InstallerArtifact,
    previous_release_id: &'a Option<String>,
    cut_at: u64,
    signing_key_id: &'a str,
}

impl ReleaseManifest {
    /// Parse a manifest from JSON (structural; rejects unknown fields).
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Always 1: a manifest carries exactly one `installer_artifact` (R-129
    /// single-installer-artifact-per-manifest, enforced by the type — the field
    /// is a single struct, not a collection).
    pub fn installer_artifact_count(&self) -> usize {
        1
    }

    /// The exact bytes the ed25519 signature covers: all fields except
    /// `signature`, compact JSON (`serde_json::to_vec`), struct-declaration
    /// order. MUST stay byte-identical to the contract-side canonicalizer in
    /// `release.ts` — the canonicalization-format pinning + a cross-language
    /// golden vector are a before-real-key SEAM (see the RCA).
    pub fn canonical_signed_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let canonical = CanonicalManifest {
            release_id: &self.release_id,
            release_class: &self.release_class,
            release_ring: &self.release_ring,
            artifacts: &self.artifacts,
            installer_artifact: &self.installer_artifact,
            previous_release_id: &self.previous_release_id,
            cut_at: self.cut_at,
            signing_key_id: &self.signing_key_id,
        };
        serde_json::to_vec(&canonical)
    }
}
