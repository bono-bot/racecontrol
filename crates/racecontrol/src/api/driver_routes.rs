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

/// GET /drivers/{id}/full-profile — comprehensive driver profile for admin
pub(crate) async fn get_driver_full_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    // Core driver info (10 fields)
    let core = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>, bool, Option<String>)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms,
                customer_id, nickname, COALESCE(has_used_trial, 0), dob
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let c = match core {
        Ok(Some(c)) => c,
        Ok(None) => return Json(json!({ "error": "Driver not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Waiver fields (separate query to stay under tuple limit)
    let waiver = sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, bool)>(
        "SELECT COALESCE(waiver_signed, 0), waiver_signed_at, waiver_version,
                guardian_name, guardian_phone, signature_data,
                COALESCE(show_nickname_on_leaderboard, 0)
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or((false, None, None, None, None, None, false));

    let is_minor = c.9.as_ref().map_or(false, |dob| {
        chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
            .map(|date| (chrono::Utc::now().date_naive() - date).num_days() / 365 < 18)
            .unwrap_or(false)
    });

    // SEC-09: mask PII for cashier role
    let email = c.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
    let phone = c.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
    let guardian_phone = waiver.4.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });

    let driver_json = json!({
        "id": c.0, "name": c.1, "email": email, "phone": phone,
        "total_laps": c.4, "total_time_ms": c.5,
        "customer_id": c.6, "nickname": c.7, "has_used_trial": c.8,
        "dob": c.9,
        "waiver_signed": waiver.0, "waiver_signed_at": waiver.1,
        "waiver_version": waiver.2, "guardian_name": waiver.3,
        "guardian_phone": guardian_phone, "has_signature": waiver.5.is_some(),
        "show_nickname_on_leaderboard": waiver.6, "is_minor": is_minor,
    });

    // Wallet
    let wallet = sqlx::query_as::<_, (i64, i64, i64, Option<String>)>(
        "SELECT balance_paise, total_credited_paise, total_debited_paise, updated_at FROM wallets WHERE driver_id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|w| json!({
        "balance_paise": w.0, "total_credited_paise": w.1,
        "total_debited_paise": w.2, "updated_at": w.3,
    }));

    // Recent wallet transactions (last 20)
    let txns = sqlx::query_as::<_, (String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, amount_paise, balance_after_paise, txn_type, reference_id, notes, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|t| json!({
        "id": t.0, "amount_paise": t.1, "balance_after_paise": t.2,
        "txn_type": t.3, "reference_id": t.4, "notes": t.5, "created_at": t.6,
    }))
    .collect::<Vec<_>>();

    // Billing sessions (last 20)
    let sessions = sqlx::query_as::<_, (String, String, i64, i64, String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status,
                bs.wallet_debit_paise, bs.started_at, bs.ended_at, pt.name
         FROM billing_sessions bs
         LEFT JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.driver_id = ?
         ORDER BY bs.started_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|s| json!({
        "id": s.0, "pod_id": s.1, "allocated_seconds": s.2,
        "driving_seconds": s.3, "status": s.4, "wallet_debit_paise": s.5,
        "started_at": s.6, "ended_at": s.7, "pricing_tier_name": s.8,
    }))
    .collect::<Vec<_>>();

    // Personal bests
    let pbs = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT track, car, best_lap_ms, achieved_at FROM personal_bests WHERE driver_id = ? ORDER BY achieved_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|p| json!({ "track": p.0, "car": p.1, "best_lap_ms": p.2, "achieved_at": p.3 }))
    .collect::<Vec<_>>();

    // Referral info
    let referral = sqlx::query_as::<_, (String,)>(
        "SELECT code FROM referrals WHERE referrer_id = ? AND code IS NOT NULL LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0);

    let referral_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND status = 'completed'"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Membership
    let membership = sqlx::query_as::<_, (String, String, f64, f64, String, bool, String)>(
        "SELECT m.id, mt.name, m.hours_used_minutes, mt.hours_included, m.expires_at, m.auto_renew, m.status
         FROM memberships m JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active' LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|m| json!({
        "id": m.0, "tier_name": m.1, "hours_used": m.2,
        "hours_included": m.3, "expires_at": m.4, "auto_renew": m.5, "status": m.6,
    }));

    // Refunds
    let refunds = sqlx::query_as::<_, (String, i64, String, String, Option<String>, String)>(
        "SELECT r.billing_session_id, r.amount_paise, r.method, r.reason, r.notes, r.created_at
         FROM refunds r WHERE r.driver_id = ? ORDER BY r.created_at DESC LIMIT 10"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|r| json!({
        "billing_session_id": r.0, "amount_paise": r.1, "method": r.2,
        "reason": r.3, "notes": r.4, "created_at": r.5,
    }))
    .collect::<Vec<_>>();

    Json(json!({
        "driver": driver_json,
        "wallet": wallet,
        "transactions": txns,
        "sessions": sessions,
        "personal_bests": pbs,
        "referral_code": referral,
        "referral_count": referral_count,
        "membership": membership,
        "refunds": refunds,
    }))
}

