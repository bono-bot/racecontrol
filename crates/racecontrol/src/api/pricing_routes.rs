#![allow(unused_imports)]
use super::auth_staff::venue_authority_guard;
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

// ─── Pricing ────────────────────────────────────────────────────────────────

pub(crate) async fn list_pricing_tiers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers ORDER BY sort_order ASC",
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
                        "price_paise": t.3, "is_trial": t.4, "is_active": t.5,
                        "sort_order": t.6,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Pricing Psychology (v14.0 Phase 94) ────────────────────────────────────

/// Public: returns pricing tiers with dynamic (time-of-day adjusted) prices.
pub(crate) async fn pricing_display_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // LEGAL-07: Refund and pricing policy text for consumer transparency (Consumer Protection Act 2019)
    const REFUND_POLICY: &str = "Unused session time is refunded to your wallet at the pro-rated session rate. \
        Refunds to original payment method are available within 7 days of top-up for unused wallet balance. \
        No refunds for completed sessions.";
    const PRICING_POLICY: &str = "All prices are inclusive of 18% GST. \
        Session billing starts when your game reaches Running state. \
        Early termination refunds unused time to your wallet. \
        Free trial: one 5-minute session per customer.";
    const GST_NOTE: &str = "Prices inclusive of 18% GST (CGST 9% + SGST 9%)";

    let mut tiers = Vec::new();
    for (id, name, duration_minutes, price_paise, is_trial, _is_active, sort_order) in &rows {
        let dynamic_price = crate::billing::compute_dynamic_price(&state, *price_paise).await;
        let has_discount = dynamic_price != *price_paise;
        tiers.push(json!({
            "id": id,
            "name": name,
            "duration_minutes": duration_minutes,
            "base_price_paise": price_paise,
            "dynamic_price_paise": dynamic_price,
            "has_discount": has_discount,
            "is_trial": is_trial,
            "sort_order": sort_order,
        }));
    }
    Json(json!({
        "tiers": tiers,
        "refund_policy": REFUND_POLICY,
        "pricing_policy": PRICING_POLICY,
        "gst_note": GST_NOTE,
    }))
}

/// Public: returns real social proof counts from billing_sessions.
pub(crate) async fn pricing_social_proof_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    let drivers_this_week: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT driver_id) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND started_at >= datetime('now', '-7 days')"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let sessions_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND date(started_at) = date('now')"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Json(json!({
        "drivers_this_week": drivers_this_week,
        "sessions_today": sessions_today
    }))
}

/// LEGAL-06: Return minor waiver liability disclosure text.
/// Public endpoint — kiosk fetches this during minor registration to display the Indian Contract Act
/// limitation text and guardian consent requirements before the guardian signs.
pub(crate) async fn minor_waiver_disclosure() -> Json<Value> {
    Json(json!({
        "disclosure_text": "Under the Indian Contract Act 1872, agreements with persons under 18 years of age are void. This waiver acknowledgment is signed by the guardian on behalf of the minor participant. Racing Point maintains additional liability insurance coverage for participants under 18. The guardian assumes responsibility for the minor's conduct and safety during the session. This acknowledgment does not constitute a binding waiver of the minor's legal rights.",
        "requires_guardian_signature": true,
        "requires_guardian_otp": true,
        "requires_guardian_presence": true,
    }))
}

