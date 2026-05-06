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
    #[error("invalid phone: {0}")]
    InvalidPhone(String),
    #[error("ambiguous phone (likely STD prefix or missing country code): {0}")]
    AmbiguousPhone(String),
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
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
}
