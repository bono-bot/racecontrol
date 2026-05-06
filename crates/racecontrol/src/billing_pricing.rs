//! Billing pricing calculations — dynamic pricing, session cost, refunds, rate tiers.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! All pricing computation lives here. Pure functions where possible.
//!
//! ## V2 PRICING DOCTRINE (§AMEND-3 / §AMEND-3.II / §AMEND-3.III, 2026-05-06)
//!
//! Per-minute billing follows an **additive tier ladder** (Way A). Cost is computed
//! by accumulating per-minute rates across tiers in order; thresholds are cumulative
//! ceilings. Customer pays each minute at its own tier's rate, never retroactively.
//!
//! Three layers (§AMEND-3.II D12):
//! - **Foundation**: an immutable accumulator. Applies whatever Strategy returns.
//!   Ships V2.0; no knowledge of pricing math.
//! - **Strategy**: pluggable cost computation (`PricingStrategy` trait below).
//!   `WayAAdditiveLadder` is the v2.0 default. Other strategies (e.g. SnapToPackage
//!   contingency) MAY be implemented later without re-touching Foundation.
//! - **Config**: tier definitions read from `billing_rates` DB table at session-tick
//!   time. Mid-session rate changes propagate live (D11 reversed in §AMEND-3.II;
//!   no snapshot infra). In-flight sessions continue at whatever rate the DB
//!   currently holds.
//!
//! Default tiers per `default_billing_rate_tiers()`: 30×₹25 + 30×₹20 + ∞×₹15.
//! Vivek canonical regression: 150 minutes = ₹2,700 (30×₹25 + 30×₹20 + 90×₹15).
//!
//! F25a (2026-05-06) introduces the trait + impls; default strategy stays Snap so
//! customer behavior is unchanged. F25b will flip the default to WayAAdditiveLadder
//! and wire live-rate tier-fetch through callers (per-PR Captain auth gated).
//!
//! ## HISTORICAL — V1-era SNAP PRICING (2026-04-16, Uday decision; superseded)
//!
//! Per §AMEND-3.III, the 2026-04-16 snap-pricing decision PRE-DATES V2 planning by
//! ~2 weeks and was a V1-era code patch, not V2 doctrine. Preserved here for audit.
//! Per-minute billing auto-snapped to package prices at tier boundaries:
//! 0-29 min: ₹25/min flat. At 30 min: snap to ₹700. 31-59: overflow at ₹23.33/min.
//! At 60 min: snap to ₹900. 61+: overflow at ₹15/min. "Customer always gets best
//! deal." See `SnapPricingStrategy` for the preserved implementation.

use std::sync::Arc;

use chrono::{Datelike, Timelike};

use crate::state::AppState;

/// Look up dynamic pricing rules and compute an adjusted price.
/// Returns the final price in paise, or None if no adjustment (use base price).
pub async fn compute_dynamic_price(
    state: &Arc<AppState>,
    base_price_paise: i64,
) -> i64 {
    let now = chrono::Local::now();
    let dow = now.weekday().num_days_from_monday() as i64; // 0=Mon .. 6=Sun
    let hour = now.hour() as i64;

    // Fetch matching rules (time-of-day rules)
    let rules = sqlx::query_as::<_, (String, f64, i64)>(
        "SELECT rule_type, multiplier, flat_adjustment_paise
         FROM pricing_rules
         WHERE is_active = 1
           AND (day_of_week IS NULL OR day_of_week = ?)
           AND (hour_start IS NULL OR ? >= hour_start)
           AND (hour_end IS NULL OR ? < hour_end)
           AND rule_type IN ('peak', 'off_peak', 'custom')
         ORDER BY
           CASE WHEN day_of_week IS NOT NULL THEN 0 ELSE 1 END,
           CASE WHEN hour_start IS NOT NULL THEN 0 ELSE 1 END
         LIMIT 1",
    )
    .bind(dow)
    .bind(hour)
    .bind(hour)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match rules {
        Some((_rule_type, multiplier, flat_adj)) => {
            let adjusted = (base_price_paise as f64 * multiplier).round() as i64 + flat_adj;
            // MMA-105: Enforce minimum price of 100 paise (₹1) to prevent free/negative sessions
            adjusted.max(100)
        }
        None => base_price_paise,
    }
}

