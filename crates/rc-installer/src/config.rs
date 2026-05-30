//! Tier-1 control-plane constants (spec §4.5 / §4.12).
//!
//! Exactly one base URL; no per-venue, per-tenant, or third-party-CDN
//! constants — the binary is identical across every venue.

/// The single tier-1 control-plane base.
pub const INSTALLER_TIER_1_BASE: &str = "https://console.racecontrol.app";

/// Locked prefix every `installer_artifact.download_url` must be a child of
/// (canonical: `release.ts` `INSTALLER_DOWNLOAD_URL_PREFIX`, R-129).
pub const INSTALLER_DOWNLOAD_URL_PREFIX: &str = "https://console.racecontrol.app/install/";
