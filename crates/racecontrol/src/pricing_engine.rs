//! Autonomous Pricing Engine (Phase 386).
//!
//! Bono computes optimal session prices from venue expense data and adjusts
//! pricing_rules automatically. Uday does NOT approve individual price changes —
//! Bono decides based on the math.
//!
//! Key principle: price must cover expenses + margin. If venue is losing money
//! at current prices, the engine raises prices. If demand is low, it offers
//! targeted discounts (via Phase 388 marketing, not blanket price cuts).

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use axum::Json;
use axum::extract::State;
use crate::state::AppState;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyExpenses {
    pub rent_paise: i64,
    pub salaries_paise: i64,
    pub utilities_paise: i64,
    pub marketing_paise: i64,
    pub cafe_inventory_paise: i64,
    pub equipment_paise: i64,
    pub misc_paise: i64,
}

impl MonthlyExpenses {
    pub fn total_paise(&self) -> i64 {
        self.rent_paise + self.salaries_paise + self.utilities_paise
            + self.marketing_paise + self.cafe_inventory_paise
            + self.equipment_paise + self.misc_paise
    }
}

#[derive(Debug, Serialize)]
pub struct BreakEvenAnalysis {
    pub monthly_expenses_paise: i64,
    pub avg_session_revenue_paise: i64,
    pub sessions_needed_per_month: i32,
    pub sessions_needed_per_day: f64,
    pub current_sessions_per_month: i32,
    pub margin_pct: f64,
    pub is_profitable: bool,
    pub recommended_min_price_paise: i64,
}

