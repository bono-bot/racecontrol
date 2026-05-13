// PACT-20260505-001 V2 Identity Primitive Phase 0 — Customer Identity
// Resolution Service skeleton.
//
// Phase 0 scope per §5: service skeleton + M1 phone-lookup endpoint live in
// substrate + cirs_lookup_audit table migration. M3 (PWA-QR) and M4 (NFC) are
// plumbed-disabled in v2.0; LookupInput recognizes them but actual
// resolution short-circuits to phone via decode/map paths shipped in Phase 3.
//
// AMPLIFIER-absorbed CAVEATs (msg=35126):
//   §3.1 phone canonicalization 4-rule table  — implemented here
//   §2.2 PWA-QR component-boundary guard      — frontend-only, not here
//   §2.3 nfc_tag_id no premature UNIQUE       — schema-only, not here
//
// Sequencing forward: Phase 1 (james-LEAD, verify-by 2026-05-19) wires this
// into HTTP handlers at POS .130 + Walk-In Guest 1+2 fallback dropdown.
// Phase 2 (mixed) wires Kiosk identity binding + PWA wallet-debit identity
// check. Phase 3 plumbs M3/M4 inputs (no hardware in v2.0).

use crate::DbPool;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum CirsError {
    // DPDP §7.8 PII-redaction: Display impl MUST NOT echo the raw phone input.
    // Inner String preserved for programmatic access (pattern-matching callers);
    // Display emits an 8-char sha256 fingerprint for duplicate-error correlation
    // without disclosing the raw digits. 8 hex chars (32 bits) is PII-safe per
    // analogous patterns and matches LookupInput::input_hash() truncation policy.
    #[error("invalid phone: <sha256:{}>", redact_phone(.0))]
    InvalidPhone(String),
    #[error("ambiguous phone (likely STD prefix or missing country code): <sha256:{}>", redact_phone(.0))]
    AmbiguousPhone(String),
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// Produce a hyphen-split 8-char sha256 hex fingerprint of a phone input for
/// safe Display in error messages. DPDP §7.8 — never echo raw phone digits in
/// 4xx response bodies. 8 hex chars = 32 bits of entropy; sufficient for
/// duplicate-error correlation in logs, insufficient for re-identification.
///
/// The hyphen split ("xxxx-xxxx") structurally guarantees the fingerprint can
/// never contain an ASCII digit run of length >= 6 even when the hash happens
/// to be all-digits — important because the cirs-lookup-pii-redaction contract
/// test scans response bodies for any 6+ digit run as a generic PII signal and
/// cannot distinguish a coincidental hex digit run from a real phone number.
fn redact_phone(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let full = format!("{:x}", h.finalize());
    format!("{}-{}", &full[..4], &full[4..8])
}

/// Input to a CIRS lookup. Exactly one variant per call.
/// M1 (phone) is the only ACTIVE method in v2.0; M3/M4/walk-in plumbed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum LookupInput {
    Phone { phone: String },
    QrPayload { payload: String },
    NfcTagId { tag_id: String },
    WalkInGuestId { guest_id: u8 },
}

impl LookupInput {
    /// Schema enum value for cirs_lookup_audit.input_method.
    pub fn input_method_db(&self) -> &'static str {
        match self {
            LookupInput::Phone { .. } => "phone",
            LookupInput::QrPayload { .. } => "qr_payload",
            LookupInput::NfcTagId { .. } => "nfc_tag_id",
            LookupInput::WalkInGuestId { .. } => "walk_in_guest_id",
        }
    }

    /// SHA256 hex of the raw input value for DPDP-minimised audit log.
    /// Walk-in guest IDs have no PII so audit row stores NULL.
    pub fn input_hash(&self) -> Option<String> {
        let raw = match self {
            LookupInput::Phone { phone } => phone.as_str(),
            LookupInput::QrPayload { payload } => payload.as_str(),
            LookupInput::NfcTagId { tag_id } => tag_id.as_str(),
            LookupInput::WalkInGuestId { .. } => return None,
        };
        let mut h = Sha256::new();
        h.update(raw.as_bytes());
        Some(format!("{:x}", h.finalize()))
    }
}

/// Lookup outcome. Phase 0 returns the audit-row tag only — full
/// ProfilePreview population (wallet balance / arrival history /
/// driver class) ships as Phase 1 (james-LEAD).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LookupResult {
    Found { customer_id: String },
    NotFound,
    Error { message: String },
}