/// Dynamic pricing lookup that works within an existing transaction (FATM-01).
/// Used by the atomic start_billing handler to avoid a separate DB connection.
pub async fn compute_dynamic_price_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    base_price_paise: i64,
) -> i64 {
    let now = chrono::Local::now();
    let dow = now.weekday().num_days_from_monday() as i64;
    let hour = now.hour() as i64;

    let rules = sqlx::query_as::<_, (String, f64, i64)>(
        "SELECT rule_type, multiplier, flat_adjustment_paise
         FROM pricing_rules
         WHERE is_active = 1
           AND (day_of_week IS NULL OR day_of_week = ?)
           AND (hour_start IS NULL OR ? >= hour_start)
           AND (hour_end IS NULL OR ? < hour_end)
           AND rule_type IN ('peak', 'off_peak', 'custom')
         ORDER BY
           CASE WHEN day_of_week IS NOT NULL THEN 0 ELSE 1 END,
           CASE WHEN hour_start IS NOT NULL THEN 0 ELSE 1 END
         LIMIT 1",
    )
    .bind(dow)
    .bind(hour)
    .bind(hour)
    .fetch_optional(&mut **tx)
    .await
    .ok()
    .flatten();

    match rules {
        Some((_rule_type, multiplier, flat_adj)) => {
            let adjusted = (base_price_paise as f64 * multiplier).round() as i64 + flat_adj;
            adjusted.max(100)
        }
        None => base_price_paise,
    }
}

// ─── Billing Rate Tiers ────────────────────────────────────────────────────

/// A per-minute billing rate tier, loaded from the `billing_rates` DB table.
/// Tiers are ordered by `tier_order` and applied additively (non-retroactive).
#[derive(Debug, Clone)]
pub struct BillingRateTier {
    pub tier_order: u32,
    pub tier_name: String,
    /// Upper boundary in minutes for this tier. 0 = unlimited (covers remaining time).
    pub threshold_minutes: u32,
    pub rate_per_min_paise: i64,
    /// None = universal rate. Some(SimType) = game-specific.
    pub sim_type: Option<rc_common::types::SimType>,
}

/// STAFF-01: Discount approval threshold — discounts above this amount require manager approval code.
/// Default: Rs.50 (5000 paise). Configurable via constant; future config migration can read from DB.
pub const DISCOUNT_APPROVAL_THRESHOLD_PAISE: i64 = 5000;

/// FATM-10: Minimum payable amount after all discounts stacked (coupon + staff + group combined).
/// 0 = no floor (disabled). Set to e.g. 10000 for a Rs.100 floor.
/// Server-side enforcement in start_billing and apply_billing_discount prevents abuse.
pub const DISCOUNT_FLOOR_PAISE: i64 = 0;

/// Default billing rate tiers (used before first DB load).
/// FATM-05: The Standard tier (2500 paise/min * 30 min = 75000 paise = Rs.750)
/// MUST match the 30-min pricing_tier.price_paise in the DB. If rates change, update both.
pub fn default_billing_rate_tiers() -> Vec<BillingRateTier> {
    vec![
        BillingRateTier { tier_order: 1, tier_name: "Standard".into(), threshold_minutes: 30, rate_per_min_paise: 2500, sim_type: None },
        BillingRateTier { tier_order: 2, tier_name: "Extended".into(), threshold_minutes: 60, rate_per_min_paise: 2000, sim_type: None },
        BillingRateTier { tier_order: 3, tier_name: "Marathon".into(), threshold_minutes: 0, rate_per_min_paise: 1500, sim_type: None },
    ]
}

