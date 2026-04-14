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

// Re-export linked racers items so `use super::customer_register::*` in routes.rs
// continues to resolve everything it needs.
pub(crate) use super::customer_linked_racers::{
    MAX_LINKED_RACERS, AddRacerRequest,
    customer_add_racer, customer_list_racers, customer_waiver_status,
};

// ─── Customer Registration ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct CustomerRegisterRequest {
    name: String,
    phone: Option<String>,
    nickname: Option<String>,
    email: Option<String>,
    dob: String,
    waiver_consent: bool,
    signature_data: Option<String>,
    guardian_name: Option<String>,
    guardian_phone: Option<String>,
}

pub(crate) async fn customer_register(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CustomerRegisterRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if !req.waiver_consent {
        return Json(json!({ "error": "Waiver consent is required" }));
    }

    // MMA-WIRED: Use centralized input validation module
    let name = match crate::input_validation::validate_name(&req.name) {
        Ok(n) => n,
        Err(e) => return Json(json!({ "error": e })),
    };
    if let Some(ref email) = req.email {
        if let Err(e) = crate::input_validation::validate_email(email) {
            return Json(json!({ "error": e }));
        }
    }
    if let Some(ref phone) = req.guardian_phone {
        if let Err(e) = crate::input_validation::validate_phone(phone) {
            return Json(json!({ "error": format!("Guardian phone: {}", e) }));
        }
    }
    if let Some(ref phone) = req.phone {
        if let Err(e) = crate::input_validation::validate_phone(phone) {
            return Json(json!({ "error": e }));
        }
    }

    // Parse and validate DOB
    let dob = match chrono::NaiveDate::parse_from_str(&req.dob, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Json(json!({ "error": "Invalid date format. Use YYYY-MM-DD" })),
    };

    let today = chrono::Utc::now().date_naive();
    let age = (today - dob).num_days() / 365;

    if age < 12 {
        return Json(json!({ "error": "Minimum age is 12 years" }));
    }

    // Guardian required for minors (12-17)
    if age < 18 {
        if req.guardian_name.as_ref().map_or(true, |n| n.trim().is_empty()) {
            return Json(json!({ "error": "Guardian name required for customers under 18" }));
        }
    }

    // Check for duplicate name + DOB (same person already registered)
    let duplicate: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE name = ? AND dob = ? AND registration_completed = 1 AND id != ?",
    )
    .bind(&name)
    .bind(&req.dob)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if duplicate.is_some() {
        return Json(json!({ "error": "An account with this name and date of birth already exists. Please sign in with your registered phone number." }));
    }

    let nickname = req.nickname.as_ref().map(|n| n.trim().to_string()).filter(|n| !n.is_empty());

    // Encrypt phone if provided (don't store plaintext)
    let (phone_hash, phone_enc): (Option<String>, Option<String>) = match &req.phone {
        Some(p) if !p.is_empty() => {
            let hash = state.field_cipher.hash_phone(p);
            match state.field_cipher.encrypt_field(p) {
                Ok(enc) => (Some(hash), Some(enc)),
                Err(e) => return Json(json!({ "error": format!("Encryption error: {}", e) })),
            }
        }
        _ => (None, None),
    };

    // Encrypt guardian phone if provided
    let (gp_hash, gp_enc): (Option<String>, Option<String>) = match &req.guardian_phone {
        Some(gp) if !gp.is_empty() => {
            let h = state.field_cipher.hash_phone(gp);
            match state.field_cipher.encrypt_field(gp) {
                Ok(e) => (Some(h), Some(e)),
                Err(_) => (None, None),
            }
        }
        _ => (None, None),
    };

    // Update driver record — store encrypted phone, not plaintext
    let result = sqlx::query(
        "UPDATE drivers SET
            name = ?, nickname = ?, email = ?, dob = ?,
            phone_hash = COALESCE(?, phone_hash),
            phone_enc = COALESCE(?, phone_enc),
            waiver_signed = 1, waiver_signed_at = datetime('now'),
            waiver_version = 'v1.0',
            signature_data = ?,
            guardian_name = ?,
            guardian_phone_hash = COALESCE(?, guardian_phone_hash),
            guardian_phone_enc = COALESCE(?, guardian_phone_enc),
            registration_completed = 1,
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&nickname)
    .bind(&req.email)
    .bind(&req.dob)
    .bind(&phone_hash)
    .bind(&phone_enc)
    .bind(&req.signature_data)
    .bind(&req.guardian_name)
    .bind(&gp_hash)
    .bind(&gp_enc)
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Auto-create wallet for new customer
            let _ = wallet::ensure_wallet(&state, &driver_id).await;

            tracing::info!("Customer {} registered (age: {}, minor: {})", driver_id, age, age < 18);
            Json(json!({
                "status": "registered",
                "driver_id": driver_id,
                "is_minor": age < 18,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Registration failed: {}", e) })),
    }
}

// ─── Venue-Local Registration (no phone required) ──────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct VenueRegisterRequest {
    name: String,
    dob: String,
    waiver_consent: bool,
    guardian_name: Option<String>,
}

