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

// ─── SEC-09: PII masking helpers ──────────────────────────────────────────

/// Mask a phone number, showing only the first 2 and last 2 digits.
/// Example: "9876543210" → "98****10"
pub(crate) fn mask_phone(phone: &str) -> String {
    if phone.len() <= 4 {
        return "****".to_string();
    }
    format!("{}****{}", &phone[..2], &phone[phone.len() - 2..])
}

/// Mask an email address, showing only the first 2 chars and the domain.
/// Example: "user@example.com" → "us***@example.com"
pub(crate) fn mask_email(email: &str) -> String {
    match email.find('@') {
        Some(at) if at > 2 => format!("{}***{}", &email[..2], &email[at..]),
        _ => "***@***".to_string(),
    }
}

/// Returns `true` if PII should be masked for the given staff claims.
/// Only manager and superadmin roles may see full PII.
pub(crate) fn should_mask_pii(claims: &Option<axum::Extension<crate::auth::middleware::StaffClaims>>) -> bool {
    match claims {
        Some(ext) => ext.role != "manager" && ext.role != "superadmin",
        None => true, // no claims = mask by default
    }
}

// ─── Re-exports from split modules (Phase 385) ─────────────────────────────
pub(crate) use super::driver_profile::get_driver_full_profile;
pub(crate) use super::session_lap_routes::{
    list_sessions, create_session, get_session,
    list_laps, session_laps,
    StaffTrackLeaderboardQuery, track_leaderboard,
    list_events, create_event, list_bookings, create_booking,
};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ListDriversQuery {
    search: Option<String>,
}

pub(crate) async fn list_drivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDriversQuery>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    let rows = if let Some(ref search) = params.search {
        // MMA-P2: Escape LIKE wildcards and limit length to prevent enumeration + DoS
        let sanitized: String = search.chars()
            .filter(|c| !matches!(c, '%' | '_'))
            .take(50)
            .collect();
        if sanitized.is_empty() {
            return Json(json!({ "error": "Search query too short or invalid" }));
        }
        let q = format!("%{}%", sanitized);
        // If search looks like a phone number (all digits, 10+ chars), also match by phone_hash
        let is_phone_query = sanitized.chars().all(|c| c.is_ascii_digit()) && sanitized.len() >= 10;
        let phone_hash = if is_phone_query {
            Some(state.field_cipher.hash_phone(&sanitized))
        } else {
            None
        };
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, bool, bool, Option<String>, Option<String>, Option<i64>, Option<String>)>(
            "SELECT d.id, d.name, d.email, d.phone, d.total_laps, d.total_time_ms, d.customer_id,
                    COALESCE(d.waiver_signed, 0), COALESCE(d.has_used_trial, 0), d.created_at, d.linked_to,
                    w.balance_paise, d.phone_enc
             FROM drivers d LEFT JOIN wallets w ON w.driver_id = d.id
             WHERE d.name LIKE ?1 COLLATE NOCASE OR d.phone LIKE ?2 OR d.customer_id LIKE ?3
                   OR d.phone_hash = ?4
             ORDER BY d.name LIMIT 50",
        )
        .bind(&q)
        .bind(&q)
        .bind(&q)
        .bind(&phone_hash)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, bool, bool, Option<String>, Option<String>, Option<i64>, Option<String>)>(
            "SELECT d.id, d.name, d.email, d.phone, d.total_laps, d.total_time_ms, d.customer_id,
                    COALESCE(d.waiver_signed, 0), COALESCE(d.has_used_trial, 0), d.created_at, d.linked_to,
                    w.balance_paise, d.phone_enc
             FROM drivers d LEFT JOIN wallets w ON w.driver_id = d.id
             ORDER BY d.created_at DESC",
        )
        .fetch_all(&state.db)
        .await
    };

    // Total count
    let total: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    let waiver_count: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers WHERE waiver_signed = 1")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    match rows {
        Ok(drivers) => {
            let list: Vec<Value> = drivers.iter().map(|d| {
                // Decrypt phone from phone_enc if plaintext phone is NULL (post-encryption migration)
                let raw_phone: Option<String> = d.3.clone().or_else(|| {
                    d.12.as_deref().and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
                });
                // SEC-09: mask PII for cashier role
                let email = d.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
                let phone = raw_phone.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
                json!({
                    "id": d.0, "name": d.1, "email": email, "phone": phone,
                    "total_laps": d.4, "total_time_ms": d.5, "customer_id": d.6,
                    "waiver_signed": d.7, "has_used_trial": d.8, "created_at": d.9,
                    "linked_to": d.10, "wallet_balance_paise": d.11.unwrap_or(0),
                })
            }).collect();
            Json(json!({ "drivers": list, "total": total, "waiver_count": waiver_count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_driver(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "drivers") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let email = body.get("email").and_then(|v| v.as_str());
    let phone = body.get("phone").and_then(|v| v.as_str());
    let steam_guid = body.get("steam_guid").and_then(|v| v.as_str());

    // Encrypt PII fields
    let phone_hash = phone.filter(|p| !p.is_empty()).map(|p| state.field_cipher.hash_phone(p));
    let phone_enc = match phone.filter(|p| !p.is_empty()).map(|p| state.field_cipher.encrypt_field(p)) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return Json(json!({ "error": format!("Encrypt error: {}", e) })).into_response(),
        None => None,
    };
    let email_enc = match email.filter(|e| !e.is_empty()).map(|e| state.field_cipher.encrypt_field(e)) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return Json(json!({ "error": format!("Encrypt error: {}", e) })).into_response(),
        None => None,
    };
    let name_enc = match state.field_cipher.encrypt_field(name) {
        Ok(v) => Some(v),
        Err(e) => return Json(json!({ "error": format!("Encrypt error: {}", e) })).into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, name_enc, phone_hash, phone_enc, email_enc, steam_guid, updated_at, venue_id) VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)"
    )
    .bind(&id)
    .bind(name) // Keep plaintext name for leaderboard backward compat
    .bind(&name_enc)
    .bind(&phone_hash)
    .bind(&phone_enc)
    .bind(&email_enc)
    .bind(steam_guid)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn get_driver(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(d)) => {
            // SEC-09: mask PII for cashier role
            let email = d.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
            let phone = d.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
            Json(json!({
                "id": d.0, "name": d.1, "email": email, "phone": phone,
                "total_laps": d.4, "total_time_ms": d.5,
            }))
        },
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

