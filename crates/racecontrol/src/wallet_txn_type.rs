//! D-CLUSTER-9 — Rust-side `txn_type` allow-list (defense-in-depth).
//!
//! ## Context
//! PR #72 (§S-260 atoms, merged 2026-05-13) intentionally DROPPED the narrow
//! `CHECK(txn_type IN (…))` constraint on `wallet_transactions` because the
//! original 12-value list was already narrower than actual code usage
//! (`gateway_topup`, `refund_cash`, `dispute_refund`, `review_reward`,
//! `referral_reward`, `referral_bonus`, `review_bonus`, `follow_bonus`,
//! `bonus_registration` / `bonus_review` / `bonus_follow` were all missing —
//! cluster RCA I-7 disposition). Captain ratified V1 ALTER + drop CHECK
//! (D-CLUSTER-1) and the schema is now permissive at the DB layer.
//!
//! ## What this module does
//! Enforces the canonical `txn_type` allow-list at the **Rust application**
//! layer, immediately before every `INSERT INTO wallet_transactions` call
//! site. A 13th value cannot enter the table without first being added here
//! AND reviewed in PR — the audit gate the DB CHECK used to provide is now a
//! code-review gate instead.
//!
//! ## §S-264 C4 caveat anchor
//! james AMPLIFIER (2026-05-13 §S-264 AGREE-WITH-CAVEATS) surfaced this exact
//! atom as a Phase γ sibling-defense follow-up. This module is the LANDED
//! disposition of that caveat (now D-CLUSTER-9).
//!
//! ## Composes-with
//! - `db/migrate_billing.rs:316-330` — permissive schema (no DB CHECK)
//! - `wallet::credit_in_tx` / `wallet::credit_wallet` / `wallet::debit_in_tx`
//!   — call-sites that invoke `validate_txn_type` BEFORE INSERT
//! - `wallet_refund::cash_refund` — INSERTs a hardcoded `'refund_cash'`
//!   literal inline; validated once at module-load via a compile-time-ish
//!   const-fn assertion (see `tests` below) PLUS a runtime call before INSERT.
//!
//! ## V2 doctrine alignment
//! Closes the "Rust-side defense-in-depth for dropped DB CHECK" gap surfaced
//! by §S-264 C4 (wallet-ledger cluster Phase γ scope). Aligns with §S-186
//! mechanism-trust-check guard contract Q5 (guards have written contracts).

use thiserror::Error;

/// Canonical `txn_type` allow-list — enumerated from production code grep
/// 2026-05-13 across `crates/racecontrol/src/{wallet*.rs, billing*.rs, api/*.rs,
/// multiplayer/**, cafe_orders.rs, psychology_retention.rs, cloud_sync_*.rs,
/// multiplayer_ops_kiosk.rs}`. Adding a new value: append here in alphabetical
/// section ordering, add a test row, and ensure the introducing call-site is
/// reviewed in the same PR.
pub const TXN_TYPE_ALLOWED: &[&str] = &[
    // ── Top-ups (rupee-class) — original CHECK constraint values ──────────
    "topup_cash",
    "topup_card",
    "topup_upi",
    "topup_online",

    // ── Gateway top-up (rupee-class; missed by original CHECK) ────────────
    // `wallet_gateway.rs:265` — payment webhook (Razorpay/etc.) credits;
    // currency_type="rupee" via `currency_type_for` in wallet.rs:14.
    "gateway_topup",

    // ── Debits (credit-class) ─────────────────────────────────────────────
    "debit_session",      // billing_timer + customer_booking + multiplayer paths
    "debit_cafe",         // cafe_orders.rs via wallet_ops debit_wallet_manual
    "debit_merchandise",  // wallet_ops debit_wallet_manual req.reason="merchandise"
    "debit_penalty",      // wallet_ops debit_wallet_manual req.reason="penalty"

    // ── Refunds (mixed currency-class) ────────────────────────────────────
    "refund_session",     // wallet_refund::refund wrapper + billing_*.rs auto-refund paths
    "refund_manual",      // api/wallet_ops.rs refund handlers (referenced + non-referenced)
    "refund_cash",        // wallet_refund.rs cash_refund (rupee-class; inline literal)
    "dispute_refund",     // api/customer_disputes.rs BILL-08 dispute approval refund

    // ── Bonuses (credit-class) ────────────────────────────────────────────
    "bonus",                // psychology_retention + wallet_staff (top-up ladder)
    "adjustment",           // manual staff adjustments (treated as bonus per D-02)

    // ── Incentive bonuses (credit-class; missed by original CHECK) ────────
    // api/billing_views.rs:133 generates `{review,follow}_bonus` via
    // `format!("{}_bonus", bonus_type)` where bonus_type ∈ {review, follow}.
    "review_bonus",
    "follow_bonus",
    // Cluster RCA + wallet.rs:319 comment also list these `bonus_*` forms;
    // currently no caller emits them but the abstraction in `credit_wallet`
    // accepts them. Listed for forward compatibility with the documented
    // taxonomy and to match the `is_bonus` branch in wallet.rs.
    "bonus_registration",
    "bonus_review",
    "bonus_follow",

    // ── Reward / referral credits (credit-class; missed by original CHECK) ─
    "review_reward",      // api/tournament_admin.rs:425 — Google review reward
    "referral_reward",    // billing_hooks.rs:352 — credits referrer
    "referral_bonus",     // billing_hooks.rs:373 — credits referee
];

