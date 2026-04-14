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

// ─── Wallet (staff-facing) ───────────────────────────────────────────────────

pub(crate) async fn wallet_bonus_tiers(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let tiers: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, min_amount_paise, bonus_percent, sort_order FROM bonus_tiers WHERE is_active = 1 ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let tiers_json: Vec<Value> = tiers.iter().map(|t| json!({
        "id": t.0,
        "min_paise": t.1,
        "bonus_pct": t.2,
        "sort_order": t.3,
    })).collect();

    Json(json!({ "tiers": tiers_json }))
}

/// v47.0 Phase 360 (SSOT): unified wallet top-up preset amounts.
/// Both the PWA wallet/topup page and the POS WalletTopupModal fetch from this
/// endpoint instead of hardcoding their own lists. Fixes the drift where PWA
/// showed [500,1000,2000,3000,4000,5000] but POS showed [500,700,900,1000,2000,3000].
///
/// Reads from `system_settings` key `wallet_topup_presets_paise` (JSON array of i64).
/// Falls back to a hardcoded safe default if unset so venues never see an empty list.
pub(crate) async fn wallet_topup_presets(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Safe default — used if system_settings has nothing configured.
    // These are the UNION of PWA + POS lists from before v47.0 Phase 360.
    const SAFE_DEFAULT_PAISE: &[i64] = &[50_000, 70_000, 90_000, 100_000, 200_000, 300_000, 400_000, 500_000];

    let stored: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM system_settings WHERE key = 'wallet_topup_presets_paise'"
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let has_stored = stored.is_some();
    let presets_paise: Vec<i64> = stored
        .and_then(|(json_str,)| serde_json::from_str::<Vec<i64>>(&json_str).ok())
        .unwrap_or_else(|| SAFE_DEFAULT_PAISE.to_vec());

    let presets_json: Vec<Value> = presets_paise.iter().map(|p| json!({
        "paise": p,
        "rupees": p / 100,
        "label": format!("{}", p / 100),
    })).collect();

    Json(json!({
        "presets": presets_json,
        "source": if has_stored { "system_settings" } else { "default" },
    }))
}

pub(crate) async fn get_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TopupRequest {
    pub(crate) amount_paise: i64,
    pub(crate) method: String, // cash, card, upi
    pub(crate) notes: Option<String>,
    pub(crate) staff_id: Option<String>,
    // FATM-02: Optional idempotency key — duplicate requests return the original result
    pub(crate) idempotency_key: Option<String>,
}

