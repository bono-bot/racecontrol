#![allow(unused_imports)]
use super::auth_staff::{cloud_authority_guard, post_write_verify_staff_pin, spawn_delayed_sync_verify, validate_pin_format, venue_authority_guard, CreateStaffRequest, ChangePinRequest, SyncPullNowRequest, SyncPullNowResponse, ChangePinResponse};
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

pub(crate) async fn create_staff(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateStaffRequest>,
) -> impl IntoResponse {
    // Phase 343: Cloud-authority guard — venue rejects staff mutations when cloud sync is enabled
    if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
        return rejection.into_response();
    }
    // Prevent duplicate names (case-insensitive)
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff_members WHERE LOWER(name) = LOWER(?) AND is_active = 1",
    )
    .bind(&req.name)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if existing > 0 {
        return Json(json!({ "error": format!("Staff member '{}' already exists. Use update instead.", req.name) })).into_response();
    }

    // Prevent duplicate PINs
    let pin_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff_members WHERE pin = ? AND is_active = 1",
    )
    .bind(&req.pin)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if pin_exists > 0 {
        return Json(json!({ "error": "PIN already in use by another staff member. Choose a different PIN." })).into_response();
    }

    let id = format!("staff_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));

    match sqlx::query(
        "INSERT INTO staff_members (id, name, phone, pin) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.phone)
    .bind(&req.pin)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            // Phase 343-02: Post-write verify for new staff PIN
            let correlation_id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = post_write_verify_staff_pin(&state.db, &id, &req.pin).await {
                tracing::error!(target: "auth", correlation_id = %correlation_id, error = %e, "create_staff post_write_verify failed");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "error": format!("PIN write verification failed: {}", e),
                    "correlation_id": correlation_id,
                }))).into_response();
            }
            // Schedule delayed verify after one sync cycle
            spawn_delayed_sync_verify(
                state.db.clone(),
                state.config.cloud.sync_interval_secs,
                id.clone(),
                req.pin.clone(),
                correlation_id.clone(),
            );
            Json(json!({
                "status": "ok",
                "id": id,
                "name": req.name,
                "verified": true,
                "correlation_id": correlation_id,
            })).into_response()
        }
        Err(e) => Json(json!({ "error": format!("{}", e) })).into_response(),
    }
}

pub(crate) async fn list_staff(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, Option<String>, Option<String>)>(
        "SELECT id, name, phone, pin, is_active, last_login_at, role FROM staff_members ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let staff: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, phone, _pin, active, last_login, role)| {
            // SEC: Never expose staff PINs in API responses
            json!({
                "id": id,
                "name": name,
                "phone": phone,
                "is_active": active,
                "last_login_at": last_login,
                "role": role.unwrap_or_else(|| "staff".to_string()),
            })
        })
        .collect();

    Json(json!({ "staff": staff }))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct UpdateStaffRequest {
    name: Option<String>,
    phone: Option<String>,
    pin: Option<String>,
    role: Option<String>,
    is_active: Option<bool>,
}

pub(crate) async fn update_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStaffRequest>,
) -> impl IntoResponse {
    // Phase 343: Cloud-authority guard
    if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
        return rejection.into_response();
    }
    // Verify staff exists
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff_members WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if exists == 0 {
        return Json(json!({ "error": "Staff member not found" })).into_response();
    }

    // Validate role if provided
    if let Some(ref role) = req.role {
        if !["staff", "cashier", "manager", "superadmin"].contains(&role.as_str()) {
            return Json(json!({ "error": format!("Invalid role: {}. Must be one of: staff, cashier, manager, superadmin", role) })).into_response();
        }
    }

    // Build dynamic UPDATE
    let mut sets: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref name) = req.name {
        sets.push("name = ?".to_string());
        binds.push(name.clone());
    }
    if let Some(ref phone) = req.phone {
        sets.push("phone = ?".to_string());
        binds.push(phone.clone());
    }
    if let Some(ref pin) = req.pin {
        // Check PIN not already in use by another active staff member
        let pin_conflict = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM staff_members WHERE pin = ? AND id != ? AND is_active = 1",
        )
        .bind(pin)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
        if pin_conflict > 0 {
            return Json(json!({ "error": "PIN already in use by another staff member." })).into_response();
        }
        sets.push("pin = ?".to_string());
        binds.push(pin.clone());
    }
    if let Some(ref role) = req.role {
        sets.push("role = ?".to_string());
        binds.push(role.clone());
    }
    if let Some(active) = req.is_active {
        sets.push("is_active = ?".to_string());
        binds.push(if active { "1".to_string() } else { "0".to_string() });
    }

    if sets.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    sets.push("updated_at = datetime('now')".to_string());
    let sql = format!("UPDATE staff_members SET {} WHERE id = ?", sets.join(", "));

    let mut query = sqlx::query(&sql);
    for val in &binds {
        query = query.bind(val);
    }
    // Bind is_active as integer separately if present
    query = query.bind(&id);

    match query.execute(&state.db).await {
        Ok(_) => {
            // Phase 343-02: Post-write verify when PIN was changed
            if let Some(ref new_pin) = req.pin {
                let correlation_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = post_write_verify_staff_pin(&state.db, &id, new_pin).await {
                    tracing::error!(target: "auth", correlation_id = %correlation_id, error = %e, "update_staff post_write_verify failed");
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                        "error": format!("PIN write verification failed: {}", e),
                        "correlation_id": correlation_id,
                    }))).into_response();
                }
                spawn_delayed_sync_verify(
                    state.db.clone(),
                    state.config.cloud.sync_interval_secs,
                    id.clone(),
                    new_pin.clone(),
                    correlation_id.clone(),
                );
                return Json(json!({
                    "status": "ok",
                    "id": id,
                    "verified": true,
                    "correlation_id": correlation_id,
                })).into_response();
            }
            Json(json!({ "status": "ok", "id": id })).into_response()
        }
        Err(e) => Json(json!({ "error": format!("{}", e) })).into_response(),
    }
}

