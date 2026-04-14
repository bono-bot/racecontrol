//! PIN redemption at the kiosk — validates PIN, claims pod, starts billing.

use std::sync::Arc;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth;
use crate::billing;
use crate::pod_reservation;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use super::PIN_LENGTH;

/// Structured error for PIN redemption — allows callers to distinguish
/// customer PIN errors (should count toward lockout) from infrastructure
/// errors (should NOT punish the customer).
pub struct RedeemPinError {
    pub message: String,
    /// `true` when the customer typed a wrong/expired/invalid PIN.
    /// `false` for capacity issues, DB errors, billing failures, etc.
    pub is_pin_error: bool,
    /// Non-None when the PIN exists but payment is still processing.
    pub is_pending_debit: bool,
}

impl RedeemPinError {
    pub(crate) fn pin(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), is_pin_error: true, is_pending_debit: false }
    }
    pub(crate) fn infra(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), is_pin_error: false, is_pending_debit: false }
    }
    pub(crate) fn pending() -> Self {
        Self {
            message: "Your booking is being processed. Please try again in a minute.".to_string(),
            is_pin_error: false,
            is_pending_debit: true,
        }
    }
}

/// Redeem a confirmed reservation PIN at the kiosk.
///
/// Flow: validate PIN -> find idle pod (with retry) -> atomic mark redeemed ->
/// assign pod -> defer billing -> launch game -> return session info.
///
/// pending_debit PINs get a distinct "being processed" message.
/// If no pods are idle, the PIN is NOT consumed.
/// Atomic UPDATE prevents double-redeem race conditions.
/// Pod assignment uses optimistic concurrency with retry to prevent TOCTOU races.
pub async fn redeem_pin(state: &Arc<AppState>, pin: &str) -> Result<Value, RedeemPinError> {
    tracing::info!("PIN redemption started for pin={}***", &pin.get(..3).unwrap_or("?"));

    // 1. Normalize PIN (numeric-only, 4 digits)
    let pin = pin.trim().to_string();
    if pin.len() != PIN_LENGTH {
        return Err(RedeemPinError::pin(format!("PIN must be exactly {} digits", PIN_LENGTH)));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(RedeemPinError::pin("PIN must contain only digits (0-9)".to_string()));
    }

    // 2. Check for pending_debit status first (distinct message)
    let pending = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM reservations WHERE pin = ? AND status = 'pending_debit' AND expires_at > datetime('now') LIMIT 1",
    )
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?;

    if pending.is_some() {
        return Err(RedeemPinError::pending());
    }

    // 3. Find idle pod with retry (fixes TOCTOU race — if pod is claimed between
    //    find_idle_pod and create_reservation, retry with a different pod)
    const MAX_POD_RETRIES: usize = 3;
    let mut pod_id = pod_reservation::find_idle_pod(state)
        .await
        .ok_or_else(|| RedeemPinError::infra(
            "All pods are currently in use. Please wait a moment and try again.".to_string(),
        ))?;

    // 4. Get pod_number from in-memory state (error instead of returning Pod 0)
    let mut pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id)
            .map(|p| p.number)
            .ok_or_else(|| RedeemPinError::infra(format!("Pod {} not found in memory", pod_id)))?
    };

    // 5. Atomic UPDATE to prevent double-redeem
    let redeemed = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now'),
           pod_number = ?, updated_at = datetime('now')
         WHERE id = (
           SELECT id FROM reservations WHERE pin = ? AND status = 'confirmed'
             AND expires_at > datetime('now') LIMIT 1
         ) AND status = 'confirmed'
         RETURNING id, driver_id, experience_id",
    )
    .bind(pod_number as i64)
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?;

    let (reservation_id, driver_id, experience_id) = match redeemed {
        Some(row) => (row.0, row.1, row.2),
        None => return Err(RedeemPinError::pin("Invalid PIN or reservation not found")),
    };

    // 6. Get pricing_tier_id from kiosk_experiences (error if missing, not silent "default")
    let pricing_tier_id: String = sqlx::query_scalar(
        "SELECT pricing_tier_id FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?
    .ok_or_else(|| {
        tracing::warn!("Experience {} has no pricing_tier_id — using default", experience_id);
        // Fallback to default but log it as a warning (B3 fix: visibility)
        RedeemPinError::infra("Experience configuration error — please see reception".to_string())
    })
    .or_else(|e| {
        // If we want to gracefully degrade rather than fail, fall back
        // For now, let it error out so staff notices the config issue
        Err(e)
    })?;

    // 7. Create pod_reservation with retry for TOCTOU race (B6 fix)
    let mut pod_claim_ok = false;
    for attempt in 0..MAX_POD_RETRIES {
        match pod_reservation::create_reservation(state, &driver_id, &pod_id).await {
            Ok(_) => {
                pod_claim_ok = true;
                break;
            }
            Err(e) => {
                tracing::warn!(
                    "Pod {} claim failed (attempt {}/{}): {} — retrying with different pod",
                    pod_id, attempt + 1, MAX_POD_RETRIES, e
                );
                // Find a different idle pod
                match pod_reservation::find_idle_pod(state).await {
                    Some(new_pod_id) => {
                        let pods = state.pods.read().await;
                        match pods.get(&new_pod_id).map(|p| p.number) {
                            Some(num) => {
                                pod_id = new_pod_id;
                                pod_number = num;
                                // Update reservation with new pod_number
                                let _ = sqlx::query(
                                    "UPDATE reservations SET pod_number = ?, updated_at = datetime('now') WHERE id = ?",
                                )
                                .bind(pod_number as i64)
                                .bind(&reservation_id)
                                .execute(&state.db)
                                .await;
                            }
                            None => continue,
                        }
                    }
                    None => break, // No more idle pods
                }
            }
        }
    }

    if !pod_claim_ok {
        // Rollback reservation status with error logging (B5 fix)
        if let Err(rollback_err) = sqlx::query(
            "UPDATE reservations SET status = 'confirmed', redeemed_at = NULL, pod_number = NULL, updated_at = datetime('now') WHERE id = ? AND status = 'redeemed'",
        )
        .bind(&reservation_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("CRITICAL: Failed to rollback reservation {}: {}", reservation_id, rollback_err);
        }
        return Err(RedeemPinError::infra("Failed to reserve pod — all pods busy or claimed"));
    }

    // 8. Create billing_session_id
    let billing_session_id = format!("deferred-{}", Uuid::new_v4());

    // 9. Defer billing start
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id.clone(),
        None, // custom_price_paise
        None, // custom_duration_minutes
        None, // staff_id
        None, // split_count
        None, // split_duration_minutes
        None, // group_session_id
    )
    .await
    {
        // Rollback reservation status on billing failure (B5 fix: log rollback errors)
        if let Err(rollback_err) = sqlx::query(
            "UPDATE reservations SET status = 'confirmed', redeemed_at = NULL, pod_number = NULL, updated_at = datetime('now') WHERE id = ? AND status = 'redeemed'",
        )
        .bind(&reservation_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("CRITICAL: Failed to rollback reservation {} after billing error: {}", reservation_id, rollback_err);
        }
        return Err(RedeemPinError::infra(format!("Failed to start billing: {}", e)));
    }

    // 10. Get driver_name
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // 11. Get pricing tier name + duration
    let tier_row = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT name, duration_minutes FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let tier_name = tier_row
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "Session".to_string());

    let duration_minutes = tier_row
        .as_ref()
        .and_then(|r| r.1)
        .unwrap_or(30);

    // 12. Clear lock screen on pod agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // 13. Launch game or show assistance screen
    auth::launch_or_assist(
        state,
        &pod_id,
        &billing_session_id,
        &Some(experience_id.clone()),
        &None,
        &driver_name,
    )
    .await;

    // 14. Update pod state (current_driver) and broadcast PodUpdate
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // 15. Get experience_name
    let experience_name: String = sqlx::query_scalar(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    tracing::info!(
        "Reservation PIN redeemed on pod {} (#{}) driver {}, billing deferred",
        pod_id, pod_number, driver_name
    );

    // 16. Return JSON
    Ok(json!({
        "pod_number": pod_number,
        "pod_id": pod_id,
        "driver_name": driver_name,
        "experience_name": experience_name,
        "tier_name": tier_name,
        "allocated_seconds": duration_minutes * 60,
        "billing_session_id": billing_session_id,
    }))
}
