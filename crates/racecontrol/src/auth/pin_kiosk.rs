//! Kiosk PIN validation + Employee debug PIN — staff and customer auth flows.
//!
//! Handles PIN-based authentication from kiosk terminals (no pod_id required),
//! employee daily rotating debug PIN, and employee detection.
//!
//! Extracted from auth/mod.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;

use crate::billing;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use super::{launch_or_assist, PinSource, INVALID_PIN_MESSAGE};

// ─── Kiosk PIN Validation (no pod_id required) ───────────────────────────────

#[derive(Debug, Serialize)]
pub struct KioskPinResult {
    pub billing_session_id: String,
    pub pod_id: String,
    pub pod_number: u32,
    pub driver_name: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
}

pub async fn validate_pin_kiosk(
    state: &Arc<AppState>,
    pin: String,
    chosen_pod_id: Option<String>,
) -> Result<KioskPinResult, String> {
    // Atomically find and consume ANY pending pin token matching this PIN (prevents double-spend)
    // If a pod_id is provided (customer chose a pod), prefer tokens for that pod first,
    // then fall back to any matching token.
    let row = if let Some(ref cpid) = chosen_pod_id {
        // Try matching the chosen pod first
        let r = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND pod_id = ? AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
        )
        .bind(&pin)
        .bind(cpid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Fall back to any matching PIN token if none found for chosen pod
        match r {
            Some(row) => Some(row),
            None => {
                sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
                    "UPDATE auth_tokens SET status = 'consuming'
                     WHERE id = (
                         SELECT id FROM auth_tokens
                         WHERE token = ? AND auth_type = 'pin' AND status = 'pending'
                           AND expires_at > datetime('now')
                         LIMIT 1
                     ) AND status = 'pending'
                     RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
                )
                .bind(&pin)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| format!("DB error: {}", e))?
            }
        }
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
        )
        .bind(&pin)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
    };

    let row = row.ok_or_else(|| INVALID_PIN_MESSAGE.to_string())?;

    let token_id = row.0;
    let token_pod_id = row.1;
    let driver_id = row.2;
    let pricing_tier_id = row.3.clone();
    let custom_price_paise = row.4.map(|p| p as u32);
    let custom_duration_minutes = row.5.map(|m| m as u32);
    let experience_id = row.6;
    let custom_launch_args = row.7;

    // Use the customer's chosen pod if provided, otherwise the token's assigned pod
    let pod_id = chosen_pod_id.unwrap_or(token_pod_id);

    // Check if this pod is part of a multiplayer group session
    let kiosk_group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM group_session_members WHERE pod_id = ? AND status = 'validated' ORDER BY validated_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id.clone(),
        custom_price_paise,
        custom_duration_minutes,
        None, // kiosk PIN validation
        None, // split_count
        None, // split_duration_minutes
        kiosk_group_session_id,
    )
    .await
    {
        // Rollback: revert token from 'consuming' back to 'pending'
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ? AND status = 'consuming'")
            .bind(&token_id)
            .execute(&state.db)
            .await;
        tracing::error!("Defer billing failed for token {}, rolled back to pending: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed (billing_session_id is deferred placeholder)
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to mark token {} as consumed: {}", token_id, e);
    }

    // Get driver name
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Get pricing tier name and duration
    let tier_row = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT name, duration_minutes FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let pricing_tier_name = tier_row
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "Session".to_string());

    let allocated_seconds = custom_duration_minutes
        .map(|m| m * 60)
        .or_else(|| tier_row.as_ref().and_then(|r| r.1.map(|m| m as u32 * 60)))
        .unwrap_or(0);

    // Get pod number
    let pod_number: i64 = sqlx::query_scalar("SELECT number FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);

    // Clear lock screen on agent
    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Update pod state to WaitingForGame
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!(
        "PIN validated via {:?} on pod {} (#{}) driver {}, billing deferred (waiting for LIVE)",
        PinSource::Kiosk, pod_id, pod_number, driver_name
    );

    Ok(KioskPinResult {
        billing_session_id,
        pod_id,
        pod_number: pod_number as u32,
        driver_name,
        pricing_tier_name,
        allocated_seconds,
    })
}

// ─── Employee Debug PIN ──────────────────────────────────────────────────────

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