pub(crate) async fn delete_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 343: Cloud-authority guard
    if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
        return rejection.into_response();
    }
    match sqlx::query(
        "UPDATE staff_members SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                Json(json!({ "error": "Staff member not found" })).into_response()
            } else {
                Json(json!({ "status": "ok", "id": id })).into_response()
            }
        }
        Err(e) => Json(json!({ "error": format!("{}", e) })).into_response(),
    }
}

pub(crate) async fn reset_staff_pin(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 343: Cloud-authority guard
    if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
        return rejection.into_response();
    }
    // Look up staff member
    let staff = sqlx::query_as::<_, (String, String)>(
        "SELECT name, phone FROM staff_members WHERE id = ? AND is_active = 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let (name, phone) = match staff {
        Some(s) => s,
        None => return Json(json!({ "error": "Staff member not found" })).into_response(),
    };

    // Generate unique PIN with collision retry
    let mut new_pin = String::new();
    for _ in 0..10 {
        let candidate = format!("{:04}", rand::thread_rng().gen_range(1000u32..=9999));
        let conflict = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM staff_members WHERE pin = ? AND id != ? AND is_active = 1",
        )
        .bind(&candidate)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
        if conflict == 0 {
            new_pin = candidate;
            break;
        }
    }
    if new_pin.is_empty() {
        return Json(json!({ "error": "Could not generate unique PIN after 10 attempts" })).into_response();
    }

    if let Err(e) = sqlx::query("UPDATE staff_members SET pin = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&new_pin)
        .bind(&id)
        .execute(&state.db)
        .await
    {
        return Json(json!({ "error": format!("Failed to reset PIN: {}", e) })).into_response();
    }

    // Phase 343-02: Post-write verify for reset PIN
    let correlation_id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = post_write_verify_staff_pin(&state.db, &id, &new_pin).await {
        tracing::error!(target: "auth", correlation_id = %correlation_id, error = %e, "reset_staff_pin post_write_verify failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("PIN write verification failed: {}", e),
            "correlation_id": correlation_id,
        }))).into_response();
    }
    spawn_delayed_sync_verify(
        state.db.clone(),
        state.config.cloud.sync_interval_secs,
        id.clone(),
        new_pin.clone(),
        correlation_id.clone(),
    );

    // Send via WhatsApp
    let msg = format!("Racing Point - Your new staff PIN is: {}\nReset by admin.", new_pin);
    whatsapp_alerter::send_whatsapp_to(&state.config, &phone, &msg).await;

    tracing::info!(target: "auth", correlation_id = %correlation_id, "Staff PIN reset for {} ({}) by admin", name, id);

    Json(json!({
        "status": "ok",
        "staff_id": id,
        "staff_name": name,
        "new_pin": new_pin,
        "verified": true,
        "correlation_id": correlation_id,
    })).into_response()
}

