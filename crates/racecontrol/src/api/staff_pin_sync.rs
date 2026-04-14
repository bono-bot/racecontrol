#![allow(unused_imports)]
use crate::api::auth_staff::{cloud_authority_guard, post_write_verify_staff_pin, spawn_delayed_sync_verify, validate_pin_format, ChangePinRequest, ChangePinResponse};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::cloud_sync;
use crate::state::AppState;

use crate::api::auth_staff::SyncPullNowRequest;
use crate::api::auth_staff::SyncPullNowResponse;

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