// ─── Pricing Strategy (§AMEND-3.II D12 Foundation/Strategy/Config separation) ───
//
// F25a (2026-05-06) introduces this trait + 2 impls. The default returned by
// `default_strategy()` is `SnapPricingStrategy` so customer behavior is unchanged
// in F25a. F25b will flip the default to `WayAAdditiveLadder` AND wire live tier-
// fetch through callers (`compute_session_cost`, `snap_debit_amount`, refund fns)
// — at which point Way A math becomes the customer-facing pricing.
//
// Per §AMEND-3.II live-rate doctrine, strategies are STATELESS. Tiers are passed
// fresh on each call so admin-edited rates propagate to the next tick without any
// snapshot infrastructure.

/// Default fallback rate when tier list is empty / invalid.
/// 2500 paise/min = ₹25/min — matches Tier 1 of `default_billing_rate_tiers()`.
pub const FALLBACK_RATE_PAISE_PER_MIN: i64 = 2500;

/// Pluggable per-minute pricing computation. Stateless per §AMEND-3.II live-rate
/// doctrine — tiers passed fresh on each call so mid-session admin rate changes
/// propagate to the next tick without snapshot infra.
///
/// Implementations are zero-sized types; trait-object dispatch is acceptable in
/// the per-second tick hot path (single virtual call ≈ 10ns; tick cadence is 1Hz).
pub trait PricingStrategy: Send + Sync {
    /// Cumulative cost in paise after `minutes` complete minutes elapsed, given
    /// the current tier configuration. Returns 0 for `minutes == 0`. For empty
    /// or invalid `tiers`, falls back to `FALLBACK_RATE_PAISE_PER_MIN × minutes`
    /// (the implementation may log a warning).
    fn cumulative_cost_paise(&self, minutes: u32, tiers: &[BillingRateTier]) -> i64;

    /// Per-minute rate for the **upcoming** (`elapsed_minutes + 1`)-th partial
    /// minute, used for per-second proration during the partial minute currently
    /// being accumulated. Note: at `elapsed_minutes == 30` with a 30-min tier
    /// boundary, this returns the NEXT tier's rate (the 31st minute is in tier 2).
    fn rate_for_next_minute_paise(&self, elapsed_minutes: u32, tiers: &[BillingRateTier]) -> i64;

    /// Human-readable name (for logs / receipts / tests).
    fn name(&self) -> &'static str;
}

// ─── Way A Additive Tier Ladder (§AMEND-3 V2.0 default) ────────────────────

/// V2.0 default strategy per §AMEND-3 (Captain ratify Q1, 2026-05-06):
/// per-minute rates accumulate additively across tiers in `tier_order` ASC.
/// Each minute is charged at its tier's rate; no retroactive re-pricing.
///
/// Vivek canonical regression (using `default_billing_rate_tiers()`):
///   30 min → 30×₹25 = ₹750
///   60 min → 30×₹25 + 30×₹20 = ₹1,350
///   150 min → 30×₹25 + 30×₹20 + 90×₹15 = ₹2,700
///
/// Sim-specific tiers (`sim_type.is_some()`) are filtered out in v2.0; sim-aware
/// pricing is deferred per kaizen discipline (§AMEND-4 — defer speculative
/// scaffolding).
#[derive(Debug, Clone, Copy, Default)]
pub struct WayAAdditiveLadder;

impl WayAAdditiveLadder {
    /// Return universal tiers (sim_type=None) sorted ASC by tier_order.
    /// Tiers with `threshold_minutes == 0` are placed at the end (unlimited tier).
    fn universal_tiers_ordered<'a>(tiers: &'a [BillingRateTier]) -> Vec<&'a BillingRateTier> {
        let mut v: Vec<&BillingRateTier> = tiers.iter().filter(|t| t.sim_type.is_none()).collect();
        // Sort by tier_order; threshold_minutes=0 (unlimited) sinks to the end as a tiebreaker
        v.sort_by(|a, b| a.tier_order.cmp(&b.tier_order));
        v
    }
}

