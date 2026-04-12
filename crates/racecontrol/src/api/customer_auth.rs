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

// ─── Customer PWA Endpoints ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct CustomerLoginRequest {
    phone: String,
}

pub(crate) async fn customer_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CustomerLoginRequest>,
) -> Json<Value> {
    match auth::send_otp(&state, &req.phone).await {
        Ok(result) => Json(json!({
            "status": "otp_sent",
            "delivered": result.delivered
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_resend_otp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CustomerLoginRequest>,
) -> Json<Value> {
    match auth::resend_otp(&state, &req.phone).await {
        Ok(result) => Json(json!({
            "status": "otp_sent",
            "delivered": result.delivered
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct VerifyOtpRequest {
    phone: String,
    otp: String,
}

pub(crate) async fn customer_verify_otp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyOtpRequest>,
) -> Json<Value> {
    match auth::verify_otp(&state, &req.phone, &req.otp).await {
        Ok(jwt) => {
            // Check registration status
            let registered = sqlx::query_as::<_, (bool,)>(
                "SELECT COALESCE(registration_completed, 0) FROM drivers WHERE phone_hash = ?",
            )
            .bind(state.field_cipher.hash_phone(&req.phone))
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.0)
            .unwrap_or(false);

            Json(json!({
                "status": "ok",
                "token": jwt,
                "registration_completed": registered,
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Extract driver_id from Authorization: Bearer <jwt> header
pub(crate) fn extract_driver_id(state: &AppState, headers: &axum::http::HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| "Invalid Authorization format".to_string())?;

    auth::verify_jwt(token, &state.config.auth.jwt_secret)
}

pub(crate) async fn customer_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, bool, Option<String>, Option<String>, bool, bool, Option<String>)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms, COALESCE(has_used_trial, 0), customer_id, nickname, COALESCE(show_nickname_on_leaderboard, 0), COALESCE(registration_completed, 0), phone_enc FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some(d)) => {
            let wallet_balance = wallet::get_balance(&state, &d.0).await.unwrap_or(0);
            let active_reservation = pod_reservation::get_active_reservation_for_driver(&state, &d.0).await;
            // Decrypt phone from phone_enc if plaintext phone is NULL
            let phone: Option<String> = d.3.clone().or_else(|| {
                d.11.as_deref().and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
            });

            Json(json!({
                "driver": {
                    "id": d.0,
                    "customer_id": d.7,
                    "name": d.1,
                    "nickname": d.8,
                    "show_nickname_on_leaderboard": d.9,
                    "email": d.2,
                    "phone": phone,
                    "total_laps": d.4,
                    "total_time_ms": d.5,
                    "has_used_trial": d.6,
                    "wallet_balance_paise": wallet_balance,
                    "active_reservation": active_reservation,
                    "registration_completed": d.10,
                }
            }))
        }
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn customer_update_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if let Some(nickname) = body.get("nickname") {
        let nick = nickname.as_str().map(|s| s.trim()).unwrap_or("");
        // MMA-R2-3: Validate nickname (XSS prevention)
        if !nick.is_empty() {
            if let Err(e) = crate::input_validation::validate_name(nick) {
                return Json(json!({ "error": format!("Invalid nickname: {}", e) }));
            }
        }
        let nick_val: Option<&str> = if nick.is_empty() { None } else { Some(nick) };
        let _ = sqlx::query("UPDATE drivers SET nickname = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(nick_val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    if let Some(show) = body.get("show_nickname_on_leaderboard") {
        let val = show.as_bool().unwrap_or(false);
        let _ = sqlx::query("UPDATE drivers SET show_nickname_on_leaderboard = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    Json(json!({ "status": "updated" }))
}

pub(crate) async fn customer_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at, bs.custom_price_paise,
                bs.discount_paise, bs.original_price_paise, bs.discount_reason
         FROM billing_sessions bs
         WHERE bs.driver_id = ?
         ORDER BY bs.created_at DESC
         LIMIT 50",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "allocated_seconds": s.2,
                        "driving_seconds": s.3,
                        "status": s.4,
                        "started_at": s.5,
                        "ended_at": s.6,
                        "custom_price_paise": s.7,
                        "discount_paise": s.8,
                        "original_price_paise": s.9,
                        "discount_reason": s.10,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

/// Compute percentile ranking for a best lap on a track+car combination.
/// Returns None if fewer than 5 unique drivers have driven this track+car,
/// or if track/car is empty.
pub(crate) async fn compute_percentile(db: &sqlx::SqlitePool, best_lap_ms: i64, track: &str, car: &str) -> Option<u32> {
    if track.is_empty() || car.is_empty() {
        return None;
    }

    let total_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT driver_id) FROM laps WHERE track = ? AND car = ? AND valid = 1",
    )
    .bind(track)
    .bind(car)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let faster_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT driver_id) FROM (
            SELECT driver_id, MIN(lap_time_ms) as best
            FROM laps WHERE track = ? AND car = ? AND valid = 1
            GROUP BY driver_id
        ) WHERE best < ?",
    )
    .bind(track)
    .bind(car)
    .bind(best_lap_ms)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    match (total_count, faster_count) {
        (Some((total,)), Some((faster,))) if total >= 5 => {
            Some(((total - faster) as f64 / total as f64 * 100.0).round() as u32)
        }
        _ => None,
    }
}