// Phase 347-01: Orchestrated safe PIN change endpoint (manager+ gated via router)
pub(crate) async fn change_staff_pin_safe(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChangePinRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let correlation_id = uuid::Uuid::new_v4().to_string();

    // Validate PIN format
    if let Err(e) = validate_pin_format(&req.new_pin) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": e,
            "correlation_id": correlation_id,
        }))).into_response();
    }

    let is_cloud = crate::config::this_instance_is_cloud(&state.config);

    let cloud_verified;

    if !is_cloud {
        // Venue instance: forward the write to cloud
        let cloud_url = match state.config.cloud.api_url.clone() {
            Some(u) => u,
            None => {
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error": "cloud_url not configured — cannot forward PIN change",
                    "correlation_id": correlation_id,
                }))).into_response();
            }
        };

        // Extract Bearer token — clone the string value before any await
        let auth_header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let target_url = format!("{}/api/v1/admin/staff/{}/change-pin", cloud_url, id);
        let body_json = serde_json::json!({ "new_pin": req.new_pin });

        let mut cloud_req = state
            .http_client
            .post(&target_url)
            .json(&body_json)
            .timeout(std::time::Duration::from_secs(15));

        if let Some(auth) = auth_header {
            cloud_req = cloud_req.header("authorization", auth);
        }

        let cloud_resp = match cloud_req.send().await {
            Ok(r) => r,
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                    "error": format!("Cloud PIN change failed: {}", e),
                    "correlation_id": correlation_id,
                }))).into_response();
            }
        };

        if !cloud_resp.status().is_success() {
            let status_code = cloud_resp.status();
            let err_body = cloud_resp.text().await.unwrap_or_default();
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("Cloud PIN change failed ({}): {}", status_code, err_body),
                "correlation_id": correlation_id,
            }))).into_response();
        }

        // Parse the ChangePinResponse from cloud
        let cloud_result: serde_json::Value = cloud_resp.json().await.unwrap_or_default();
        cloud_verified = cloud_result.get("cloud_verified").and_then(|v| v.as_bool()).unwrap_or(false);
    } else {
        // Cloud instance: apply the write locally
        // cloud_authority_guard is a no-op on cloud, but call it for consistency
        if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
            return rejection.into_response();
        }

        let new_pin_clone = req.new_pin.clone();
        let id_clone = id.clone();
        let result = sqlx::query(
            "UPDATE staff_members SET pin = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&new_pin_clone)
        .bind(&id_clone)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() == 0 => {
                return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                    "error": "Staff member not found",
                    "correlation_id": correlation_id,
                }))).into_response();
            }
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                    "error": format!("Failed to update PIN: {}", e),
                    "correlation_id": correlation_id,
                }))).into_response();
            }
            Ok(_) => {}
        }

        cloud_verified = post_write_verify_staff_pin(&state.db, &id, &req.new_pin).await.is_ok();
    }

    // Trigger immediate sync: pull staff_members from cloud to venue
    // (log warning on failure but don't fail the whole operation)
    if let Err(e) = cloud_sync::pull_tables_now(&state, &["staff_members"]).await {
        tracing::warn!(
            target: "auth",
            correlation_id = %correlation_id,
            "change_staff_pin_safe: pull_tables_now failed (non-fatal): {}",
            e
        );
    }

    // Verify venue-side PIN after sync
    let venue_verified = post_write_verify_staff_pin(&state.db, &id, &req.new_pin).await.is_ok();

    // Spawn delayed verify to catch sync reversions
    spawn_delayed_sync_verify(
        state.db.clone(),
        state.config.cloud.sync_interval_secs,
        id.clone(),
        req.new_pin.clone(),
        correlation_id.clone(),
    );

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = if cloud_verified && venue_verified { "ok" } else { "partial" };

    tracing::info!(
        target: "auth",
        correlation_id = %correlation_id,
        staff_id = %id,
        cloud_verified = cloud_verified,
        venue_verified = venue_verified,
        latency_ms = latency_ms,
        "change_staff_pin_safe completed with status={}",
        status
    );

    (StatusCode::OK, Json(ChangePinResponse {
        status: status.to_string(),
        cloud_verified,
        venue_verified,
        latency_ms,
        correlation_id,
    })).into_response()
}

// Phase 347-01: Allowed sync tables (subset of SYNC_TABLES constant in cloud_sync.rs)
pub(crate) const ALLOWED_SYNC_TABLES: &[&str] = &[
    "drivers",
    "wallets",
    "pricing_tiers",
    "pricing_rules",
    "billing_rates",
    "kiosk_experiences",
    "kiosk_settings",
    "auth_tokens",
    "reservations",
    "debit_intents",
    "staff_members",
    "driver_ratings",
    "fleet_solutions",
    "model_evaluations",
];

// Phase 347-01: On-demand filtered cloud pull endpoint (manager+ gated via router)
pub(crate) async fn sync_pull_now_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncPullNowRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    if req.tables.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "tables list must not be empty",
        }))).into_response();
    }

    // Validate each table name against the allowed list
    for table in &req.tables {
        if !ALLOWED_SYNC_TABLES.contains(&table.as_str()) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("unknown table: '{}'", table),
                "allowed": ALLOWED_SYNC_TABLES,
            }))).into_response();
        }
    }

    let table_refs: Vec<&str> = req.tables.iter().map(|s| s.as_str()).collect();

    match cloud_sync::pull_tables_now(&state, &table_refs).await {
        Ok(()) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            (StatusCode::OK, Json(SyncPullNowResponse {
                status: "ok".to_string(),
                tables_synced: req.tables,
                latency_ms,
            })).into_response()
        }
        Err(e) => {
            tracing::error!("sync_pull_now_handler: pull_tables_now failed: {}", e);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("Pull failed: {}", e),
            }))).into_response()
        }
    }
}
