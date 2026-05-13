//! V2 Wave 1 W1-S1 + W1-S10 — SessionBillingService primary engine.
//!
//! Per V2 Phase 1 Wave 1 PLAN.md (PACT-20260508-001 + §12 SUBSTANTIVE-REPLY):
//! - Class: substrate (V2 critical-path; first wave shipping a working economic
//!   transaction through V2-DB).
//! - W1-S1 = SessionBillingService primary engine consuming F25a Strategy trait.
//! - W1-S10 = per-minute granularity + round-up rounding helpers (§S-92 P5
//!   LOCKED).
//!
//! ## A1.e disposition (kaizen-N=1 escalation from §12.2 A1.c)
//!
//! §12.2 A1.c (single-module `crates/v2-db/src/billing.rs` migration) was the
//! disposition at PLAN authoring time (commit `b3a88630`). Empirical grounding
//! in this Session 1 W1-S1 fired §12.2 escalation triggers:
//!
//! 1. Workspace dep direction is `racecontrol-crate → v2-db` (`Cargo.toml`
//!    line 53). v2-db cannot import from `racecontrol::*` (circular).
//!    A1.d INFEASIBLE.
//! 2. F25a substrate is 21 public items / 625 lines in `billing_pricing.rs` —
//!    far beyond the >5 caller-files threshold §12.2 set as the migration-cost
//!    proxy. Migrating the trait + tier types + impls + validation + session
//!    cost + refund helpers + tier-game lookup is multi-hour scope, beyond
//!    Session 1 W1-S1 kaizen-min budget (1-2 h per PLAN §2).
//!
//! Resolution: NEW option **A1.e** — `SessionBillingService` lands in
//! racecontrol alongside `billing_pricing.rs`. F25a stays put; migration to
//! v2-db is deferred to its own sub-PACT (sibling to PACT-20260508-001) once
//! migration cost is empirically grounded across Wave 1 sub-steps.
//! Doctrine-vs-physical-layout drift (NF-3 in §12.6) is acknowledged but NOT
//! closed in Session 1.
//!
//! kaizen-discipline: smallest invariant for observed requirement; defer
//! scope-creep to its own substantive PACT cycle.
//!
//! ## Composes-with
//!
//! - `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` §12
//!   (james §SUBSTANTIVE-REPLY)
//! - `racecontrol/src/billing_pricing.rs` (F25a Strategy trait + tier types)
//! - `comms-link/V2-MASTER-STATE.md` §S-92 P5 LOCKED (per-minute granularity)
//! - `comms-link/proposals/PACT-DRAFT-pact-001-phase-1-wave-1-static-billing-engine.md`

use crate::billing_pricing::{BillingRateTier, PricingStrategy};

/// V2 Wave 1 primary billing engine. Consumes F25a Strategy trait + a live
/// `BillingRateTier` slice; computes per-minute charges using the supplied
/// strategy (use [`crate::billing_pricing::default_strategy`] for the V2.0
/// canonical default).
///
/// Stateless per §AMEND-3.II live-rate doctrine: tiers are passed fresh on
/// each call so admin-edited rates propagate to the next tick without
/// snapshot infrastructure.
#[derive(Debug, Default)]
pub struct SessionBillingService;

impl SessionBillingService {
    /// Compute cumulative cost in paise for a session given `elapsed_minutes`
    /// (use [`round_up_minutes`] to round partial seconds per §S-92 P5),
    /// the live `tiers` slice (read fresh per tick per §AMEND-3.II), and the
    /// active `strategy` (Way A / Snap / future).
    ///
    /// Returns 0 for `elapsed_minutes == 0`. For empty/invalid tiers, the
    /// strategy's own fallback handles graceful degradation (logged warning +
    /// `FALLBACK_RATE_PAISE_PER_MIN × minutes`).
    pub fn compute_charge(
        &self,
        elapsed_minutes: u32,
        tiers: &[BillingRateTier],
        strategy: &dyn PricingStrategy,
    ) -> i64 {
        strategy.cumulative_cost_paise(elapsed_minutes, tiers)
    }
}

