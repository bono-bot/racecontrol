#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use super::routes::anonymize_driver_pii;
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

/// GET /api/v1/customer/data-export
/// Returns a JSON dump of all customer data with decrypted PII fields.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_export(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<(axum::http::StatusCode, Json<Value>), (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, name_enc, email_enc, phone_enc, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let d = match driver {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_export DB error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Decrypt PII fields; fallback to plaintext columns if decryption fails or enc is NULL
    let name = d.4.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or_else(|| Some(d.1.clone()));
    let email = d.5.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.2.clone());
    let phone = d.6.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.3.clone());

    // Fetch nickname
    let nickname: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT nickname FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.0);

    // Fetch wallet balance
    let wallet_balance: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(balance, 0) FROM wallets WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or(0);

    let exported_at = chrono::Utc::now().to_rfc3339();
    tracing::info!("Data export requested by driver {}", driver_id);

    Ok((
        axum::http::StatusCode::OK,
        Json(json!({
            "driver_id": d.0,
            "name": name,
            "email": email,
            "phone": phone,
            "nickname": nickname,
            "total_laps": d.7,
            "total_time_ms": d.8,
            "wallet_balance": wallet_balance,
            "exported_at": exported_at,
        })),
    ))
}

/// DELETE /api/v1/customer/data-delete
/// Cascades deletion to all child tables and the driver record in a single transaction.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_delete(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    // Verify driver exists
    let exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match exists {
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_delete lookup error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
        Ok(Some(_)) => {}
    }

    // Begin transaction -- cascade delete all child tables
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("data_delete transaction start error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Delete from child tables (children first, then parent)
    // wallet_transactions before wallets (wallet_transactions references wallets)
    let _ = sqlx::query("DELETE FROM wallet_transactions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM wallets WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM billing_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM laps WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM customer_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friend_requests WHERE sender_id = ? OR receiver_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friendships WHERE driver_a_id = ? OR driver_b_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM group_session_members WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM tournament_registrations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM pod_reservations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM auth_tokens WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM personal_bests WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM event_entries WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_feedback WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM coupon_redemptions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM memberships WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM referrals WHERE referrer_id = ? OR referee_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_highlights WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM review_nudges WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM multiplayer_results WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM driver_ratings WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Delete the driver record itself
    let _ = sqlx::query("DELETE FROM drivers WHERE id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Commit transaction
    if let Err(e) = tx.commit().await {
        tracing::error!("data_delete commit error for driver {}: {}", driver_id, e);
        return Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Database error" })),
        ));
    }

    tracing::info!("Customer {} deleted their data (DPDP compliance)", driver_id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── LEGAL-09: Consent revocation (customer-initiated via PWA) ───────────────
/// POST /api/v1/customer/revoke-consent
///
/// Allows a driver (or guardian acting on behalf of a minor) to invoke the DPDP Act
/// right of erasure. Anonymizes PII immediately. Financial records (journal entries,
/// invoices, billing_sessions) are NOT deleted — they must be retained for 8 years
/// per the Income Tax Act.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("customer_request");
    anonymize_driver_pii(&state, &driver_id, reason, None).await
}

// ─── LEGAL-09: Consent revocation (staff-initiated for guardian requests) ────
/// POST /api/v1/drivers/{id}/revoke-consent
///
/// Staff endpoint for guardian-initiated revocation — guardian calls the venue,
/// staff (cashier+) processes the data deletion request.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn staff_revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("guardian_request");
    anonymize_driver_pii(&state, &driver_id, reason, Some("staff")).await
}

// ─── BILL-08: Customer charge dispute portal ─────────────────────────────────

