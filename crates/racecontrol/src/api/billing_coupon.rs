#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Discount / Coupon helpers ───────────────────────────────────────────────

/// Validated coupon info ready to apply as a discount.
#[allow(dead_code)]
#[allow(dead_code)]
#[allow(dead_code)]
pub(crate) struct CouponDiscount {
    pub(crate) coupon_id: String,
    pub(crate) coupon_type: String,
    pub(crate) value: f64,
    pub(crate) discount_paise: i64,
    pub(crate) description: String,
}

/// Validate a coupon code and calculate the discount for a given price.
/// Returns Ok(CouponDiscount) or Err(error string).
pub(crate) async fn validate_and_calc_coupon(
    state: &Arc<AppState>,
    code: &str,
    driver_id: &str,
    price_paise: i64,
) -> Result<CouponDiscount, String> {
    let code_upper = code.to_uppercase();

    // Find coupon — FATM-08: only 'available' coupons can be validated
    let coupon: Option<(String, String, i64, i64, Option<String>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT id, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only
         FROM coupons WHERE code = ? AND is_active = 1 AND coupon_status = 'available'",
    )
    .bind(&code_upper)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let coupon = coupon.ok_or("Invalid or expired coupon code")?;

    // Check usage count
    let used: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ?",
    )
    .bind(&coupon.0)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if used.0 >= coupon.3 {
        return Err("Coupon has reached maximum uses".to_string());
    }

    // Check if already used by this driver
    let driver_used: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ? AND driver_id = ?",
    )
    .bind(&coupon.0)
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if driver_used.0 > 0 {
        return Err("You have already used this coupon".to_string());
    }

    // Check min_spend
    if let Some(min) = coupon.6 {
        if price_paise < min {
            return Err(format!("Minimum spend of {} credits required", min / 100));
        }
    }

    // Check first_session_only
    if coupon.7 {
        let session_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status IN ('completed', 'active')",
        )
        .bind(driver_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if session_count.0 > 0 {
            return Err("This coupon is only valid for first-time sessions".to_string());
        }
    }

    // Calculate discount
    let (discount_paise, description) = match coupon.1.as_str() {
        "percent" => {
            let disc = ((price_paise as f64) * (coupon.2 as f64) / 100.0).round() as i64;
            let disc = disc.min(price_paise); // never exceed price
            (disc, format!("{}% off", coupon.2))
        }
        "flat" => {
            let disc = coupon.2.min(price_paise);
            (disc, format!("{} credits off", disc / 100))
        }
        "free_minutes" => {
            // free_minutes doesn't reduce price, it extends time — handled separately
            (0, format!("{} free minutes", coupon.2 as i64))
        }
        _ => return Err("Unknown coupon type".to_string()),
    };

    Ok(CouponDiscount {
        coupon_id: coupon.0,
        coupon_type: coupon.1,
        value: coupon.2 as f64,
        discount_paise,
        description,
    })
}

/// Record a coupon redemption in the DB.
pub(crate) async fn record_coupon_redemption(
    state: &Arc<AppState>,
    coupon_id: &str,
    driver_id: &str,
    billing_session_id: &str,
    discount_paise: i64,
) {
    let _ = sqlx::query(
        "INSERT INTO coupon_redemptions (id, coupon_id, driver_id, billing_session_id, discount_paise, venue_id)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(coupon_id)
    .bind(driver_id)
    .bind(billing_session_id)
    .bind(discount_paise)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    // Increment used_count on coupon
    let _ = sqlx::query("UPDATE coupons SET used_count = used_count + 1 WHERE id = ?")
        .bind(coupon_id)
        .execute(&state.db)
        .await;
}

// ─── FATM-08: Coupon lifecycle FSM ──────────────────────────────────────────

/// Reserve a coupon for a session (available → reserved).
/// Uses SQL CAS (UPDATE WHERE coupon_status = 'available') to prevent races.
pub(crate) async fn reserve_coupon(
    pool: &sqlx::SqlitePool,
    coupon_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let result = sqlx::query(
        "UPDATE coupons SET coupon_status = 'reserved', reserved_at = datetime('now'), \
         reserved_for_session = ? WHERE id = ? AND coupon_status = 'available'",
    )
    .bind(session_id)
    .bind(coupon_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error reserving coupon: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("Coupon is no longer available (concurrent reservation or already used)".to_string());
    }
    Ok(())
}

/// Mark a coupon as redeemed (reserved → redeemed).
/// Called after billing session commits successfully.
pub(crate) async fn redeem_coupon(pool: &sqlx::SqlitePool, coupon_id: &str) -> Result<(), String> {
    let _ = sqlx::query(
        "UPDATE coupons SET coupon_status = 'redeemed' WHERE id = ? AND coupon_status = 'reserved'",
    )
    .bind(coupon_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error redeeming coupon: {}", e))?;
    Ok(())
}

/// FATM-09: Restore a coupon to available when its session is cancelled/failed.
/// Also deletes the coupon_redemption row so the count is not inflated.
/// pub so billing.rs can call it from the cancel path.
pub async fn restore_coupon_on_cancel(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(), String> {
    // Restore coupon: clear reservation fields and decrement used_count
    let _ = sqlx::query(
        "UPDATE coupons SET coupon_status = 'available', reserved_at = NULL, \
         reserved_for_session = NULL, used_count = MAX(used_count - 1, 0) \
         WHERE reserved_for_session = ? AND coupon_status IN ('reserved', 'redeemed')",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error restoring coupon: {}", e))?;

    // Remove the redemption record so used_count stays accurate
    let _ = sqlx::query("DELETE FROM coupon_redemptions WHERE billing_session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("DB error deleting coupon redemption: {}", e))?;

    Ok(())
}