pub(crate) async fn topup_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<TopupRequest>,
) -> Json<Value> {
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "amount_paise must be greater than 0" }));
    }

    // SEC-05: Staff self-top-up block — cashier/manager cannot top up their own wallet.
    // Superadmin is exempt (audit trail exists for all transactions).
    if let Some(ref ext) = claims {
        if ext.0.sub == driver_id && ext.0.role != "superadmin" {
            tracing::warn!(
                staff_id = %ext.0.sub,
                target_driver = %driver_id,
                role = %ext.0.role,
                "SEC-05: Self-topup blocked for non-superadmin"
            );
            return Json(json!({ "error": "Staff cannot top up their own wallet. Contact a superadmin." }));
        }
    }

    // FATM-02: Idempotency check for topup
    if let Some(ref key) = req.idempotency_key {
        let existing = sqlx::query_as::<_, (i64, i64)>(
            "SELECT amount_paise, balance_after_paise FROM wallet_transactions WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((_amount, balance)) = existing {
            return Json(json!({
                "status": "ok",
                "new_balance_credits": balance,
                "bonus_credits_granted": 0,
                "rupee_amount": req.amount_paise,
                "idempotent_replay": true,
            }));
        }
    }

    let txn_type = match req.method.as_str() {
        "cash" => "topup_cash",
        "card" => "topup_card",
        "upi" => "topup_upi",
        "online" => "topup_online",
        _ => "topup_cash",
    };

    let mut new_balance = match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        txn_type,
        None,
        req.notes.as_deref(),
        req.staff_id.as_deref(),
    )
    .await
    {
        Ok(b) => b,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Bonus credit tiers — read from DB
    let bonus_row: Option<(i64,)> = sqlx::query_as(
        "SELECT bonus_percent FROM bonus_tiers WHERE is_active = 1 AND min_amount_paise <= ? ORDER BY min_amount_paise DESC LIMIT 1"
    )
    .bind(req.amount_paise)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    let bonus_pct = bonus_row.map(|r| r.0).unwrap_or(0);

    let bonus_paise = if bonus_pct > 0 {
        let bp = req.amount_paise * bonus_pct / 100;
        let _ = wallet::credit(
            &state,
            &driver_id,
            bp,
            "bonus",
            None,
            Some(&format!("{}% topup bonus on {} credits", bonus_pct, req.amount_paise / 100)),
            req.staff_id.as_deref(),
        )
        .await;
        new_balance = wallet::get_balance(&state, &driver_id).await.unwrap_or(new_balance);
        bp
    } else {
        0
    };

    // Audit trail + WhatsApp alert for wallet topup (HIGH sensitivity)
    accounting::log_admin_action(
        &state, "wallet_topup",
        &json!({"driver_id": driver_id, "amount_paise": req.amount_paise, "method": req.method}).to_string(),
        req.staff_id.as_deref(), None,
    ).await;
    whatsapp_alerter::send_admin_alert(
        &state.config, "Wallet Topup",
        &format!("{} paise for driver {}", req.amount_paise, driver_id),
    ).await;

    // Send WhatsApp receipt to customer (best-effort, non-blocking)
    {
        let receipt_state = state.clone();
        let receipt_driver_id = driver_id.clone();
        let receipt_amount = req.amount_paise;
        let receipt_method = req.method.clone();
        let receipt_balance = new_balance;
        let receipt_bonus = bonus_paise;
        tokio::spawn(async move {
            // Look up customer phone
            let phone: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT phone FROM drivers WHERE id = ?",
            )
            .bind(&receipt_driver_id)
            .fetch_optional(&receipt_state.db)
            .await
            .unwrap_or(None);

            if let Some((Some(phone),)) = phone {
                if !phone.is_empty() {
                    let bonus_text = if receipt_bonus > 0 {
                        format!(" (+{} bonus credits)", receipt_bonus / 100)
                    } else {
                        String::new()
                    };
                    let msg = format!(
                        "Racing Point - {} credits added to your wallet via {}{}.\\nNew balance: {} credits.\\nThank you for racing with us!",
                        receipt_amount / 100,
                        receipt_method,
                        bonus_text,
                        receipt_balance / 100,
                    );
                    whatsapp_alerter::send_whatsapp_to(&receipt_state.config, &phone, &msg).await;
                }
            }
        });
    }

    // LEGAL-08: Update last_activity_at — wallet topup is customer activity.
    // Non-critical: failure does not affect the topup result.
    let _ = sqlx::query(
        "UPDATE drivers SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    let max_cash_refund = wallet::get_max_cash_refund(&state, &driver_id).await.unwrap_or(0);
    Json(json!({
        "status": "ok",
        "new_balance_credits": new_balance,
        "bonus_credits_granted": bonus_paise,
        "rupee_amount": req.amount_paise,
        "max_cash_refund": max_cash_refund,
    }))
}

// ─── FATM-11: Payment gateway webhook (extracted to wallet_gateway.rs) ──────
pub(crate) use super::wallet_gateway::payment_gateway_webhook;

/// UX-02: OTP fallback display endpoint.
/// Customer polls this URL if WhatsApp delivery failed.
/// Returns the OTP payload from the notification outbox via a one-time token.
/// Token is consumed (marked 'delivered') on first successful read.
pub(crate) async fn otp_fallback_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> axum::response::Response {
    if token.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "error": "token is required" })),
        )
            .into_response();
    }

    match crate::notification_outbox::get_otp_by_fallback_token(&state.db, &token).await {
        Ok(Some(otp)) => {
            tracing::info!(target: "notification_outbox", "OTP fallback token consumed");
            Json(json!({ "otp": otp })).into_response()
        }
        Ok(None) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "OTP not available. WhatsApp delivery may still be in progress, or token already used." })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(target: "notification_outbox", "OTP fallback lookup error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "lookup error" })),
            )
                .into_response()
        }
    }
}
