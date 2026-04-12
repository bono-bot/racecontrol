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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DebitRequest {
    amount_paise: i64,
    reason: String, // cafe, merchandise, penalty, etc.
    reference_id: Option<String>,
    notes: Option<String>,
}

pub(crate) async fn debit_wallet_manual(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<DebitRequest>,
) -> Json<Value> {
    let txn_type = format!("debit_{}", req.reason);

    match wallet::debit(
        &state,
        &driver_id,
        req.amount_paise,
        &txn_type,
        req.reference_id.as_deref(),
        req.notes.as_deref(),
    )
    .await
    {
        Ok((new_balance, txn_id)) => Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
            "txn_id": txn_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn wallet_transactions(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let txns = wallet::get_transactions(&state, &driver_id, limit).await;
    Json(json!({ "transactions": txns }))
}

pub(crate) async fn all_wallet_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let date = params.get("date").cloned().unwrap_or(today);
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(200i64);

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String, String, Option<String>, String)>(
        "SELECT wt.id, wt.driver_id, wt.amount_paise, wt.balance_after_paise, wt.txn_type, \
         wt.reference_id, wt.notes, wt.staff_id, wt.created_at, \
         COALESCE(d.name, 'Unknown') as driver_name, d.phone as driver_phone, \
         COALESCE(wt.currency_type, 'credit') as currency_type \
         FROM wallet_transactions wt \
         LEFT JOIN drivers d ON d.id = wt.driver_id \
         WHERE date(wt.created_at) = ? \
         ORDER BY wt.created_at DESC LIMIT ?",
    )
    .bind(&date)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut total_credits: i64 = 0;
    let mut total_debits: i64 = 0;
    let mut total_rupee_deposits: i64 = 0;
    let mut total_bonus_credits: i64 = 0;
    let mut total_cash_refunds: i64 = 0;

    let txns: Vec<Value> = rows.iter().map(|r| {
        let is_credit = r.4.starts_with("topup") || r.4 == "bonus" || r.4.starts_with("refund");
        if is_credit {
            total_credits += r.2;
        } else {
            total_debits += r.2;
        }
        // D-22: Track rupee/bonus/cash-refund totals
        if r.4.starts_with("topup") || r.4 == "gateway_topup" {
            total_rupee_deposits += r.2;
        }
        if r.4 == "bonus" || r.4 == "adjustment" {
            total_bonus_credits += r.2;
        }
        if r.4 == "refund_cash" {
            total_cash_refunds += r.2;
        }
        json!({
            "id": r.0,
            "driver_id": r.1,
            "amount_paise": r.2,
            "balance_after_paise": r.3,
            "txn_type": r.4,
            "reference_id": r.5,
            "notes": r.6,
            "staff_id": r.7,
            "created_at": r.8,
            "driver_name": r.9,
            "driver_phone": r.10,
            "currency_type": r.11,
        })
    }).collect();

    Json(json!({
        "transactions": txns,
        "summary": {
            "total_credits_paise": total_credits,
            "total_debits_paise": total_debits,
            "net_paise": total_credits - total_debits,
            "count": txns.len(),
            "total_rupee_deposits": total_rupee_deposits,
            "total_bonus_credits": total_bonus_credits,
            "total_cash_refunds": total_cash_refunds,
        }
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct RefundRequest {
    amount_paise: i64,
    notes: Option<String>,
    reference_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct CashRefundRequest {
    amount_paise: i64,
    notes: Option<String>,
}

pub(crate) async fn refund_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<RefundRequest>,
) -> Json<Value> {
    // MMA-203: Extract staff_id from JWT for audit trail (POS-05)
    let staff_id = claims.map(|c| c.0.sub.clone());

    // MMA-203: Validate refund amount bounds
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "Refund amount must be positive" }));
    }
    const MAX_MANUAL_REFUND_PAISE: i64 = 500_000; // ₹5,000 cap for manual wallet refunds
    if req.amount_paise > MAX_MANUAL_REFUND_PAISE {
        return Json(json!({ "error": format!("Refund exceeds maximum allowed (₹{})", MAX_MANUAL_REFUND_PAISE / 100) }));
    }

    // R2-3: Without reference_id, apply a stricter cap to prevent abuse
    if req.reference_id.is_none() && req.amount_paise > 50_000 {
        // ₹500 cap for unreferenced manual refunds (goodwill gestures)
        return Json(json!({ "error": "Refunds above ₹500 require a billing session reference_id" }));
    }

    // MMA-203: If reference_id is provided, validate it maps to a real billing session
    if let Some(ref ref_id) = req.reference_id {
        let session = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>)>(
            "SELECT id, driver_id, custom_price_paise, \
             (SELECT COALESCE(SUM(CASE WHEN txn_type='billing' THEN amount_paise ELSE 0 END), 0) \
              FROM wallet_transactions WHERE reference_id = bs.id) as session_cost \
             FROM billing_sessions bs WHERE id = ?"
        )
        .bind(ref_id)
        .fetch_optional(&state.db)
        .await;

        // P3: Extract session cost for cumulative cap enforcement
        let session_cost_paise: i64;
        match session {
            Ok(Some((_, session_driver_id, custom_price, cost))) => {
                // Verify the refund is for the correct driver
                if session_driver_id != driver_id {
                    return Json(json!({ "error": "Billing session does not belong to this driver" }));
                }
                session_cost_paise = custom_price.unwrap_or(0).max(cost.unwrap_or(0)).max(MAX_MANUAL_REFUND_PAISE);
            }
            Ok(None) => {
                return Json(json!({ "error": "Referenced billing session not found" }));
            }
            Err(e) => {
                return Json(json!({ "error": format!("DB error validating reference: {}", e) }));
            }
        }

        // R3-4: Atomic refund check+credit in a single DB transaction to prevent TOCTOU race.
        // Check BOTH wallet_transactions AND refunds tables, then credit atomically.
        let mut conn = match state.db.acquire().await {
            Ok(c) => c,
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
        };
        let mut tx = match sqlx::Acquire::begin(&mut *conn).await {
            Ok(t) => t,
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
        };

        let already_refunded_wallet = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(amount_paise), 0) FROM wallet_transactions \
             WHERE reference_id = ? AND txn_type IN ('refund_manual', 'refund_session')"
        )
        .bind(ref_id)
        .fetch_one(&mut *tx)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

        let already_refunded_billing = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(amount_paise), 0) FROM refunds WHERE billing_session_id = ?"
        )
        .bind(ref_id)
        .fetch_one(&mut *tx)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

        // P3: Cumulative refund cap — allow partial refunds but prevent over-refunding
        let total_refunded = already_refunded_wallet + already_refunded_billing;
        let would_be_total = total_refunded + req.amount_paise;
        if would_be_total > session_cost_paise {
            tracing::warn!(
                "Refund exceeds session cost: driver={}, reference_id={}, already_refunded={}p, requested={}p, session_cost={}p",
                driver_id, ref_id, total_refunded, req.amount_paise, session_cost_paise
            );
            let remaining = (session_cost_paise - total_refunded).max(0);
            // tx rolls back on drop
            return Json(json!({ "error": format!("Refund would exceed session cost. Already refunded: {}p, max remaining: {}p", total_refunded, remaining) }));
        }

        // Perform the credit within the same transaction
        wallet::ensure_wallet(&state, &driver_id).await.ok();
        let txn_id = uuid::Uuid::new_v4().to_string();

        if let Err(e) = sqlx::query(
            "UPDATE wallets SET balance_paise = balance_paise + ?, total_credited_paise = total_credited_paise + ?, updated_at = datetime('now') WHERE driver_id = ?"
        )
        .bind(req.amount_paise)
        .bind(req.amount_paise)
        .bind(&driver_id)
        .execute(&mut *tx)
        .await {
            return Json(json!({ "error": format!("DB error: {}", e) }));
        }

        if let Err(e) = sqlx::query(
            "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, venue_id) \
             VALUES (?, ?, ?, (SELECT balance_paise FROM wallets WHERE driver_id = ?), 'refund_manual', ?, ?, ?, ?)"
        )
        .bind(&txn_id)
        .bind(&driver_id)
        .bind(req.amount_paise)
        .bind(&driver_id)
        .bind(ref_id.as_str())
        .bind(req.notes.as_deref())
        .bind(staff_id.as_deref())
        .bind(&state.config.venue.venue_id)
        .execute(&mut *tx)
        .await {
            return Json(json!({ "error": format!("DB error: {}", e) }));
        }

        if let Err(e) = tx.commit().await {
            return Json(json!({ "error": format!("DB commit error: {}", e) }));
        }

        // Get new balance after commit
        let new_balance = wallet::get_balance(&state, &driver_id).await.unwrap_or(0);
        let max_cash = wallet::get_max_cash_refund(&state, &driver_id).await.unwrap_or(0);
        return Json(json!({
            "status": "ok",
            "type": "credit_refund",
            "new_balance_credits": new_balance,
            "max_cash_refund": max_cash,
        }));
    }

    // Non-referenced refund path (no reference_id)
    match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        "refund_manual",
        req.reference_id.as_deref(),
        req.notes.as_deref(),
        staff_id.as_deref(),
    )
    .await
    {
        Ok(new_balance) => {
            let max_cash = wallet::get_max_cash_refund(&state, &driver_id).await.unwrap_or(0);
            Json(json!({
                "status": "ok",
                "type": "credit_refund",
                "new_balance_credits": new_balance,
                "max_cash_refund": max_cash,
            }))
        },
        Err(e) => Json(json!({ "error": e })),
    }
}

