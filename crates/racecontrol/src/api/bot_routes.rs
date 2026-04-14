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

// Re-export query/info endpoints from submodule so `use super::bot_routes::*` still works
pub(crate) use super::bot_routes_query::*;

// ─── Bot endpoints (WhatsApp bot, terminal_secret auth) ─────────────────────

pub(crate) fn validate_bot_secret(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Json<Value>> {
    let secret = state.config.cloud.terminal_secret.as_deref()
        .ok_or_else(|| Json(json!({ "error": "Terminal secret not configured" })))?;
    let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
    if provided != Some(secret) {
        return Err(Json(json!({ "error": "Unauthorized" })));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BotLookupQuery {
    phone: String,
}

pub(crate) async fn bot_lookup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotLookupQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let phone = params.phone.trim();
    if phone.is_empty() {
        return Json(json!({ "error": "Phone number required" }));
    }

    // Look up driver by phone hash
    let ph = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        "SELECT id, name, phone_enc, COALESCE(has_used_trial, 0) FROM drivers WHERE phone_hash = ?",
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some((id, name, _phone, has_used_trial))) => {
            // Get wallet balance
            let balance = wallet::get_balance(&state, &id).await.unwrap_or(0);

            Json(json!({
                "registered": true,
                "driver_id": id,
                "name": name,
                "wallet_balance_paise": balance,
                "has_used_trial": has_used_trial,
            }))
        }
        Ok(None) => Json(json!({
            "registered": false,
            "message": "No account found for this phone number",
        })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn bot_pricing(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BotBookRequest {
    phone: String,
    pricing_tier_id: String,
    experience_id: Option<String>,
}

pub(crate) async fn bot_book(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BotBookRequest>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Look up driver by phone hash
    let ph = state.field_cipher.hash_phone(&req.phone);
    let driver = sqlx::query_as::<_, (String, String, bool, bool)>(
        "SELECT id, name, COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0) FROM drivers WHERE phone_hash = ?",
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    let (driver_id, driver_name, has_used_trial, unlimited_trials) = match driver {
        Ok(Some(d)) => d,
        Ok(None) => return Json(json!({
            "status": "error",
            "error": "not_registered",
            "message": "No account found for this phone number. Please register at app.racingpoint.cloud first.",
        })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    // Validate pricing tier
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "status": "error", "error": "invalid_tier", "message": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    let is_trial = tier.4;
    let price_paise = tier.3;
    let duration_minutes = tier.2;

    // Trial check — per-racer AND per-account group cap (skip for unlimited_trials)
    if is_trial && !unlimited_trials {
        if has_used_trial {
            return Json(json!({
                "status": "error",
                "error": "trial_used",
                "message": "You've already used your free trial.",
            }));
        }
        let group_parent = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT linked_to FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.0);
        let root_id = group_parent.as_deref().unwrap_or(&driver_id);
        let group_trials: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM drivers WHERE (id = ?1 OR linked_to = ?1) AND has_used_trial = 1",
        )
        .bind(root_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));
        if group_trials.0 >= 4 {
            return Json(json!({
                "status": "error",
                "error": "trial_used",
                "message": "Maximum free trials reached for this account.",
            }));
        }
    }

    // Wallet balance check for non-trial
    if !is_trial {
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "status": "error",
                "error": "insufficient_balance",
                "message": format!("Insufficient balance. You have ₹{} but need ₹{}.", balance / 100, price_paise / 100),
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }
    }

    // Check for existing active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "status": "error",
            "error": "active_reservation",
            "message": "You already have an active reservation.",
            "reservation_id": existing.id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({
            "status": "error",
            "error": "no_pods",
            "message": "No pods available right now. Please try again shortly or visit us to get in the queue.",
        })),
    };

    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial)
    let (wallet_txn_id, wallet_debit) = if !is_trial && price_paise > 0 {
        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("{} on Pod {} (WhatsApp)", tier.1, pod_number)),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(price_paise)),
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": e }));
        }
    };

    // Create auth token (PIN type)
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None,
        None,
        experience_id,
        None,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": format!("Failed to create auth: {}", e) }));
        }
    };

    Json(json!({
        "status": "booked",
        "booking_id": reservation_id,
        "driver_name": driver_name,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "duration_minutes": duration_minutes,
        "tier_name": tier.1,
        "wallet_debit_paise": wallet_debit,
        "message": format!(
            "Session booked! Head to Pod {} and enter PIN {} on the screen. You have {} minutes.",
            pod_number, auth_token.token, duration_minutes
        ),
    }))
}

// Query/info endpoints moved to bot_routes_query.rs
