//! Privileged action enumeration — Q5-A enforcement substrate.
//!
//! PACT-20260506-001 §AMEND-1.E (FILED 2026-05-06; §AMEND-1 ABSORB-IN-FULL of NF-bono-5
//! 2026-05-07 ~03:36 IST per first-mover-LEAD §E.1).
//!
//! Q5-A (session-cookie sufficient for non-privileged actions; PIN re-entry only on
//! privileged actions) requires a discrete enum so the manager-pill UI render-condition
//! is `if action ∈ PrivilegedAction { manager-pill + PIN-prompt }` — NOT ad-hoc
//! per-call developer judgment. Without this enum, list drift across handlers becomes
//! invisible and security-debt accrues silently (NF-bono-5).
//!
//! Per-handler trait contract: each handler implements `requires_pin(action) -> bool`
//! by checking enum membership. Privileged handlers explicitly opt-in via membership
//! — NOT via boolean flag scattered across handler files. This makes the PIN-required
//! surface non-fuzzy and auditable.
//!
//! NF-james-D candidate (deferred to PR-author implementation review): membership
//! exhaustiveness review. The 12-entry list derives from V2 customer workflows §10
//! missed scenarios + Wallet Framing C + DoD §3.3 + Layer 1 sketches; PR review may
//! surface missing entries (sub-PACT candidate or §AMEND-2 if material).
//!
//! Composes-with:
//! - V2 Design Layer 1 manager-pill UI (web-v2 component condition)
//! - Open-by-Default Flagged-to-Close Security Debt (Captain Q2 ratify 2026-05-05)
//! - security-debt-ledger row 6 (auth-debt class; closure mechanism = enum-membership change)
//! - customer-satisfaction-first / minimal-compromise (Captain 2026-05-07 ~03:05 IST):
//!   non-privileged surfaces (CIRS lookup, ProfilePreview, intake) carry NO PIN-friction
//!
//! Captain-reserve preserved: ₹X refund threshold for `ApproveRefundOverThreshold`
//! (refunds above ₹X require manager-mode rather than staff-PIN-only) — Captain
//! Q-DECISION queue per §AMEND-1.I.

use serde::{Deserialize, Serialize};

// ─── Privileged Action Categories ──────────────────────────────────────────

/// Classification of a privileged action for audit/UI grouping.
///
/// The PIN-prompt requirement is uniform across all categories — categories
/// exist for display grouping (manager-pill ordering, audit log filtering)
/// and are NOT a per-category trust ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedActionCategory {
    /// Wallet/billing irreversibles — money state changes that cannot be undone
    /// without a manual reversal trail.
    WalletBillingIrreversible,
    /// Identity/customer-record changes — bind/merge/override on customer records.
    IdentityRecord,
    /// Session/billing terminals — session end-states with billing implications.
    SessionTerminal,
    /// Manager-role escalations — transitions into elevated authority.
    ManagerEscalation,
}

// ─── PrivilegedAction Enum (§AMEND-1.E — 12 variants × 4 categories) ──────

