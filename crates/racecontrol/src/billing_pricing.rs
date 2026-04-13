//! Billing pricing calculations — dynamic pricing, session cost, refunds, rate tiers.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! All pricing computation lives here. Pure functions where possible.

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

/// Compute session cost from elapsed seconds using non-retroactive tiered pricing.
///
/// MMA-P1: Uses integer arithmetic (seconds * paise_per_min / 60) to avoid f64 rounding errors.
/// Each tier applies only to the seconds within its range (additive, not retroactive).
/// Default tiers: 25 cr/min (0-30 min), 20 cr/min (31-60 min), 15 cr/min (60+ min).
///
/// Example: 45 min = (1800s × 2500/60) + (900s × 2000/60) = 75000 + 30000 = 105000 paise.
pub fn compute_session_cost(elapsed_seconds: u32, tiers: &[BillingRateTier]) -> SessionCost {
    let elapsed_secs = elapsed_seconds as i64;
    let elapsed_minutes_whole = elapsed_seconds / 60;

    let mut total_paise: i64 = 0;
    let mut prev_threshold_secs: i64 = 0;
    let mut current_tier_name = String::new();
    let mut current_rate: i64 = 0;
    let mut minutes_to_next: Option<u32> = None;

    for (i, tier) in tiers.iter().enumerate() {
        let tier_ceiling_secs: i64 = if tier.threshold_minutes == 0 {
            i64::MAX / 2 // "unlimited" tier — avoid overflow
        } else {
            tier.threshold_minutes as i64 * 60
        };

        if elapsed_secs < prev_threshold_secs {
            break;
        }

        let seconds_in_tier = if elapsed_secs <= tier_ceiling_secs {
            elapsed_secs - prev_threshold_secs
        } else {
            tier_ceiling_secs - prev_threshold_secs
        };

        // MMA-P1+P2: Integer arithmetic with round-to-nearest.
        // (seconds * rate + 30) / 60 rounds to nearest paise (banker's rounding).
        // Maximum intermediate value: 10800s * 10000 paise/min + 30 = 108,000,030 — fits in i64.
        total_paise += (seconds_in_tier * tier.rate_per_min_paise + 30) / 60;
        current_tier_name = tier.tier_name.clone();
        current_rate = tier.rate_per_min_paise;

        // Minutes to next tier: only if currently in this tier and there IS a next tier
        if elapsed_secs <= tier_ceiling_secs && tier.threshold_minutes > 0 && i + 1 < tiers.len() {
            minutes_to_next = Some(tier.threshold_minutes.saturating_sub(elapsed_minutes_whole));
        }

        prev_threshold_secs = tier_ceiling_secs;
        if elapsed_secs <= tier_ceiling_secs {
            break;
        }
    }

    SessionCost {
        total_paise,
        rate_per_min_paise: current_rate,
        tier_name: current_tier_name,
        minutes_to_next_tier: minutes_to_next,
    }
}

/// Compute proportional refund for an early-ended or timed-out session (FATM-06).
///
/// Uses integer arithmetic only (no f64) to prevent rounding drift.
/// Package customers who end early pay the best rate for their actual usage:
/// - 0-29 min: per-minute rate (e.g. 2500p/min)
/// - 30-59 min: 30-min package price + per-minute for extra minutes
/// - 60+ min: 60-min package price (always the best deal)
pub fn best_rate_for_minutes(
    minutes_used: u32,
    per_min_rate_paise: i64,
    pkg_30_price_paise: i64,
    pkg_60_price_paise: i64,
) -> i64 {
    if minutes_used == 0 {
        return 0;
    }
    if minutes_used >= 60 {
        return pkg_60_price_paise;
    }
    if minutes_used >= 30 {
        let extra_minutes = (minutes_used - 30) as i64;
        let tiered_cost = pkg_30_price_paise + extra_minutes * per_min_rate_paise;
        return tiered_cost.min(pkg_60_price_paise);
    }
    (minutes_used as i64) * per_min_rate_paise
}

/// Compute refund for a package session that ended early.
pub fn compute_refund(
    allocated_seconds: i64,
    driving_seconds: i64,
    wallet_debit_paise: i64,
) -> i64 {
    compute_refund_with_rates(allocated_seconds, driving_seconds, wallet_debit_paise, 2500, 75000, 90000)
}

/// Compute refund with explicit rates from DB (no hardcoded fallback).
pub fn compute_refund_with_rates(
    allocated_seconds: i64,
    driving_seconds: i64,
    wallet_debit_paise: i64,
    per_min_rate_paise: i64,
    pkg_30_price_paise: i64,
    pkg_60_price_paise: i64,
) -> i64 {
    if allocated_seconds <= 0 || wallet_debit_paise <= 0 || driving_seconds >= allocated_seconds {
        return 0;
    }
    let minutes_used = ((driving_seconds + 59) / 60) as u32; // round up to complete minutes
    let actual_cost = best_rate_for_minutes(minutes_used, per_min_rate_paise, pkg_30_price_paise, pkg_60_price_paise);
    let refund = wallet_debit_paise - actual_cost;
    if refund > 0 { refund } else { 0 }
}

/// Compute refund for a per-minute session that ended early.
pub fn compute_per_minute_refund(
    wallet_debit_paise: i64,
    _total_debited_paise: i64,
    rate_paise_per_minute: i64,
    driving_seconds: i64,
) -> i64 {
    if wallet_debit_paise <= 0 {
        return 0;
    }
    let minutes_used = (driving_seconds / 60) as i64; // truncate (customer-favorable)
    let actual_charge = minutes_used * rate_paise_per_minute;
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
