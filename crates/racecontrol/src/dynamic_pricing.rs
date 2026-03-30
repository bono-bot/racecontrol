//! v29.0 Phase 19: Dynamic pricing recommendations based on demand + psychology (v14.0).
//! All recommendations require admin approval — never auto-apply.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PricingRecommendation {
    pub date: String,
    pub current_price_paise: i64,
    pub recommended_price_paise: i64,
    pub change_pct: f32,
    pub reason: String,
    pub confidence: f32,
    pub requires_approval: bool, // always true
}

/// Generate pricing recommendations based on forecasted demand
pub fn recommend_pricing(
    forecast_occupancy_pct: f32,
    current_price_paise: i64,
    is_peak: bool,
    is_weekend: bool,
) -> PricingRecommendation {
    let mut change_pct: f32 = 0.0;
    let mut reason = String::new();

    // High demand (>80% occupancy) — suggest premium
    if forecast_occupancy_pct > 80.0 {
        change_pct = if is_peak { 15.0 } else { 10.0 };
        reason = format!(
            "High forecasted demand ({:.0}% occupancy)",
            forecast_occupancy_pct
        );
    }
    // Low demand (<30% occupancy) — suggest discount
    else if forecast_occupancy_pct < 30.0 {
        change_pct = if is_weekend { -10.0 } else { -15.0 };
        reason = format!(
            "Low forecasted demand ({:.0}% occupancy) — discount to drive traffic",
            forecast_occupancy_pct
        );
    }
    // Normal demand — no change
    else {
        reason = format!(
            "Normal demand ({:.0}% occupancy) — no change recommended",
            forecast_occupancy_pct
        );
    }

    // MMA-v29: Zero base price — can't compute percentage change, return early
    if current_price_paise == 0 {
        return PricingRecommendation {
            date: chrono::Utc::now().to_rfc3339(),
            current_price_paise: 0,
            recommended_price_paise: 0,
            change_pct: 0.0,
            reason: "Current price is zero — cannot compute percentage change".to_string(),
            confidence: 0.0,
            requires_approval: true,
        };
    }

    // P1-4: Integer arithmetic for money — avoid f64 rounding errors.
    // change_pct is in whole percent (e.g. 15.0 = 15%). Convert to basis points for integer math.
    // MMA-v29: Clamp basis points to prevent extreme price swings
    let change_bp = ((change_pct * 100.0).round() as i64).clamp(-10000, 10000);
    // MMA-R1: Use checked arithmetic to prevent overflow on large prices
    let recommended = current_price_paise
        .checked_mul(change_bp)
        .and_then(|v| v.checked_div(10000))
        .and_then(|delta| current_price_paise.checked_add(delta))
        .unwrap_or(current_price_paise) // on overflow, keep current price
        .max(0); // MMA-v29: Never recommend negative prices

    PricingRecommendation {
        date: chrono::Utc::now().to_rfc3339(),
        current_price_paise,
        recommended_price_paise: recommended,
        change_pct,
        reason,
        confidence: if forecast_occupancy_pct > 0.0 {
            0.5
        } else {
            0.1
        },
        requires_approval: true, // ALWAYS true — never auto-apply pricing
    }
}
