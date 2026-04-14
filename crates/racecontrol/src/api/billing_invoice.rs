#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
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

/// Staff endpoint: GET /billing/sessions/{id}/invoice
/// Returns the GST-compliant invoice for a billing session (manager+ access via RBAC).
pub(crate) async fn get_session_invoice(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, i64, String, String, i64, f64, i64, i64, i64, String)>(
        "SELECT id, invoice_number, driver_name, venue_gstin, \
         taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise, total_paise, created_at \
         FROM invoices WHERE billing_session_id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((id, invoice_number, driver_name, venue_gstin,
                 taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise,
                 total_paise, created_at))) => {
            Json(json!({
                "id": id,
                "invoice_number": invoice_number,
                "billing_session_id": session_id,
                "driver_name": driver_name,
                "venue_gstin": venue_gstin,
                "sac_code": "999692",
                "taxable_value_paise": taxable_value_paise,
                "gst_rate_percent": gst_rate_percent,
                "cgst_paise": cgst_paise,
                "sgst_paise": sgst_paise,
                "total_paise": total_paise,
                "created_at": created_at,
            }))
        }
        Ok(None) => Json(json!({ "error": "Invoice not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Customer endpoint: GET /customer/sessions/{id}/invoice
/// Returns the GST invoice for the authenticated customer's own session.
pub(crate) async fn customer_session_invoice(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    // Extract driver_id from JWT — only the session owner can fetch their invoice
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let row = sqlx::query_as::<_, (String, i64, String, String, i64, f64, i64, i64, i64, String)>(
        "SELECT inv.id, inv.invoice_number, inv.driver_name, inv.venue_gstin, \
         inv.taxable_value_paise, inv.gst_rate_percent, inv.cgst_paise, inv.sgst_paise, \
         inv.total_paise, inv.created_at \
         FROM invoices inv \
         JOIN billing_sessions bs ON inv.billing_session_id = bs.id \
         WHERE inv.billing_session_id = ? AND bs.driver_id = ?",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((id, invoice_number, driver_name, venue_gstin,
                 taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise,
                 total_paise, created_at))) => {
            Json(json!({
                "id": id,
                "invoice_number": invoice_number,
                "billing_session_id": session_id,
                "driver_name": driver_name,
                "venue_gstin": venue_gstin,
                "sac_code": "999692",
                "taxable_value_paise": taxable_value_paise,
                "gst_rate_percent": gst_rate_percent,
                "cgst_paise": cgst_paise,
                "sgst_paise": sgst_paise,
                "total_paise": total_paise,
                "created_at": created_at,
            }))
        }
        Ok(None) => Json(json!({ "error": "Invoice not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── UX-03: Customer session receipt ─────────────────────────────────────

/// Customer endpoint: GET /customer/sessions/{id}/receipt
/// Returns full financial breakdown: charges, GST breakup, refund, before/after balance.
/// Only the session owner (driver_id from JWT) can access their own receipt.
pub(crate) async fn customer_session_receipt(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch session row — verify session belongs to this driver
    let session = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT bs.id, d.id, d.name, bs.driving_seconds, bs.wallet_debit_paise, bs.status,
                bs.started_at, bs.ended_at, bs.refund_paise, bs.allocated_seconds
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (sid, _did, driver_name, driving_seconds, wallet_debit_paise, status,
         started_at, ended_at, refund_paise_opt, _allocated) = session;
    let refund_paise = refund_paise_opt.unwrap_or(0);

    // Current wallet balance (after-balance)
    let balance_after_paise: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN txn_type LIKE 'credit%' OR txn_type LIKE 'refund%' THEN amount_paise ELSE -amount_paise END), 0)
         FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // Reconstruct before-balance: add back what was charged, subtract back the refund
    let balance_before_paise = balance_after_paise + wallet_debit_paise - refund_paise;

    // GST breakup from invoices table (generated by Phase 255 post_session_debit_gst)
    let invoice = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT taxable_value_paise, cgst_paise, sgst_paise, total_paise
         FROM invoices WHERE billing_session_id = ?",
    )
    .bind(&sid)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (net_paise, cgst_paise, sgst_paise, gst_total_paise) = invoice
        .map(|i| (i.0, i.1, i.2, i.1 + i.2))
        .unwrap_or_else(|| {
            // Fallback: compute 18% inclusive GST split from wallet_debit_paise
            let net = wallet_debit_paise * 100 / 118;
            let gst = wallet_debit_paise - net;
            let half = gst / 2;
            (net, half, gst - half, gst)
        });

    Json(json!({
        "session_id": sid,
        "driver_name": driver_name,
        "started_at": started_at,
        "ended_at": ended_at,
        "duration_seconds": driving_seconds,
        "charges_paise": wallet_debit_paise,
        "gst_paise": gst_total_paise,
        "cgst_paise": cgst_paise,
        "sgst_paise": sgst_paise,
        "net_paise": net_paise,
        "refund_paise": refund_paise,
        "balance_before_paise": balance_before_paise,
        "balance_after_paise": balance_after_paise,
        "status": status,
    }))
}

// ─── Billing Refund ───────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct BillingRefundRequest {
    amount_paise: i64,
    method: String,       // "wallet", "cash", "upi"
    reason: String,
    notes: Option<String>,
    // FATM-02: Optional idempotency key — duplicate requests return the original result
    idempotency_key: Option<String>,
}

pub(crate) async fn refund_billing_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<BillingRefundRequest>,
) -> Json<Value> {
    // Extract staff_id from JWT (POS-05: audit trail with staff_id)
    let staff_id = claims.map(|c| c.0.sub.clone());

    // FATM-02: Idempotency check for refund
    if let Some(ref key) = req.idempotency_key {
        let existing = sqlx::query_as::<_, (String, i64)>(
            "SELECT id, amount_paise FROM refunds WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((existing_id, existing_amount)) = existing {
            return Json(json!({
                "status": "ok",
                "refund_id": existing_id,
                "amount_paise": existing_amount,
                "idempotent_replay": true,
            }));
        }
    }

    // Validate method
    if !["wallet", "cash", "upi"].contains(&req.method.as_str()) {
        return Json(json!({ "error": "method must be wallet, cash, or upi" }));
    }
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "amount_paise must be positive" }));
    }
    if req.reason.trim().is_empty() {
        return Json(json!({ "error": "reason is required" }));
    }

    // Fetch session
    let session = sqlx::query_as::<_, (String, String, Option<i64>, String)>(
        "SELECT bs.id, bs.driver_id, bs.wallet_debit_paise, d.name
         FROM billing_sessions bs JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    let (_sid, driver_id, debit_paise, driver_name) = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let max_refundable = debit_paise.unwrap_or(0);

    // Check total already refunded for this session
    let already_refunded: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM refunds WHERE billing_session_id = ?",
    )
    .bind(&session_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let remaining = max_refundable - already_refunded;
    if req.amount_paise > remaining {
        return Json(json!({
            "error": format!("Refund exceeds remaining refundable amount. Charged: {}, already refunded: {}, remaining: {}", max_refundable, already_refunded, remaining)
        }));
    }

    let refund_id = uuid::Uuid::new_v4().to_string();
    let mut wallet_txn_id: Option<String> = None;

    // If wallet refund, credit the wallet
    if req.method == "wallet" {
        match wallet::credit(
            &state,
            &driver_id,
            req.amount_paise,
            "refund_session",
            Some(&session_id),
            Some(&format!("Refund: {}", req.reason)),
            None,
        )
        .await
        {
            Ok(_new_balance) => {
                // Get the txn_id from the most recent transaction
                let txn = sqlx::query_as::<_, (String,)>(
                    "SELECT id FROM wallet_transactions WHERE driver_id = ? AND txn_type = 'refund_session' ORDER BY created_at DESC LIMIT 1"
                )
                .bind(&driver_id)
                .fetch_optional(&state.db)
                .await;
                wallet_txn_id = txn.ok().flatten().map(|r| r.0);
                tracing::info!("Refund to wallet: {} +{}p (session {})", driver_id, req.amount_paise, session_id);
            }
            Err(e) => return Json(json!({ "error": format!("Wallet credit failed: {}", e) })),
        }
    } else {
        tracing::info!("Refund via {}: {} {}p (session {})", req.method, driver_id, req.amount_paise, session_id);
    }

    // Record in refunds table (include idempotency_key for FATM-02)
    let result = sqlx::query(
        "INSERT INTO refunds (id, billing_session_id, driver_id, amount_paise, method, reason, notes, staff_id, wallet_txn_id, idempotency_key, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&refund_id)
    .bind(&session_id)
    .bind(&driver_id)
    .bind(req.amount_paise)
    .bind(&req.method)
    .bind(&req.reason)
    .bind(req.notes.as_deref())
    .bind(staff_id.as_deref()) // staff_id from JWT (POS-05)
    .bind(wallet_txn_id.as_deref())
    .bind(req.idempotency_key.as_deref())
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Audit trail with staff_id from JWT (POS-05)
            accounting::log_audit(
                &state,
                "refunds",
                &refund_id,
                "create",
                None,
                Some(&serde_json::json!({
                    "billing_session_id": session_id,
                    "driver_id": driver_id,
                    "amount_paise": req.amount_paise,
                    "method": req.method,
                    "reason": req.reason,
                }).to_string()),
                staff_id.as_deref(),
            )
            .await;

            // Update refund_paise on billing_sessions for cloud sync
            let _ = sqlx::query(
                "UPDATE billing_sessions SET refund_paise = COALESCE(refund_paise, 0) + ? WHERE id = ?",
            )
            .bind(req.amount_paise)
            .bind(&session_id)
            .execute(&state.db)
            .await;

            Json(json!({
                "status": "ok",
                "refund_id": refund_id,
                "amount_paise": req.amount_paise,
                "method": req.method,
                "driver_name": driver_name,
                "total_refunded_paise": already_refunded + req.amount_paise,
                "max_refundable_paise": max_refundable,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to record refund: {}", e) })),
    }
}

pub(crate) async fn get_billing_refunds(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let refunds = sqlx::query_as::<_, (String, String, String, i64, String, String, Option<String>, Option<String>, String)>(
        "SELECT r.id, r.billing_session_id, r.driver_id, r.amount_paise, r.method, r.reason, r.notes, r.wallet_txn_id, r.created_at
         FROM refunds r WHERE r.billing_session_id = ? ORDER BY r.created_at DESC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await;

    match refunds {
        Ok(rows) => {
            let list: Vec<Value> = rows.iter().map(|r| json!({
                "id": r.0,
                "billing_session_id": r.1,
                "driver_id": r.2,
                "amount_paise": r.3,
                "method": r.4,
                "reason": r.5,
                "notes": r.6,
                "wallet_txn_id": r.7,
                "created_at": r.8,
            })).collect();
            let total: i64 = rows.iter().map(|r| r.3).sum();
            Json(json!({ "refunds": list, "total_refunded_paise": total }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e), "refunds": [] })),
    }
}

// ─── Daily Report (extracted to billing_daily_report.rs) ─────────────────
pub(crate) use super::billing_daily_report::daily_billing_report;