/// LEGAL-04: Send an OTP to a minor's guardian phone for consent verification.
/// Staff trigger this at the counter when processing a minor customer.
/// Body: { "driver_id": "...", "guardian_phone": "+91XXXXXXXXXX" }
pub(crate) async fn send_guardian_otp_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match body.get("driver_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "driver_id is required" })),
    };
    let guardian_phone = match body.get("guardian_phone").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return Json(json!({ "error": "guardian_phone is required" })),
    };
    match auth::send_guardian_otp(&state, driver_id, guardian_phone).await {
        Ok(result) => Json(json!({ "ok": true, "driver_id": result.driver_id, "delivered": result.delivered })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// LEGAL-04: Verify the OTP entered by the guardian at the counter.
/// On success, sets guardian_otp_verified=1 on the driver record.
/// Body: { "driver_id": "...", "otp": "123456" }
pub(crate) async fn verify_guardian_otp_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match body.get("driver_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "driver_id is required" })),
    };
    let otp = match body.get("otp").and_then(|v| v.as_str()) {
        Some(o) => o,
        None => return Json(json!({ "error": "otp is required" })),
    };
    match auth::verify_guardian_otp(&state, driver_id, otp).await {
        Ok(verified) => Json(json!({ "ok": true, "verified": verified })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn create_pricing_tier(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_tiers") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_trial = body.get("is_trial").and_then(|v| v.as_bool()).unwrap_or(false);
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(duration_minutes)
    .bind(price_paise)
    .bind(is_trial)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            accounting::log_admin_action(
                &state, "pricing_create",
                &json!({"tier_id": id, "name": name, "duration_minutes": duration_minutes, "price_paise": price_paise}).to_string(),
                None, None,
            ).await;
            Json(json!({ "id": id, "name": name })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn update_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_tiers") {
        return rejection.into_response();
    }
    // Snapshot before change for audit trail
    let old_snapshot = accounting::snapshot_row(&state, "pricing_tiers", &id).await;

    let name = body.get("name").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64());
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());

    // Build dynamic update query.
    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(d) = duration_minutes {
        updates.push("duration_minutes = ?");
        binds.push(d.to_string());
    }
    if let Some(p) = price_paise {
        updates.push("price_paise = ?");
        binds.push(p.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE pricing_tiers SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_tiers", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_update",
                &json!({"tier_id": id, "changes": body}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_tiers") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "pricing_tiers", &id).await;

    // Soft delete: set is_active = 0
    match sqlx::query("UPDATE pricing_tiers SET is_active = 0, updated_at = datetime('now') WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(_) => {
            accounting::log_audit(
                &state, "pricing_tiers", &id, "delete",
                old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_delete",
                &json!({"tier_id": id}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

// ─── Billing Rate Tiers (per-minute rates) ──────────────────────────────────

pub(crate) async fn list_billing_rates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, i64, i64, bool, Option<String>)>(
        "SELECT id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active, sim_type
         FROM billing_rates ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rates) => {
            let list: Vec<Value> = rates
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0, "tier_order": r.1, "tier_name": r.2,
                        "threshold_minutes": r.3, "rate_per_min_paise": r.4,
                        "is_active": r.5, "sim_type": r.6,
                    })
                })
                .collect();
            Json(json!({ "rates": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_billing_rate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64()).unwrap_or(1);
    let tier_name = body.get("tier_name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64()).unwrap_or(0);
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64()).unwrap_or(2500);

    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(tier_order)
    .bind(tier_name)
    .bind(threshold_minutes)
    .bind(rate_per_min_paise)
    .bind(&sim_type)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            (axum::http::StatusCode::CREATED, Json(json!({ "id": id, "tier_name": tier_name }))).into_response()
        }
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

pub(crate) async fn update_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    let tier_name = body.get("tier_name").and_then(|v| v.as_str());
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64());
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64());
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());
    // sim_type: present in body = update (even if null to clear); absent = don't touch
    let sim_type_in_body = body.get("sim_type").map(|v| v.as_str().map(|s| s.to_string()));

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = tier_name {
        updates.push("tier_name = ?");
        binds.push(n.to_string());
    }
    if let Some(o) = tier_order {
        updates.push("tier_order = ?");
        binds.push(o.to_string());
    }
    if let Some(t) = threshold_minutes {
        updates.push("threshold_minutes = ?");
        binds.push(t.to_string());
    }
    if let Some(r) = rate_per_min_paise {
        updates.push("rate_per_min_paise = ?");
        binds.push(r.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }
    let sim_type_val: Option<String> = if let Some(opt_s) = sim_type_in_body {
        updates.push("sim_type = ?");
        binds.push(opt_s.clone().unwrap_or_default());
        opt_s
    } else {
        None
    };
    let _ = sim_type_val; // used via binds above

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE billing_rates SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "billing_rates", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    match sqlx::query(
        "UPDATE billing_rates SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            accounting::log_audit(
                &state,
                "billing_rates",
                &id,
                "delete",
                old_snapshot.as_deref(),
                Some("{\"is_active\":false}"),
                None,
            )
            .await;
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("delete_billing_rate DB error for {}: {}", id, e);
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
    }
}
