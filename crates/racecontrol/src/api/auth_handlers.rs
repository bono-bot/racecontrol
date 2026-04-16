#![allow(unused_imports)]
use super::kiosk_handlers::{PIN_REDEEM_MAX_ATTEMPTS, PIN_REDEEM_LOCKOUT_SECONDS};
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

// ─── Auth Endpoints ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct AssignCustomerRequest {
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    auth_type: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    experience_id: Option<String>,
}

pub(crate) async fn assign_customer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AssignCustomerRequest>,
) -> Json<Value> {
    match auth::create_auth_token(
        &state,
        req.pod_id,
        req.driver_id,
        req.pricing_tier_id,
        req.auth_type,
        req.custom_price_paise,
        req.custom_duration_minutes,
        req.experience_id,
        None, // custom_launch_args (staff assign doesn't use custom booking)
    )
    .await
    {
        Ok(token_info) => Json(json!({ "token": token_info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn cancel_assignment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match auth::cancel_auth_token(&state, id).await {
        Ok(()) => Json(json!({ "status": "cancelled" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn pending_auth_tokens(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    Json(json!({ "tokens": tokens }))
}

pub(crate) async fn pending_auth_token_for_pod(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    let token = tokens.into_iter().find(|t| t.pod_id == pod_id);
    match token {
        Some(t) => Json(json!({ "token": t })),
        None => Json(json!({ "token": null })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct ValidatePinRequest {
    pod_id: String,
    pin: String,
}

pub(crate) async fn validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin(&state, req.pod_id, req.pin).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => {
            state.record_api_error("auth/validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct KioskValidatePinRequest {
    pin: String,
    pod_id: Option<String>,
}

pub(crate) async fn kiosk_validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KioskValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin_kiosk(&state, req.pin, req.pod_id).await {
        Ok(result) => Json(json!({
            "status": "ok",
            "billing_session_id": result.billing_session_id,
            "pod_id": result.pod_id,
            "pod_number": result.pod_number,
            "driver_name": result.driver_name,
            "pricing_tier_name": result.pricing_tier_name,
            "allocated_seconds": result.allocated_seconds,
        })),
        Err(e) => {
            state.record_api_error("auth/kiosk-validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

// ─── PIN Redemption Lockout ─────────────────────────────────────────────────

#[allow(dead_code)]
pub(crate) struct PinLockoutState {
    pub(crate) fail_count: u32,
    pub(crate) last_attempt: std::time::Instant,
    pub(crate) locked_until: Option<std::time::Instant>,
}

pub(crate) static PIN_LOCKOUT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, PinLockoutState>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Prune lockout entries older than 10 minutes to prevent unbounded memory growth.
pub(crate) fn prune_pin_lockout_entries(map: &mut std::collections::HashMap<std::net::IpAddr, PinLockoutState>) {
    let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(600);
    map.retain(|_, v| v.last_attempt > cutoff);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct KioskRedeemPinRequest {
    pin: String,
}

pub(crate) async fn kiosk_redeem_pin(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<KioskRedeemPinRequest>,
) -> Json<Value> {
    let client_ip = addr.ip();

    // Check lockout FIRST
    {
        let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());

        // Prune old entries periodically (when map grows large)
        if lockout_map.len() > 1000 {
            prune_pin_lockout_entries(&mut lockout_map);
        }

        if let Some(entry) = lockout_map.get_mut(&client_ip)
            && let Some(locked_until) = entry.locked_until {
                let now = std::time::Instant::now();
                if now < locked_until {
                    let remaining = locked_until.duration_since(now);
                    let remaining_secs = remaining.as_secs();
                    let minutes = remaining_secs / 60;
                    let seconds = remaining_secs % 60;
                    let time_str = if minutes > 0 {
                        format!("{} minutes and {} seconds", minutes, seconds)
                    } else {
                        format!("{} seconds", seconds)
                    };
                    return Json(json!({
                        "error": format!("Too many failed attempts. Please wait {}.", time_str),
                        "lockout_remaining_seconds": remaining_secs,
                    }));
                } else {
                    // Lockout expired, reset
                    entry.fail_count = 0;
                    entry.locked_until = None;
                }
            }
    }

    match reservation::redeem_pin(&state, &req.pin).await {
        Ok(result) => {
            // Success: reset lockout for this IP
            let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
            lockout_map.remove(&client_ip);
            Json(result)
        }
        Err(e) => {
            state.record_api_error("kiosk/redeem-pin");

            // B1 fix: Only count actual PIN errors toward lockout.
            // "All pods busy", "DB error", "billing failed" should NOT punish the customer.
            if e.is_pin_error {
                let remaining_attempts = {
                    let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
                    let entry = lockout_map.entry(client_ip).or_insert(PinLockoutState {
                        fail_count: 0,
                        last_attempt: std::time::Instant::now(),
                        locked_until: None,
                    });
                    entry.fail_count += 1;
                    entry.last_attempt = std::time::Instant::now();

                    if entry.fail_count >= PIN_REDEEM_MAX_ATTEMPTS {
                        entry.locked_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(PIN_REDEEM_LOCKOUT_SECONDS as u64));
                        0u32
                    } else {
                        PIN_REDEEM_MAX_ATTEMPTS - entry.fail_count
                    }
                };

                if remaining_attempts == 0 {
                    let lockout_min = PIN_REDEEM_LOCKOUT_SECONDS / 60;
                    let lockout_sec = PIN_REDEEM_LOCKOUT_SECONDS % 60;
                    Json(json!({
                        "error": format!("Too many failed attempts. Please wait {} minutes and {} seconds.", lockout_min, lockout_sec),
                        "lockout_remaining_seconds": PIN_REDEEM_LOCKOUT_SECONDS,
                        "status": "lockout",
                    }))
                } else {
                    Json(json!({
                        "error": e.message,
                        "remaining_attempts": remaining_attempts,
                        "status": "invalid_pin",
                    }))
                }
            } else if e.is_pending_debit {
                // F4 fix: dedicated status field instead of relying on string matching
                Json(json!({
                    "error": e.message,
                    "status": "pending_debit",
                }))
            } else {
                // Infrastructure error — no lockout penalty
                Json(json!({
                    "error": e.message,
                    "status": "error",
                }))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct StartNowRequest {
    token_id: String,
}

pub(crate) async fn start_now(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartNowRequest>,
) -> Json<Value> {
    match auth::start_now(&state, req.token_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ValidateQrRequest {
    qr_token: String,
    driver_id: String,
}

pub(crate) async fn validate_qr(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidateQrRequest>,
) -> Json<Value> {
    match auth::validate_qr(&state, req.qr_token, req.driver_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}