#[derive(Debug, Serialize)]
pub struct PricingRecommendation {
    pub tier_id: String,
    pub tier_name: String,
    pub current_price_paise: i64,
    pub recommended_price_paise: i64,
    pub reason: String,
    pub factors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpensesRequest {
    pub rent_paise: Option<i64>,
    pub salaries_paise: Option<i64>,
    pub utilities_paise: Option<i64>,
    pub marketing_paise: Option<i64>,
    pub cafe_inventory_paise: Option<i64>,
    pub equipment_paise: Option<i64>,
    pub misc_paise: Option<i64>,
}

// ─── DB Operations ──────────────────────────────────────────────────────────

async fn get_expenses(state: &Arc<AppState>) -> Option<MonthlyExpenses> {
    sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(
        "SELECT rent_paise, salaries_paise, utilities_paise, marketing_paise,
                cafe_inventory_paise, equipment_paise, misc_paise
         FROM business_expenses ORDER BY updated_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(r, s, u, m, c, e, mi)| MonthlyExpenses {
        rent_paise: r, salaries_paise: s, utilities_paise: u,
        marketing_paise: m, cafe_inventory_paise: c,
        equipment_paise: e, misc_paise: mi,
    })
}

async fn get_session_stats(state: &Arc<AppState>) -> (i32, i64) {
    // Sessions and avg revenue in last 30 days
    let row: Option<(i32, i64)> = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(AVG(wallet_debit_paise), 0)
         FROM billing_sessions
         WHERE status IN ('completed', 'ended')
           AND started_at > datetime('now', '-30 days')"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    row.unwrap_or((0, 0))
}

// ─── Break-Even Calculator ──────────────────────────────────────────────────

pub async fn calculate_break_even(state: &Arc<AppState>) -> Option<BreakEvenAnalysis> {
    let expenses = get_expenses(state).await?;
    let (session_count, avg_revenue) = get_session_stats(state).await;

    let total_expenses = expenses.total_paise();

    // Avoid division by zero
    let sessions_needed = if avg_revenue > 0 {
        ((total_expenses as f64 / avg_revenue as f64).ceil()) as i32
    } else {
        i32::MAX
    };

    let sessions_per_day = sessions_needed as f64 / 30.0;

    let monthly_revenue = session_count as i64 * avg_revenue;
    let margin_pct = if total_expenses > 0 {
        ((monthly_revenue - total_expenses) as f64 / total_expenses as f64) * 100.0
    } else {
        0.0
    };

    // Minimum price per session to break even with current volume
    let min_price = if session_count > 0 {
        total_expenses / session_count as i64
    } else {
        total_expenses / 30 // assume 1 session/day minimum
    };

    Some(BreakEvenAnalysis {
        monthly_expenses_paise: total_expenses,
        avg_session_revenue_paise: avg_revenue,
        sessions_needed_per_month: sessions_needed,
        sessions_needed_per_day: sessions_per_day,
        current_sessions_per_month: session_count,
        margin_pct,
        is_profitable: monthly_revenue > total_expenses,
        recommended_min_price_paise: min_price,
    })
}

// ─── Pricing Recommendations ────────────────────────────────────────────────

pub async fn generate_recommendations(state: &Arc<AppState>) -> Vec<PricingRecommendation> {
    let break_even = match calculate_break_even(state).await {
        Some(be) => be,
        None => return vec![],
    };

    let tiers: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, name, price_paise, duration_minutes FROM pricing_tiers
         WHERE is_active = 1 AND is_trial = 0 AND price_paise > 0
         ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut recommendations = Vec::new();

    for (tier_id, tier_name, price_paise, _duration) in &tiers {
        let mut factors = Vec::new();
        let mut recommended = *price_paise;

        // Factor 1: Below break-even minimum
        if *price_paise < break_even.recommended_min_price_paise {
            factors.push(format!(
                "Below break-even minimum ({})",
                format_paise(break_even.recommended_min_price_paise)
            ));
            recommended = break_even.recommended_min_price_paise;
        }

        // Factor 2: Margin too thin (<10%)
        if break_even.margin_pct < 10.0 && break_even.margin_pct >= 0.0 {
            factors.push(format!("Thin margin ({:.1}% — target 20%)", break_even.margin_pct));
            // Suggest 10% increase
            recommended = recommended.max((*price_paise as f64 * 1.10) as i64);
        }

        // Factor 3: Negative margin (losing money)
        if break_even.margin_pct < 0.0 {
            factors.push(format!("NEGATIVE margin ({:.1}%) — venue losing money", break_even.margin_pct));
            // Suggest 20% increase
            recommended = recommended.max((*price_paise as f64 * 1.20) as i64);
        }

        // Factor 4: High demand (>80% utilization) — can charge more
        let utilization = break_even.current_sessions_per_month as f64
            / (8.0 * 13.0 * 30.0) // 8 pods * 13 hours/day * 30 days = max possible
            * 100.0;
        if utilization > 60.0 {
            factors.push(format!("High utilization ({:.0}%) — demand supports higher price", utilization));
            recommended = recommended.max((*price_paise as f64 * 1.05) as i64);
        }

        // Round to nearest 100 paise (₹1)
        recommended = ((recommended + 50) / 100) * 100;

        let reason = if factors.is_empty() {
            "Price is healthy — no change needed".to_string()
        } else {
            factors.join("; ")
        };

        recommendations.push(PricingRecommendation {
            tier_id: tier_id.clone(),
            tier_name: tier_name.clone(),
            current_price_paise: *price_paise,
            recommended_price_paise: recommended,
            reason: reason.clone(),
            factors,
        });
    }

    recommendations
}

fn format_paise(paise: i64) -> String {
    format!("Rs.{}.{:02}", paise / 100, paise.abs() % 100)
}

// ─── API Handlers ───────────────────────────────────────────────────────────

/// GET /pricing/engine/expenses — current venue expenses
pub(crate) async fn get_expenses_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    match get_expenses(&state).await {
        Some(exp) => Json(json!({
            "expenses": {
                "rent": exp.rent_paise,
                "salaries": exp.salaries_paise,
                "utilities": exp.utilities_paise,
                "marketing": exp.marketing_paise,
                "cafe_inventory": exp.cafe_inventory_paise,
                "equipment": exp.equipment_paise,
                "misc": exp.misc_paise,
                "total": exp.total_paise(),
            }
        })),
        None => Json(json!({"error": "No expenses configured. POST /pricing/engine/expenses to set up."})),
    }
}