pub(crate) async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT id, type, sim_type, track, status, started_at FROM sessions ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions.iter().map(|s| json!({
                "id": s.0, "type": s.1, "sim_type": s.2,
                "track": s.3, "status": s.4, "started_at": s.5,
            })).collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_sessions") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let session_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("practice");
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("monza");
    let car_class = body.get("car_class").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track, car_class, status, venue_id) VALUES (?, ?, ?, ?, ?, 'pending', ?)"
    )
    .bind(&id)
    .bind(session_type)
    .bind(sim_type)
    .bind(track)
    .bind(car_class)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "type": session_type, "track": track })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT bs.id, bs.driver_id, d.name as driver_name, bs.pod_id,
                bs.pricing_tier_id, pt.name as tier_name,
                bs.allocated_seconds, bs.driving_seconds, bs.status,
                COALESCE(bs.custom_price_paise, pt.price_paise) as price_paise,
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name as experience_name,
                bs.car, bs.track, bs.sim_type,
                bs.reservation_id, bs.wallet_txn_id,
                bs.wallet_debit_paise, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get laps count and best lap for this session
    let lap_stats = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT COUNT(*), MIN(CASE WHEN valid = 1 THEN lap_time_ms END)
         FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, None));

    Json(json!({
        "session": {
            "id": row.get::<String, _>("id"),
            "driver_id": row.get::<String, _>("driver_id"),
            "driver_name": row.get::<String, _>("driver_name"),
            "pod_id": row.get::<String, _>("pod_id"),
            "pricing_tier_id": row.get::<String, _>("pricing_tier_id"),
            "pricing_tier_name": row.get::<String, _>("tier_name"),
            "allocated_seconds": row.get::<i64, _>("allocated_seconds"),
            "driving_seconds": row.get::<i64, _>("driving_seconds"),
            "status": row.get::<String, _>("status"),
            "price_paise": row.get::<i64, _>("price_paise"),
            "started_at": row.get::<Option<String>, _>("started_at"),
            "ended_at": row.get::<Option<String>, _>("ended_at"),
            "experience_id": row.get::<Option<String>, _>("experience_id"),
            "experience_name": row.get::<Option<String>, _>("experience_name"),
            "car": row.get::<Option<String>, _>("car"),
            "track": row.get::<Option<String>, _>("track"),
            "sim_type": row.get::<Option<String>, _>("sim_type"),
            "reservation_id": row.get::<Option<String>, _>("reservation_id"),
            "wallet_txn_id": row.get::<Option<String>, _>("wallet_txn_id"),
            "wallet_debit_paise": row.get::<Option<i64>, _>("wallet_debit_paise"),
            "created_at": row.get::<String, _>("created_at"),
            "total_laps": lap_stats.0,
            "best_lap_ms": lap_stats.1,
        }
    }))
}

pub(crate) async fn list_laps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT id, driver_id, track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid FROM laps ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps.iter().map(|l| json!({
                "id": l.0, "driver_id": l.1, "track": l.2, "car": l.3,
                "lap_time_ms": l.4, "sector1_ms": l.5, "sector2_ms": l.6,
                "sector3_ms": l.7, "valid": l.8,
            })).collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn session_laps(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, i64, i64,
        Option<i64>, Option<i64>, Option<i64>, bool, String,
    )>(
        "SELECT l.id, l.driver_id, l.pod_id, l.track, l.car, l.lap_number, l.lap_time_ms,
                l.sector1_ms, l.sector2_ms, l.sector3_ms, l.valid, l.created_at
         FROM laps l
         WHERE l.session_id = ?
         ORDER BY l.lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "driver_id": l.1,
                        "pod_id": l.2,
                        "track": l.3,
                        "car": l.4,
                        "lap_number": l.5,
                        "lap_time_ms": l.6,
                        "sector1_ms": l.7,
                        "sector2_ms": l.8,
                        "sector3_ms": l.9,
                        "valid": l.10,
                        "created_at": l.11,
                    })
                })
                .collect();
            let count = list.len();
            Json(json!({ "session_id": id, "laps": list, "count": count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct StaffTrackLeaderboardQuery {
    sim_type: Option<String>,
    /// Filter by car class — UX-05 segmentation (staff view)
    car_class: Option<String>,
    /// Filter by assist tier — UX-05 segmentation (staff view)
    assist_tier: Option<String>,
}

pub(crate) async fn track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<StaffTrackLeaderboardQuery>,
) -> Json<Value> {
    // UX-04: JOIN laps to apply billing_session_id IS NOT NULL filter
    // UX-05: car_class + assist_tier segmentation for staff consistency with public view
    // UX-07: validity = 'valid' enforced — staff never see unverifiable laps
    let sim_clause = if params.sim_type.is_some() { " AND tr.sim_type = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { " AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { " AND l.assist_tier = ?" } else { "" };

    let query_str = format!(
        "SELECT tr.track, tr.car,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                tr.best_lap_ms, tr.achieved_at, tr.lap_id, tr.sim_type
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         LEFT JOIN laps l ON l.id = tr.lap_id
         WHERE tr.track = ?
           AND (l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)
           AND (l.validity IS NULL OR l.validity = 'valid')
           {}{}{}
         ORDER BY tr.best_lap_ms ASC",
        sim_clause, car_class_clause, assist_tier_clause
    );

    let mut q = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, String)>(&query_str);
    q = q.bind(&track);
    if let Some(ref st) = params.sim_type { q = q.bind(st); }
    if let Some(ref cc) = params.car_class { q = q.bind(cc); }
    if let Some(ref at) = params.assist_tier { q = q.bind(at); }
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(records) => {
            let list: Vec<Value> = records.iter().enumerate().map(|(i, r)| json!({
                "position": i + 1,
                "track": r.0, "car": r.1, "driver": r.2,
                "best_lap_ms": r.3, "achieved_at": r.4, "lap_id": r.5,
                "sim_type": r.6,
            })).collect();
            Json(json!({
                "track": track,
                "sim_type": params.sim_type,
                "car_class": params.car_class,
                "assist_tier": params.assist_tier,
                "records": list,
                "last_updated": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn list_events(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "events": [] }))
}

pub(crate) async fn create_event(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_event" }))
}

pub(crate) async fn list_bookings(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "bookings": [] }))
}

pub(crate) async fn create_booking(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_booking" }))
}
