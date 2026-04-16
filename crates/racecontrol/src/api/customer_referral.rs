#![allow(unused_imports)]
use crate::api::customer_auth::extract_driver_id;
use axum::{
    Json,
    extract::State,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// ─── Referral System ─────────────────────────────────────────────────────────

pub(crate) async fn customer_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code: Option<(String,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referral_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND reward_credited = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "referral_code": code.and_then(|c| if c.0.is_empty() { None } else { Some(c.0) }),
        "successful_referrals": referral_count.map(|c| c.0).unwrap_or(0),
    }))
}

pub(crate) async fn customer_generate_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check if already has a code
    let existing: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((Some(code),)) = &existing
        && !code.is_empty() {
            return Json(json!({ "referral_code": code }));
        }

    // Generate 6-char alphanumeric code from UUID
    let code = format!("RP{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..6].to_uppercase());

    let _ = sqlx::query("UPDATE drivers SET referral_code = ? WHERE id = ?")
        .bind(&code)
        .bind(&driver_id)
        .execute(&state.db)
        .await;

    Json(json!({ "referral_code": code }))
}

pub(crate) async fn customer_redeem_referral(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "code required" })),
    };

    // Find referrer
    let referrer: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE referral_code = ?",
    )
    .bind(code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referrer_id = match referrer {
        Some((id,)) => {
            if id == driver_id {
                return Json(json!({ "error": "Cannot redeem your own code" }));
            }
            id
        }
        None => return Json(json!({ "error": "Invalid referral code" })),
    };

    // Check not already referred
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referee_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if existing.map(|e| e.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "Already used a referral code" }));
    }

    let referral_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO referrals (id, referrer_id, referee_id, code, reward_credited)
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(&referral_id)
    .bind(&referrer_id)
    .bind(&driver_id)
    .bind(code)
    .execute(&state.db)
    .await;

    Json(json!({ "ok": true, "message": "Referral code applied! Rewards will be credited after your first session." }))
}

// ─── Coupons ─────────────────────────────────────────────────────────────────

pub(crate) async fn customer_apply_coupon(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_uppercase(),
        None => return Json(json!({ "error": "code required" })),
    };

    // Find coupon
    let coupon: Option<(String, String, i64, i64, Option<String>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT id, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only
         FROM coupons WHERE code = ? AND is_active = 1",
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let coupon = match coupon {
        Some(c) => c,
        None => return Json(json!({ "error": "Invalid or expired coupon code" })),
    };

    // Check usage count
    let used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ?",
    )
    .bind(&coupon.0)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if used.map(|u| u.0 >= coupon.3).unwrap_or(false) {
        return Json(json!({ "error": "Coupon has reached maximum uses" }));
    }

    // Check if already used by this driver
    let driver_used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ? AND driver_id = ?",
    )
    .bind(&coupon.0)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if driver_used.map(|u| u.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You have already used this coupon" }));
    }

    // Return coupon details for the client to apply at checkout
    let discount_description = match coupon.1.as_str() {
        "percent" => format!("{}% off", coupon.2),
        "flat" => format!("₹{} off", coupon.2 / 100),
        "free_minutes" => format!("{} free minutes", coupon.2 as i64),
        _ => "Discount".to_string(),
    };

    Json(json!({
        "valid": true,
        "coupon_id": coupon.0,
        "coupon_type": coupon.1,
        "value": coupon.2,
        "description": discount_description,
        "min_spend_paise": coupon.6,
        "first_session_only": coupon.7,
    }))
}

// ─── Packages ────────────────────────────────────────────────────────────────

pub(crate) async fn customer_list_packages(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, i64, i64, bool, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT id, name, description, num_rigs, duration_minutes, price_paise,
                includes_cafe, day_restriction, hour_start, hour_end
         FROM packages WHERE is_active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(packages) => {
            let list: Vec<Value> = packages.iter().map(|p| json!({
                "id": p.0,
                "name": p.1,
                "description": p.2,
                "num_rigs": p.3,
                "duration_minutes": p.4,
                "price_paise": p.5,
                "price_display": format!("₹{}", p.5 / 100),
                "includes_cafe": p.6,
                "day_restriction": p.7,
                "hour_start": p.8,
                "hour_end": p.9,
            })).collect();
            Json(json!({ "packages": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
