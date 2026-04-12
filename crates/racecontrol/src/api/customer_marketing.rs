#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use super::customer_auth::compute_percentile;
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

pub(crate) async fn customer_session_share(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch billing session
    let session = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.car, bs.track
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get driver name
    let driver_name: String = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "Driver".to_string());

    // Get laps
    let laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len();
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.5).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.1).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.1).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };
    let consistency = if valid_laps.len() >= 3 {
        let mean = valid_laps.iter().map(|l| l.1 as f64).sum::<f64>() / valid_laps.len() as f64;
        let variance = valid_laps.iter().map(|l| {
            let diff = l.1 as f64 - mean;
            diff * diff
        }).sum::<f64>() / valid_laps.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / mean * 100.0;
        // Lower CV = more consistent. <2% = excellent, <5% = good, <10% = average
        Some(json!({
            "std_dev_ms": std_dev.round() as i64,
            "coefficient_of_variation": (cv * 100.0).round() / 100.0,
            "rating": if cv < 2.0 { "Excellent" } else if cv < 5.0 { "Good" } else if cv < 10.0 { "Average" } else { "Inconsistent" },
        }))
    } else {
        None
    };

    // Determine track/car from laps or session
    let track = laps.first().map(|l| l.6.clone()).or(session.10.clone()).unwrap_or_default();
    let car = laps.first().map(|l| l.7.clone()).or(session.9.clone()).unwrap_or_default();

    // Percentile ranking: how does this best lap compare to all laps on this track+car?
    let percentile = if let Some(best) = best_lap_ms {
        compute_percentile(&state.db, best, &track, &car).await
    } else {
        None
    };

    // Track record for comparison
    let track_record: Option<(i64, String)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT tr.best_lap_ms, d.name FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Personal best for this track+car
    let personal_best: Option<(i64,)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
        )
        .bind(&driver_id)
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Improvement: compare first valid lap to best valid lap
    let improvement_ms = if valid_laps.len() >= 2 {
        match (valid_laps.first(), best_lap_ms) {
            (Some(first), Some(best)) => Some(first.1 - best),
            _ => None,
        }
    } else {
        None
    };

    // Build share card data
    let driving_minutes = session.4 / 60;

    Json(json!({
        "share_report": {
            "driver_name": driver_name,
            "track": track,
            "car": car,
            "date": session.7,
            "driving_time_seconds": session.4,
            "driving_time_display": format!("{}m {}s", driving_minutes, session.4 % 60),
            "total_laps": total_laps,
            "valid_laps": valid_laps.len(),
            "best_lap_ms": best_lap_ms,
            "best_lap_display": best_lap_ms.map(|ms| format!("{}:{:02}.{:03}", ms / 60000, (ms % 60000) / 1000, ms % 1000)),
            "average_lap_ms": avg_lap_ms,
            "improvement_ms": improvement_ms,
            "consistency": consistency,
            "percentile_rank": percentile,
            "percentile_text": percentile.map(|p| format!("Top {}% of drivers", 100 - p.min(99))),
            "track_record": track_record.as_ref().map(|(ms, name)| json!({
                "time_ms": ms,
                "holder": name,
                "gap_ms": best_lap_ms.map(|b| b - ms),
            })),
            "personal_best_ms": personal_best.map(|pb| pb.0),
            "is_new_pb": personal_best.map(|pb| best_lap_ms == Some(pb.0)).unwrap_or(false),
            "laps": laps.iter().map(|l| json!({
                "lap": l.0, "time_ms": l.1,
                "s1": l.2, "s2": l.3, "s3": l.4,
                "valid": l.5,
            })).collect::<Vec<_>>(),
            "venue": "RacingPoint",
            "tagline": "May the Fastest Win.",
        }
    }))
}

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

    if let Some((Some(code),)) = &existing {
        if !code.is_empty() {
            return Json(json!({ "referral_code": code }));
        }
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

// ─── Memberships ─────────────────────────────────────────────────────────────

pub(crate) async fn customer_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get active membership
    let membership: Option<(String, String, String, f64, f64, String, bool, String)> = sqlx::query_as(
        "SELECT m.id, mt.name, mt.perks, m.hours_used_minutes, mt.hours_included,
                m.expires_at, m.auto_renew, m.status
         FROM memberships m
         JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active'
         ORDER BY m.created_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get available tiers
    let tiers = sqlx::query_as::<_, (String, String, f64, i64, String)>(
        "SELECT id, name, hours_included, price_paise, perks
         FROM membership_tiers WHERE is_active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let tiers_json: Vec<Value> = tiers.iter().map(|t| {
        let perks: Value = serde_json::from_str(&t.4).unwrap_or(json!([]));
        json!({
            "id": t.0,
            "name": t.1,
            "hours_included": t.2,
            "price_paise": t.3,
            "price_display": format!("₹{}/month", t.3 / 100),
            "perks": perks,
        })
    }).collect();

    Json(json!({
        "membership": membership.map(|m| {
            let perks: Value = serde_json::from_str(&m.2).unwrap_or(json!([]));
            json!({
                "id": m.0,
                "tier_name": m.1,
                "perks": perks,
                "hours_used": m.3,
                "hours_included": m.4,
                "hours_remaining": (m.4 - m.3).max(0.0),
                "expires_at": m.5,
                "auto_renew": m.6,
                "status": m.7,
            })
        }),
        "available_tiers": tiers_json,
    }))
}

pub(crate) async fn customer_subscribe_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let tier_id = match body.get("tier_id").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "tier_id required" })),
    };

    // Check tier exists
    let tier: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, price_paise FROM membership_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let tier = match tier {
        Some(t) => t,
        None => return Json(json!({ "error": "Invalid membership tier" })),
    };

    // Check no active membership
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM memberships WHERE driver_id = ? AND status = 'active'",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if active.map(|a| a.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You already have an active membership" }));
    }

    let membership_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO memberships (id, driver_id, tier_id, hours_used_minutes, price_paise, expires_at, auto_renew, status, venue_id)
         VALUES (?, ?, ?, 0, ?, datetime('now', '+30 days'), 0, 'active', ?)",
    )
    .bind(&membership_id)
    .bind(&driver_id)
    .bind(tier_id)
    .bind(tier.1)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    Json(json!({
        "ok": true,
        "membership_id": membership_id,
        "tier_name": tier.0,
        "message": format!("Welcome to {} membership!", tier.0),
    }))
}