impl PricingStrategy for WayAAdditiveLadder {
    fn cumulative_cost_paise(&self, minutes: u32, tiers: &[BillingRateTier]) -> i64 {
        if minutes == 0 { return 0; }
        let ordered = Self::universal_tiers_ordered(tiers);
        if ordered.is_empty() {
            tracing::warn!(target: "billing", "WayAAdditiveLadder: empty universal tier list — using fallback rate {}p/min", FALLBACK_RATE_PAISE_PER_MIN);
            return (minutes as i64) * FALLBACK_RATE_PAISE_PER_MIN;
        }
        let mut total: i64 = 0;
        let mut consumed: u32 = 0;
        for tier in &ordered {
            if consumed >= minutes { break; }
            let tier_capacity = if tier.threshold_minutes == 0 {
                // Unlimited tier — absorbs all remaining minutes.
                u32::MAX
            } else {
                tier.threshold_minutes.saturating_sub(consumed)
            };
            let billable_in_tier = (minutes - consumed).min(tier_capacity);
            total = total.saturating_add((billable_in_tier as i64).saturating_mul(tier.rate_per_min_paise));
            consumed = consumed.saturating_add(billable_in_tier);
        }
        // If non-zero minutes remain after exhausting all tiers (last tier was bounded
        // and customer exceeded it), continue at the last tier's rate. This protects
        // against misconfigured tier sets where the highest tier_order has
        // threshold_minutes != 0.
        if consumed < minutes {
            if let Some(last) = ordered.last() {
                let extra = (minutes - consumed) as i64;
                total = total.saturating_add(extra.saturating_mul(last.rate_per_min_paise));
            }
        }
        total
    }

    fn rate_for_next_minute_paise(&self, elapsed_minutes: u32, tiers: &[BillingRateTier]) -> i64 {
        let ordered = Self::universal_tiers_ordered(tiers);
        if ordered.is_empty() {
            return FALLBACK_RATE_PAISE_PER_MIN;
        }
        // Walk tiers in order; the first tier whose cumulative threshold has NOT
        // been reached owns the next minute. `elapsed_minutes < threshold_minutes`
        // is the correct comparison: at elapsed=30 with threshold=30, we've
        // completed 30 minutes (filling tier 1); the 31st minute belongs to tier 2.
        for tier in &ordered {
            if tier.threshold_minutes == 0 || elapsed_minutes < tier.threshold_minutes {
                return tier.rate_per_min_paise;
            }
        }
        // Fall through (all bounded tiers exhausted): use last tier's rate.
        ordered.last().map(|t| t.rate_per_min_paise).unwrap_or(FALLBACK_RATE_PAISE_PER_MIN)
    }

    fn name(&self) -> &'static str { "WayAAdditiveLadder" }
}

// ─── Snap-to-Package Strategy (V1-era preserved per §AMEND-3.III) ──────────

/// V1-era snap-to-package pricing (Uday 2026-04-16, superseded by §AMEND-3 in
/// V2). Preserved as a `PricingStrategy` impl for:
/// 1. F25a default (no behavior change vs HEAD `989883c2`);
/// 2. low-cost contingency if Way A produces unforeseen customer pushback at V2
///    launch (per §AMEND-3.III pluggable-strategies residual).
///
/// Hardcoded rates: per_min=₹25, pkg_30=₹700, pkg_60=₹900. Ignores `tiers` arg.
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapPricingStrategy;

impl PricingStrategy for SnapPricingStrategy {
    fn cumulative_cost_paise(&self, minutes: u32, _tiers: &[BillingRateTier]) -> i64 {
        // Delegate to the existing free function so behavior is byte-identical
        // to pre-F25a code paths during the F25a transition.
        snap_cost_for_minutes(minutes, 2500, 70000, 90000)
    }

