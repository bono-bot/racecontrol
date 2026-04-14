use std::sync::Arc;

use serde::Serialize;

use crate::billing;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::AuthTokenInfo;

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

// ─── Cancel Auth Token ─────────────────────────────────────────────────────

pub async fn cancel_auth_token(
    state: &Arc<AppState>,
    token_id: String,
) -> Result<(), String> {
    // Get pod_id before updating
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT pod_id FROM auth_tokens WHERE id = ? AND status = 'pending'",
    )
    .bind(&token_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Token not found or not pending".to_string())?;

    let pod_id = row.0;

    // Update status
    sqlx::query("UPDATE auth_tokens SET status = 'cancelled' WHERE id = ?")
        .bind(&token_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    // Clear lock screen on agent
    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }

    // Broadcast cleared event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCleared {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        reason: "cancelled".to_string(),
    });

    tracing::info!("Auth token {} cancelled for pod {}", token_id, pod_id);
    Ok(())
}

// ─── Expire Stale Tokens ───────────────────────────────────────────────────

pub async fn expire_stale_tokens(state: &Arc<AppState>) {
    let expired = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pod_id FROM auth_tokens WHERE status = 'pending' AND expires_at <= datetime('now')",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if expired.is_empty() {
        return;
    }

    for (token_id, pod_id) in &expired {
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'expired' WHERE id = ?")
            .bind(token_id)
            .execute(&state.db)
            .await;

        // Clear lock screen (clone sender, drop lock before .await)
        let sender = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(pod_id).cloned()
        };
        if let Some(sender) = sender {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
        }

        let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCleared {
            token_id: token_id.clone(),
            pod_id: pod_id.clone(),
            reason: "expired".to_string(),
        });
    }

    tracing::info!("Expired {} stale auth tokens", expired.len());
}

// ─── Get Pending Tokens ────────────────────────────────────────────────────

pub async fn get_pending_tokens(state: &Arc<AppState>) -> Vec<AuthTokenInfo> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, Option<i64>, Option<i64>, String, String)>(
        "SELECT at.id, at.pod_id, at.driver_id, d.name, at.pricing_tier_id, pt.name, at.auth_type, at.token, at.custom_price_paise, at.custom_duration_minutes, at.created_at, at.expires_at
         FROM auth_tokens at
         JOIN drivers d ON at.driver_id = d.id
         JOIN pricing_tiers pt ON at.pricing_tier_id = pt.id
         WHERE at.status = 'pending' AND at.expires_at > datetime('now')
         ORDER BY at.created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let duration_query = "SELECT duration_minutes FROM pricing_tiers WHERE id = ?";

    let mut tokens = Vec::new();
    for r in rows {
        let duration_minutes = r.9.unwrap_or_else(|| {
            // Can't do async here, use a default
            0
        });

        let tier_duration = sqlx::query_as::<_, (i64,)>(duration_query)
            .bind(&r.4)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|t| t.0 as u32)
            .unwrap_or(0);

        let actual_minutes = if duration_minutes > 0 {
            duration_minutes as u32
        } else {
            tier_duration
        };

        tokens.push(AuthTokenInfo {
            id: r.0,
            pod_id: r.1,
            driver_id: r.2,
            driver_name: r.3,
            pricing_tier_id: r.4,
            pricing_tier_name: r.5,
            auth_type: r.6,
            token: r.7,
            status: "pending".to_string(),
            allocated_seconds: actual_minutes * 60,
            custom_price_paise: r.8.map(|p| p as u32),
            custom_duration_minutes: r.9.map(|m| m as u32),
            created_at: r.10,
            expires_at: r.11,
        });
    }

    tokens
}

// ─── Handle Dashboard Commands ─────────────────────────────────────────────

pub async fn handle_dashboard_command(
    state: &Arc<AppState>,
    cmd: rc_common::protocol::DashboardCommand,
) {
    match cmd {
        rc_common::protocol::DashboardCommand::AssignCustomer {
            pod_id,
            driver_id,
            pricing_tier_id,
            auth_type,
            custom_price_paise,
            custom_duration_minutes,
        } => {
            if let Err(e) = super::create_auth_token(
                state,
                pod_id,
                driver_id,
                pricing_tier_id,
                auth_type,
                custom_price_paise,
                custom_duration_minutes,
                None, // experience_id — set via REST API
                None, // custom_launch_args — set via REST API
            )
            .await
            {
                tracing::error!("Failed to assign customer: {}", e);
            }
        }
        rc_common::protocol::DashboardCommand::CancelAssignment { token_id } => {
            if let Err(e) = cancel_auth_token(state, token_id).await {
                tracing::error!("Failed to cancel assignment: {}", e);
            }
        }
        rc_common::protocol::DashboardCommand::AcknowledgeAssistance { pod_id } => {
            tracing::info!("Staff acknowledged assistance for pod {}", pod_id);
            // Clear the assistance screen on the agent (clone sender, drop lock before .await)
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
            }
        }
        _ => {}
    }
}

// ─── Handle Agent PIN Entry ────────────────────────────────────────────────

pub async fn handle_pin_entered(state: &Arc<AppState>, pod_id: String, pin: String) {
    match validate_pin(state, pod_id.clone(), pin).await {
        Ok(billing_session_id) => {
            tracing::info!(
                "PIN auth success on pod {}: billing session {}",
                pod_id,
                billing_session_id
            );
        }
        Err(e) => {
            tracing::warn!("PIN auth failed on pod {}: {}", pod_id, e);
            // Send failure feedback to agent (clone sender, drop lock before .await)
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::PinFailed {
                        reason: e.clone(),
                    }))
                    .await;
            }
        }
    }
}

// ─── Kiosk PIN Validation (no pod_id required) ───────────────────────────

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
