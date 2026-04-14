//! Types, helpers, and promo evaluation logic for the cafe promos module.

use serde::{Deserialize, Serialize};

// ─── Types ───────────────────────────────────────────────────────────────────

/// Serializable active promo returned by the public endpoint and used in evaluate_promos.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActivePromo {
    pub id: String,
    pub name: String,
    pub promo_type: String,
    pub config: serde_json::Value,
    pub stacking_group: Option<String>,
    pub time_label: Option<String>, // e.g. "Active until 6:00 PM" or None
}

/// Result from evaluate_promos — best discount to apply.
#[derive(Debug, Default, Clone)]
pub struct PromoEvalResult {
    pub applied_promo_id: Option<String>,
    pub promo_name: Option<String>,
    pub discount_paise: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafePromo {
    pub id: String,
    pub name: String,
    pub promo_type: String,
    pub config: String,
    pub is_active: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafePromoRequest {
    pub name: String,
    pub promo_type: String,
    pub config: serde_json::Value,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCafePromoRequest {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn validate_promo_type(promo_type: &str) -> bool {
    matches!(promo_type, "combo" | "happy_hour" | "gaming_bundle")
}

/// Returns current IST time as "HH:MM" string.
pub fn ist_now_hhmm() -> String {
    // IST = UTC+5:30 = UTC + 19800 seconds
    let now_utc = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let ist_secs = now_utc + 19800;
    let hours = (ist_secs / 3600) % 24;
    let minutes = (ist_secs % 3600) / 60;
    format!("{:02}:{:02}", hours, minutes)
}

/// Returns true if `now` (HH:MM) is within [start, end) window.
/// Handles overnight wrap (e.g. 22:00 to 02:00) correctly.
pub fn time_in_window(now: &str, start: &str, end: &str) -> bool {
    if start == end {
        // MMA iter2: start==end means "no window restriction" — always active
        return true;
    }
    if start < end {
        now >= start && now < end
    } else {
        // overnight: e.g. 22:00 to 02:00
        now >= start || now < end
    }
}

/// Format "15:00" as "3:00 PM".
pub fn fmt_hhmm(hhmm: &str) -> String {
    let parts: Vec<&str> = hhmm.splitn(2, ':').collect();
    if parts.len() != 2 {
        return hhmm.to_string();
    }
    let h: u32 = parts[0].parse().unwrap_or(0);
    let m = parts[1];
    let period = if h < 12 { "AM" } else { "PM" };
    let h12 = match h % 12 {
        0 => 12,
        v => v,
    };
    format!("{}:{} {}", h12, m, period)
}

/// Evaluate which promo (if any) applies to this cart and return the best discount.
/// cart_items: Vec<(item_id, quantity)>
/// active_promos: slice of currently-active promos (time-filtered)
/// total_paise: gross cart total before any discount (needed for happy_hour %)
pub fn evaluate_promos(
    cart_items: &[(String, i64)],
    active_promos: &[ActivePromo],
    total_paise: i64,
) -> PromoEvalResult {
    let cart_map: std::collections::HashMap<&str, i64> =
        cart_items.iter().map(|(id, qty)| (id.as_str(), *qty)).collect();

    // group_key -> (discount_paise, promo_id, promo_name)
    let mut group_best: std::collections::HashMap<String, (i64, String, String)> =
        std::collections::HashMap::new();

    for promo in active_promos {
        let discount = calc_promo_discount(promo, &cart_map, total_paise);
        if discount <= 0 {
            continue;
        }
        let key = promo
            .stacking_group
            .clone()
            .unwrap_or_else(|| promo.id.clone());
        let entry = group_best
            .entry(key)
            .or_insert((0, promo.id.clone(), promo.name.clone()));
        if discount > entry.0 {
            *entry = (discount, promo.id.clone(), promo.name.clone());
        }
    }

    // Pick the single largest discount across all stacking groups (v1 simplification)
    if let Some((discount, id, name)) = group_best.values().max_by_key(|(d, _, _)| *d) {
        PromoEvalResult {
            applied_promo_id: Some(id.clone()),
            promo_name: Some(name.clone()),
            discount_paise: *discount,
        }
    } else {
        PromoEvalResult::default()
    }
}

/// Calculate the discount in paise that a single promo gives for this cart.
/// Returns 0 if promo conditions are not met.
fn calc_promo_discount(
    promo: &ActivePromo,
    cart_map: &std::collections::HashMap<&str, i64>,
    total_paise: i64,
) -> i64 {
    match promo.promo_type.as_str() {
        "combo" => {
            let items = match promo.config.get("items").and_then(|v| v.as_array()) {
                Some(arr) => arr.clone(),
                None => return 0,
            };
            let mut gross: i64 = 0;
            for req in &items {
                let item_id = match req.get("item_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return 0,
                };
                let req_qty = req.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1);
                if cart_map.get(item_id).copied().unwrap_or(0) < req_qty {
                    return 0; // condition not met
                }
                let unit_price = req
                    .get("unit_price_paise")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                gross += unit_price * req_qty;
            }
            if let Some(bundle_price) = promo
                .config
                .get("bundle_price_paise")
                .and_then(|v| v.as_i64())
            {
                return (gross - bundle_price).max(0);
            }
            if let Some(pct) = promo
                .config
                .get("discount_percent")
                .and_then(|v| v.as_i64())
            {
                return (gross * pct / 100).max(0);
            }
            0
        }
        "happy_hour" => {
            let pct = match promo
                .config
                .get("discount_percent")
                .and_then(|v| v.as_i64())
            {
                Some(p) if p > 0 && p <= 100 => p,
                _ => return 0,
            };
            (total_paise * pct / 100).max(0)
        }
        _ => 0, // gaming_bundle: display only, no auto-apply in v1
    }
}
