//! Pure SPKI cert-pin decision logic (INC-2H, MMA F17 hardening).
//!
//! No TLS, no network — fully testable in the default (pure) build. The actual
//! certificate chain is supplied by the feature-gated transport at the INC-7 / R-129
//! wire-up; this module owns only the *decision*: does a presented SPKI match a pin.
//!
//! Pins are the hex-encoded SHA-256 of a cert's DER SubjectPublicKeyInfo (`hex` is
//! already a crate dependency; no base64/x509 deps added to the default tree).
//! Posture: HPKP-style (any-of pins), fail-closed (empty pin set trusts nothing).
//! The shipped console pins are human-readable placeholders that intentionally fail
//! to decode, so `load_console_pins()` returns empty until real pins are provisioned.

use crate::config;
use crate::error::{InstallError, Result};
use sha2::{Digest, Sha256};

/// A pinned SPKI SHA-256 (32 raw bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpkiPin(pub [u8; 32]);

impl SpkiPin {
    /// Parse a hex-encoded (64-char) SHA-256 SPKI pin. Rejects non-hex or non-32-byte.
    pub fn from_hex(s: &str) -> Result<Self> {
        let raw = hex::decode(s.trim())
            .map_err(|e| InstallError::Malformed(format!("spki pin not hex: {e}")))?;
        let arr: [u8; 32] = raw.as_slice().try_into().map_err(|_| {
            InstallError::Malformed(format!("spki pin must be 32 bytes, got {}", raw.len()))
        })?;
        Ok(SpkiPin(arr))
    }
}

/// SHA-256 of a certificate's SubjectPublicKeyInfo DER. The transport extracts the leaf
/// SPKI bytes from the presented chain and hashes them here before matching.
pub fn sha256_spki(spki_der: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(spki_der);
    h.finalize().into()
}

/// True iff ANY presented SPKI hash matches ANY configured pin (rotation-friendly).
/// Fail-closed: an empty pin set never trusts anything.
pub fn pin_satisfied(pins: &[SpkiPin], presented_spki_sha256: &[[u8; 32]]) -> bool {
    if pins.is_empty() {
        return false;
    }
    presented_spki_sha256
        .iter()
        .any(|p| pins.iter().any(|pin| &pin.0 == p))
}

/// Transport guard: Ok only if at least one presented SPKI matches a configured pin.
pub fn enforce_pin(pins: &[SpkiPin], presented_spki_sha256: &[[u8; 32]]) -> Result<()> {
    if pin_satisfied(pins, presented_spki_sha256) {
        Ok(())
    } else {
        Err(InstallError::PinMismatch)
    }
}

/// Parse the configured console pins, skipping placeholders (un-decodable). Returns only
/// real 32-byte pins; an empty result means pins are not yet provisioned (fail-closed).
pub fn load_console_pins() -> Vec<SpkiPin> {
    config::CONSOLE_SPKI_PINS_HEX
        .iter()
        .filter_map(|s| SpkiPin::from_hex(s).ok())
        .collect()
}

/// True iff at least one REAL console SPKI pin is configured (placeholders do not count).
pub fn console_pins_configured() -> bool {
    !load_console_pins().is_empty()
}