/// Maximum txn_type byte length — defends against pathological inputs (very
/// long strings would never match the allow-list but also shouldn't waste
/// scan cycles or land in DB error messages).
pub const TXN_TYPE_MAX_LEN: usize = 64;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WalletTxnTypeError {
    #[error("txn_type cannot be empty")]
    Empty,

    #[error("txn_type exceeds max length ({TXN_TYPE_MAX_LEN} bytes): {got} bytes")]
    TooLong { got: usize },

    #[error("txn_type '{0}' is not in the allow-list (see wallet_txn_type::TXN_TYPE_ALLOWED). \
             If this is a legitimate new transaction class, add it to TXN_TYPE_ALLOWED in \
             crates/racecontrol/src/wallet_txn_type.rs and review the introducing call-site in \
             the same PR (D-CLUSTER-9 defense-in-depth gate).")]
    NotAllowed(String),
}

/// Validate a `txn_type` string against the canonical allow-list.
///
/// Returns `Ok(())` if `t` is in `TXN_TYPE_ALLOWED`; otherwise returns a
/// descriptive `WalletTxnTypeError`. Callers should invoke this immediately
/// before any `INSERT INTO wallet_transactions` (whether via
/// `wallet::credit_in_tx` / `wallet::debit_in_tx` / `wallet::credit_wallet` or
/// raw SQL such as `wallet_refund::cash_refund`'s inline literal).
///
/// This is a defense-in-depth gate: the DB-layer `CHECK` constraint on
/// `wallet_transactions.txn_type` was intentionally dropped in §S-260 atom 4
/// (D-CLUSTER-1 disposition 2026-05-13). Without this Rust-side gate, an
/// arbitrary string introduced by a future caller would land in the ledger
/// without any boundary enforcement.
pub fn validate_txn_type(t: &str) -> Result<(), WalletTxnTypeError> {
    if t.is_empty() {
        return Err(WalletTxnTypeError::Empty);
    }
    if t.len() > TXN_TYPE_MAX_LEN {
        return Err(WalletTxnTypeError::TooLong { got: t.len() });
    }
    if !TXN_TYPE_ALLOWED.contains(&t) {
        return Err(WalletTxnTypeError::NotAllowed(t.to_string()));
    }
    Ok(())
}

/// Convenience adapter for callers whose existing error channel is `String`
/// (most of the `wallet::*` API surface). Equivalent to
/// `validate_txn_type(t).map_err(|e| e.to_string())`.
pub fn validate_txn_type_string_err(t: &str) -> Result<(), String> {
    validate_txn_type(t).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_allowed_value_passes_validation() {
        for &t in TXN_TYPE_ALLOWED {
            assert!(
                validate_txn_type(t).is_ok(),
                "expected allow-list entry {:?} to pass validate_txn_type, but it did not",
                t
            );
        }
    }

    #[test]
    fn invalid_value_is_rejected() {
        let res = validate_txn_type("invalid_xyz");
        assert!(matches!(res, Err(WalletTxnTypeError::NotAllowed(ref s)) if s == "invalid_xyz"));
    }

    #[test]
    fn empty_string_is_rejected() {
        assert_eq!(validate_txn_type(""), Err(WalletTxnTypeError::Empty));
    }

    #[test]
    fn excessive_length_is_rejected() {
        let big = "x".repeat(TXN_TYPE_MAX_LEN + 1);
        assert!(matches!(
            validate_txn_type(&big),
            Err(WalletTxnTypeError::TooLong { got }) if got == TXN_TYPE_MAX_LEN + 1
        ));
    }

    #[test]
    fn known_call_site_literals_are_in_allow_list() {
        // Defense-in-depth: enumerate the literal strings used at every
        // `INSERT INTO wallet_transactions` call-site in the racecontrol
        // crate as of 2026-05-13. If a future commit introduces a new value
        // at any call-site without updating TXN_TYPE_ALLOWED, this assertion
        // will fail at `cargo test` time before runtime would.
        let call_site_literals = &[
            // wallet::credit() literal-matched arms (wallet.rs:234-250)
            "topup_cash", "topup_card", "topup_upi", "topup_online",
            "bonus", "adjustment", "refund_session", "refund_manual",
            // wallet_refund::cash_refund inline literal (wallet_refund.rs:140)
            "refund_cash",
            // wallet_gateway.rs:265
            "gateway_topup",
            // api/customer_disputes.rs:348
            "dispute_refund",
            // api/tournament_admin.rs:425
            "review_reward",
            // billing_hooks.rs:352
            "referral_reward",
            // billing_hooks.rs:373
            "referral_bonus",
            // api/billing_views.rs:133 — format!("{}_bonus", "review"|"follow")
            "review_bonus", "follow_bonus",
            // api/wallet_ops.rs:68 — format!("debit_{}", req.reason)
            // req.reason ∈ {cafe, merchandise, penalty} per DebitRequest doc comment
            "debit_cafe", "debit_merchandise", "debit_penalty",
            // multiplayer + booking + timer call-sites
            "debit_session",
        ];
        for t in call_site_literals {
            assert!(
                TXN_TYPE_ALLOWED.contains(t),
                "INSERT call-site literal {:?} is missing from TXN_TYPE_ALLOWED — add it before merging",
                t
            );
        }
    }

    #[test]
    fn string_err_adapter_preserves_rejection() {
        assert!(validate_txn_type_string_err("nope").is_err());
        assert!(validate_txn_type_string_err("topup_cash").is_ok());
    }

    #[test]
    fn allow_list_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &t in TXN_TYPE_ALLOWED {
            assert!(
                seen.insert(t),
                "duplicate entry in TXN_TYPE_ALLOWED: {:?}",
                t
            );
        }
    }
}
