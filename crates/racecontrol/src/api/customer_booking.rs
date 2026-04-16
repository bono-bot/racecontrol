#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use super::billing_coupon::{validate_and_calc_coupon, record_coupon_redemption};
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

// ─── Customer Booking ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CustomBookingOptions {
    game: String,
    game_mode: Option<String>,
    track: String,
    car: String,
    difficulty: String,
    transmission: String,
    #[serde(default = "default_ffb_preset")]
    ffb: String,
    #[serde(default)]
    session_type: Option<String>,
}

pub(crate) fn default_ffb_preset() -> String { "medium".to_string() }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct BookSessionRequest {
    experience_id: Option<String>,
    pricing_tier_id: String,
    custom: Option<CustomBookingOptions>,
    coupon_code: Option<String>,
}

pub(crate) async fn customer_book_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BookSessionRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Validate pricing tier and get price
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let is_trial = tier.4;
    let base_price_paise = tier.3;

    // Apply coupon discount if provided
    let mut applied_discount_paise: i64 = 0;
    let mut applied_coupon_id: Option<String> = None;
    let mut applied_discount_reason: Option<String> = None;

    if !is_trial
        && let Some(ref code) = req.coupon_code {
            match validate_and_calc_coupon(&state, code, &driver_id, base_price_paise).await {
                Ok(cd) => {
                    applied_discount_paise = cd.discount_paise;
                    applied_coupon_id = Some(cd.coupon_id);
                    applied_discount_reason = Some(format!("Coupon {}: {}", code.to_uppercase(), cd.description));
                }
                Err(e) => return Json(json!({ "error": e })),
            }
        }

    let final_price_paise = base_price_paise - applied_discount_paise;

    // Handle trial booking (skip for unlimited_trials drivers)
    if is_trial {
        let trial_info = sqlx::query_as::<_, (bool, bool)>(
            "SELECT COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0) FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await;

        match trial_info {
            Ok(Some((true, false))) => return Json(json!({ "error": "Free trial already used" })),
            Ok(None) => return Json(json!({ "error": "Driver not found" })),
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
            _ => {} // OK to proceed (hasn't used trial, or has unlimited_trials)
        }
    } else {
        // Validate wallet balance for non-trial (using discounted price)
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": e })),
        };

        if balance < final_price_paise {
            return Json(json!({
                "error": "Insufficient wallet balance",
                "balance_paise": balance,
                "required_paise": final_price_paise,
            }));
        }
    }

    // Check if driver already has an active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "error": "You already have an active reservation",
            "reservation_id": existing.id,
            "pod_id": existing.pod_id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({ "error": "No pods available right now. Please try again shortly." })),
    };

    // Get pod number for display
    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial) — uses discounted price
    let (wallet_txn_id, wallet_debit) = if !is_trial && final_price_paise > 0 {
        let debit_notes = if applied_discount_paise > 0 {
            format!("{} on Pod {} — {} credits discount", tier.1, pod_number, applied_discount_paise / 100)
        } else {
            format!("{} on Pod {}", tier.1, pod_number)
        };
        match wallet::debit(
            &state,
            &driver_id,
            final_price_paise,
            "debit_session",
            None,
            Some(&debit_notes),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(final_price_paise)),
            Err(e) => return Json(json!({ "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            // Refund if we already debited
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": e }));
        }
    };

    // Validate: must have either experience_id or custom, not both, not neither
    if req.experience_id.is_none() && req.custom.is_none() {
        // Refund if we already debited
        if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
            let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
        }
        let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
        return Json(json!({ "error": "Either experience_id or custom must be provided" }));
    }

    // Build custom launch args if custom booking
    let custom_launch_args = req.custom.as_ref().map(|c| {
        // Get driver name for launch args
        let driver_name_for_args = "Driver"; // Will be set properly by launch_or_assist
        catalog::build_custom_launch_args(
            &c.car, &c.track, driver_name_for_args, &c.difficulty, &c.transmission, &c.ffb,
            c.session_type.as_deref().unwrap_or("practice"),
        ).to_string()
    });

    // For custom bookings, also embed game info in the launch args
    let custom_launch_args = if let Some(ref args) = custom_launch_args {
        if let Some(ref c) = req.custom {
            let mut parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
            parsed["game"] = serde_json::json!(c.game);
            parsed["game_mode"] = serde_json::json!(c.game_mode.as_deref().unwrap_or("single"));
            parsed["session_type"] = serde_json::json!(c.session_type.as_deref().unwrap_or("practice"));
            Some(parsed.to_string())
        } else {
            custom_launch_args
        }
    } else {
        None
    };

    // Create auth token (PIN type) for this pod
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None, // custom_price_paise
        None, // custom_duration_minutes
        experience_id,
        custom_launch_args,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            // Cleanup: end reservation + refund
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": format!("Failed to create auth: {}", e) }));
        }
    };

    // Record coupon redemption if applicable
    // We use reservation_id as a stand-in since the billing_session isn't created until PIN auth
    if let Some(ref cid) = applied_coupon_id {
        record_coupon_redemption(&state, cid, &driver_id, &reservation_id, applied_discount_paise).await;
    }

    Json(json!({
        "status": "booked",
        "reservation_id": reservation_id,
        "pod_id": pod_id,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "wallet_debit_paise": wallet_debit,
        "wallet_txn_id": wallet_txn_id,
        "discount_paise": applied_discount_paise,
        "original_price_paise": base_price_paise,
        "discount_reason": applied_discount_reason,
    }))
}

pub(crate) async fn customer_active_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await;

    match reservation {
        Some(res) => {
            // Get pod number
            let pod_number = {
                let pods = state.pods.read().await;
                pods.get(&res.pod_id).map(|p| p.number).unwrap_or(0)
            };

            // Check if there's an active billing session on this pod
            let active_billing = {
                let rate_tiers = state.billing.rate_tiers.read().await;
                let timers = state.billing.active_timers.read().await;
                timers.get(&res.pod_id).map(|t| t.to_info(&rate_tiers))
            };

            Json(json!({
                "reservation": res,
                "pod_number": pod_number,
                "active_billing": active_billing,
            }))
        }
        None => Json(json!({ "reservation": null })),
    }
}

pub(crate) async fn customer_end_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = match pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation" })),
    };

    // End any active billing on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if let Some(timer) = timers.get(&reservation.pod_id) {
            let session_id = timer.session_id.clone();
            drop(timers);

            // Proportional refund
            let billing = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                "SELECT allocated_seconds, driving_seconds, wallet_debit_paise FROM billing_sessions WHERE id = ?",
            )
            .bind(&session_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((allocated, driving, Some(debit))) = billing
                && debit > 0 && driving < allocated {
                    let remaining = allocated - driving;
                    let refund_amount = (remaining * debit) / allocated;
                    if refund_amount > 0 {
                        let _ = wallet::refund(
                            &state,
                            &driver_id,
                            refund_amount,
                            Some(&session_id),
                            Some("Early end — proportional refund"),
                        )
                        .await;
                    }
                }

            billing::end_billing_session_public(&state, &session_id, rc_common::types::BillingSessionStatus::EndedEarly, None).await;
        }
    }

    // End the reservation
    let _ = pod_reservation::end_reservation(&state, &reservation.id).await;

    Json(json!({ "status": "ok" }))
}

// Continue session (multi-sub-session) handler is in customer_booking_continue.rs
