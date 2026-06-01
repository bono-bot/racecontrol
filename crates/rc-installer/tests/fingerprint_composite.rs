//! INC-7a — machine-fingerprint composite recipe parity (Replit-pinned 2026-06-01).
//!
//! The composite is Gate 4's pre-image: the client recomputes it from local hardware
//! and the verifier requires `heartbeat.machine_fingerprint == composite`. These
//! golden vectors lock the recipe byte-for-byte against the console's
//! `packages/contracts` canonicalizer (Replit GV1/GV2). A drift here means every real
//! claim fails Gate 4, so the frozen hashes are load-bearing.

use rc_installer::fingerprint::machine_fingerprint_composite;

const MACHINE_GUID: &str = "d3b07384-d9a4-4f1e-8c2a-1b6f5e0a7c91";
const DISK: &str = "S3PWNF0M812345";
const TPM: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// GV1 (TPM-present) — frozen anchor from Replit's pinned recipe.
#[test]
fn gv1_tpm_present_matches_pinned_composite() {
    assert_eq!(
        machine_fingerprint_composite(MACHINE_GUID, Some(TPM), DISK),
        "4d81ea8f4ab8be575a087222ffc13cc2cedb8a8f521981758d64e4a3c1b69312"
    );
}

/// GV2 (TPM-less → null) — frozen anchor; a distinct composite from GV1 (a TPM that
/// later goes missing changes the fingerprint → re-claim, by design).
#[test]
fn gv2_tpm_less_matches_pinned_composite() {
    assert_eq!(
        machine_fingerprint_composite(MACHINE_GUID, None, DISK),
        "d70ff49a141685d1d74d50e27d41e1b65cbe03dfc628fb48e79793b533f0c3ff"
    );
}

/// Non-vacuity: TPM-present and TPM-less are different composites (the null is not
/// silently treated as an empty string or omitted).
#[test]
fn tpm_present_and_absent_differ() {
    assert_ne!(
        machine_fingerprint_composite(MACHINE_GUID, Some(TPM), DISK),
        machine_fingerprint_composite(MACHINE_GUID, None, DISK)
    );
}

/// Non-vacuity: a different disk serial changes the composite (content-sensitive).
#[test]
fn different_disk_changes_composite() {
    assert_ne!(
        machine_fingerprint_composite(MACHINE_GUID, Some(TPM), DISK),
        machine_fingerprint_composite(MACHINE_GUID, Some(TPM), "OTHER-DISK-SERIAL")
    );
}