    fn rate_for_next_minute_paise(&self, elapsed_minutes: u32, _tiers: &[BillingRateTier]) -> i64 {
        // Match the existing `overflow_rate_at_minute` semantics.
        overflow_rate_at_minute(elapsed_minutes, 2500, 70000, 90000)
    }

    fn name(&self) -> &'static str { "SnapPricingStrategy" }
}

// ─── Default strategy selection ────────────────────────────────────────────

/// Static singletons — strategies are zero-sized, so these are zero-cost.
pub static SNAP_STRATEGY: SnapPricingStrategy = SnapPricingStrategy;
pub static WAY_A_STRATEGY: WayAAdditiveLadder = WayAAdditiveLadder;

/// Returns the active default pricing strategy. F25a returns `&SNAP_STRATEGY`
/// (no behavior change vs HEAD). F25b will change this to `&WAY_A_STRATEGY` —
/// the SINGLE LINE flip that activates Way A across every billing path that
/// already routes through `default_strategy()`.
///
/// Per §AMEND-3.II live-rate doctrine, this function's return type is a static
/// trait reference (no per-call construction cost; no per-session strategy
/// state; mid-session rate changes via DB tiers, not via re-instantiating the
/// strategy).
pub fn default_strategy() -> &'static dyn PricingStrategy {
    &SNAP_STRATEGY
}

// ─── Tier validation (§AMEND-3.II Config layer integrity) ──────────────────

/// Validation outcome for a tier list before it's committed to the in-memory cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierValidation {
    Valid,
    Empty,
    /// `threshold_minutes == 0` (unlimited) found on a tier that is not the
    /// highest-`tier_order` entry. Way A would skip subsequent tiers — fail-loud.
    UnlimitedNotLast { offending_tier_order: u32 },
    /// Two tiers share the same `tier_order` — sort order ambiguous.
    DuplicateTierOrder { tier_order: u32 },
    /// A tier has non-positive `rate_per_min_paise`. Refunds + accumulators would
    /// silently undercharge / produce negative deltas.
    NonPositiveRate { tier_order: u32 },
    /// Cumulative thresholds are not strictly increasing across the universal
    /// tier ladder (excluding the unlimited last tier).
    ThresholdsNotIncreasing,
    /// A bounded tier carries `threshold_minutes == u32::MAX`. While the math
    /// would not break (saturating_sub would clamp), this almost certainly
    /// indicates a misconfiguration — admins meant `0` (unlimited last tier)
    /// or a finite ceiling. F25a MMA Step 4 VERIFY consensus (Mistral V3 +
    /// Nemotron V2) flagged this degenerate config; reject loudly.
    PathologicalThreshold { tier_order: u32 },
}

