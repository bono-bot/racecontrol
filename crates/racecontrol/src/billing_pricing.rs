//! Billing pricing calculations — dynamic pricing, session cost, refunds, rate tiers.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! All pricing computation lives here. Pure functions where possible.
//!
//! SNAP PRICING (2026-04-16, Uday decision):
//! Per-minute billing auto-snaps to package prices at tier boundaries.
//! 0-29 min: ₹25/min flat. At 30 min: snap to ₹700. 31-59: overflow at ₹23.33/min.
//! At 60 min: snap to ₹900. 61+: overflow at ₹15/min. Customer always gets best deal.

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

/// Refresh the in-memory rate tier cache from the database.
pub async fn refresh_rate_tiers(state: &Arc<AppState>) {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>)>(
        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type
         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    if let Ok(rows) = rows {
        if !rows.is_empty() {
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
            *state.billing.rate_tiers.write().await = tiers;
            tracing::info!("Billing rate tiers refreshed from DB");
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

/// Snap-to-package cost for N whole minutes. Customer always gets best deal.
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
    (minutes as i64) * per_min_rate
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
pub fn compute_per_minute_refund(wallet_debit_paise: i64, _total_debited_paise: i64, _rate_paise_per_minute: i64, driving_seconds: i64) -> i64 {
    if wallet_debit_paise <= 0 { return 0; }
    let minutes_used = (driving_seconds / 60) as u32;
    let actual_charge = snap_cost_for_minutes(minutes_used, 2500, 70000, 90000);
    let refund = wallet_debit_paise - actual_charge;
    if refund > 0 { refund } else { 0 }
}

/// Get tiers for a specific game. Falls back to universal tiers if no game-specific tiers exist.
pub fn get_tiers_for_game<'a>(tiers: &'a [BillingRateTier], sim_type: Option<rc_common::types::SimType>) -> Vec<&'a BillingRateTier> {
    let game_specific: Vec<_> = tiers.iter()
        .filter(|t| sim_type.is_some() && t.sim_type == sim_type)
        .collect();
    if !game_specific.is_empty() {
        game_specific
    } else {
        tiers.iter().filter(|t| t.sim_type.is_none()).collect()
    }
}
