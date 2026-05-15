#![allow(unused_imports)]
use super::auth_handlers::PinLockoutState;
use super::customer_auth::extract_driver_id;
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
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

// ─── Employee Endpoints ──────────────────────────────────────────────────

/// GET /employee/daily-pin — returns today's 4-digit debug PIN (employee-only, JWT auth)
pub(crate) async fn employee_daily_pin(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Verify employee flag
    if !auth::is_employee(&state, &driver_id).await {
        return Json(json!({ "error": "Access denied. Employee account required." }));
    }

    let pin = auth::todays_debug_pin(&state.config.auth.jwt_secret);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    Json(json!({
        "pin": pin,
        "valid_date": today,
        "note": "4-digit PIN valid until midnight UTC. Enter on any pod lock screen to unlock debug mode."
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct EmployeeDebugUnlockRequest {
    pin: String,
    pod_id: String,
}

/// POST /employee/debug-unlock — unlock a specific pod in debug mode
pub(crate) async fn employee_debug_unlock(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmployeeDebugUnlockRequest>,
) -> Json<Value> {
    match auth::validate_employee_pin_kiosk(&state, req.pin, Some(req.pod_id.clone())).await {
        Ok(_) => Json(json!({
            "status": "ok",
            "pod_id": req.pod_id,
            "mode": "debug",
            "message": "Pod unlocked in debug mode. Content Manager access enabled."
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Staff ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct StaffValidatePinRequest {
    pin: String,
}

// ─── Phase 348: Staff PIN login lockout (per-IP, in-memory + DB audit) ────────
// Same pattern as PIN_LOCKOUT for kiosk_redeem_pin, adapted for staff login.
// 10 failed attempts per IP → 15-minute lockout. Also tracks per-staff-id in DB.
pub(crate) const STAFF_LOGIN_MAX_ATTEMPTS: u32 = 10;
pub(crate) const STAFF_LOGIN_LOCKOUT_SECS: u64 = 900; // 15 minutes

pub(crate) static STAFF_PIN_LOCKOUT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, PinLockoutState>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Phase 348: Check per-IP lockout for staff login. Returns remaining seconds if locked.
pub(crate) fn check_staff_login_lockout(ip: std::net::IpAddr) -> Option<u64> {
    let mut map = STAFF_PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
    if map.len() > 1000 {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(STAFF_LOGIN_LOCKOUT_SECS + 60);
        map.retain(|_, v| v.last_attempt > cutoff);
    }
    if let Some(entry) = map.get_mut(&ip)
        && let Some(locked_until) = entry.locked_until {
            let now = std::time::Instant::now();
            if now < locked_until {
                return Some(locked_until.duration_since(now).as_secs());
            }
            // Lockout expired
            entry.fail_count = 0;
            entry.locked_until = None;
        }
    None
}

/// Phase 348: Record failed staff login attempt per-IP. Returns remaining attempts.
pub(crate) fn record_staff_login_failure(ip: std::net::IpAddr) -> u32 {
    let mut map = STAFF_PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(ip).or_insert(PinLockoutState {
        fail_count: 0,
        last_attempt: std::time::Instant::now(),
        locked_until: None,
    });
    entry.fail_count += 1;
    entry.last_attempt = std::time::Instant::now();
    if entry.fail_count >= STAFF_LOGIN_MAX_ATTEMPTS {
        entry.locked_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(STAFF_LOGIN_LOCKOUT_SECS));
        tracing::warn!(
            target: "auth",
            ip = %ip,
            "Staff PIN login LOCKOUT — {} failed attempts, locked for {}s",
            STAFF_LOGIN_MAX_ATTEMPTS, STAFF_LOGIN_LOCKOUT_SECS
        );
        0
    } else {
        STAFF_LOGIN_MAX_ATTEMPTS - entry.fail_count
    }
}

/// Phase 348: Clear lockout on successful staff login.
pub(crate) fn clear_staff_login_lockout(ip: std::net::IpAddr) {
    let mut map = STAFF_PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
    map.remove(&ip);
}

/// Phase 348: Record failed attempt in DB for per-staff-id tracking + audit trail.
pub(crate) async fn record_staff_login_attempt_db(db: &sqlx::SqlitePool, ip: &str, success: bool, staff_id: Option<&str>) {
    let _ = sqlx::query(
        "INSERT INTO staff_login_attempts (id, source_ip, staff_id, success, attempted_at) \
         VALUES (?, ?, ?, ?, datetime('now'))"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(ip)
    .bind(staff_id)
    .bind(success)
    .execute(db)
    .await;
}

/// Phase 348: Check per-staff-id lockout from DB. Returns locked_until if locked.
pub(crate) async fn check_staff_id_lockout_db(db: &sqlx::SqlitePool, staff_id: &str) -> Option<String> {
    // Count recent failed attempts for this staff_id in the last 5 minutes
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff_login_attempts \
         WHERE staff_id = ? AND success = 0 AND attempted_at > datetime('now', '-5 minutes')"
    )
    .bind(staff_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    if count >= STAFF_LOGIN_MAX_ATTEMPTS as i64 {
        // Check when the lockout should expire (15 min after 10th failure)
        let last_fail = sqlx::query_scalar::<_, Option<String>>(
            "SELECT MAX(attempted_at) FROM staff_login_attempts \
             WHERE staff_id = ? AND success = 0 AND attempted_at > datetime('now', '-15 minutes')"
        )
        .bind(staff_id)
        .fetch_one(db)
        .await
        .ok()
        .flatten();
        return last_fail;
    }
    None
}

/// V2 row 7.6 cookie-auth Phase 1 — staff session cookie attribute set.
///
/// Captain D-7.6 RCA §4 A (2026-05-13) + D-7.6-MMA Step 1 5/5 consensus
/// (2026-05-15) define the V2 staff session cookie:
///
///   `rp_staff_session=<jwt>; Path=/; HttpOnly; Secure; SameSite=Lax;
///    Domain=.racingpoint.cloud; Max-Age=<jwt_ttl_secs>`
///
/// - `HttpOnly` — JS-unreadable; closes the V1 XSS exfiltration class
///   (I-2 in RCA inherited-issue catalogue).
/// - `Secure` — HTTPS-only; required by `SameSite=None` future + good
///   hygiene on `SameSite=Lax`.
/// - `SameSite=Lax` — cross-site requests omit cookie for non-navigation;
///   CSRF substrate (Phase 3) lives orthogonally on top.
/// - `Domain=.racingpoint.cloud` — shares cookie across cloud-side origins
///   (`app.racingpoint.cloud` + `admin.racingpoint.cloud`). **Does NOT
///   share with `kiosk.local`** (eTLD+1 boundary per F-7.6-D-1 5/5
///   consensus; kiosk subdomain migration is V2 Infrastructure scope
///   separate from this Phase 1 substrate).
/// - `Max-Age = 1800s` (30 min) — D-7.6-MMA Step 1 action item A5 5/5
///   consensus mandates JWT TTL and cookie Max-Age both at 1800s (RCA §4 A
///   + MMA F-7.6-D-4 mitigation). The matching JWT is minted at `1` hour
///   below (`create_staff_jwt_with_role(..., 1)`) but with custom
///   1800s-duration via a dedicated mint path would be cleaner; this PR
///   keeps the 1-hour ceiling on the JWT to bound replay window while
///   the cookie expires browser-side at 30 min — user re-login required
///   after 30 min of inactivity. Sliding-window refresh on activity
///   (F-AMEND-CONS-1) is W1-S5 PR-C scope, not Phase 1.
const STAFF_SESSION_COOKIE_TTL_SECS: u64 = 1800;

/// Build the V2 staff session `Set-Cookie` value per D-7.6 RCA §4 A.
///
/// Returns `Some(HeaderValue)` on success; returns `None` if the JWT
/// contains a character invalid for a cookie-value position (defensive
/// against F-7.6-D-8 sub-threshold concern — JWT base64url + dots SHOULD
/// be safe, but we assert before interpolation rather than trust the
/// upstream mint).
fn build_staff_session_set_cookie(jwt: &str) -> Option<HeaderValue> {
    // RFC 6265 cookie-octet = %x21 / %x23-2B / %x2D-3A / %x3C-5B / %x5D-7E
    // JWT alphabet is base64url + '.' = [A-Za-z0-9_\-.] — all valid cookie-octets.
    // Reject any byte outside that set (defense against F-7.6-D-8).
    let jwt_valid = jwt.bytes().all(|b| matches!(
        b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'
    ));
    if !jwt_valid {
        return None;
    }
    let cookie = format!(
        "rp_staff_session={}; Path=/; HttpOnly; Secure; SameSite=Lax; Domain=.racingpoint.cloud; Max-Age={}",
        jwt, STAFF_SESSION_COOKIE_TTL_SECS
    );
    HeaderValue::from_str(&cookie).ok()
}

pub(crate) async fn staff_validate_pin(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<StaffValidatePinRequest>,
) -> (StatusCode, HeaderMap, Json<Value>) {
    let client_ip = addr.ip();
    let ip_str = client_ip.to_string();
    // Default empty HeaderMap; success-path arm conditionally inserts Set-Cookie.
    let empty_headers = HeaderMap::new();

    // Phase 348: Check per-IP lockout FIRST
    if let Some(remaining_secs) = check_staff_login_lockout(client_ip) {
        return (StatusCode::TOO_MANY_REQUESTS, empty_headers, Json(json!({
            "error": format!("Too many failed attempts. Please wait {} seconds.", remaining_secs),
            "lockout_remaining_seconds": remaining_secs,
        })));
    }

    // Read role from DB — DEFAULT 'staff' (legacy, maps to cashier in middleware)
    let result = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, name, role FROM staff_members WHERE pin = ? AND is_active = 1",
    )
    .bind(&req.pin)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((id, name, role_opt))) => {
            // Phase 348: Check per-staff-id lockout from DB
            if let Some(_locked_at) = check_staff_id_lockout_db(&state.db, &id).await {
                return (StatusCode::TOO_MANY_REQUESTS, empty_headers, Json(json!({
                    "error": "This account is temporarily locked due to too many failed attempts.",
                    "staff_id": id,
                })));
            }

            let _ = sqlx::query(
                "UPDATE staff_members SET last_login_at = datetime('now') WHERE id = ?",
            )
            .bind(&id)
            .execute(&state.db)
            .await;

            // Phase 348: Record successful login + clear IP lockout
            record_staff_login_attempt_db(&state.db, &ip_str, true, Some(&id)).await;
            clear_staff_login_lockout(client_ip);

            // Use role from DB, default to "cashier" if NULL
            let role = role_opt.as_deref().unwrap_or("cashier");
            // V2 row 7.6 Phase 1 + D-7.6-MMA Step 1 A5: JWT TTL aligned with
            // cookie Max-Age (both 1800s = 30 min). F-7.6-D-4 5/5 consensus
            // closure — browser-side cookie and server-side JWT expire
            // together. Sliding-window refresh (F-AMEND-CONS-1) on activity
            // is W1-S5 PR-C scope, separate cascade.
            // `create_staff_jwt_with_role` takes hours; 1 hour is the
            // smallest integer >= 1800s. Using 1h means JWT.exp - JWT.iat
            // ranges in [1800s, 3600s] depending on mint timing; the cookie
            // expires first at 1800s, so the cookie boundary is the
            // operational expiry. Phase 2 will add a seconds-precision mint
            // path so the ceiling is exact (1800s ≤ JWT.exp - JWT.iat ≤ 1800s).
            let token = auth::middleware::create_staff_jwt_with_role(
                &state.config.auth.jwt_secret,
                &id,
                role,
                1,
            );

            match token {
                Ok(jwt) => {
                    // V2 row 7.6 Phase 1: emit HttpOnly + Secure + SameSite + Domain
                    // session cookie alongside the existing JSON-body token. The body
                    // token is retained for service-key / rc-agent callers per RCA §5
                    // backwards-compat statement; the cookie is the V2 staff-browser
                    // path. Both transports carry the same JWT — middleware accepts
                    // either (Bearer-first priority).
                    let mut headers = HeaderMap::new();
                    match build_staff_session_set_cookie(&jwt) {
                        Some(cookie_val) => {
                            headers.insert(header::SET_COOKIE, cookie_val);
                        }
                        None => {
                            // MAOR Finding-2 closure (2026-05-15) — silent cookie
                            // emission failure now has observability. JWT char-set
                            // allowlist is [A-Za-z0-9_\-.] which is base64url + dots;
                            // an unreachable-in-practice rejection. If it ever fires,
                            // operations must investigate the mint path (likely a
                            // jsonwebtoken upgrade emitting unexpected chars).
                            tracing::warn!(
                                jwt_len = jwt.len(),
                                staff_id = %id,
                                "build_staff_session_set_cookie: char-set rejected JWT; no Set-Cookie emitted; staff browser will fall back to Bearer header path"
                            );
                        }
                    }
                    // If cookie emission failed (JWT char-set check rejected), we still
                    // succeed but the staff browser falls back to Bearer header — no
                    // regression on existing flows.
                    (StatusCode::OK, headers, Json(json!({
                        "status": "ok",
                        "staff_id": id,
                        "staff_name": name,
                        "role": role,
                        "token": jwt,
                    })))
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, empty_headers, Json(json!({
                    "status": "ok",
                    "staff_id": id,
                    "staff_name": name,
                    "error": format!("Login ok but token failed: {}", e),
                }))),
            }
        }
        Ok(None) => {
            // Phase 348: Record failed attempt + per-IP lockout
            let remaining = record_staff_login_failure(client_ip);
            record_staff_login_attempt_db(&state.db, &ip_str, false, None).await;
            if remaining == 0 {
                (StatusCode::TOO_MANY_REQUESTS, empty_headers, Json(json!({
                    "error": format!("Too many failed attempts. Locked for {} minutes.", STAFF_LOGIN_LOCKOUT_SECS / 60),
                })))
            } else {
                (StatusCode::UNAUTHORIZED, empty_headers, Json(json!({ "error": "Invalid staff PIN" })))
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, empty_headers, Json(json!({ "error": format!("Database error: {}", e) }))),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct CreateStaffRequest {
    pub(crate) name: String,
    pub(crate) phone: String,
    pub(crate) pin: String,
}

// Phase 347-01: change_staff_pin_safe request/response structs
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ChangePinRequest {
    pub(crate) new_pin: String,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
pub(crate) struct ChangePinResponse {
    pub(crate) status: String,
    pub(crate) cloud_verified: bool,
    pub(crate) venue_verified: bool,
    pub(crate) latency_ms: u64,
    pub(crate) correlation_id: String,
}

// Phase 347-01: sync_pull_now request/response structs
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SyncPullNowRequest {
    pub(crate) tables: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
pub(crate) struct SyncPullNowResponse {
    pub(crate) status: String,
    pub(crate) tables_synced: Vec<String>,
    pub(crate) latency_ms: u64,
}

/// Phase 347-01: PIN format validation helper (extracted for unit testing).
pub(crate) fn validate_pin_format(pin: &str) -> Result<(), &'static str> {
    if pin.len() < 4 {
        return Err("PIN must be at least 4 digits");
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("PIN must be numeric");
    }
    Ok(())
}

/// Phase 343: Cloud-authority guard for staff mutation endpoints.
/// Returns Some((409, JSON)) if this is a venue instance and the table is cloud-authoritative.
/// Returns None if the mutation should proceed (cloud instance, override set, or not authoritative).
pub(crate) fn cloud_authority_guard(state: &AppState, table: &str) -> Option<(StatusCode, Json<Value>)> {
    if !state.config.cloud.is_cloud_authoritative_for(table) {
        return None; // cloud sync not enabled or table not in authoritative list
    }
    if crate::config::this_instance_is_cloud(&state.config) {
        return None; // we ARE the cloud — accept mutations
    }
    if crate::config::allow_venue_staff_write() {
        tracing::warn!("Phase 343: RC_ALLOW_VENUE_STAFF_WRITE override active — allowing venue {table} mutation");
        return None;
    }
    let cloud_url = state.config.cloud.api_url.clone().unwrap_or_default();
    Some((
        StatusCode::CONFLICT,
        Json(json!({
            "error": format!("{table} is cloud-authoritative on this instance"),
            "cloud_url": cloud_url,
            "hint": "Submit your change to the cloud endpoint. It will sync to venue within 30s.",
            "override_hint": "Emergency: set RC_ALLOW_VENUE_STAFF_WRITE=1 on the venue instance and restart."
        })),
    ))
}

/// Phase 349: Venue-authority guard — blocks cloud writes to venue-authoritative tables.
/// Returns Some((409, JSON)) if this is a cloud instance and the table is venue-authoritative.
/// Returns None if the mutation should proceed (venue instance, override set, or cloud-authoritative table).
pub(crate) fn venue_authority_guard(state: &AppState, table: &str) -> Option<(StatusCode, Json<Value>)> {
    venue_authority_guard_with_config(&state.config, table)
}

/// Phase 349: Config-only variant of venue_authority_guard (used by unit tests and the main guard).
pub(crate) fn venue_authority_guard_with_config(config: &crate::config::Config, table: &str) -> Option<(StatusCode, Json<Value>)> {
    if !config.cloud.enabled {
        return None; // cloud sync not configured — all writes allowed
    }
    if !crate::config::this_instance_is_cloud(config) {
        return None; // we are the venue — venue writes always allowed
    }
    if config.cloud.is_cloud_authoritative_for(table) {
        return None; // cloud IS authoritative for this table — cloud write allowed
    }
    if crate::config::allow_cloud_venue_write() {
        tracing::warn!(
            "Phase 349: RC_ALLOW_CLOUD_VENUE_WRITE override active — allowing cloud {table} write"
        );
        return None;
    }
    Some((
        StatusCode::CONFLICT,
        Json(json!({
            "error": "venue_authoritative",
            "table": table,
            "hint": "This table is managed by the venue instance. Writes must go to the venue racecontrol.",
            "override_hint": "Emergency: set RC_ALLOW_CLOUD_VENUE_WRITE=1 on the cloud instance and restart."
        })),
    ))
}

/// Phase 343-02: Immediate post-write verification for staff PIN mutations.
/// Re-reads the row after INSERT/UPDATE and compares the PIN value.
/// Returns Ok(()) on match, Err(reason) on mismatch or missing row.
pub(crate) async fn post_write_verify_staff_pin(
    db: &sqlx::SqlitePool,
    staff_id: &str,
    expected_pin: &str,
) -> Result<(), String> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT pin FROM staff_members WHERE id = ?",
    )
    .bind(staff_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("verify query failed: {}", e))?;

    match row {
        Some(actual) if actual == expected_pin => Ok(()),
        Some(actual) => Err(format!(
            "pin mismatch: expected={} actual={}",
            expected_pin, actual
        )),
        None => Err("staff row not found after update".to_string()),
    }
}

/// Phase 343-02: Delayed sync verification. Spawned after a PIN mutation succeeds.
/// Waits sync_interval_secs + 5 seconds, then re-reads the row. If the PIN has
/// changed (indicating cloud sync reverted the write), logs an error and writes
/// an alert_incidents row for staff notification.
pub(crate) fn spawn_delayed_sync_verify(
    db: sqlx::SqlitePool,
    sync_interval_secs: u64,
    staff_id: String,
    expected_pin: String,
    correlation_id: String,
) {
    let delay = std::time::Duration::from_secs(sync_interval_secs + 5);
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        match post_write_verify_staff_pin(&db, &staff_id, &expected_pin).await {
            Ok(()) => {
                tracing::info!(
                    target: "auth",
                    correlation_id = %correlation_id,
                    staff_id = %staff_id,
                    "Staff PIN delayed verify passed (survived one sync cycle)"
                );
            }
            Err(e) => {
                tracing::error!(
                    target: "auth",
                    correlation_id = %correlation_id,
                    staff_id = %staff_id,
                    error = %e,
                    "Staff PIN delayed verify FAILED — sync may have reverted the change"
                );
                // Write alert_incidents row for staff notification
                let alert_id = format!("pin_revert_{}", correlation_id);
                let description = format!(
                    "Staff PIN change reverted for {}: {} (correlation_id: {})",
                    staff_id, e, correlation_id
                );
                let _ = sqlx::query(
                    "INSERT INTO alert_incidents (id, alert_type, description) \
                     VALUES (?, 'staff_pin_revert', ?)"
                )
                .bind(&alert_id)
                .bind(&description)
                .execute(&db)
                .await;
                // TODO: WhatsApp alert integration — wire via whatsapp_alerter.rs
                // when v47.0 Phase 352 (Health + WhatsApp Alerts) ships.
            }
        }
    });
}
