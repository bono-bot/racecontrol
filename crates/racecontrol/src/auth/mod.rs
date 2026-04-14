pub mod admin;
mod game_helpers;
pub mod middleware;
pub mod otp;
pub mod rate_limit;
mod token_consume;
mod token_manage;
mod token_validation;

pub use admin::{admin_login, hash_admin_pin, verify_admin_pin};
pub use game_helpers::{check_pod_has_game, parse_sim_type};
pub(crate) use game_helpers::launch_or_assist;
pub use middleware::{StaffClaims, require_staff_jwt, create_staff_jwt};
pub use otp::{OtpSendResult, send_otp, resend_otp, verify_otp, send_guardian_otp, verify_guardian_otp};
pub use token_validation::{
    cancel_auth_token, expire_stale_tokens, get_pending_tokens, handle_dashboard_command,
    handle_pin_entered, start_now, validate_pin, validate_pin_kiosk, validate_qr, KioskPinResult,
};

use std::sync::Arc;

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::AuthTokenInfo;

// ─── PIN Validation Constants ─────────────────────────────────────────────

/// Standardized PIN error message — identical across pod lock screen, kiosk, and PWA paths.
/// AUTH-01 requires identical error message on all 3 entry points.
pub(crate) const INVALID_PIN_MESSAGE: &str =
    "Invalid PIN \u{2014} please try again or see reception.";

/// Maximum customer PIN failures before the pod's customer path is locked.
/// Staff path (employee debug PIN) has no such ceiling — see PIN-02.
const CUSTOMER_PIN_LOCKOUT_THRESHOLD: u32 = 5;

// ─── PinSource Enum ────────────────────────────────────────────────────────

/// Source of PIN entry — used for logging only. Validation behavior is identical across all sources.
#[derive(Debug, Clone, Copy)]
pub enum PinSource {
    Pod,   // Entered on physical pod lock screen
    Kiosk, // Staff kiosk endpoint
    Pwa,   // Customer PWA (goes through kiosk endpoint)
}

// ─── JWT Claims ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // driver_id
    pub exp: usize,
    pub iat: usize,
}

// ─── Create Auth Token ─────────────────────────────────────────────────────

pub async fn create_auth_token(
    state: &Arc<AppState>,
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    auth_type: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    experience_id: Option<String>,
    custom_launch_args: Option<String>,
) -> Result<AuthTokenInfo, String> {
    // Cancel any existing pending token for this pod
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM auth_tokens WHERE pod_id = ? AND status = 'pending'",
    )
    .bind(&pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (id,) in existing {
        let _ = cancel_auth_token(state, id).await;
    }

    // Guard: driver cannot be on another pod already
    let active_on_other = sqlx::query_as::<_, (String,)>(
        "SELECT pod_id FROM billing_sessions WHERE driver_id = ? AND status IN ('active', 'paused_manual') AND pod_id != ?",
    )
    .bind(&driver_id)
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if let Some((other_pod,)) = active_on_other {
        return Err(format!(
            "Driver already has an active session on {}",
            other_pod
        ));
    }

    // Verify driver exists and get name
    let driver = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Driver {} not found", driver_id))?;

    let driver_name = driver.1;

    // Verify pricing tier exists
    let tier = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, name, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Pricing tier {} not found", pricing_tier_id))?;

    let pricing_tier_name = tier.1;
    let duration_minutes = custom_duration_minutes.unwrap_or(tier.2 as u32);
    let allocated_seconds = duration_minutes * 60;

    // Generate token (with collision retry for PINs)
    let token = match auth_type.as_str() {
        "pin" => {
            let mut pin_str = String::new();
            for _ in 0..10 {
                let pin: u32 = rand::thread_rng().gen_range(1000..=9999);
                let candidate = format!("{:04}", pin);
                // Check no active (pending) auth_token already uses this PIN
                let collision = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM auth_tokens WHERE token = ? AND status = 'pending' AND expires_at > datetime('now')",
                )
                .bind(&candidate)
                .fetch_one(&state.db)
                .await
                .unwrap_or(0);
                if collision == 0 {
                    pin_str = candidate;
                    break;
                }
            }
            if pin_str.is_empty() {
                return Err("Could not generate a unique PIN after 10 attempts".to_string());
            }
            pin_str
        }
        "qr" => Uuid::new_v4().to_string(),
        _ => return Err("auth_type must be 'pin' or 'qr'".to_string()),
    };

    let token_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.config.auth.pin_expiry_secs as i64);

    // Insert into DB
    sqlx::query(
        "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args, created_at, expires_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&token_id)
    .bind(&pod_id)
    .bind(&driver_id)
    .bind(&pricing_tier_id)
    .bind(&auth_type)
    .bind(&token)
    .bind(custom_price_paise.map(|p| p as i64))
    .bind(custom_duration_minutes.map(|m| m as i64))
    .bind(&experience_id)
    .bind(&custom_launch_args)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB insert error: {}", e))?;

    let info = AuthTokenInfo {
        id: token_id.clone(),
        pod_id: pod_id.clone(),
        driver_id: driver_id.clone(),
        driver_name: driver_name.clone(),
        pricing_tier_id: pricing_tier_id.clone(),
        pricing_tier_name: pricing_tier_name.clone(),
        auth_type: auth_type.clone(),
        token: token.clone(),
        status: "pending".to_string(),
        allocated_seconds,
        custom_price_paise,
        custom_duration_minutes,
        created_at: now.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
    };

    // Send lock screen to agent (clone sender, drop lock before .await — prevents deadlock)
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let msg = match auth_type.as_str() {
            "pin" => CoreToAgentMessage::ShowPinLockScreen {
                token_id: token_id.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: pricing_tier_name.clone(),
                allocated_seconds,
            },
            _ => CoreToAgentMessage::ShowQrLockScreen {
                token_id: token_id.clone(),
                qr_payload: token.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: pricing_tier_name.clone(),
                allocated_seconds,
            },
        };
        let _ = sender.send(CoreMessage::wrap(msg)).await;
    }

    // Broadcast to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCreated(info.clone()));

    tracing::info!(
        "Auth token created: {} ({}) for {} on pod {} (expires in {}s)",
        token_id,
        auth_type,
        driver_name,
        pod_id,
        state.config.auth.pin_expiry_secs
    );

    Ok(info)
}

