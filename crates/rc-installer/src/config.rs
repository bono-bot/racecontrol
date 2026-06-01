//! Tier-1 control-plane constants (spec §4.5 / §4.12).
//!
//! Exactly one base URL; no per-venue, per-tenant, or third-party-CDN
//! constants — the binary is identical across every venue.

/// The single tier-1 control-plane base.
pub const INSTALLER_TIER_1_BASE: &str = "https://console.racecontrol.in";

/// Locked prefix every `installer_artifact.download_url` must be a child of
/// (canonical: `release.ts` `INSTALLER_DOWNLOAD_URL_PREFIX`, R-129).
pub const INSTALLER_DOWNLOAD_URL_PREFIX: &str = "https://console.racecontrol.in/install/";

/// SPKI SHA-256 pins (hex-encoded, 64 chars each) for `console.racecontrol.in` —
/// INC-2H / MMA F17. Hex of the SHA-256 of the cert's DER SubjectPublicKeyInfo
/// (`hex` is already a dependency; no base64/x509 dep added).
///
/// ROTATE-ME: ship TWO real pins (current + next) so console cert rotation never bricks
/// installs. The values below are HUMAN-READABLE PLACEHOLDERS — they intentionally fail
/// hex decode, so `tls_pin::load_console_pins()` returns empty and the pin check is
/// fail-closed until the real pins are provisioned at the INC-7 / R-129 ceremony.
pub const CONSOLE_SPKI_PINS_HEX: [&str; 2] = [
    "REPLACE_WITH_REAL_console.racecontrol.in_SPKI_PIN_AT_INC7_R129",
    "REPLACE_WITH_BACKUP_console.racecontrol.in_SPKI_PIN_FOR_ROTATION",
];

// NOTE: real-pin enforcement (reject an unprovisioned/all-placeholder pin set in
// production) is wired at INC-7 / R-129, when the rustls handshake verifier lands and has
// a presented cert to check. There is intentionally NO inert `ALLOW_PLACEHOLDER_PINS`
// gate here — a const that nothing reads would be a footgun (flipping it to false at
// INC-10 would silently do nothing). `tls_pin::console_pins_configured()` is the real,
// tested signal that pins are provisioned.
