//! rc-installer V2 trust core — ed25519 + sha256 release-trust verification.
//!
//! This library is the pure-Rust verification core for the web-distributed
//! installer (spec §7). It is platform-independent: it compiles and unit-tests
//! on Linux and cross-compiles to `x86_64-pc-windows-gnu`. The Tauri GUI shell,
//! the CDN/Velopack fetch path, and agent provisioning are later increments and
//! are NOT part of this library.
//!
//! The V1 pendrive-copier `[[bin]]` (`src/main.rs`) is Windows-only and does not
//! use this library; host builds of this crate must scope to `--lib --tests`.
//!
//! Canonical data model: `rp-v2-apps/packages/contracts/src/release.ts`.

pub mod artifact_stager;
pub mod config;
pub mod error;
#[cfg(feature = "redeem_client")]
pub mod http_fetch;
pub mod identity;
pub mod manifest;
pub mod manifest_fetcher;
pub mod profile;
#[cfg(feature = "redeem_client")]
pub mod redeem_client;
pub mod signature_verifier;
pub mod trusted_keys;

// INC-2H hardening (pure cores — testable in the default build, no new deps).
pub mod host_allowlist;
pub mod journal;
pub mod tls_pin;

// INC-7a license-heartbeat verifier (pure; runtime inputs via VerifyContext).
pub mod heartbeat;
// INC-7a machine-fingerprint composite recipe (pure; Gate-4 pre-image).
pub mod fingerprint;

pub use error::InstallError;
pub use fingerprint::machine_fingerprint_composite;
pub use heartbeat::{
    verify_heartbeat, HeartbeatError, LicenseClass, LicenseHeartbeat, SignedLicenseHeartbeat,
    VerifyContext,
};
pub use identity::{RedeemError, RedeemedIdentity};
pub use manifest::ReleaseManifest;
pub use manifest_fetcher::{FetchError, Fetcher};
pub use profile::Profile;