/// Validate a `BillingRateTier` set destined for the in-memory cache. Returns
/// `TierValidation::Valid` only if the universal-tier ladder is well-formed for
/// `WayAAdditiveLadder` consumption. Validates the universal subset; sim-specific
/// tiers are not yet sanity-checked (kaizen — defer until sim-aware pricing).
///
/// Defense-in-depth: the same checks should ideally exist as DB CHECK constraints.
/// That migration is deferred to a separate sub-PACT (per MMA consensus).
pub fn validate_tier_set(tiers: &[BillingRateTier]) -> TierValidation {
    let universal: Vec<&BillingRateTier> = tiers.iter().filter(|t| t.sim_type.is_none()).collect();
    if universal.is_empty() {
        return TierValidation::Empty;
    }

    // Duplicate tier_order check (across the universal subset)
    let mut orders: Vec<u32> = universal.iter().map(|t| t.tier_order).collect();
    orders.sort_unstable();
    for w in orders.windows(2) {
        if w[0] == w[1] {
            return TierValidation::DuplicateTierOrder { tier_order: w[0] };
        }
    }

    // Sort by tier_order ASC for the remaining checks
    let mut ordered: Vec<&BillingRateTier> = universal.clone();
    ordered.sort_by(|a, b| a.tier_order.cmp(&b.tier_order));

    // Non-positive rate check
    for t in &ordered {
        if t.rate_per_min_paise <= 0 {
            return TierValidation::NonPositiveRate { tier_order: t.tier_order };
        }
    }

    // Pathological u32::MAX threshold check (F25a Step 4 VERIFY follow-up).
    // Bounded tiers with threshold_minutes == u32::MAX would absorb every
    // possible session and silently shadow subsequent tiers. Admins almost
    // certainly intended 0 (unlimited last tier) or a finite ceiling.
    for t in &ordered {
        if t.threshold_minutes == u32::MAX {
            return TierValidation::PathologicalThreshold { tier_order: t.tier_order };
        }
    }

    // Unlimited (threshold=0) only on the last tier
    let last_idx = ordered.len() - 1;
    for (i, t) in ordered.iter().enumerate() {
        if t.threshold_minutes == 0 && i != last_idx {
            return TierValidation::UnlimitedNotLast { offending_tier_order: t.tier_order };
        }
    }

    // Cumulative thresholds strictly increasing (excluding the optional unlimited last)
    let bounded_count = ordered.iter().filter(|t| t.threshold_minutes != 0).count();
    let bounded: Vec<&&BillingRateTier> = ordered.iter().take(bounded_count).collect();
    for w in bounded.windows(2) {
        if w[1].threshold_minutes <= w[0].threshold_minutes {
            return TierValidation::ThresholdsNotIncreasing;
        }
    }

    TierValidation::Valid
}

/// Refresh the in-memory rate tier cache from the database.
///
/// F25a: validates the fetched tier set via `validate_tier_set()` before committing
/// to the cache. On invalid state, the previous in-memory tiers are retained and an
/// error is logged. This ensures `WayAAdditiveLadder` never operates on a malformed
/// ladder (which would otherwise silently skip tiers or accumulate negative cost).
pub async fn refresh_rate_tiers(state: &Arc<AppState>) {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>)>(
        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type
         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    if let Ok(rows) = rows
        && !rows.is_empty() {
            let tiers: Vec<BillingRateTier> = rows
                .into_iter()
                .map(|(order, name, thresh, rate, sim_str)| {
                    let sim_type = sim_str.as_deref().and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok());
                    BillingRateTier {
                        tier_order: order as u32,
                        tier_name: name,
                        threshold_minutes: thresh as u32,
                        rate_per_min_paise: rate,
                        sim_type,
                    }
                })
                .collect();
            match validate_tier_set(&tiers) {
                TierValidation::Valid => {
                    *state.billing.rate_tiers.write().await = tiers;
                    tracing::info!(target: "billing", "Billing rate tiers refreshed from DB");
                }
                invalid => {
                    tracing::error!(
                        target: "billing",
                        "Refusing to apply invalid billing_rates: {:?}. Retaining previous in-memory tiers.",
                        invalid
                    );
                }
            }
        }
}

// ─── Session Cost Calculation ──────────────────────────────────────────────

/// Result of per-minute session cost calculation.
pub struct SessionCost {
    /// Total cost in paise for the entire elapsed duration
    pub total_paise: i64,
    /// Current rate per minute in paise
    pub rate_per_min_paise: i64,
    /// Current pricing tier name
    pub tier_name: String,
    /// Minutes remaining until next cheaper tier. None if on cheapest tier.
    pub minutes_to_next_tier: Option<u32>,
}