// ─── JWT Helpers ───────────────────────────────────────────────────────────

pub fn create_jwt(driver_id: &str, secret: &str) -> Result<String, String> {
    let now = Utc::now();
    let exp = now + Duration::days(30);

    let claims = Claims {
        sub: driver_id.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<String, String> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("JWT decode error: {}", e))?;

    Ok(data.claims.sub)
}

// ─── Employee Debug PIN ──────────────────────────────────────────────────

/// Generate a deterministic 4-digit daily PIN for employees.
/// PIN = hash(secret + "YYYY-MM-DD") mod 10_000, formatted as 4 digits.
/// Changes at midnight UTC each day.
pub fn generate_daily_pin(secret: &str, date: &str) -> String {
    let input = format!("{}-employee-debug-{}", secret, date);
    // Simple hash: sum bytes with position-weighted mixing
    let mut hash: u64 = 0;
    for (i, b) in input.bytes().enumerate() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u64).wrapping_add(i as u64);
    }
    // Mix further to avoid patterns
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x45d9f3b);
    hash ^= hash >> 16;
    // 4-digit PIN (1000-9999 range to avoid leading zeros confusion)
    let pin = (hash % 9000 + 1000) as u32;
    format!("{:04}", pin)
}

/// Get today's employee debug PIN
pub fn todays_debug_pin(secret: &str) -> String {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    generate_daily_pin(secret, &today)
}

/// Validate an employee debug PIN on a specific pod.
/// If valid: clears lock screen, enters debug mode, no billing.
/// PIN-02 invariant: this function NEVER reads or writes customer_pin_failures.
pub async fn validate_employee_pin(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
) -> Result<String, String> {
    let expected = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin != expected {
        // PIN-01: increment STAFF failure counter — never customer counter
        {
            let mut failures = state.staff_pin_failures.write().await;
            *failures.entry(pod_id.clone()).or_insert(0) += 1;
        }
        return Err("Invalid employee PIN".to_string());
    }

    // PIN-01: reset staff failure counter on successful auth
    state.staff_pin_failures.write().await.remove(&pod_id);

    // Clear lock screen and enter debug mode (clone sender, drop lock before .await)
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterDebugMode {
            employee_name: "Staff".to_string(),
        })).await;
    }

    tracing::info!("Employee debug PIN validated on pod {}", pod_id);

    Ok("debug_mode".to_string())
}

/// Validate employee debug PIN from kiosk (no pod_id — unlock a specific pod chosen by staff).
pub async fn validate_employee_pin_kiosk(
    state: &Arc<AppState>,
    pin: String,
    pod_id: Option<String>,
) -> Result<String, String> {
    let expected = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin != expected {
        return Err("Invalid employee PIN".to_string());
    }

    // If pod_id specified, enter debug mode on that pod
    if let Some(ref pid) = pod_id {
        // Clone sender, drop lock before .await — prevents deadlock
        let sender = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(pid).cloned()
        };
        if let Some(sender) = sender {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterDebugMode {
                employee_name: "Staff".to_string(),
            })).await;
        }
        tracing::info!("Employee debug mode on pod {} (kiosk)", pid);
    }

    Ok("debug_mode".to_string())
}

/// Check if a driver is an employee
pub async fn is_employee(state: &Arc<AppState>, driver_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT COALESCE(is_employee, 0) FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
