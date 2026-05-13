// V2 W1-S3 + W1-S4 — Refund 3-band routing + reason-code substrate.
//
// PACT: PACT-DRAFT-pact-001-phase-1-wave-1-static-billing-engine
// PLAN: .planning/specs/v2/PHASE-1-WAVE-1-PLAN.md §1.1 W1-S3, W1-S4
// Captain Q2 disposition (V2-MASTER-STATE §S-82): 3-band routing with reason codes.
//
// This module is the API-layer wrapper that dispatches refund requests to the
// correct privileged-action gate based on amount band. The actual journal-entry
// primitives live in `accounting::post_refund` / `post_cash_refund`,
// `api::billing_invoice::refund_billing_session`, and `api::wallet_ops::refund_wallet`.
// W1-S3 wrapper does NOT replace those primitives — it routes to them.
//
// Band thresholds (Captain Q2):
//   Band A   <₹1000          staff PIN only
//   Band B   ₹1000-2999      staff PIN + reason code
//   Band C   ≥₹3000          PrivilegedAction::ApproveRefundOverThreshold (manager mode)
//
// Reason codes (Captain Q2 — 5 variants):
//   SimPs5Crash       — sim or PS5 crashed mid-session
//   ServiceDispute    — customer-service quality dispute
//   BookingError      — booking/scheduling mistake
//   WalletAdjustment  — staff-initiated wallet correction
//   Other(String)     — free-text up to 100 chars
//
// W1-S4 reason-code persistence reuses existing `audit_log` table via
// `accounting_audit::log_audit` — zero schema migration. Routing fn lands in
// the W1-S3 follow-up commit.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::accounting::{log_admin_action, post_refund};
use crate::auth::middleware::StaffClaims;
use crate::state::AppState;

/// Maximum allowed length for the free-text reason in `RefundReason::Other`.
/// Captain Q2 disposition: 100 characters.
pub const MAX_OTHER_REASON_LEN: usize = 100;

/// Refund band classification per Captain Q2 disposition. Each band maps to a
/// distinct authorization gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundBand {
    /// `< ₹1000` — staff PIN only.
    A,
    /// `₹1000-2999` — staff PIN + reason code.
    B,
    /// `≥ ₹3000` — PrivilegedAction::ApproveRefundOverThreshold (manager mode).
    C,
}

impl RefundBand {
    /// Stable string identifier for audit-log `action` column.
    /// Format: `refund_3band_<band>` (matches V2-doctrine snake_case convention).
    pub const fn audit_action(&self) -> &'static str {
        match self {
            RefundBand::A => "refund_3band_a",
            RefundBand::B => "refund_3band_b",
            RefundBand::C => "refund_3band_c",
        }
    }

    /// Returns true if this band requires a reason code on the refund request.
    /// Band A is staff-PIN-only with no reason; B and C require a reason.
    pub const fn requires_reason(&self) -> bool {
        match self {
            RefundBand::A => false,
            RefundBand::B | RefundBand::C => true,
        }
    }

    /// Returns true if this band requires a manager-mode privileged action
    /// (`ApproveRefundOverThreshold`) rather than a staff PIN.
    pub const fn requires_manager_mode(&self) -> bool {
        matches!(self, RefundBand::C)
    }
}

/// Reason codes for refunds in Bands B + C. Captain Q2 disposition:
/// 5 variants total; `Other` carries free-text up to 100 chars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefundReason {
    SimPs5Crash,
    ServiceDispute,
    BookingError,
    WalletAdjustment,
    Other(String),
}

