#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
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

// ─── Kiosk ──────────────────────────────────────────────────────────────────

pub(crate) async fn list_kiosk_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64, Option<String>)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active, pricing_tier_id
         FROM kiosk_experiences WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(experiences) => {
            let list: Vec<Value> = experiences
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "name": e.1, "game": e.2,
                        "track": e.3, "car": e.4, "car_class": e.5,
                        "duration_minutes": e.6, "start_type": e.7,
                        "ac_preset_id": e.8, "sort_order": e.9,
                        "is_active": e.10 != 0,
                        "pricing_tier_id": e.11,
                    })
                })
                .collect();
            Json(json!({ "experiences": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "kiosk_experiences") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("New Experience");
    let game = body.get("game").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("spa");
    let car = body.get("car").and_then(|v| v.as_str()).unwrap_or("ks_ferrari_sf15t");
    let car_class = body.get("car_class").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let start_type = body.get("start_type").and_then(|v| v.as_str()).unwrap_or("pitlane");
    let ac_preset_id = body.get("ac_preset_id").and_then(|v| v.as_str());
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(game)
    .bind(track)
    .bind(car)
    .bind(car_class)
    .bind(duration_minutes)
    .bind(start_type)
    .bind(ac_preset_id)
    .bind(sort_order)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn get_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active
         FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(e)) => Json(json!({
            "id": e.0, "name": e.1, "game": e.2,
            "track": e.3, "car": e.4, "car_class": e.5,
            "duration_minutes": e.6, "start_type": e.7,
            "ac_preset_id": e.8, "sort_order": e.9,
            "is_active": e.10 != 0,
        })),
        Ok(None) => Json(json!({ "error": "Experience not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn update_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "kiosk_experiences") {
        return rejection.into_response();
    }
    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(v) = body.get("name").and_then(|v| v.as_str()) {
        updates.push("name = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("game").and_then(|v| v.as_str()) {
        updates.push("game = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("track").and_then(|v| v.as_str()) {
        updates.push("track = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car").and_then(|v| v.as_str()) {
        updates.push("car = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car_class").and_then(|v| v.as_str()) {
        updates.push("car_class = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("duration_minutes").and_then(|v| v.as_i64()) {
        updates.push("duration_minutes = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("start_type").and_then(|v| v.as_str()) {
        updates.push("start_type = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("ac_preset_id").and_then(|v| v.as_str()) {
        updates.push("ac_preset_id = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("sort_order").and_then(|v| v.as_i64()) {
        updates.push("sort_order = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("is_active").and_then(|v| v.as_bool()) {
        updates.push("is_active = ?");
        binds.push(if v { "1".to_string() } else { "0".to_string() });
    }
    if let Some(v) = body.get("pricing_tier_id").and_then(|v| v.as_str()) {
        updates.push("pricing_tier_id = ?");
        binds.push(v.to_string());
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    let query = format!("UPDATE kiosk_experiences SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Experience not found" })).into_response(),
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "kiosk_experiences") {
        return rejection.into_response();
    }
    match sqlx::query("UPDATE kiosk_experiences SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })).into_response(),
        Ok(_) => Json(json!({ "error": "Experience not found" })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn kiosk_pod_launch_experience(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = match body["pod_id"].as_str() {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "pod_id required" })),
    };
    let experience_id = match body["experience_id"].as_str() {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "experience_id required" })),
    };

    // Verify pod exists (pods are in-memory, not DB)
    {
        let pods = state.pods.read().await;
        if !pods.contains_key(&pod_id) {
            return Json(json!({ "error": "Pod not found" }));
        }
    }

    // Find active billing session for this pod (join drivers for name)
    let billing = sqlx::query_as::<_, (String, String, String)>(
        "SELECT bs.id, bs.driver_id, d.name FROM billing_sessions bs JOIN drivers d ON d.id = bs.driver_id WHERE bs.pod_id = ? AND bs.status = 'active' ORDER BY bs.started_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await;

    let (billing_session_id, _driver_id, driver_name) = match billing {
        Ok(Some(b)) => b,
        Ok(None) => return Json(json!({ "error": "No active billing session on this pod" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Verify experience exists
    let exp = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM kiosk_experiences WHERE id = ? AND is_active = 1",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await;

    if exp.ok().flatten().is_none() {
        return Json(json!({ "error": "Experience not found" }));
    }

    // Launch or show assistance
    let exp_id_opt = Some(experience_id);
    auth::launch_or_assist(
        &state,
        &pod_id,
        &billing_session_id,
        &exp_id_opt,
        &None,
        &driver_name,
    )
    .await;

    Json(json!({ "ok": true, "billing_session_id": billing_session_id }))
}

/// Kiosk self-service multiplayer booking.
/// Customers call this after authenticating via phone+OTP.
/// Creates a multiplayer group session, allocates pods, generates unique PINs per pod,
/// and auto-starts the AC server.
pub(crate) async fn kiosk_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Extract driver_id from Bearer token (same auth as customer_book_session)
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(json!({ "error": e }))),
    };

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Missing 'pricing_tier_id'" }))),
    };

    let pod_count = match req.get("pod_count").and_then(|v| v.as_u64()) {
        Some(n) => n as usize,
        None => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Missing 'pod_count'" }))),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str()).map(String::from);

    let custom = req.get("custom").and_then(|v| {
        let game = v.get("game")?.as_str()?.to_string();
        let track = v.get("track")?.as_str()?.to_string();
        let car = v.get("car")?.as_str()?.to_string();
        Some((game, track, car))
    });

    if experience_id.is_none() && custom.is_none() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Must provide 'experience_id' or 'custom' payload" })));
    }

    match multiplayer::book_multiplayer_kiosk(
        &state,
        &driver_id,
        &pricing_tier_id,
        pod_count,
        experience_id.as_deref(),
        custom,
    )
    .await
    {
        Ok(result) => (StatusCode::OK, Json(json!({
            "status": "ok",
            "group_session_id": result.group_session_id,
            "experience_name": result.experience_name,
            "tier_name": result.tier_name,
            "allocated_seconds": result.allocated_seconds,
            "assignments": result.assignments,
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))),
    }
}

/// PIN configuration constants — exposed via kiosk settings so the frontend
/// reads config truth instead of hardcoding (standing rule: UI must reflect config truth).
pub(crate) const PIN_REDEEM_LENGTH: u32 = 4;
pub(crate) const PIN_REDEEM_MAX_ATTEMPTS: u32 = 10;
pub(crate) const PIN_REDEEM_LOCKOUT_SECONDS: u32 = 300;

pub(crate) async fn get_kiosk_settings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM kiosk_settings",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(settings) => {
            let mut map = serde_json::Map::new();
            for (key, value) in &settings {
                map.insert(key.clone(), json!(value));
            }
            // Inject PIN config constants so frontend reads from server, not hardcoded (C3)
            map.insert("pin_length".to_string(), json!(PIN_REDEEM_LENGTH.to_string()));
            map.insert("pin_max_attempts".to_string(), json!(PIN_REDEEM_MAX_ATTEMPTS.to_string()));
            map.insert("pin_lockout_seconds".to_string(), json!(PIN_REDEEM_LOCKOUT_SECONDS.to_string()));
            Json(json!({ "settings": map }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn update_kiosk_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "kiosk_settings") {
        return rejection.into_response();
    }
    let obj = match body.as_object() {
        Some(o) => o,
        None => return Json(json!({ "error": "Expected a JSON object of key-value pairs" })).into_response(),
    };

    let mut updated = 0;
    for (key, value) in obj {
        let val_str = match value.as_str() {
            Some(s) => s.to_string(),
            None => value.to_string(),
        };

        let result = sqlx::query(
            "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(&val_str)
        .execute(&state.db)
        .await;

        if result.is_ok() {
            updated += 1;
        }
    }

    // Broadcast updated settings to all connected agents (with per-pod blanking override)
    if updated > 0 {
        let settings_map: std::collections::HashMap<String, String> = obj
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(&v.to_string()).to_string()))
            .collect();
        state.broadcast_settings(&settings_map).await;
    }

    Json(json!({ "ok": true, "updated": updated })).into_response()
}

// ─── POS Lockdown ─────────────────────────────────────────────────────────

/// GET /pos/lockdown — returns current lockdown state for POS polling script.
///
/// MMA Round 1 fixes (2/3 consensus):
/// - P2: Moved to public_routes — POS agent polls without JWT
///
/// MMA Round 2 fixes (2/3 consensus):
/// - P1: On DB error, serve last-known-good cached state (fail-safe)
///   Uses a static AtomicBool cache updated on each successful DB read.
///   Only defaults to unlocked on first-ever read before any DB contact.
/// - P3: Don't leak "db_error" field to unauthenticated clients
pub(crate) async fn get_pos_lockdown(State(state): State<Arc<AppState>>) -> Json<Value> {
    use std::sync::atomic::{AtomicBool, Ordering};
    // Cache last-known-good lockdown state (MMA Round 2 P1: fail-safe, not fail-open)
    static CACHED_LOCKED: AtomicBool = AtomicBool::new(false);
    static CACHE_INITIALIZED: AtomicBool = AtomicBool::new(false);

    match sqlx::query_scalar::<_, String>(
        "SELECT value FROM kiosk_settings WHERE key = 'pos_lockdown'",
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(val)) => {
            let locked = val == "true";
            CACHED_LOCKED.store(locked, Ordering::Relaxed);
            CACHE_INITIALIZED.store(true, Ordering::Relaxed);
            Json(json!({ "locked": locked }))
        }
        Ok(None) => {
            CACHED_LOCKED.store(false, Ordering::Relaxed);
            CACHE_INITIALIZED.store(true, Ordering::Relaxed);
            Json(json!({ "locked": false }))
        }
        Err(e) => {
            tracing::warn!("POS lockdown DB query failed, serving cached state: {e}");
            let cached = if CACHE_INITIALIZED.load(Ordering::Relaxed) {
                CACHED_LOCKED.load(Ordering::Relaxed)
            } else {
                false // No cache yet (fresh startup) — default unlocked
            };
            Json(json!({ "locked": cached }))
        }
    }
}

/// POST /pos/lockdown — toggle lockdown state from admin dashboard
pub(crate) async fn set_pos_lockdown(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked").and_then(|v| v.as_bool()).unwrap_or(true);
    let val = if locked { "true" } else { "false" };

    let result = sqlx::query(
        "INSERT INTO kiosk_settings (key, value) VALUES ('pos_lockdown', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(val)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "locked": locked })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