/// Compute session cost using snap-to-package tiered pricing.
///
/// # P0-2 gap (not yet fixed — deferred pending pricing-snapshot infrastructure)
///
/// The `_tiers` parameter is currently **unused**. Rates are hardcoded to the
/// production defaults (₹25/min, ₹700/30min, ₹900/60min). If admin changes
/// `billing_rates` via the pricing editor, `refresh_rate_tiers()` updates
/// `state.billing.rate_tiers` but this function ignores the cache and continues
/// to charge the old rates. A proper fix requires either:
///   (a) deriving pkg_30 / pkg_60 from the tier set (impossible today — the
///       tier schema has `rate_per_min_paise` but no package-price column); or
///   (b) introducing a `PricingSnapshot { per_min, pkg_30, pkg_60 }` captured
///       at session start and threaded through every cost-accounting path, so
///       mid-session rate changes do not retroactively apply.
///
/// Option (b) is the right fix and requires a DB migration + session-column
/// + multiple handler touchpoints. Tracked in audit report (2026-04-22 billing
/// gap audit) as P0-2. Do not remove `_tiers` parameter — it is a placeholder
/// for the future signature.
pub fn compute_session_cost(elapsed_seconds: u32, _tiers: &[BillingRateTier]) -> SessionCost {
    let per_min_rate: i64 = 2500;
    let pkg_30: i64 = 70000;
    let pkg_60: i64 = 90000;

    let elapsed_secs = elapsed_seconds as i64;
    let whole_minutes = elapsed_secs / 60;
    let partial_seconds = elapsed_secs % 60;

    let base_cost = snap_cost_for_minutes(whole_minutes as u32, per_min_rate, pkg_30, pkg_60);
    let partial_cost = if partial_seconds > 0 {
        let current_rate = overflow_rate_at_minute(whole_minutes as u32, per_min_rate, pkg_30, pkg_60);
        (partial_seconds * current_rate) / 60
    } else {
        0
    };

    let total_paise = base_cost + partial_cost;
    let current_rate = overflow_rate_at_minute(whole_minutes as u32, per_min_rate, pkg_30, pkg_60);
    let (tier_name, minutes_to_next) = if whole_minutes >= 60 {
        ("Marathon".to_string(), None)
    } else if whole_minutes >= 30 {
        ("Extended".to_string(), Some(60u32.saturating_sub(whole_minutes as u32)))
    } else {
        ("Standard".to_string(), Some(30u32.saturating_sub(whole_minutes as u32)))
    };

    SessionCost {
        total_paise,
        rate_per_min_paise: current_rate,
        tier_name,
        minutes_to_next_tier: minutes_to_next,
    }
}

/// Snap-to-package cost for N whole minutes. Customer always gets best deal under
/// the snap-pricing strategy (never penalized for early quit; per-minute charge
/// clamped at the relevant package price).
///
/// **F25a note (2026-05-06):** This free function is preserved for backward-compat
/// with existing callers and is now reachable via `SnapPricingStrategy` trait impl.
/// `WayAAdditiveLadder` (Way A V2.0 default in F25b onward) computes cost
/// additively across tiers and does not snap — see `WayAAdditiveLadder::cumulative_cost_paise`.
///
/// P0-1 fix (2026-04-22): the 0-29 branch previously returned the raw per-minute
/// accumulation with no ceiling. At per_min_rate=2500 + pkg_30=70000 that produced
/// a boundary inversion — 29 min = 72500 (₹725) while 30 min = 70000 (₹700). A
/// customer who quit 1 min early paid MORE than a customer who stayed the full
/// half-hour, violating the snap-strategy "Customer always gets best deal"
/// contract. We now clamp per-minute accumulation at pkg_30_price so it can never
/// exceed the package. Similarly for 30-59 clamping at pkg_60_price (already
/// present pre-fix) and for 60+ clamping is implicit in the pkg_60 base.
pub fn snap_cost_for_minutes(minutes: u32, per_min_rate: i64, pkg_30_price: i64, pkg_60_price: i64) -> i64 {
    if minutes == 0 { return 0; }
    if minutes >= 60 {
        let extra = (minutes - 60) as i64;
        return pkg_60_price + extra * (pkg_60_price / 60);
    }
    if minutes >= 30 {
        let extra = (minutes - 30) as i64;
        let cost = pkg_30_price + extra * (pkg_30_price / 30);
        return cost.min(pkg_60_price);
    }
    let linear = (minutes as i64) * per_min_rate;
    linear.min(pkg_30_price)
}