impl RefundReason {
    /// Stable serde-tag identifier (without the `Other` payload), suitable
    /// for audit-log `new_values` JSON or quick categorization.
    pub fn code(&self) -> &'static str {
        match self {
            RefundReason::SimPs5Crash => "sim_ps5_crash",
            RefundReason::ServiceDispute => "service_dispute",
            RefundReason::BookingError => "booking_error",
            RefundReason::WalletAdjustment => "wallet_adjustment",
            RefundReason::Other(_) => "other",
        }
    }

    /// Validates the reason. For `Other`, the free-text body must be
    /// non-empty and ≤ `MAX_OTHER_REASON_LEN`. All canned variants are
    /// always valid.
    pub fn validate(&self) -> Result<(), RefundReasonError> {
        match self {
            RefundReason::Other(s) if s.is_empty() => {
                Err(RefundReasonError::OtherEmpty)
            }
            RefundReason::Other(s) if s.chars().count() > MAX_OTHER_REASON_LEN => {
                Err(RefundReasonError::OtherTooLong {
                    max: MAX_OTHER_REASON_LEN,
                    actual: s.chars().count(),
                })
            }
            _ => Ok(()),
        }
    }
}

/// Errors from `RefundReason::validate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefundReasonError {
    OtherEmpty,
    OtherTooLong { max: usize, actual: usize },
}

impl std::fmt::Display for RefundReasonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefundReasonError::OtherEmpty => {
                write!(f, "RefundReason::Other free-text body is empty")
            }
            RefundReasonError::OtherTooLong { max, actual } => write!(
                f,
                "RefundReason::Other body length {} exceeds max {}",
                actual, max
            ),
        }
    }
}

impl std::error::Error for RefundReasonError {}

/// Classify a refund amount (in paise) into its 3-band routing tier per
/// Captain Q2 disposition.
///
/// Boundaries:
///   `< 100_000` paise (₹1000)            → Band A
///   `100_000 ..= 299_999` paise (₹1000-2999) → Band B
///   `≥ 300_000` paise (₹3000)            → Band C
///
/// Negative or zero amounts return Band A; callers should reject those
/// upstream (refund amount must be positive — see `wallet_ops::refund_wallet`
/// MMA-203 precedent).
pub const fn band_for_amount(amount_paise: i64) -> RefundBand {
    const BAND_B_FLOOR_PAISE: i64 = 100_000; // ₹1000
    const BAND_C_FLOOR_PAISE: i64 = 300_000; // ₹3000

    if amount_paise < BAND_B_FLOOR_PAISE {
        RefundBand::A
    } else if amount_paise < BAND_C_FLOOR_PAISE {
        RefundBand::B
    } else {
        RefundBand::C
    }
}

// ─── HTTP handler ────────────────────────────────────────────────────────────

/// Inbound payload for `POST /api/v1/refund/3band`.
///
/// Validation rules:
/// - `amount_paise` must be `> 0`
/// - `reason` is **required** for Band B and Band C (`band.requires_reason()`)
/// - `reason` is **forbidden** for Band A (`!band.requires_reason()`) — staff PIN only
/// - `RefundReason::Other(s)` — `s` is non-empty + ≤ `MAX_OTHER_REASON_LEN` chars
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRefundRequest {
    pub amount_paise: i64,
    pub driver_id: String,
    pub reason: Option<RefundReason>,
    pub reference_id: Option<String>,
}