/// POST /wallet/{driver_id}/cash-refund — Return real money to customer (D-11).
/// Admin/manager only — same auth tier as refund_wallet.
pub(crate) async fn cash_refund_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<CashRefundRequest>,
) -> Json<Value> {
    // D-15: Extract staff_id from StaffClaims
    let staff_id = claims.map(|c| c.0.sub.clone());

    // D-16: Pre-check — show cap in error message (actual enforcement is TOCTOU-safe inside cash_refund)
    let max_refund = wallet::get_max_cash_refund(&state, &driver_id).await.unwrap_or(0);
    if req.amount_paise > max_refund {
        return Json(json!({
            "error": format!("Cash refund of {}p exceeds maximum allowed {}p", req.amount_paise, max_refund),
            "max_cash_refund": max_refund,
        }));
    }

    // D-15: Call wallet::cash_refund — staff_id extracted from StaffClaims
    match wallet::cash_refund(
        &state,
        &driver_id,
        req.amount_paise,
        staff_id.as_deref(),
        req.notes.as_deref(),
    ).await {
        Ok((new_balance, _txn_id)) => {
            // D-14: Success response with type differentiation
            let remaining = wallet::get_max_cash_refund(&state, &driver_id).await.unwrap_or(0);
            Json(json!({
                "status": "ok",
                "type": "cash_refund",
                "amount": req.amount_paise,
                "new_balance_credits": new_balance,
                "max_cash_refund_remaining": remaining,
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}
