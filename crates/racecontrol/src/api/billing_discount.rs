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

// ─── STAFF-01: Discount approval gate ────────────────────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct ApplyDiscountRequest {
    discount_paise: i64,
    reason_code: String,
    manager_approval_code: Option<String>,
}

/// POST /api/v1/billing/{id}/discount
/// STAFF-01: Apply a discount to an active billing session.
/// Discounts above DISCOUNT_APPROVAL_THRESHOLD_PAISE require manager approval code.
pub(crate) async fn apply_billing_discount(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Path(session_id): Path<String>,
    Json(req): Json<ApplyDiscountRequest>,
) -> Json<Value> {
    if req.discount_paise <= 0 {
        return Json(json!({ "error": "discount_paise must be greater than 0" }));
    }
    if req.reason_code.trim().is_empty() {
        return Json(json!({ "error": "reason_code is required" }));
    }

    let threshold = billing::DISCOUNT_APPROVAL_THRESHOLD_PAISE;
    let mut manager_approved = false;

    // STAFF-01: Discounts above threshold require manager approval code
    if req.discount_paise > threshold {
        match req.manager_approval_code.as_deref() {
            None | Some("") => {
                tracing::warn!(
                    actor_id = %claims.sub,
                    session_id = %session_id,
                    discount_paise = req.discount_paise,
                    threshold = threshold,
                    "STAFF-01: Discount above threshold rejected — no manager approval code"
                );
                return Json(json!({
                    "error": "Discount above threshold requires manager approval code",
                    "threshold_paise": threshold,
                }));
            }
            Some(code) => {
                // Validate manager approval code: look up staff with matching PIN where role is manager or superadmin
                let result = sqlx::query_as::<_, (String, String)>(
                    "SELECT id, role FROM staff_members WHERE pin = ? AND is_active = 1 AND role IN ('manager', 'superadmin')",
                )
                .bind(code)
                .fetch_optional(&state.db)
                .await;

                match result {
                    Ok(Some(_)) => {
                        manager_approved = true;
                    }
                    Ok(None) => {
                        tracing::warn!(
                            actor_id = %claims.sub,
                            session_id = %session_id,
                            "STAFF-01: Manager approval code invalid or not a manager"
                        );
                        return Json(json!({
                            "error": "Invalid manager approval code — must be a manager or superadmin PIN",
                        }));
                    }
                    Err(e) => {
                        tracing::error!("STAFF-01: DB error validating manager approval code: {}", e);
                        return Json(json!({ "error": "Database error validating approval code" }));
                    }
                }
            }
        }
    }

    // FATM-10: Enforce discount floor — fetch current price/discount to check cap
    let session_prices = sqlx::query_as::<_, (Option<i64>, i64)>(
        "SELECT original_price_paise, COALESCE(discount_paise, 0) FROM billing_sessions WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery')",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    let effective_discount_paise = match session_prices {
        Ok(Some((original_price_opt, current_discount))) => {
            let floor = billing::DISCOUNT_FLOOR_PAISE;
            if floor > 0 {
                let base_price = original_price_opt.unwrap_or(0);
                let max_total_discount = base_price - floor;
                let remaining_headroom = max_total_discount - current_discount;
                if remaining_headroom <= 0 {
                    tracing::info!(
                        "FATM-10: Discount floor already reached for session {} (base={}p, current_discount={}p, floor={}p) — discount rejected",
                        session_id, base_price, current_discount, floor
                    );
                    return Json(json!({
                        "error": "Discount floor already reached — no further discount allowed",
                        "discount_floor_paise": floor,
                        "session_id": session_id,
                    }));
                }
                let capped = req.discount_paise.min(remaining_headroom);
                if capped < req.discount_paise {
                    tracing::info!(
                        "FATM-10: Discount floor enforced for session {} — requested {}p capped to {}p (floor={}p, base={}p, current_discount={}p)",
                        session_id, req.discount_paise, capped, floor, base_price, current_discount
                    );
                }
                capped
            } else {
                req.discount_paise
            }
        }
        Ok(None) => {
            return Json(json!({
                "error": "Session not found or not in an active/paused state",
                "session_id": session_id,
            }));
        }
        Err(e) => {
            tracing::error!("FATM-10: DB error reading session for floor check: {}", e);
            return Json(json!({ "error": "Database error checking discount floor" }));
        }
    };

    // Apply discount: UPDATE billing_sessions for active/paused sessions only
    let update_result = sqlx::query(
        "UPDATE billing_sessions
         SET discount_paise = COALESCE(discount_paise, 0) + ?,
             discount_reason = ?
         WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery')",
    )
    .bind(effective_discount_paise)
    .bind(&req.reason_code)
    .bind(&session_id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(r) if r.rows_affected() == 0 => {
            return Json(json!({
                "error": "Session not found or not in an active/paused state",
                "session_id": session_id,
            }));
        }
        Err(e) => {
            tracing::error!("STAFF-01: Failed to apply discount for session {}: {}", session_id, e);
            return Json(json!({ "error": "Database error applying discount" }));
        }
        Ok(_) => {}
    }

    // Insert audit_log entry
    let audit_details = json!({
        "session_id": session_id,
        "discount_paise": effective_discount_paise,
        "requested_discount_paise": req.discount_paise,
        "reason_code": req.reason_code,
        "manager_approved": manager_approved,
        "actor_id": claims.sub,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
    });
    accounting::log_admin_action(
        &state,
        "discount_applied",
        &audit_details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        session_id = %session_id,
        discount_paise = effective_discount_paise,
        manager_approved = manager_approved,
        reason_code = %req.reason_code,
        "STAFF-01: Discount applied"
    );

    Json(json!({
        "status": "ok",
        "session_id": session_id,
        "discount_paise": effective_discount_paise,
        "manager_approved": manager_approved,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
    }))
}

// ─── STAFF-03: Daily override report ─────────────────────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct DailyOverridesQuery {
    /// Optional date in YYYY-MM-DD format (IST). Defaults to today IST.
    date: Option<String>,
}