/// POST /pricing/engine/expenses — update venue expenses
pub(crate) async fn update_expenses_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateExpensesRequest>,
) -> Json<Value> {
    // Default to known venue costs from MEMORY (₹4.62L/month)
    let rent = req.rent_paise.unwrap_or(16_000_00);       // ₹1.6L
    let salaries = req.salaries_paise.unwrap_or(8_000_00); // ₹80K
    let utilities = req.utilities_paise.unwrap_or(6_000_00); // ₹60K
    let marketing = req.marketing_paise.unwrap_or(15_000_00); // ₹1.5L
    let cafe = req.cafe_inventory_paise.unwrap_or(1_200_00);  // ₹12K
    let equipment = req.equipment_paise.unwrap_or(0);
    let misc = req.misc_paise.unwrap_or(0);

    let result = sqlx::query(
        "INSERT INTO business_expenses (id, rent_paise, salaries_paise, utilities_paise, marketing_paise, cafe_inventory_paise, equipment_paise, misc_paise, updated_at)
         VALUES ('current', ?, ?, ?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
            rent_paise = excluded.rent_paise,
            salaries_paise = excluded.salaries_paise,
            utilities_paise = excluded.utilities_paise,
            marketing_paise = excluded.marketing_paise,
            cafe_inventory_paise = excluded.cafe_inventory_paise,
            equipment_paise = excluded.equipment_paise,
            misc_paise = excluded.misc_paise,
            updated_at = datetime('now')"
    )
    .bind(rent).bind(salaries).bind(utilities).bind(marketing)
    .bind(cafe).bind(equipment).bind(misc)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let total = rent + salaries + utilities + marketing + cafe + equipment + misc;
            Json(json!({ "ok": true, "total_paise": total }))
        }
        Err(e) => {
            tracing::error!("Failed to update expenses: {}", e);
            Json(json!({"error": "Failed to update expenses"}))
        }
    }
}

/// GET /pricing/engine/break-even — break-even analysis
pub(crate) async fn break_even_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    match calculate_break_even(&state).await {
        Some(be) => Json(json!(be)),
        None => Json(json!({"error": "No expenses configured"})),
    }
}

/// GET /pricing/engine/recommendations — pricing recommendations
pub(crate) async fn recommendations_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let recs = generate_recommendations(&state).await;
    Json(json!({ "recommendations": recs }))
}

/// POST /pricing/engine/apply — apply a recommendation to pricing_rules
pub(crate) async fn apply_recommendation_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let tier_id = match req.get("tier_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({"error": "tier_id required"})),
    };
    let new_price = match req.get("price_paise").and_then(|v| v.as_i64()) {
        Some(p) => p,
        None => return Json(json!({"error": "price_paise required"})),
    };

    let result = sqlx::query(
        "UPDATE pricing_tiers SET price_paise = ?, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(new_price)
    .bind(tier_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!(target: "pricing_engine", "Applied price change: {} → {} paise", tier_id, new_price);
            // Log to activity for audit trail
            let _ = sqlx::query(
                "INSERT INTO activity_log (event_type, details, created_at)
                 VALUES ('pricing_engine_apply', ?, datetime('now'))"
            )
            .bind(format!("Tier {} price set to {} paise by pricing engine", tier_id, new_price))
            .execute(&state.db)
            .await;
            Json(json!({ "ok": true, "tier_id": tier_id, "new_price_paise": new_price }))
        }
        Ok(_) => Json(json!({"error": "Tier not found"})),
        Err(e) => {
            tracing::error!("Failed to apply price: {}", e);
            Json(json!({"error": "Failed to apply price"}))
        }
    }
}
