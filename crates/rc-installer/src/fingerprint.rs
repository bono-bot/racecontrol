//! Machine-fingerprint composite recipe (INC-7a). PURE: components → composite hex.
//!
//! The HARDWARE collection of `{machine_guid, tpm_ek_hash, primary_disk_serial}` is
//! INC-7b (platform/Windows). This is the canonical composite recipe that BOTH the
//! client and the console (`packages/contracts`) mirror byte-for-byte — Gate 4 of
//! [`crate::heartbeat::verify_heartbeat`] compares `heartbeat.machine_fingerprint`
//! to this composite, so a 1-byte recipe divergence rejects every real claim.
//!
//! Recipe (Replit-pinned 2026-06-01; `threat_model Q-INSTALLER-32` / R-135 — and
//! the Gate-4 byte-for-byte model SUPERSEDES the earlier "2-of-3 component-wise"
//! note): canonical JSON, fields in DECLARATION order `machine_guid`,
//! `tpm_ek_hash` (JSON `null` when TPM-less, key always present), `primary_disk_serial`;
//! components verbatim (no lower-casing / trimming / normalization);
//! `sha256(utf8(canonical_json))` → lowercase hex (64 chars). Locked by
//! `tests/fingerprint_composite.rs` (golden vectors GV1 TPM-present / GV2 TPM-less).

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Borrowing view serialized in declaration order (NOT sorted) — the canonical
/// pre-image of the composite hash.
#[derive(Serialize)]
struct CanonicalComponents<'a> {
    machine_guid: &'a str,
    tpm_ek_hash: Option<&'a str>,
    primary_disk_serial: &'a str,
}

/// Compute the composite machine fingerprint from its components. `tpm_ek_hash`
/// is `None` on a TPM-less machine (serialized as JSON `null`). Returns 64-char
/// lowercase hex. This is what the client compares against the signed heartbeat.
pub fn machine_fingerprint_composite(
    machine_guid: &str,
    tpm_ek_hash: Option<&str>,
    primary_disk_serial: &str,
) -> String {
    let canonical = serde_json::to_vec(&CanonicalComponents {
        machine_guid,
        tpm_ek_hash,
        primary_disk_serial,
    })
    .expect("canonical components serialize");
    hex::encode(Sha256::digest(&canonical))
}