/// POST /api/v1/customer/dispute
///
/// Allows a customer to flag a billing session charge for review by staff.
/// Only completed or ended_early sessions can be disputed (not active ones).
/// Enforces one active dispute per session via the UNIQUE index on dispute_requests.
///
/// Body: `{ "billing_session_id": "...", "reason": "..." }`
/// Returns: `{ "ok": true, "dispute_id": "..." }`
/// BILL-08
pub(crate) async fn create_dispute_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let billing_session_id = match body.get("billing_session_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return Json(json!({ "error": "billing_session_id is required" })),
    };

    let reason = match body.get("reason").and_then(|v| v.as_str()) {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => return Json(json!({ "error": "reason is required" })),
    };

    // BILL-08: Validate session exists and belongs to this driver
    let session_info: Option<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM billing_sessions WHERE id = ? AND driver_id = ?",
    )
    .bind(&billing_session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let session_status = match session_info {
        Some((_, status)) => status,
        None => return Json(json!({ "error": "Billing session not found or does not belong to you" })),
    };

    // BILL-08: Only completed or ended_early sessions can be disputed (not active sessions)
    if session_status != "completed" && session_status != "ended_early" {
        return Json(json!({
            "error": format!("Cannot dispute a session with status '{}'. Only completed or ended_early sessions can be disputed.", session_status)
        }));
    }

    let dispute_id = uuid::Uuid::new_v4().to_string();

    // BILL-08: Insert dispute — UNIQUE index will reject duplicates
    let insert_result = sqlx::query(
        "INSERT INTO dispute_requests (id, billing_session_id, driver_id, reason, status, venue_id)
         VALUES (?, ?, ?, ?, 'pending', ?)",
    )
    .bind(&dispute_id)
    .bind(&billing_session_id)
    .bind(&driver_id)
    .bind(&reason)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        let msg = e.to_string();
        if msg.contains("UNIQUE") || msg.contains("constraint") {
            return Json(json!({ "error": "A dispute is already open for this session" }));
        }
        tracing::error!("BILL-08: Failed to insert dispute for session {}: {}", billing_session_id, e);
        return Json(json!({ "error": "Failed to create dispute" }));
    }

    // Look up driver name for broadcast
    let driver_name: String = sqlx::query_scalar(
        "SELECT COALESCE(name, id) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| driver_id.clone());

    // BILL-08: Broadcast DisputeCreated — staff dashboard gets a notification
    let _ = state.dashboard_tx.send(DashboardEvent::DisputeCreated {
        dispute_id: dispute_id.clone(),
        driver_name,
        session_id: billing_session_id.clone(),
        reason: reason.clone(),
    });

    tracing::info!(
        "BILL-08: Dispute {} created by driver {} for session {}",
        dispute_id, driver_id, billing_session_id
    );

    Json(json!({ "ok": true, "dispute_id": dispute_id }))
}

/// GET /api/v1/admin/disputes
///
/// Returns list of disputes for staff review. Optional ?status=pending filter.
/// Includes: dispute_id, driver_name, session_id, pod_id, session_duration,
///           amount_charged, reason, status, created_at.
/// BILL-08
pub(crate) async fn list_disputes_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let status_filter = params.get("status").cloned();

    let rows = if let Some(ref status) = status_filter {
        sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, String, String)>(
            "SELECT dr.id, COALESCE(d.name, dr.driver_id) AS driver_name,
                    dr.billing_session_id, bs.pod_id,
                    d.id AS driver_id,
                    bs.driving_seconds, bs.wallet_debit_paise,
                    dr.reason, dr.status
             FROM dispute_requests dr
             LEFT JOIN billing_sessions bs ON bs.id = dr.billing_session_id
             LEFT JOIN drivers d ON d.id = dr.driver_id
             WHERE dr.status = ?
             ORDER BY dr.created_at DESC
             LIMIT 100",
        )
        .bind(status)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, String, String)>(
            "SELECT dr.id, COALESCE(d.name, dr.driver_id) AS driver_name,
                    dr.billing_session_id, bs.pod_id,
                    d.id AS driver_id,
                    bs.driving_seconds, bs.wallet_debit_paise,
                    dr.reason, dr.status
             FROM dispute_requests dr
             LEFT JOIN billing_sessions bs ON bs.id = dr.billing_session_id
             LEFT JOIN drivers d ON d.id = dr.driver_id
             ORDER BY dr.created_at DESC
             LIMIT 100",
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let disputes: Vec<Value> = rows
        .into_iter()
        .map(|(id, driver_name, session_id, pod_id, driver_id, driving_seconds, amount_charged, reason, status)| {
            json!({
                "dispute_id": id,
                "driver_name": driver_name,
                "driver_id": driver_id,
                "session_id": session_id,
                "pod_id": pod_id,
                "session_duration_seconds": driving_seconds,
                "amount_charged_paise": amount_charged,
                "reason": reason,
                "status": status
            })
        })
        .collect();

    Json(json!({ "disputes": disputes, "count": disputes.len() }))
}

