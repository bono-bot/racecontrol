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

// ─── Waivers (admin-facing) ──────────────────────────────────────────────────

pub(crate) async fn list_waivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let page: i64 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1).max(1);
    let per_page: i64 = params.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * per_page;

    let total = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM drivers WHERE waiver_signed = 1",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, phone, email, dob, waiver_signed_at, waiver_version, guardian_name, guardian_phone, signature_data
         FROM drivers WHERE waiver_signed = 1
         ORDER BY waiver_signed_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(waivers) => {
            let list: Vec<Value> = waivers.iter().map(|w| {
                let is_minor = w.4.as_ref().is_some_and(|dob| {
                    chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
                        .map(|d| (chrono::Utc::now().date_naive() - d).num_days() / 365 < 18)
                        .unwrap_or(false)
                });
                json!({
                    "driver_id": w.0,
                    "name": w.1,
                    "phone": w.2,
                    "email": w.3,
                    "dob": w.4,
                    "waiver_signed_at": w.5,
                    "waiver_version": w.6,
                    "guardian_name": w.7,
                    "guardian_phone": w.8,
                    "has_signature": w.9.is_some(),
                    "is_minor": is_minor,
                })
            }).collect();
            Json(json!({ "waivers": list, "total": total, "page": page, "per_page": per_page }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn check_waiver(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let phone = params.get("phone");
    let email = params.get("email");

    if phone.is_none() && email.is_none() {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    }

    let row = if let Some(phone) = phone {
        // Normalize: strip non-digits, use last 10 for hash lookup (full match only)
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let last10 = if digits.len() >= 10 { &digits[digits.len() - 10..] } else { &digits };
        let ph = state.field_cipher.hash_phone(last10);
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone_enc, COALESCE(waiver_signed, 0) FROM drivers WHERE phone_hash = ?",
        )
        .bind(&ph)
        .fetch_optional(&state.db)
        .await
    } else if let Some(email) = email {
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone_enc, COALESCE(waiver_signed, 0) FROM drivers WHERE LOWER(email) = LOWER(?)",
        )
        .bind(email)
        .fetch_optional(&state.db)
        .await
    } else {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    };

    match row {
        Ok(Some((id, name, phone_enc, signed))) => {
            let phone = phone_enc.and_then(|enc| state.field_cipher.decrypt_field(&enc).ok());
            Json(json!({
                "signed": signed,
                "driver": { "id": id, "name": name, "phone": phone },
            }))
        }
        Ok(None) => Json(json!({ "signed": false, "driver": null })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

pub(crate) async fn get_waiver_signature(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT signature_data FROM drivers WHERE id = ? AND waiver_signed = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((Some(sig),))) => Json(json!({ "signature_data": sig })),
        Ok(Some((None,))) => Json(json!({ "error": "No signature on file" })),
        Ok(None) => Json(json!({ "error": "Waiver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}