impl LookupResult {
    /// Schema enum value for cirs_lookup_audit.result.
    pub fn db_tag(&self) -> &'static str {
        match self {
            LookupResult::Found { .. } => "found",
            LookupResult::NotFound => "not_found",
            LookupResult::Error { .. } => "error",
        }
    }
}

/// Canonicalize a phone string to E.164 per PACT-001 §3.1 (CAVEAT-1).
///
/// Rules in priority order:
///   1. starts with "+"            → already canonical (validate digits + length 8-15)
///   2. length == 10 && all digits → "+91" + input (auto-prefix Indian mobile)
///   3. length == 11 && starts "0" → REJECT AmbiguousPhone (likely stale STD prefix)
///   4. length >= 11 && all digits → REJECT AmbiguousPhone (missing country code)
///   5. anything else              → REJECT InvalidPhone
pub fn canonicalize_phone(input: &str) -> Result<String, CirsError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CirsError::InvalidPhone("empty".to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix('+') {
        if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
            return Err(CirsError::InvalidPhone(input.to_string()));
        }
        if rest.len() < 8 || rest.len() > 15 {
            return Err(CirsError::InvalidPhone(input.to_string()));
        }
        return Ok(format!("+{rest}"));
    }
    if trimmed.len() == 10 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Ok(format!("+91{trimmed}"));
    }
    if trimmed.len() == 11
        && trimmed.starts_with('0')
        && trimmed.chars().all(|c| c.is_ascii_digit())
    {
        return Err(CirsError::AmbiguousPhone(input.to_string()));
    }
    if trimmed.len() >= 11 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err(CirsError::AmbiguousPhone(input.to_string()));
    }
    Err(CirsError::InvalidPhone(input.to_string()))
}

/// M1 phone-lookup against `customers.phone` (idx_customers_phone hits).
/// Phase 0 returns Found/NotFound only; full ProfilePreview is Phase 1.
pub async fn lookup_by_phone(pool: &DbPool, phone: &str) -> Result<LookupResult, CirsError> {
    let canonical = canonicalize_phone(phone)?;
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM customers WHERE phone = ? LIMIT 1")
            .bind(&canonical)
            .fetch_optional(pool)
            .await?;
    Ok(match row {
        Some((customer_id,)) => LookupResult::Found { customer_id },
        None => LookupResult::NotFound,
    })
}

