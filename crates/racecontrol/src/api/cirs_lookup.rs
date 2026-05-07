//! CIRS lookup HTTP handler — PACT-20260506-001 Phase 1 wire-up Session 1.
//!
//! Customer Identity Resolution Service (CIRS) HTTP surface at
//! `POST /api/v1/cirs/lookup`. Substrate at `crates/v2-db/src/cirs.rs` is
//! Phase 0 (MERGED `483562ac`). This module is the Phase 1 (james-LEAD,
//! verify-by 2026-05-19) HTTP layer per PACT-20260506-001.
//!
//! ## Session 1 scope (THIS SESSION, scaffolding only)
//!
//! - Request/Response DTOs matching PACT §2.1 + §AMEND-1.A `balance_credits`
//! - Handler stub returning HTTP 501 (Session 2 lands real logic)
//! - Serde round-trip tests for every method variant
//!
//! ## Session 2 scope (NEXT)
//!
//! - Add `v2-db` Cargo dependency to racecontrol-crate
//! - Replace local `CirsLookupRequest` with `v2_db::cirs::LookupInput`
//! - Implement ProfilePreview substrate joins (customers + customer_profiles
//!   + wallets + sessions)
//! - `record_lookup` post-call discipline on every Found/NotFound/Error path
//!
//! ## Auth surface (per §AMEND-1.E + Q5-A)
//!
//! `cirs::lookup_by_phone` is **non-privileged** — staff session cookie
//! (via `auth::middleware::require_staff_jwt`) is sufficient. NO PIN re-entry.
//! See `crate::auth::privileged_actions::PrivilegedAction` for the enum that
//! gates PIN-required surfaces (refunds, manager-mode, comp-session, etc.).
//!
//! ## NF-james-B Indian-mobile-prefix WARN gate (§AMEND-1.B)
//!
//! Per §AMEND-1.B: gate evaluates raw input string **BEFORE** substrate
//! `canonicalize_phone`. Gate is staff-typed-input-discipline (bare 10-digit
//! assumption holds for Indian-mobile-only-V2.0 frame), NOT
//! substrate-input-discipline. Gate lives at the UI layer
//! (`web-v2/src/components/v2/pos/PhoneLookupInput.tsx`), NOT here. Substrate
//! stays permissive for E.164 / cross-tz international customers.
//!
//! ## Composes-with
//!
//! - PACT-20260506-001 (FILED `b45cf13`); §AMEND-1 ABSORB-IN-FULL of NF-bono-1..7
//! - PACT-20260505-001 Phase 0 substrate (MERGED `483562ac`)
//! - PACT-20260503-018 staff_id FK (MERGED `3119da30`)
//! - PrivilegedAction enum (`crate::auth::privileged_actions`) — non-membership
//!   confirms CIRS lookup is non-privileged

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

// ─── Request DTO ───────────────────────────────────────────────────────────

/// CIRS lookup request — wire-format mirror of `v2_db::cirs::LookupInput`.
///
/// **Session 1 NOTE:** redefined locally because racecontrol-crate does not
/// yet depend on the `v2-db` crate. Session 2 adds the Cargo dep and
/// replaces this with `pub use v2_db::cirs::LookupInput as CirsLookupRequest;`
/// (zero wire-format change).
///
/// Q1-A disposition (bono pre-FILE pass): single route + discriminated method
/// enum, NOT split routes. Method enum mirrors v2-db substrate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum CirsLookupRequest {
    /// M1 — phone-number lookup (V2.0 ACTIVE).
    Phone { phone: String },
    /// M3 — PWA-QR payload lookup (V2.0 plumbed-disabled; Phase 3 activates).
    QrPayload { payload: String },
    /// M4 — NFC tag lookup (V2.0 plumbed-disabled; Phase 3 activates).
    NfcTagId { tag_id: String },
    /// Walk-In Guest fallback (DoD §1.2 path B; `discount_ineligible: true`).
    WalkInGuestId { guest_id: u8 },
}

// ─── Response DTO — ProfilePreview ─────────────────────────────────────────

/// ProfilePreview — DoD §3.3 substrate.
///
/// Field naming per §AMEND-1.A NF-bono-1 absorption: `balance_credits` mirrors
/// canonical `wallets.balance_credits`. Earlier draft used `wallet_balance_credits`
/// — drift removed at AMEND time.
///
/// Wallet Framing C (single shared wallet per family) — `balance_credits` is
/// the family-pool balance, not per-profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfilePreview {
    pub customer_id: String,
    pub primary_phone: String,
    pub name: String,
    pub profiles: Vec<ProfileSummary>,
    pub balance_credits: i64,
    pub last_visit_ts: Option<String>,
    pub arrival_history_count_30d: u32,
    pub discount_ineligible: bool,
}

/// One row of the `profiles[]` list — V2 customer workflows Scenario 3 caps
/// at 4 profiles per family.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummary {
    pub profile_id: String,
    pub name: String,
    pub is_default: bool,
    pub discount_ineligible: bool,
}

// ─── Handler stub (Session 1 — returns 501) ────────────────────────────────