/// Privileged staff actions requiring PIN re-entry via the manager-pill UI.
///
/// Membership is the PIN gate — handlers check `action.is_privileged()` (always
/// `true` for any `PrivilegedAction` value, by definition) rather than a
/// scattered boolean flag. To extend, add a variant here and the handler
/// inherits the gate automatically.
///
/// **Non-privileged surfaces (session-cookie only; NO PIN re-entry):**
/// - `cirs::lookup_by_phone` (Q5-A this PACT)
/// - ProfilePreview render
/// - WalkInGuest 1/2 selection
/// - intake (sim-racing M1 + walk-in anonymous)
/// - session start (normal billing flow)
/// - cafe order entry / KOT preview / mark-delivered
/// - top-up (wallet credit purchase via normal flow)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegedAction {
    // ─── Wallet/billing irreversibles (4) ─────────────────────────────────

    /// Staff-discretion goodwill credit applied to a customer wallet.
    ApplyApologyCredit,

    /// Money-out reversal on a billed transaction.
    IssueRefund,

    /// Out-of-band wallet write (NOT a normal top-up — used for corrections,
    /// staff promotions, manual reconciliation).
    AdjustWalletBalance,

    /// Toggle the `discount_ineligible` flag on a customer profile (e.g.,
    /// chargeback history, refund-loop suspect, staff-flagged for abuse).
    OverrideDiscountIneligible,

    // ─── Identity/customer record changes (3) ─────────────────────────────

    /// Duplicate-customer cleanup — merge two customer rows into one.
    MergeCustomerProfiles,

    /// Walk-in guest → bound customer (commits anonymous session history to
    /// a registered identity).
    PromoteWalkInToRegistered,

    /// Staff-confirmed-bind override — e.g., driver swap mid-session, where
    /// the original bound identity is replaced by another.
    OverrideIdentityBinding,

    // ─── Session/billing terminals (3) ────────────────────────────────────

    /// Pre-meter-stop session end (refund-implied — money owed back if charged
    /// for unused minutes).
    EndSessionEarly,

    /// Staff override on payment marker (e.g., customer paid in cash before
    /// terminal capture, or POS device failure).
    MarkPaidOutOfBand,

    /// Session-on-the-house (zero-bill, full attribution preserved for audit).
    CompSession,

    // ─── Manager-role escalations (2) ─────────────────────────────────────

    /// Manager mode toggle — gates other actions that require manager-role
    /// (e.g., refund-over-threshold, identity-binding overrides on
    /// flagged accounts).
    EnterManagerMode,

    /// Refunds above the Captain-defined ₹X threshold require manager-mode
    /// (vs staff-PIN-only). Threshold itself is Captain-reserve per
    /// §AMEND-1.I.
    ApproveRefundOverThreshold,
}

// ─── Behavior ─────────────────────────────────────────────────────────────

impl PrivilegedAction {
    /// Returns the category grouping for audit log + UI display.
    pub const fn category(&self) -> PrivilegedActionCategory {
        use PrivilegedAction::*;
        use PrivilegedActionCategory::*;
        match self {
            ApplyApologyCredit
            | IssueRefund
            | AdjustWalletBalance
            | OverrideDiscountIneligible => WalletBillingIrreversible,

            MergeCustomerProfiles
            | PromoteWalkInToRegistered
            | OverrideIdentityBinding => IdentityRecord,

            EndSessionEarly | MarkPaidOutOfBand | CompSession => SessionTerminal,

            EnterManagerMode | ApproveRefundOverThreshold => ManagerEscalation,
        }
    }

    /// Returns `true` for every variant — by construction, any `PrivilegedAction`
    /// value is privileged. Provided as a uniform handler-side check so the
    /// gate code reads `if action.requires_pin() { prompt_pin() }` regardless
    /// of action type.
    pub const fn requires_pin(&self) -> bool {
        true
    }

    /// Stable string identifier for audit logs and UI rendering.
    /// Format matches `serde(rename_all = "snake_case")` for cross-layer parity.
    pub const fn as_str(&self) -> &'static str {
        use PrivilegedAction::*;
        match self {
            ApplyApologyCredit => "apply_apology_credit",
            IssueRefund => "issue_refund",
            AdjustWalletBalance => "adjust_wallet_balance",
            OverrideDiscountIneligible => "override_discount_ineligible",
            MergeCustomerProfiles => "merge_customer_profiles",
            PromoteWalkInToRegistered => "promote_walk_in_to_registered",
            OverrideIdentityBinding => "override_identity_binding",
            EndSessionEarly => "end_session_early",
            MarkPaidOutOfBand => "mark_paid_out_of_band",
            CompSession => "comp_session",
            EnterManagerMode => "enter_manager_mode",
            ApproveRefundOverThreshold => "approve_refund_over_threshold",
        }
    }

    /// Full enumeration for completeness checks (audit tooling, test
    /// exhaustiveness assertions, UI render lists).
    pub const fn all() -> &'static [PrivilegedAction] {
        use PrivilegedAction::*;
        &[
            ApplyApologyCredit,
            IssueRefund,
            AdjustWalletBalance,
            OverrideDiscountIneligible,
            MergeCustomerProfiles,
            PromoteWalkInToRegistered,
            OverrideIdentityBinding,
            EndSessionEarly,
            MarkPaidOutOfBand,
            CompSession,
            EnterManagerMode,
            ApproveRefundOverThreshold,
        ]
    }
}