/// Append a row to cirs_lookup_audit. Called by every CIRS invocation
/// (post Phase 1 wire-up).
pub async fn record_lookup(
    pool: &DbPool,
    staff_id: &str,
    customer_id: Option<&str>,
    input: &LookupInput,
    result: &LookupResult,
) -> Result<(), CirsError> {
    sqlx::query(
        "INSERT INTO cirs_lookup_audit (staff_id, customer_id, input_method, input_hash, result) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(staff_id)
    .bind(customer_id)
    .bind(input.input_method_db())
    .bind(input.input_hash())
    .bind(result.db_tag())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_plus_prefix_canonical() {
        assert_eq!(canonicalize_phone("+919876543210").unwrap(), "+919876543210");
    }

    #[test]
    fn canonicalize_10_digit_auto_91() {
        assert_eq!(canonicalize_phone("9876543210").unwrap(), "+919876543210");
    }

    #[test]
    fn canonicalize_11_digit_zero_prefix_rejects() {
        let err = canonicalize_phone("09876543210").unwrap_err();
        assert!(matches!(err, CirsError::AmbiguousPhone(_)));
    }

    #[test]
    fn canonicalize_11_digit_no_plus_rejects() {
        let err = canonicalize_phone("19876543210").unwrap_err();
        assert!(matches!(err, CirsError::AmbiguousPhone(_)));
    }

    #[test]
    fn canonicalize_empty_rejects() {
        let err = canonicalize_phone("").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_whitespace_only_rejects() {
        let err = canonicalize_phone("   ").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_letters_in_plus_rejects() {
        let err = canonicalize_phone("+91abc1234567").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_plus_too_short_rejects() {
        let err = canonicalize_phone("+1234567").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_plus_too_long_rejects() {
        let err = canonicalize_phone("+1234567890123456").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_short_digits_rejects() {
        let err = canonicalize_phone("12345").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_trims_whitespace() {
        assert_eq!(
            canonicalize_phone("  +919876543210  ").unwrap(),
            "+919876543210"
        );
        assert_eq!(canonicalize_phone(" 9876543210 ").unwrap(), "+919876543210");
    }

    #[test]
    fn input_method_db_strings() {
        assert_eq!(
            LookupInput::Phone { phone: "+919999999999".to_string() }.input_method_db(),
            "phone"
        );
        assert_eq!(
            LookupInput::QrPayload { payload: "x".to_string() }.input_method_db(),
            "qr_payload"
        );
        assert_eq!(
            LookupInput::NfcTagId { tag_id: "x".to_string() }.input_method_db(),
            "nfc_tag_id"
        );
        assert_eq!(
            LookupInput::WalkInGuestId { guest_id: 1 }.input_method_db(),
            "walk_in_guest_id"
        );
    }

    #[test]
    fn walk_in_input_hash_is_none() {
        assert!(LookupInput::WalkInGuestId { guest_id: 1 }.input_hash().is_none());
    }

    #[test]
    fn phone_input_hash_is_sha256_hex() {
        let h = LookupInput::Phone { phone: "+919999999999".to_string() }
            .input_hash()
            .unwrap();
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        let h2 = LookupInput::Phone { phone: "+919999999998".to_string() }
            .input_hash()
            .unwrap();
        assert_ne!(h, h2);
    }

    #[test]
    fn result_db_tag_strings() {
        assert_eq!(
            LookupResult::Found { customer_id: "x".to_string() }.db_tag(),
            "found"
        );
        assert_eq!(LookupResult::NotFound.db_tag(), "not_found");
        assert_eq!(
            LookupResult::Error { message: "oops".to_string() }.db_tag(),
            "error"
        );
    }

    // NF-james-B sibling coverage: spec §3.1 ASCII-digit-only scope.
    // canonicalize_phone() must reject non-ASCII digit forms and intra-string
    // whitespace variants (substrate stays permissive only for ASCII digits;
    // Phase 1 POS .130 input gate adds Indian-mobile-prefix shape validation
    // separately as a UI-surface kaizen — see james AMPLIFIER NF-B).

    #[test]
    fn canonicalize_arabic_indic_digits_rejects() {
        let err = canonicalize_phone("\u{0669}\u{0668}\u{0667}\u{0666}\u{0665}\u{0664}\u{0663}\u{0662}\u{0661}\u{0660}").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_fullwidth_digits_rejects() {
        let err = canonicalize_phone("\u{FF19}\u{FF18}\u{FF17}\u{FF16}\u{FF15}\u{FF14}\u{FF13}\u{FF12}\u{FF11}\u{FF10}").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_internal_whitespace_rejects() {
        let err = canonicalize_phone("987 654 3210").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_nbsp_internal_rejects() {
        let err = canonicalize_phone("9876\u{00A0}543210").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_rtl_mark_in_plus_rejects() {
        let err = canonicalize_phone("+\u{200F}919876543210").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    #[test]
    fn canonicalize_fullwidth_plus_rejects() {
        let err = canonicalize_phone("\u{FF0B}919876543210").unwrap_err();
        assert!(matches!(err, CirsError::InvalidPhone(_)));
    }

    // DPDP §7.8 PII-redaction coverage — Display impl of CirsError variants
    // MUST NOT echo raw phone digits. Acceptance test
    // tests/contract/cirs-lookup-pii-redaction.spec.ts (commit 7c15f18e, james
    // §S-219) asserts no digit-run >= 6 in any 4xx response body across the
    // CIRS lookup HTTP surface. These unit tests are the substrate-side
    // contract: Display(CirsError) is the source of the leak the integration
    // test asserts against.

    /// Returns true if `s` contains any run of >= 6 consecutive ASCII digits.
    fn contains_phone_digit_run(s: &str) -> bool {
        let mut run = 0usize;
        for c in s.chars() {
            if c.is_ascii_digit() {
                run += 1;
                if run >= 6 {
                    return true;
                }
            } else {
                run = 0;
            }
        }
        false
    }

    #[test]
    fn contains_phone_digit_run_helper_smoke() {
        // Helper self-check so the assertions below are meaningful.
        assert!(contains_phone_digit_run("9876543210"));
        assert!(contains_phone_digit_run("abc9876543210xyz"));
        assert!(contains_phone_digit_run("123456"));
        assert!(!contains_phone_digit_run("12345"));
        assert!(!contains_phone_digit_run("123 456"));
        assert!(!contains_phone_digit_run("abc12def34ghi56"));
    }

    #[test]
    fn invalid_phone_display_redacts_raw_digits() {
        let cases = [
            "9876543210",
            "+919876543210",
            "09876543210",
            "98765 43210",
            "+1234567",
            "12345",
        ];
        for raw in cases {
            let msg = CirsError::InvalidPhone(raw.to_string()).to_string();
            assert!(
                !contains_phone_digit_run(&msg),
                "InvalidPhone Display leaked digit-run >= 6 for input {raw:?} -> message {msg:?}"
            );
        }
    }

    #[test]
    fn ambiguous_phone_display_redacts_raw_digits() {
        let cases = [
            "9876543210",
            "+919876543210",
            "09876543210",
            "98765 43210",
            "19876543210",
        ];
        for raw in cases {
            let msg = CirsError::AmbiguousPhone(raw.to_string()).to_string();
            assert!(
                !contains_phone_digit_run(&msg),
                "AmbiguousPhone Display leaked digit-run >= 6 for input {raw:?} -> message {msg:?}"
            );
        }
    }

    #[test]
    fn redacted_display_is_deterministic_per_input() {
        // Same input -> same fingerprint (duplicate-error correlation property).
        let a = CirsError::InvalidPhone("9876543210".to_string()).to_string();
        let b = CirsError::InvalidPhone("9876543210".to_string()).to_string();
        assert_eq!(a, b);
        // Different input -> different fingerprint (no collision on realistic pairs).
        let c = CirsError::InvalidPhone("9876543211".to_string()).to_string();
        assert_ne!(a, c);
    }

    #[test]
    fn redacted_display_fingerprint_shape() {
        // Expected shape: "invalid phone: <sha256:XXXX-XXXX>" (4-hyphen-4).
        // The hyphen split structurally caps digit-run length at 4 inside the
        // fingerprint, so even all-digit hash prefixes can't trip the
        // contract's >=6 digit-run scanner.
        let msg = CirsError::InvalidPhone("9876543210".to_string()).to_string();
        let idx = msg.find("<sha256:").expect("redacted tag present");
        let after = &msg[idx + "<sha256:".len()..];
        let close = after.find('>').expect("redacted tag closed");
        let fp = &after[..close];
        assert_eq!(fp.len(), 9, "fingerprint must be 4-hyphen-4 = 9 chars");
        let parts: Vec<&str> = fp.split('-').collect();
        assert_eq!(parts.len(), 2, "fingerprint must have exactly one hyphen");
        assert_eq!(parts[0].len(), 4);
        assert_eq!(parts[1].len(), 4);
        assert!(parts[0].chars().all(|c| c.is_ascii_hexdigit()));
        assert!(parts[1].chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// MAOR Tier-1 Finding #2 cross-verification: mirrors the substring-containment
    /// detection strategy of the integration-level contract test at
    /// `tests/contract/cirs-lookup-pii-redaction.spec.ts` (function `leakSubstrings`).
    /// The contract test scans response bodies for raw input / digits-only / +91-prefix
    /// / 0-prefix substrings of length >=6. This unit test independently verifies the
    /// Display output cannot leak any such candidate, closing the cross-verification
    /// gap between digit-run-count (other tests) and substring-containment (contract).
    #[test]
    fn redacted_display_no_leak_substring_class() {
        // leak-substring candidates per contract leakSubstrings() logic
        fn leak_candidates(input: &str) -> Vec<String> {
            let trimmed = input.trim().to_string();
            let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
            let mut out: Vec<String> = Vec::new();
            if trimmed.len() >= 6 {
                out.push(trimmed.clone());
            }
            if digits.len() >= 6 {
                out.push(digits.clone());
                out.push(format!("+91{digits}"));
                out.push(format!("0{digits}"));
            }
            out
        }
        let realistic_phones = [
            "9876543210",
            "+919876543210",
            "09876543210",
            "98765 43210",
            "1234567890",
        ];
        for raw in realistic_phones {
            let invalid_msg = CirsError::InvalidPhone(raw.to_string()).to_string();
            let ambiguous_msg = CirsError::AmbiguousPhone(raw.to_string()).to_string();
            for candidate in leak_candidates(raw) {
                assert!(
                    !invalid_msg.contains(&candidate),
                    "InvalidPhone Display leaked substring {candidate:?} for input {raw:?} -> {invalid_msg:?}"
                );
                assert!(
                    !ambiguous_msg.contains(&candidate),
                    "AmbiguousPhone Display leaked substring {candidate:?} for input {raw:?} -> {ambiguous_msg:?}"
                );
            }
        }
    }
}