/// GET /api/v1/admin/disputes/{id}/details
///
/// Returns full dispute context: dispute info + billing_events + billing_session details.
/// Gives staff the complete audit trail to make an informed decision.
/// BILL-08
pub(crate) async fn dispute_details_handler(
    State(state): State<Arc<AppState>>,
    Path(dispute_id): Path<String>,
) -> Json<Value> {
    // Fetch the dispute
    let dispute: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, billing_session_id, driver_id, reason, status
         FROM dispute_requests WHERE id = ?",
    )
    .bind(&dispute_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (_, billing_session_id, driver_id, reason, status) = match dispute {
        Some(d) => d,
        None => return Json(json!({ "error": "Dispute not found" })),
    };

    // Fetch billing session details
    let session: Option<(Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT pod_id, pricing_tier_id, allocated_seconds, driving_seconds, wallet_debit_paise, started_at, ended_at
         FROM billing_sessions WHERE id = ?",
    )
    .bind(&billing_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Fetch billing events for this session (audit trail)
    let events: Vec<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT event_type, driving_seconds_at_event, metadata
         FROM billing_events WHERE billing_session_id = ?
         ORDER BY rowid ASC LIMIT 50",
    )
    .bind(&billing_session_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let events_json: Vec<Value> = events
        .into_iter()
        .map(|(event_type, driving_secs, metadata)| {
            json!({
                "event_type": event_type,
                "driving_seconds": driving_secs,
                "metadata": metadata
            })
        })
        .collect();

    let session_json = if let Some((pod_id, tier_id, alloc, driving, debit, started, ended)) = session {
        json!({
            "pod_id": pod_id,
            "pricing_tier_id": tier_id,
            "allocated_seconds": alloc,
            "driving_seconds": driving,
            "wallet_debit_paise": debit,
            "started_at": started,
            "ended_at": ended
        })
    } else {
        json!(null)
    };

    Json(json!({
        "dispute_id": dispute_id,
        "billing_session_id": billing_session_id,
        "driver_id": driver_id,
        "reason": reason,
        "status": status,
        "billing_session": session_json,
        "billing_events": events_json
    }))
}

/// POST /api/v1/admin/disputes/{id}/resolve
///
/// Staff resolves a pending dispute. Action must be "approve" or "deny".
/// Approve: computes refund via compute_refund(), credits wallet via credit_in_tx(),
///          logs 'dispute_refund' billing_event.
/// Deny: logs 'dispute_denied' billing_event with reason.
///
/// Body: `{ "action": "approve" | "deny", "reason": "..." }`
/// Returns: `{ "ok": true, "status": "approved" | "denied", "refund_amount_paise": N }`
/// BILL-08
pub(crate) async fn resolve_dispute_handler(
    State(state): State<Arc<AppState>>,
    Path(dispute_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let staff_id = claims.map(|c| c.0.sub.clone()).unwrap_or_else(|| "unknown".to_string());

    let action = match body.get("action").and_then(|v| v.as_str()) {
        Some("approve") => "approve",
        Some("deny") => "deny",
        _ => return Json(json!({ "error": "action must be 'approve' or 'deny'" })),
    };

    let resolution_reason = body.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // BILL-08: Fetch dispute — must exist and be pending
    let dispute: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, billing_session_id, driver_id, status FROM dispute_requests WHERE id = ?",
    )
    .bind(&dispute_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (_, billing_session_id, driver_id, current_status) = match dispute {
        Some(d) => d,
        None => return Json(json!({ "error": "Dispute not found" })),
    };

    // BILL-08: Guard — only pending disputes can be resolved
    if current_status != "pending" {
        return Json(json!({
            "error": format!("Dispute is already resolved (status: {})", current_status)
        }));
    }

    if action == "approve" {
        // BILL-08: Compute refund using unified compute_refund() (FATM-06 path)
        let session_data: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT COALESCE(allocated_seconds, 0), COALESCE(driving_seconds, 0), COALESCE(wallet_debit_paise, 0)
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&billing_session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let (allocated, driving, debit) = session_data.unwrap_or((0, 0, 0));
        let refund_paise = crate::billing::compute_refund(allocated, driving, debit);

        if refund_paise > 0 {
            // BILL-08: Credit wallet via credit_in_tx (FATM-03 atomic credit path)
            let mut tx = match state.db.begin().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("BILL-08: Failed to begin transaction for dispute refund {}: {}", dispute_id, e);
                    return Json(json!({ "error": "Failed to process refund" }));
                }
            };

            match crate::wallet::credit_in_tx(
                &mut tx,
                &driver_id,
                refund_paise,
                "dispute_refund",
                Some(&billing_session_id),
                Some(&format!("Dispute {} approved by staff", dispute_id)),
                Some(&staff_id),
                None,
                &state.config.venue.venue_id,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("BILL-08: Failed to credit wallet for dispute {}: {}", dispute_id, e);
                    return Json(json!({ "error": "Failed to credit wallet" }));
                }
            }

            if let Err(e) = tx.commit().await {
                tracing::error!("BILL-08: Failed to commit wallet credit for dispute {}: {}", dispute_id, e);
                return Json(json!({ "error": "Failed to commit refund" }));
            }
        }

        // BILL-08: Update dispute to approved
        let _ = sqlx::query(
            "UPDATE dispute_requests SET status='approved', resolved_at=datetime('now'),
             resolved_by=?, refund_amount_paise=? WHERE id=?",
        )
        .bind(&staff_id)
        .bind(refund_paise)
        .bind(&dispute_id)
        .execute(&state.db)
        .await;

        // BILL-08: Log 'dispute_refund' billing_event for audit trail
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'dispute_refund', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&billing_session_id)
        .bind(0i64)
        .bind(format!(
            "{{\"dispute_id\":\"{}\",\"refund_paise\":{},\"resolved_by\":\"{}\"}}",
            dispute_id, refund_paise, staff_id
        ))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-08: Failed to log dispute_refund event: {}", e));

        tracing::info!(
            "BILL-08: Dispute {} approved by {} — refund {}p to driver {}",
            dispute_id, staff_id, refund_paise, driver_id
        );

        Json(json!({
            "ok": true,
            "status": "approved",
            "refund_amount_paise": refund_paise
        }))
    } else {
        // deny path
        // BILL-08: Update dispute to denied
        let _ = sqlx::query(
            "UPDATE dispute_requests SET status='denied', resolved_at=datetime('now'),
             resolved_by=?, resolution_reason=? WHERE id=?",
        )
        .bind(&staff_id)
        .bind(&resolution_reason)
        .bind(&dispute_id)
        .execute(&state.db)
        .await;

        // BILL-08: Log 'dispute_denied' billing_event for audit trail
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'dispute_denied', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&billing_session_id)
        .bind(0i64)
        .bind(format!(
            "{{\"dispute_id\":\"{}\",\"reason\":\"{}\",\"resolved_by\":\"{}\"}}",
            dispute_id,
            resolution_reason.replace('"', "'"),
            staff_id
        ))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-08: Failed to log dispute_denied event: {}", e));

        tracing::info!(
            "BILL-08: Dispute {} denied by {} — reason: {}",
            dispute_id, staff_id, resolution_reason
        );

        Json(json!({
            "ok": true,
            "status": "denied",
            "refund_amount_paise": 0
        }))
    }
}