/// GET /api/v1/admin/reports/daily-overrides?date=YYYY-MM-DD
/// STAFF-03: Returns all discounts, manual refunds, and tier changes with actor_id for a given day.
pub(crate) async fn daily_overrides_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyOverridesQuery>,
) -> Json<Value> {
    // Default to today IST (UTC+5:30)
    let target_date = params.date.unwrap_or_else(|| {
        let now_utc = chrono::Utc::now();
        // east_opt(19800) = UTC+5:30 (IST). 19800 is always valid; fallback to east_opt(0) which is also always valid.
        let ist_offset = chrono::FixedOffset::east_opt(19800).unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("UTC offset 0 is always valid"));
        let now_ist = now_utc.with_timezone(&ist_offset);
        now_ist.format("%Y-%m-%d").to_string()
    });

    // Discount entries from billing_sessions
    let discounts = sqlx::query_as::<_, (String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.discount_paise, bs.discount_reason, bs.driver_id,
                al.staff_id
         FROM billing_sessions bs
         LEFT JOIN audit_log al ON al.action_type = 'discount_applied'
                               AND json_extract(al.new_values, '$.session_id') = bs.id
         WHERE bs.discount_paise > 0
           AND date(bs.started_at) = ?
         ORDER BY bs.started_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Manual refunds from wallet_transactions
    let refunds = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, String)>(
        "SELECT id, amount_paise, driver_id, staff_id, created_at
         FROM wallet_transactions
         WHERE type = 'manual_refund'
           AND date(created_at) = ?
         ORDER BY created_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // All override audit_log entries for the day
    let audit_entries = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        "SELECT id, action_type, staff_id, new_values, created_at
         FROM audit_log
         WHERE action_type IN ('discount_applied', 'tier_change', 'manual_refund')
           AND date(created_at) = ?
         ORDER BY created_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let discount_entries: Vec<Value> = discounts
        .into_iter()
        .map(|(session_id, discount_paise, reason, driver_id, actor_id)| {
            json!({
                "action_type": "discount_applied",
                "session_id": session_id,
                "actor_id": actor_id,
                "target_driver": driver_id,
                "amount_paise": discount_paise.unwrap_or(0),
                "reason": reason,
            })
        })
        .collect();

    let refund_entries: Vec<Value> = refunds
        .into_iter()
        .map(|(id, amount_paise, driver_id, staff_id, created_at)| {
            json!({
                "action_type": "manual_refund",
                "transaction_id": id,
                "actor_id": staff_id,
                "target_driver": driver_id,
                "amount_paise": amount_paise,
                "timestamp": created_at,
            })
        })
        .collect();

    let audit_override_entries: Vec<Value> = audit_entries
        .into_iter()
        .map(|(id, action_type, staff_id, details, created_at)| {
            json!({
                "action_type": action_type,
                "audit_id": id,
                "actor_id": staff_id,
                "details": details,
                "timestamp": created_at,
            })
        })
        .collect();

    Json(json!({
        "status": "ok",
        "date": target_date,
        "discounts": discount_entries,
        "manual_refunds": refund_entries,
        "audit_overrides": audit_override_entries,
    }))
}