/// Per-minute overflow rate at a given elapsed minute.
pub fn overflow_rate_at_minute(elapsed_minutes: u32, per_min_rate: i64, pkg_30_price: i64, pkg_60_price: i64) -> i64 {
    if elapsed_minutes >= 60 { pkg_60_price / 60 }
    else if elapsed_minutes >= 30 { pkg_30_price / 30 }
    else { per_min_rate }
}

/// Backward-compat wrapper — delegates to snap_cost_for_minutes.
pub fn best_rate_for_minutes(minutes_used: u32, per_min_rate_paise: i64, pkg_30_price_paise: i64, pkg_60_price_paise: i64) -> i64 {
    snap_cost_for_minutes(minutes_used, per_min_rate_paise, pkg_30_price_paise, pkg_60_price_paise)
}

/// Compute refund for a package session that ended early.
pub fn compute_refund(allocated_seconds: i64, driving_seconds: i64, wallet_debit_paise: i64) -> i64 {
    compute_refund_with_rates(allocated_seconds, driving_seconds, wallet_debit_paise, 2500, 70000, 90000)
}

/// Compute refund with explicit rates.
pub fn compute_refund_with_rates(allocated_seconds: i64, driving_seconds: i64, wallet_debit_paise: i64, per_min_rate_paise: i64, pkg_30_price_paise: i64, pkg_60_price_paise: i64) -> i64 {
    if allocated_seconds <= 0 || wallet_debit_paise <= 0 || driving_seconds >= allocated_seconds { return 0; }
    let minutes_used = ((driving_seconds + 59) / 60) as u32;
    let actual_cost = snap_cost_for_minutes(minutes_used, per_min_rate_paise, pkg_30_price_paise, pkg_60_price_paise);
    let refund = wallet_debit_paise - actual_cost;
    if refund > 0 { refund } else { 0 }
}

/// Compute refund for a per-minute session that ended early. Uses snap pricing.
///
/// # P0-3 gap (not yet fixed — same blocker as P0-2)
///
/// The `_total_debited_paise` and `_rate_paise_per_minute` parameters are
/// ignored; rates are hardcoded to ₹25/₹700/₹900 defaults. When admin changes
/// pricing, refunds computed at session-end will still use the old rates,
/// producing discrepancies with what the session was actually charged.
///
/// # P2 gap: minute-rounding asymmetry with `compute_refund_with_rates`
///
/// This function uses floor-minutes (`driving_seconds / 60`) while
/// `compute_refund_with_rates` uses ceiling-minutes (`(driving_seconds + 59) / 60`).
/// That asymmetry is unintentional — per-minute sessions currently get one
/// partial minute free on refund. Bundle this with the P0-3 pricing-snapshot
/// fix when that infrastructure lands. Tracked in audit report (2026-04-22) P2-2.
pub fn compute_per_minute_refund(wallet_debit_paise: i64, _total_debited_paise: i64, _rate_paise_per_minute: i64, driving_seconds: i64) -> i64 {
    if wallet_debit_paise <= 0 { return 0; }
    let minutes_used = (driving_seconds / 60) as u32;
    let actual_charge = snap_cost_for_minutes(minutes_used, 2500, 70000, 90000);
    let refund = wallet_debit_paise - actual_charge;
    if refund > 0 { refund } else { 0 }
}

/// Get tiers for a specific game. Falls back to universal tiers if no game-specific tiers exist.
pub fn get_tiers_for_game(tiers: &[BillingRateTier], sim_type: Option<rc_common::types::SimType>) -> Vec<&BillingRateTier> {
    let game_specific: Vec<_> = tiers.iter()
        .filter(|t| sim_type.is_some() && t.sim_type == sim_type)
        .collect();
    if !game_specific.is_empty() {
        game_specific
    } else {
        tiers.iter().filter(|t| t.sim_type.is_none()).collect()
    }
}
