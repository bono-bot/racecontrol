use std::sync::Arc;

use crate::billing;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use super::{
    launch_or_assist, todays_debug_pin, validate_employee_pin, CUSTOMER_PIN_LOCKOUT_THRESHOLD,
    INVALID_PIN_MESSAGE, PinSource,
};

// ─── Validate PIN ──────────────────────────────────────────────────────────

pub async fn validate_pin(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
) -> Result<String, String> {
    // Check employee debug PIN first (4-digit daily rotating PIN)
    let daily_pin = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin == daily_pin {
        return validate_employee_pin(state, pod_id, pin).await;
    }

    // PIN-01: check customer lockout before attempting DB lookup
    {
        let failures = state.customer_pin_failures.read().await;
        let count = failures.get(pod_id.as_str()).copied().unwrap_or(0);
        if count >= CUSTOMER_PIN_LOCKOUT_THRESHOLD {
            return Err(
                "Too many failed attempts. Please see reception to reset your session."
                    .to_string(),
            );
        }
    }

    // SESS-03: Begin transaction for atomic token consumption + billing deferral + finalization.
    // If any step fails, the entire token state change rolls back automatically.
    let mut tx = state.db.begin().await
        .map_err(|e| format!("Transaction start failed: {}", e))?;

    // Atomically find and consume pending token within transaction (prevents double-spend race condition)
    let row = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&pod_id)
    .bind(&pin)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // PIN-01: if token lookup failed, increment customer failure counter before returning Err
    let row = match row {
        Some(r) => r,
        None => {
            // Rollback the (empty) transaction before returning
            tx.rollback().await.ok();
            // PIN-01: increment customer failure counter for this pod
            {
                let mut failures = state.customer_pin_failures.write().await;
                *failures.entry(pod_id.clone()).or_insert(0) += 1;
            }
            return Err(INVALID_PIN_MESSAGE.to_string());
        }
    };

    let token_id = row.0;
    let driver_id = row.1;
    let pricing_tier_id = row.2;
    let custom_price_paise = row.3.map(|p| p as u32);
    let custom_duration_minutes = row.4.map(|m| m as u32);
    let experience_id = row.5;
    let custom_launch_args = row.6;

    // Check if this token belongs to a multiplayer group session
    let group_info = crate::multiplayer::find_group_session_for_token(state, &token_id).await;

    let (group_session_id, is_group_member) = if let Some((gsid, _gdriver)) = &group_info {
        // Call on_member_validated to track this PIN validation
        // billing_session_id is a deferred placeholder at this point
        let billing_session_id_placeholder = format!("deferred-{}", uuid::Uuid::new_v4());
        match crate::multiplayer::on_member_validated(state, gsid, &driver_id, &billing_session_id_placeholder).await {
            Ok(all_validated) => {
                tracing::info!(
                    "Group member {} validated on pod {} (all_validated={})",
                    driver_id, pod_id, all_validated
                );
            }
            Err(e) => {
                tracing::error!("Failed to call on_member_validated for group {}: {}", gsid, e);
            }
        }
        (Some(gsid.clone()), true)
    } else {
        (None, false)
    };

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    // Billing session will be created by billing::handle_game_status_update() when Live received
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());

    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer PIN auth
        None, // split_count
        None, // split_duration_minutes
        group_session_id,
    )
    .await
    {
        // SESS-03: Transaction rollback atomically reverts token from 'consuming' back to 'pending'
        tx.rollback().await.ok();
        tracing::error!("Defer billing failed for token {}, transaction rolled back: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed within the same transaction
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&mut *tx)
    .await
    {
        tx.rollback().await.ok();
        tracing::error!("Failed to mark token {} as consumed, rolling back: {}", token_id, e);
        return Err(format!("Token finalization failed: {}", e));
    }

    // SESS-03: Commit the transaction — token consumption is now atomic
    tx.commit().await
        .map_err(|e| format!("Transaction commit failed: {}", e))?;

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent
    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }

    // Reservation linking deferred until actual billing session starts on Live
    // link_reservation_to_billing will be called inside start_billing_session()

    // GROUP-01: For group members, do NOT auto-launch individually.
    // on_member_validated() handles coordinated launch via start_ac_lan_for_group()
    // when all members are validated. For non-group, launch as before.
    if !is_group_member {
        launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;
    }

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

    // PIN-01: reset customer failure counter on successful auth
    state.customer_pin_failures.write().await.remove(&pod_id);

    tracing::info!("PIN validated via {:?} on pod {}, billing deferred (waiting for LIVE)", PinSource::Pod, pod_id);

    Ok(billing_session_id)
}

// ─── Validate QR ───────────────────────────────────────────────────────────

pub async fn validate_qr(
    state: &Arc<AppState>,
    qr_token: String,
    driver_id: String,
) -> Result<String, String> {
    // Atomically find and consume pending QR token (prevents double-spend)
    let row = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE token = ? AND auth_type = 'qr' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&qr_token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid or expired QR token".to_string())?;

    let token_id = row.0;
    let pod_id = row.1;
    let token_driver_id = row.2;
    let pricing_tier_id = row.3;
    let custom_price_paise = row.4.map(|p| p as u32);
    let custom_duration_minutes = row.5.map(|m| m as u32);
    let experience_id = row.6;
    let custom_launch_args = row.7;

    // Verify driver matches the assignment
    if token_driver_id != driver_id {
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ?")
            .bind(&token_id).execute(&state.db).await;
        return Err("QR token is not assigned to this customer".to_string());
    }

    // Check if this pod is part of a multiplayer group session
    let qr_group_session_id: Option<String> = sqlx::query_scalar(
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
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer QR auth
        None, // split_count
        None, // split_duration_minutes
        qr_group_session_id,
    )
    .await
    {
        // Rollback: revert token from 'consuming' back to 'pending'
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ? AND status = 'consuming'")
            .bind(&token_id)
            .execute(&state.db)
            .await;
        tracing::error!("Defer billing failed for QR token {}, rolled back to pending: {}", token_id, e);
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

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent
    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }

    // Reservation linking deferred until actual billing session starts on Live

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

    tracing::info!("QR validated on pod {}, billing deferred (waiting for LIVE)", pod_id);

    Ok(billing_session_id)
}

// ─── Start Now (Staff Override) ───────────────────────────────────────────

/// Atomically consume a pending auth token and start billing without requiring PIN/QR.
/// Used by the kiosk "Start Now" button as a staff override.
pub async fn start_now(
    state: &Arc<AppState>,
    token_id: String,
) -> Result<String, String> {
    // Atomically find and consume the pending token (prevents double-spend)
    let row = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = ? AND status = 'pending'
         RETURNING pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&token_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Token not found or not pending".to_string())?;

    let pod_id = row.0;
    let driver_id = row.1;
    let pricing_tier_id = row.2;
    let custom_price_paise = row.3.map(|p| p as u32);
    let custom_duration_minutes = row.4.map(|m| m as u32);
    let experience_id = row.5;
    let custom_launch_args = row.6;

    // Check if this pod is part of a multiplayer group session
    let pwa_group_session_id: Option<String> = sqlx::query_scalar(
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
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // PWA token auth
        None, // split_count
        None, // split_duration_minutes
        pwa_group_session_id,
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

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent
    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }

    // Reservation linking deferred until actual billing session starts on Live

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

    tracing::info!("Start Now on pod {}: token {} consumed, billing deferred (waiting for LIVE)", pod_id, token_id);

    Ok(billing_session_id)
}