impl std::fmt::Display for PrivilegedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_returns_twelve_variants_per_amend_1_e() {
        // §AMEND-1.E specifies 12 entries × 4 categories. Drift here implies
        // either an out-of-band variant addition (which should ship via
        // §AMEND-2 sub-PACT or new §AMEND on PACT-001) or a deletion
        // (which would silently weaken the PIN gate). Either case is a
        // contract violation surfaceable by this assertion.
        assert_eq!(PrivilegedAction::all().len(), 12);
    }

    #[test]
    fn every_variant_is_privileged_by_construction() {
        // Handler-side `if action.requires_pin()` must always be true on
        // a `PrivilegedAction` value — that's the type-system contract.
        for action in PrivilegedAction::all() {
            assert!(
                action.requires_pin(),
                "{action:?} must report requires_pin=true (membership = privilege)"
            );
        }
    }

    #[test]
    fn category_distribution_matches_amend_1_e_spec() {
        // §AMEND-1.E: Wallet 4 / Identity 3 / SessionTerminal 3 / ManagerEscalation 2.
        let mut wallet = 0;
        let mut identity = 0;
        let mut session = 0;
        let mut manager = 0;
        for action in PrivilegedAction::all() {
            match action.category() {
                PrivilegedActionCategory::WalletBillingIrreversible => wallet += 1,
                PrivilegedActionCategory::IdentityRecord => identity += 1,
                PrivilegedActionCategory::SessionTerminal => session += 1,
                PrivilegedActionCategory::ManagerEscalation => manager += 1,
            }
        }
        assert_eq!(wallet, 4, "WalletBillingIrreversible category must have 4 variants");
        assert_eq!(identity, 3, "IdentityRecord category must have 3 variants");
        assert_eq!(session, 3, "SessionTerminal category must have 3 variants");
        assert_eq!(manager, 2, "ManagerEscalation category must have 2 variants");
    }

    #[test]
    fn as_str_matches_serde_snake_case_rendering() {
        // Cross-layer parity: the audit-log identifier (as_str) must equal
        // the JSON serialization output. Drift here means UI/log/db can
        // disagree on action name.
        for action in PrivilegedAction::all() {
            let json = serde_json::to_string(action).expect("serialize");
            // Strip surrounding quotes from JSON string literal.
            let json_stripped = json.trim_matches('"');
            assert_eq!(
                action.as_str(),
                json_stripped,
                "as_str/serde mismatch for {action:?}"
            );
        }
    }

    #[test]
    fn serialization_round_trip_is_stable() {
        for action in PrivilegedAction::all() {
            let json = serde_json::to_string(action).expect("serialize");
            let deserialized: PrivilegedAction =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*action, deserialized);
        }
    }

    #[test]
    fn display_impl_emits_stable_audit_identifier() {
        // Logging code uses {action} formatting; the rendered string MUST
        // be the canonical audit identifier (snake_case, stable across versions).
        assert_eq!(format!("{}", PrivilegedAction::IssueRefund), "issue_refund");
        assert_eq!(
            format!("{}", PrivilegedAction::ApproveRefundOverThreshold),
            "approve_refund_over_threshold"
        );
    }

    #[test]
    fn approve_refund_over_threshold_is_manager_escalation_not_wallet() {
        // §AMEND-1.E intentionally classifies this as ManagerEscalation (not
        // WalletBillingIrreversible) because the gate is the manager-mode
        // transition, not the refund-money-out movement. The Rs-X threshold
        // (Captain-reserve) decides which refund flows route through this
        // variant vs `IssueRefund`.
        assert_eq!(
            PrivilegedAction::ApproveRefundOverThreshold.category(),
            PrivilegedActionCategory::ManagerEscalation
        );
    }

    #[test]
    fn enter_manager_mode_is_distinct_from_individual_privileged_actions() {
        // Manager mode is itself a privileged transition (PIN required to
        // enter), distinct from privileged actions performed *while in*
        // manager mode. A flat enum is intentional per kaizen-discipline
        // (smallest invariant for observed requirement) — no nested
        // mode-state machine at this layer.
        assert!(PrivilegedAction::EnterManagerMode.requires_pin());
    }
}