/// CIRS lookup handler — Session 1 scaffolding stub.
///
/// Returns HTTP 501 NotImplemented with a structured body pointing at the
/// PLAN.md for Session 2 implementation.
///
/// Wire path (when active): `POST /api/v1/cirs/lookup` under staff-JWT
/// protected sub-router. Route registration lands in Session 3.
pub async fn cirs_lookup_handler(
    State(_state): State<Arc<AppState>>,
    Json(_request): Json<CirsLookupRequest>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "cirs_lookup_not_yet_implemented",
            "phase": "PACT-20260506-001 Phase 1 wire-up Session 1 ships scaffolding only",
            "next_session_lands": "ProfilePreview substrate joins + record_lookup audit discipline",
            "plan_anchor": ".planning/specs/v2/PHASE-1-WIREUP-PLAN.md"
        })),
    )
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cirs_lookup_request_serde_phone_roundtrips() {
        let req = CirsLookupRequest::Phone {
            phone: "9876543210".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert_eq!(json, r#"{"method":"phone","phone":"9876543210"}"#);
        let parsed: CirsLookupRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_qr_payload_roundtrips() {
        let req = CirsLookupRequest::QrPayload {
            payload: "rp:v1:c:9876543210:abc123".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains(r#""method":"qr_payload""#));
        assert!(json.contains(r#""payload":"rp:v1:c:9876543210:abc123""#));
        let parsed: CirsLookupRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_nfc_tag_id_roundtrips() {
        let req = CirsLookupRequest::NfcTagId {
            tag_id: "04:1A:2B:3C:4D:5E:6F".to_string(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        assert!(json.contains(r#""method":"nfc_tag_id""#));
        let parsed: CirsLookupRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn cirs_lookup_request_serde_walk_in_guest_id_roundtrips() {
        let req = CirsLookupRequest::WalkInGuestId { guest_id: 1 };
        let json = serde_json::to_string(&req).expect("serialize");
        assert_eq!(json, r#"{"method":"walk_in_guest_id","guest_id":1}"#);
        let parsed: CirsLookupRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);

        // Guest 2 (the second hardcoded fallback per DoD §1.2 path B)
        let req2 = CirsLookupRequest::WalkInGuestId { guest_id: 2 };
        let parsed2: CirsLookupRequest =
            serde_json::from_str(r#"{"method":"walk_in_guest_id","guest_id":2}"#)
                .expect("deserialize");
        assert_eq!(parsed2, req2);
    }

    #[test]
    fn cirs_lookup_request_method_discriminator_is_required() {
        // Missing `method` field MUST fail to parse — defends against a UI
        // bug that drops the discriminator and silently coerces to a default.
        let parsed: Result<CirsLookupRequest, _> =
            serde_json::from_str(r#"{"phone":"9876543210"}"#);
        assert!(parsed.is_err(), "missing method discriminator must reject");
    }

    #[test]
    fn cirs_lookup_request_unknown_method_rejects() {
        // Forward-compat guard — adding a method server-side without updating
        // clients should fail loud, not silently coerce. Same for typos.
        let parsed: Result<CirsLookupRequest, _> = serde_json::from_str(
            r#"{"method":"biometric_face_scan","value":"…"}"#,
        );
        assert!(parsed.is_err(), "unknown method must reject");
    }

    #[test]
    fn profile_preview_serde_uses_balance_credits_naming_per_amend_1_a() {
        // §AMEND-1.A NF-bono-1 absorbed: field is `balance_credits`, NOT
        // `wallet_balance_credits`. This test is the contract drift detector
        // — if anyone renames back to `wallet_balance_credits`, this fails.
        let preview = ProfilePreview {
            customer_id: "00000000-0000-0000-0000-000000000001".to_string(),
            primary_phone: "+919876543210".to_string(),
            name: "Test Customer".to_string(),
            profiles: vec![ProfileSummary {
                profile_id: "00000000-0000-0000-0000-0000000000a1".to_string(),
                name: "Default".to_string(),
                is_default: true,
                discount_ineligible: false,
            }],
            balance_credits: 480,
            last_visit_ts: Some("2026-05-05T14:25:00+05:30".to_string()),
            arrival_history_count_30d: 4,
            discount_ineligible: false,
        };

        let json = serde_json::to_string(&preview).expect("serialize");
        assert!(
            json.contains(r#""balance_credits":480"#),
            "ProfilePreview must serialize wallet field as `balance_credits` per §AMEND-1.A — got: {json}"
        );
        assert!(
            !json.contains("wallet_balance_credits"),
            "drift to `wallet_balance_credits` detected — §AMEND-1.A reverted: {json}"
        );

        let parsed: ProfilePreview = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, preview);
    }

    #[test]
    fn profile_preview_walk_in_guest_shape_marks_discount_ineligible() {
        // V2 customer workflows DoD §1.2 path B — walk-in guests carry
        // discount_ineligible=true. The PreviewCard rendering layer reads
        // this flag to surface the "no discount applicable" badge.
        let walk_in = ProfilePreview {
            customer_id: "walk_in_guest_1".to_string(),
            primary_phone: "".to_string(),
            name: "Walk-In Guest 1".to_string(),
            profiles: vec![],
            balance_credits: 0,
            last_visit_ts: None,
            arrival_history_count_30d: 0,
            discount_ineligible: true,
        };
        let json = serde_json::to_string(&walk_in).expect("serialize");
        assert!(json.contains(r#""discount_ineligible":true"#));
    }
}