pub(crate) async fn venue_register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VenueRegisterRequest>,
) -> Json<Value> {
    if !req.waiver_consent {
        return Json(json!({ "error": "Waiver consent is required" }));
    }

    let name = match crate::input_validation::validate_name(&req.name) {
        Ok(n) => n,
        Err(e) => return Json(json!({ "error": e })),
    };

    let dob = match chrono::NaiveDate::parse_from_str(&req.dob, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Json(json!({ "error": "Invalid date format. Use YYYY-MM-DD" })),
    };

    let today = chrono::Utc::now().date_naive();
    let age = (today - dob).num_days() / 365;

    if age < 5 {
        return Json(json!({ "error": "Minimum age is 5 years" }));
    }

    if age < 18 {
        if req.guardian_name.as_ref().map_or(true, |n| n.trim().is_empty()) {
            return Json(json!({ "error": "Guardian name required for customers under 18" }));
        }
    }

    // Duplicate check
    let duplicate: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, customer_id FROM drivers WHERE name = ? AND dob = ? AND registration_completed = 1",
    )
    .bind(&name)
    .bind(&req.dob)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if let Some((existing_id, existing_cid)) = duplicate {
        return Json(json!({
            "status": "existing",
            "driver_id": existing_id,
            "customer_id": existing_cid,
            "message": "Customer already registered"
        }));
    }

    let driver_id = format!("drv_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));
    let customer_id = {
        // Customer IDs use "RP###" format (e.g. RP001..RP999+).
        // CAST returns INTEGER from SQLite — decode as Option<i64>, not String.
        // Legacy "000001"/"000002" IDs from pre-RP format are ignored by the LIKE 'RP%' filter.
        let max: Option<(Option<i64>,)> = sqlx::query_as(
            "SELECT MAX(CAST(SUBSTR(customer_id, 3) AS INTEGER)) FROM drivers WHERE customer_id LIKE 'RP%'",
        )
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);
        let next_num = max
            .and_then(|m| m.0)
            .map(|n| n as u64)
            .unwrap_or(0) + 1;
        format!("RP{:03}", next_num)
    };

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, dob, customer_id, waiver_signed, waiver_signed_at, waiver_version, guardian_name, registration_completed, created_at)
         VALUES (?, ?, ?, ?, 1, datetime('now'), 'v1.0', ?, 1, datetime('now'))",
    )
    .bind(&driver_id)
    .bind(&name)
    .bind(&req.dob)
    .bind(&customer_id)
    .bind(&req.guardian_name)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let _ = wallet::ensure_wallet(&state, &driver_id).await;
            tracing::info!("Venue registration: {} (age: {}, id: {}, cid: {})", name, age, driver_id, customer_id);
            Json(json!({
                "status": "registered",
                "driver_id": driver_id,
                "customer_id": customer_id,
                "name": name,
                "is_minor": age < 18,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Registration failed: {}", e) })),
    }
}
