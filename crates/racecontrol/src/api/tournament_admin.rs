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

// ─── Dynamic Pricing Admin ───────────────────────────────────────────────────

pub(crate) async fn list_pricing_rules(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>, Option<i64>, f64, i64, bool)>(
        "SELECT id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, is_active
         FROM pricing_rules ORDER BY rule_type, day_of_week, hour_start",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rules) => {
            let list: Vec<Value> = rules.iter().map(|r| json!({
                "id": r.0, "rule_type": r.1,
                "day_of_week": r.2, "hour_start": r.3, "hour_end": r.4,
                "multiplier": r.5, "flat_adjustment_paise": r.6, "is_active": r.7,
            })).collect();
            Json(json!({ "rules": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_pricing_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_rules") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let rule_type = body.get("rule_type").and_then(|v| v.as_str()).unwrap_or("custom");
    let multiplier = body.get("multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let flat_adj = body.get("flat_adjustment_paise").and_then(|v| v.as_i64()).unwrap_or(0);

    let result = sqlx::query(
        "INSERT INTO pricing_rules (id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, is_active)
         VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(rule_type)
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(multiplier)
    .bind(flat_adj)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_rules", &id, "create",
                None, new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_rule_create",
                &json!({"rule_id": id, "rule_type": rule_type}).to_string(),
                None, None,
            ).await;
            // Phase 307 AUDIT-03: Log config change for hash chain
            crate::activity_log::log_pod_activity(
                &state, "server", "config", "Pricing Rule Created",
                &format!("id={} type={} multiplier={}", id, rule_type, multiplier),
                "staff",
                None,
            );
            Json(json!({ "id": id })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn update_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_rules") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "pricing_rules", &id).await;

    let result = sqlx::query(
        "UPDATE pricing_rules SET
            rule_type = COALESCE(?, rule_type),
            day_of_week = ?,
            hour_start = ?,
            hour_end = ?,
            multiplier = COALESCE(?, multiplier),
            flat_adjustment_paise = COALESCE(?, flat_adjustment_paise),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("rule_type").and_then(|v| v.as_str()))
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(body.get("multiplier").and_then(|v| v.as_f64()))
    .bind(body.get("flat_adjustment_paise").and_then(|v| v.as_i64()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_rules", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_rule_update",
                &json!({"rule_id": id, "changes": body}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "pricing_rules") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "pricing_rules", &id).await;

    // Soft delete instead of hard delete (preserves audit trail)
    let _ = sqlx::query("UPDATE pricing_rules SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    accounting::log_audit(
        &state, "pricing_rules", &id, "delete",
        old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
    ).await;
    accounting::log_admin_action(
        &state, "pricing_rule_delete",
        &json!({"rule_id": id}).to_string(),
        None, None,
    ).await;

    Json(json!({ "ok": true })).into_response()
}

// ─── Coupons Admin ───────────────────────────────────────────────────────────

pub(crate) async fn list_coupons(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool, bool)>(
        "SELECT id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, is_active
         FROM coupons ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(coupons) => {
            let list: Vec<Value> = coupons.iter().map(|c| json!({
                "id": c.0, "code": c.1, "coupon_type": c.2, "value": c.3,
                "max_uses": c.4, "valid_from": c.5, "valid_until": c.6,
                "min_spend_paise": c.7, "first_session_only": c.8, "is_active": c.9,
            })).collect();
            Json(json!({ "coupons": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_coupon(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "coupons") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("").to_uppercase();
    if code.is_empty() {
        return Json(json!({ "error": "code required" })).into_response();
    }

    let result = sqlx::query(
        "INSERT INTO coupons (id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, is_active)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(&code)
    .bind(body.get("coupon_type").and_then(|v| v.as_str()).unwrap_or("percent"))
    .bind(body.get("value").and_then(|v| v.as_f64()).unwrap_or(10.0))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()).unwrap_or(100))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()).unwrap_or(false))
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "coupons", &id, "create",
                None, new_values.as_deref(), None,
            ).await;
            Json(json!({ "id": id, "code": code })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn update_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "coupons") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "coupons", &id).await;

    let result = sqlx::query(
        "UPDATE coupons SET
            code = COALESCE(?, code),
            coupon_type = COALESCE(?, coupon_type),
            value = COALESCE(?, value),
            max_uses = COALESCE(?, max_uses),
            valid_from = ?,
            valid_until = ?,
            min_spend_paise = ?,
            first_session_only = COALESCE(?, first_session_only),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("code").and_then(|v| v.as_str()))
    .bind(body.get("coupon_type").and_then(|v| v.as_str()))
    .bind(body.get("value").and_then(|v| v.as_f64()))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "coupons", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "coupons") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "coupons", &id).await;

    // Soft delete instead of hard delete
    let _ = sqlx::query("UPDATE coupons SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    accounting::log_audit(
        &state, "coupons", &id, "delete",
        old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
    ).await;

    Json(json!({ "ok": true })).into_response()
}

// ─── Review Nudges ───────────────────────────────────────────────────────────

pub(crate) async fn pending_review_nudges(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT rn.id, rn.driver_id, d.name, d.phone
         FROM review_nudges rn
         JOIN drivers d ON rn.driver_id = d.id
         WHERE rn.sent_at IS NULL
         ORDER BY rn.created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(nudges) => {
            let list: Vec<Value> = nudges.iter().map(|n| json!({
                "id": n.0, "driver_id": n.1, "driver_name": n.2, "phone": n.3,
            })).collect();
            Json(json!({ "nudges": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn mark_nudge_sent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let review_credited = body.get("review_credited").and_then(|v| v.as_bool()).unwrap_or(false);

    let _ = sqlx::query(
        "UPDATE review_nudges SET sent_at = datetime('now'), review_credited = ? WHERE id = ?",
    )
    .bind(review_credited)
    .bind(&id)
    .execute(&state.db)
    .await;

    // If they left a review, credit 50 credits (₹50)
    if review_credited {
        let driver: Option<(String,)> = sqlx::query_as(
            "SELECT driver_id FROM review_nudges WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((driver_id,)) = driver {
            let _ = crate::wallet::credit(
                &state,
                &driver_id,
                5000,
                "review_reward",
                Some(&id),
                Some("Thank you for your Google review!"),
                None,
            )
            .await;
        }
    }

    Json(json!({ "ok": true }))
}