// ─── STAFF-04: Cash drawer reconciliation ─────────────────────────────────────

/// GET /api/v1/admin/reports/cash-drawer
/// STAFF-04: Returns system cash total for today (sum of wallet_transactions with method='cash').
pub(crate) async fn cash_drawer_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // IST today
    let now_utc = chrono::Utc::now();
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
    let today_ist = now_utc.with_timezone(&ist_offset).format("%Y-%m-%d").to_string();

    let total: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT SUM(amount_paise) FROM wallet_transactions
         WHERE (type = 'topup_cash' OR notes LIKE '%cash%')
           AND date(created_at) = ?",
    )
    .bind(&today_ist)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let system_cash_total_paise = total.and_then(|(v,)| v).unwrap_or(0);

    Json(json!({
        "status": "ok",
        "date": today_ist,
        "system_cash_total_paise": system_cash_total_paise,
    }))
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct CashDrawerCloseRequest {
    physical_count_paise: i64,
}

/// POST /api/v1/admin/reports/cash-drawer/close
/// STAFF-04: Log end-of-day cash drawer close with physical count vs system total discrepancy.
pub(crate) async fn cash_drawer_close(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<CashDrawerCloseRequest>,
) -> Json<Value> {
    if req.physical_count_paise < 0 {
        return Json(json!({ "error": "physical_count_paise cannot be negative" }));
    }

    // Compute system total for today IST
    let now_utc = chrono::Utc::now();
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
    let today_ist = now_utc.with_timezone(&ist_offset).format("%Y-%m-%d").to_string();

    let total: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT SUM(amount_paise) FROM wallet_transactions
         WHERE (type = 'topup_cash' OR notes LIKE '%cash%')
           AND date(created_at) = ?",
    )
    .bind(&today_ist)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let system_total_paise = total.and_then(|(v,)| v).unwrap_or(0);
    let discrepancy_paise = req.physical_count_paise - system_total_paise;

    let status = match discrepancy_paise.cmp(&0) {
        std::cmp::Ordering::Greater => "over",
        std::cmp::Ordering::Less => "under",
        std::cmp::Ordering::Equal => "balanced",
    };

    // Insert audit_log entry
    let audit_details = json!({
        "date": today_ist,
        "system_total_paise": system_total_paise,
        "physical_count_paise": req.physical_count_paise,
        "discrepancy_paise": discrepancy_paise,
        "status": status,
        "actor_id": claims.sub,
    });
    accounting::log_admin_action(
        &state,
        "cash_drawer_close",
        &audit_details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        date = %today_ist,
        system_total_paise = system_total_paise,
        physical_count_paise = req.physical_count_paise,
        discrepancy_paise = discrepancy_paise,
        status = %status,
        "STAFF-04: Cash drawer closed"
    );

    Json(json!({
        "status": "ok",
        "date": today_ist,
        "system_total_paise": system_total_paise,
        "physical_count_paise": req.physical_count_paise,
        "discrepancy_paise": discrepancy_paise,
        "drawer_status": status,
    }))
}