/// 3-band refund routing handler. Wrapper-NOT-replacement: routes to existing
/// `accounting::post_refund` journal-entry primitive after authorization +
/// reason-code validation, then persists the band+reason via
/// `accounting_audit::log_admin_action` (action_type sibling column per
/// PACT-091 — audit_log.action CHECK constraint is CRUD-only).
///
/// Authorization gates per Captain Q2:
/// - Band A `<₹1000`: staff PIN only (any cashier+ role)
/// - Band B `₹1000-2999`: staff PIN + reason code
/// - Band C `≥₹3000`: requires `manager` or `superadmin` role
///   (PrivilegedAction::ApproveRefundOverThreshold proxy at this layer; the
///   formal PrivilegedAction enforcement happens at the manager-mode gate)
pub async fn route_refund(
    State(state): State<Arc<AppState>>,
    claims: Option<Extension<StaffClaims>>,
    Json(req): Json<RouteRefundRequest>,
) -> (StatusCode, Json<Value>) {
    // Validate amount
    if req.amount_paise <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "amount_paise must be positive" })),
        );
    }

    let band = band_for_amount(req.amount_paise);

    // Validate reason matches band
    match (&req.reason, band.requires_reason()) {
        (None, true) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("band {:?} requires a reason code", band),
                    "band": band,
                })),
            );
        }
        (Some(_), false) => {
            // Band A: reason is staff-PIN-only. Reject explicit reason to keep
            // policy boundary clean (reason codes carry V2 customer-pricing
            // signal that should NOT be set when the band doesn't require one).
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("band {:?} does not accept a reason code", band),
                    "band": band,
                })),
            );
        }
        (Some(r), true) => {
            if let Err(e) = r.validate() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("invalid reason: {}", e) })),
                );
            }
        }
        (None, false) => { /* Band A no-reason: ok */ }
    }

    // Manager-mode gate for Band C
    let staff_id = claims.as_ref().map(|c| c.0.sub.clone());
    if band.requires_manager_mode() {
        let has_manager = claims
            .as_ref()
            .map(|c| c.0.has_role(&["manager", "superadmin"]))
            .unwrap_or(false);
        if !has_manager {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "band C (≥₹3000) refund requires manager-mode (PrivilegedAction::ApproveRefundOverThreshold)",
                    "band": band,
                })),
            );
        }
    }

    let refund_id = Uuid::new_v4().to_string();

    // Dispatch to existing journal-entry primitive (fire-and-forget; logs its
    // own DB errors). Band A/B/C all route through the same primitive — the
    // band difference is in the AUTHORIZATION gate above + reason-code audit.
    post_refund(
        &state,
        &req.driver_id,
        req.amount_paise,
        req.reference_id.as_deref(),
    )
    .await;

    // Persist band + reason-code via PACT-091 action_type sibling (bypasses
    // audit_log.action CHECK CRUD-only).
    let details = json!({
        "refund_id": refund_id,
        "amount_paise": req.amount_paise,
        "driver_id": req.driver_id,
        "band": band,
        "reason_code": req.reason.as_ref().map(|r| r.code()),
        "reason_other_text": req.reason.as_ref().and_then(|r| match r {
            RefundReason::Other(s) => Some(s.clone()),
            _ => None,
        }),
        "reference_id": req.reference_id,
    })
    .to_string();
    log_admin_action(&state, band.audit_action(), &details, staff_id.as_deref(), None).await;

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "refund_id": refund_id,
            "band": band,
            "reason_code": req.reason.as_ref().map(|r| r.code()),
            "amount_paise": req.amount_paise,
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── band_for_amount: boundary cases ─────────────────────────────────────

    #[test]
    fn band_a_for_below_one_thousand_rupees() {
        // ₹999 = 99_900 paise → Band A
        assert_eq!(band_for_amount(99_900), RefundBand::A);
        // ₹1 (smallest positive) → Band A
        assert_eq!(band_for_amount(100), RefundBand::A);
    }

    #[test]
    fn band_b_at_one_thousand_rupee_floor() {
        // ₹1000 = 100_000 paise → Band B (Band A exclusive upper bound)
        assert_eq!(band_for_amount(100_000), RefundBand::B);
    }

    #[test]
    fn band_b_for_two_thousand_nine_hundred_ninety_nine_rupees() {
        // ₹2999 = 299_900 paise → Band B (Band C exclusive lower bound)
        assert_eq!(band_for_amount(299_900), RefundBand::B);
        assert_eq!(band_for_amount(299_999), RefundBand::B);
    }

    #[test]
    fn band_c_at_three_thousand_rupee_floor() {
        // ₹3000 = 300_000 paise → Band C
        assert_eq!(band_for_amount(300_000), RefundBand::C);
        // Larger amounts also Band C
        assert_eq!(band_for_amount(1_000_000), RefundBand::C);
    }

    #[test]
    fn band_a_for_zero_or_negative_amounts() {
        // Defensive: zero and negative classify as Band A. Callers must
        // reject these upstream (positive-amount precondition per MMA-203).
        assert_eq!(band_for_amount(0), RefundBand::A);
        assert_eq!(band_for_amount(-1), RefundBand::A);
    }

    // ─── RefundBand metadata ─────────────────────────────────────────────────

    #[test]
    fn band_audit_action_strings_are_stable() {
        assert_eq!(RefundBand::A.audit_action(), "refund_3band_a");
        assert_eq!(RefundBand::B.audit_action(), "refund_3band_b");
        assert_eq!(RefundBand::C.audit_action(), "refund_3band_c");
    }

    #[test]
    fn only_band_a_skips_reason() {
        assert!(!RefundBand::A.requires_reason());
        assert!(RefundBand::B.requires_reason());
        assert!(RefundBand::C.requires_reason());
    }

    #[test]
    fn only_band_c_requires_manager_mode() {
        assert!(!RefundBand::A.requires_manager_mode());
        assert!(!RefundBand::B.requires_manager_mode());
        assert!(RefundBand::C.requires_manager_mode());
    }

    // ─── RefundReason ────────────────────────────────────────────────────────

    #[test]
    fn canned_reasons_have_stable_codes() {
        assert_eq!(RefundReason::SimPs5Crash.code(), "sim_ps5_crash");
        assert_eq!(RefundReason::ServiceDispute.code(), "service_dispute");
        assert_eq!(RefundReason::BookingError.code(), "booking_error");
        assert_eq!(RefundReason::WalletAdjustment.code(), "wallet_adjustment");
        assert_eq!(
            RefundReason::Other("anything".to_string()).code(),
            "other"
        );
    }

    #[test]
    fn canned_reasons_validate_ok() {
        assert!(RefundReason::SimPs5Crash.validate().is_ok());
        assert!(RefundReason::ServiceDispute.validate().is_ok());
        assert!(RefundReason::BookingError.validate().is_ok());
        assert!(RefundReason::WalletAdjustment.validate().is_ok());
    }

    #[test]
    fn other_reason_within_bounds_validates_ok() {
        assert!(RefundReason::Other("brief note".to_string())
            .validate()
            .is_ok());
        // Exactly at the boundary: 100 chars
        let exactly_100 = "a".repeat(MAX_OTHER_REASON_LEN);
        assert!(RefundReason::Other(exactly_100).validate().is_ok());
    }

    #[test]
    fn other_reason_empty_string_rejects() {
        let err = RefundReason::Other(String::new()).validate().unwrap_err();
        assert_eq!(err, RefundReasonError::OtherEmpty);
    }

    #[test]
    fn other_reason_over_max_rejects() {
        let too_long = "a".repeat(MAX_OTHER_REASON_LEN + 1);
        let err = RefundReason::Other(too_long).validate().unwrap_err();
        match err {
            RefundReasonError::OtherTooLong { max, actual } => {
                assert_eq!(max, MAX_OTHER_REASON_LEN);
                assert_eq!(actual, MAX_OTHER_REASON_LEN + 1);
            }
            _ => panic!("expected OtherTooLong"),
        }
    }

    // ─── Serde round-trip on canned variants ─────────────────────────────────

    #[test]
    fn refund_band_serializes_snake_case() {
        let json_a = serde_json::to_string(&RefundBand::A).unwrap();
        let json_b = serde_json::to_string(&RefundBand::B).unwrap();
        let json_c = serde_json::to_string(&RefundBand::C).unwrap();
        assert_eq!(json_a, "\"a\"");
        assert_eq!(json_b, "\"b\"");
        assert_eq!(json_c, "\"c\"");
    }

    #[test]
    fn refund_reason_canned_round_trips() {
        for variant in [
            RefundReason::SimPs5Crash,
            RefundReason::ServiceDispute,
            RefundReason::BookingError,
            RefundReason::WalletAdjustment,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: RefundReason = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn refund_reason_other_round_trips_with_payload() {
        let r = RefundReason::Other("ACS crashed lap 4".to_string());
        let json = serde_json::to_string(&r).unwrap();
        let back: RefundReason = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