// ─── LEGAL-08: Data retention background job ─────────────────────────────────
/// Spawned at server startup (in main.rs). Runs daily with a 1-hour initial delay
/// to avoid congestion at boot. Reads pii_inactive_months from data_retention_config
/// and anonymizes drivers who have been inactive beyond that threshold.
///
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are never touched — retained for 8 years per Income Tax Act.
pub async fn spawn_data_retention_job(state: Arc<AppState>) {
    tracing::info!(
        target: "data_retention",
        "data-retention task started (86400s interval, 3600s initial delay)"
    );
    // Initial delay: 1 hour — avoid boot congestion alongside orphan detector, reconciler, etc.
    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
    loop {
        interval.tick().await;
        run_pii_anonymization_cycle(state.clone()).await;
    }
}

/// Single anonymization cycle — called daily by spawn_data_retention_job.
pub(crate) async fn run_pii_anonymization_cycle(state: Arc<AppState>) {
    // Read retention policy from config table
    let policy: Option<(i64,)> = sqlx::query_as(
        "SELECT pii_inactive_months FROM data_retention_config WHERE id = 'default'",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let pii_inactive_months = policy.map(|r| r.0).unwrap_or(24);

    // Find drivers inactive beyond the threshold who have not yet been anonymized
    // and have not already revoked consent (those are handled immediately on revocation).
    // BUG FIX (2026-04-06): last_activity_at IS NULL matched newly registered drivers
    // who hadn't raced yet. Added created_at check to exclude drivers created within
    // the retention window — a driver registered 17 minutes ago is NOT "inactive for 24 months."
    let inactive_drivers: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers
         WHERE (last_activity_at IS NULL
                OR last_activity_at < datetime('now', '-' || ? || ' months'))
           AND created_at < datetime('now', '-' || ? || ' months')
           AND COALESCE(pii_anonymized, 0) = 0
           AND COALESCE(consent_revoked, 0) = 0
         LIMIT 500",
    )
    .bind(pii_inactive_months)
    .bind(pii_inactive_months)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut anonymized_count: u32 = 0;
    for (driver_id,) in inactive_drivers {
        let result = sqlx::query(
            "UPDATE drivers SET
                name = 'ANONYMIZED-' || substr(id, 1, 8),
                email = NULL,
                phone = NULL,
                phone_hash = NULL,
                guardian_name = NULL,
                guardian_phone = NULL,
                guardian_phone_hash = NULL,
                dob = NULL,
                pii_anonymized = 1,
                pii_anonymized_at = datetime('now')
            WHERE id = ? AND COALESCE(pii_anonymized, 0) = 0",
        )
        .bind(&driver_id)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => anonymized_count += 1,
            Ok(_) => {} // already anonymized between SELECT and UPDATE — idempotent
            Err(e) => tracing::warn!(
                target: "data_retention",
                driver_id = %driver_id,
                "Failed to anonymize inactive driver: {}",
                e
            ),
        }
    }

    tracing::info!(
        target: "data_retention",
        count = anonymized_count,
        threshold_months = pii_inactive_months,
        "PII anonymization cycle complete"
    );
}
