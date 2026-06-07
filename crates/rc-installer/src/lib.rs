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

pub mod config;
pub mod error;
pub mod manifest;
pub mod profile;
pub mod signature_verifier;
pub mod trusted_keys;
pub mod trusted_ssh_keys;
pub mod venue_type;

pub use manifest::ReleaseManifest;
pub use profile::Profile;
pub use venue_type::{VenueDecision, VenueType};