/// W1-S10: round elapsed seconds up to whole minutes per §S-92 P5 LOCKED
/// (per-minute granularity + round-up rounding semantics).
///
/// 0 sec → 0 min · 1..=60 sec → 1 min · 61..=120 sec → 2 min · etc.
pub fn round_up_minutes(elapsed_seconds: u32) -> u32 {
    elapsed_seconds.div_ceil(60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing_pricing::{
        SnapPricingStrategy, WayAAdditiveLadder, default_billing_rate_tiers, default_strategy,
    };

    /// W1-S10: round-up rounding semantics per §S-92 P5 LOCKED.
    #[test]
    fn round_up_minutes_semantics() {
        assert_eq!(round_up_minutes(0), 0);
        assert_eq!(round_up_minutes(1), 1);
        assert_eq!(round_up_minutes(59), 1);
        assert_eq!(round_up_minutes(60), 1);
        assert_eq!(round_up_minutes(61), 2);
        assert_eq!(round_up_minutes(120), 2);
        assert_eq!(round_up_minutes(121), 3);
        // Vivek-anchor session lengths
        assert_eq!(round_up_minutes(30 * 60), 30);
        assert_eq!(round_up_minutes(60 * 60), 60);
        assert_eq!(round_up_minutes(150 * 60), 150);
    }

    /// W1-S1: SessionBillingService.compute_charge invokes WayAAdditiveLadder.
    /// Vivek canonical regression: `default_billing_rate_tiers()` × Way A.
    #[test]
    fn compute_charge_way_a_vivek_anchor() {
        let svc = SessionBillingService;
        let tiers = default_billing_rate_tiers();
        let strategy = WayAAdditiveLadder;
        // 30 min × ₹25 = ₹750 = 75000 paise
        assert_eq!(svc.compute_charge(30, &tiers, &strategy), 75_000);
        // 60 min = 30×₹25 + 30×₹20 = ₹1,350 = 135000 paise
        assert_eq!(svc.compute_charge(60, &tiers, &strategy), 135_000);
        // 150 min = 30×₹25 + 30×₹20 + 90×₹15 = ₹2,700 = 270000 paise
        assert_eq!(svc.compute_charge(150, &tiers, &strategy), 270_000);
    }

    /// W1-S1: SessionBillingService.compute_charge invokes SnapPricingStrategy
    /// (V1-era preserved per §AMEND-3.III; F25a default).
    #[test]
    fn compute_charge_snap_strategy() {
        let svc = SessionBillingService;
        let tiers = default_billing_rate_tiers();
        let strategy = SnapPricingStrategy;
        // 30 min snaps to ₹700 package = 70000 paise
        assert_eq!(svc.compute_charge(30, &tiers, &strategy), 70_000);
        // 60 min snaps to ₹900 package = 90000 paise
        assert_eq!(svc.compute_charge(60, &tiers, &strategy), 90_000);
    }

    /// W1-S1: SessionBillingService composes with default_strategy() — the
    /// canonical entry point for the active V2.0 strategy.
    #[test]
    fn compute_charge_via_default_strategy() {
        let svc = SessionBillingService;
        let tiers = default_billing_rate_tiers();
        let strategy = default_strategy();
        // F25a returns SnapPricingStrategy; 30 min → ₹700 = 70000 paise
        assert_eq!(svc.compute_charge(30, &tiers, strategy), 70_000);
    }

    /// W1-S1: zero-minutes edge case across both strategies.
    #[test]
    fn compute_charge_zero_minutes() {
        let svc = SessionBillingService;
        let tiers = default_billing_rate_tiers();
        assert_eq!(svc.compute_charge(0, &tiers, &WayAAdditiveLadder), 0);
        assert_eq!(svc.compute_charge(0, &tiers, &SnapPricingStrategy), 0);
    }
}
