use std::sync::Arc;

use crate::auth;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{GroupMemberInfo, GroupSessionInfo};

use super::ac_launch::start_ac_lan_for_group;
use super::helpers::{
    build_group_session_info, check_all_responded, get_customer_id, get_driver_name,
    get_pod_number,
};

/// Accept a group session invite. Debits invitee wallet, creates reservation + auth token.
pub async fn accept_group_invite(
    state: &Arc<AppState>,
    group_session_id: &str,
    driver_id: &str,
) -> Result<GroupMemberInfo, String> {
    // Verify the member record
    let member = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, pod_id FROM group_session_members
         WHERE group_session_id = ? AND driver_id = ? AND status = 'pending'",
    )
    .bind(group_session_id)
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Invite not found or already responded")?;

    let (member_id, pod_id) = member;
    let pod_id = pod_id.ok_or("No pod assigned for this invite")?;

    // Get group session details
    let session = sqlx::query_as::<_, (String, String, String)>(
        "SELECT pricing_tier_id, shared_pin, experience_id FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let (pricing_tier_id, shared_pin, experience_id) = session;

    // Get pricing
    let tier = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, price_paise, duration_minutes FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Pricing tier not found")?;

    let (tier_name, price_paise, duration_minutes) = tier;

    // Debit invitee wallet
    wallet::ensure_wallet(state, driver_id).await?;
    let (_, wallet_txn_id) = wallet::debit(
        state,
        driver_id,
        price_paise,
        "debit_session",
        Some(group_session_id),
        Some("Multiplayer session invite accepted"),
    )
    .await?;

    // Reserve pod — refund wallet if reservation fails (GAME-02 MMA fix)
    let reservation_id = match pod_reservation::create_reservation(state, driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("accept_group_invite: reservation failed, refunding wallet: {}", e);
            // MMA iter2: handle refund failure explicitly (not let _ =)
            match wallet::refund(state, driver_id, price_paise, Some(group_session_id),
                Some("Refund: multiplayer reservation failed")).await {
                Ok(_) => tracing::info!("accept_group_invite: refund issued for failed reservation"),
                Err(re) => tracing::error!("CRITICAL: accept_group_invite: refund FAILED for driver {} ({}p): {}", driver_id, price_paise, re),
            }
            return Err(e);
        }
    };

    // Create auth token with shared PIN
    let token = auth::create_auth_token(
        state,
        pod_id.clone(),
        driver_id.to_string(),
        pricing_tier_id.clone(),
        "pin".to_string(),
        None,
        Some(duration_minutes as u32),
        Some(experience_id),
        None,
    )
    .await?;

    // Override PIN with shared PIN
    sqlx::query("UPDATE auth_tokens SET token = ? WHERE id = ?")
        .bind(&shared_pin)
        .bind(&token.id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    // Re-send lock screen with shared PIN
    let driver_name = get_driver_name(state, driver_id).await;
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender
            .send(CoreMessage::wrap(CoreToAgentMessage::ShowPinLockScreen {
                token_id: token.id.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: tier_name,
                allocated_seconds: duration_minutes as u32 * 60,
            }))
            .await;
    }
    drop(agent_senders);

    // Update member record
    sqlx::query(
        "UPDATE group_session_members
         SET status = 'accepted', reservation_id = ?, auth_token_id = ?, wallet_txn_id = ?, accepted_at = datetime('now')
         WHERE id = ?",
    )
    .bind(&reservation_id)
    .bind(&token.id)
    .bind(&wallet_txn_id)
    .bind(&member_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Check if all invitees have responded → update group status to 'ready'
    check_all_responded(state, group_session_id).await;

    // Broadcast member update
    let _ = state.dashboard_tx.send(DashboardEvent::GroupMemberUpdate {
        group_session_id: group_session_id.to_string(),
        driver_id: driver_id.to_string(),
        status: "accepted".to_string(),
        pod_id: Some(pod_id.clone()),
    });

    let customer_id = get_customer_id(state, driver_id).await;
    let pod_number = get_pod_number(state, &pod_id).await;

    tracing::info!(
        "Group invite accepted: {} joined group session {}",
        driver_id,
        group_session_id
    );

    Ok(GroupMemberInfo {
        driver_id: driver_id.to_string(),
        driver_name,
        customer_id,
        role: "invitee".to_string(),
        status: "accepted".to_string(),
        pod_id: Some(pod_id),
        pod_number,
    })
}

/// Decline a group session invite. Releases pre-assigned pod.
pub async fn decline_group_invite(
    state: &Arc<AppState>,
    group_session_id: &str,
    driver_id: &str,
) -> Result<(), String> {
    let result = sqlx::query(
        "UPDATE group_session_members
         SET status = 'declined'
         WHERE group_session_id = ? AND driver_id = ? AND status = 'pending'",
    )
    .bind(group_session_id)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("Invite not found or already responded".to_string());
    }

    // Update total_members count
    sqlx::query("UPDATE group_sessions SET total_members = total_members - 1 WHERE id = ?")
        .bind(group_session_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    // Check if all remaining invitees have responded
    check_all_responded(state, group_session_id).await;

    // If everyone declined, cancel the session
    let remaining: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM group_session_members
         WHERE group_session_id = ? AND status IN ('accepted', 'pending')",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if remaining.map(|r| r.0).unwrap_or(0) <= 1 {
        // Only host left (or nobody) — check if host is the only accepted
        let accepted: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM group_session_members
             WHERE group_session_id = ? AND status = 'accepted' AND role = 'invitee'",
        )
        .bind(group_session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if accepted.map(|r| r.0).unwrap_or(0) == 0 {
            // No friends accepted — but session can still work as solo, don't auto-cancel
            tracing::info!(
                "All invitees declined group session {} — host can still play solo",
                group_session_id
            );
        }
    }

    // Broadcast
    let _ = state.dashboard_tx.send(DashboardEvent::GroupMemberUpdate {
        group_session_id: group_session_id.to_string(),
        driver_id: driver_id.to_string(),
        status: "declined".to_string(),
        pod_id: None,
    });

    tracing::info!(
        "Group invite declined: {} for group session {}",
        driver_id,
        group_session_id
    );

    Ok(())
}

/// Called after a group member validates their PIN and billing starts.
/// Returns true if all members are now validated (AC LAN should start).
pub async fn on_member_validated(
    state: &Arc<AppState>,
    group_session_id: &str,
    driver_id: &str,
    billing_session_id: &str,
) -> Result<bool, String> {
    // Update member record
    sqlx::query(
        "UPDATE group_session_members
         SET status = 'validated', billing_session_id = ?, validated_at = datetime('now')
         WHERE group_session_id = ? AND driver_id = ? AND status = 'accepted'",
    )
    .bind(billing_session_id)
    .bind(group_session_id)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Increment validated_count
    sqlx::query(
        "UPDATE group_sessions SET validated_count = validated_count + 1, status = 'active',
         started_at = COALESCE(started_at, datetime('now'))
         WHERE id = ?",
    )
    .bind(group_session_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Check if all accepted members are validated
    let counts = sqlx::query_as::<_, (i64, i64)>(
        "SELECT
            (SELECT COUNT(*) FROM group_session_members WHERE group_session_id = ? AND status IN ('accepted', 'validated')),
            (SELECT COUNT(*) FROM group_session_members WHERE group_session_id = ? AND status = 'validated')",
    )
    .bind(group_session_id)
    .bind(group_session_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (accepted_total, validated_total) = counts;
    let all_validated = validated_total >= accepted_total && accepted_total > 0;

    if all_validated {
        sqlx::query("UPDATE group_sessions SET status = 'all_validated' WHERE id = ?")
            .bind(group_session_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        // Auto-start AC LAN session — propagate errors so callers know launch failed
        if let Err(e) = start_ac_lan_for_group(state, group_session_id).await {
            tracing::error!("AC LAN launch failed for group {}: {}", group_session_id, e);
            // Mark session as failed so dashboard/API reflects the error
            let _ = sqlx::query(
                "UPDATE group_sessions SET status = 'ac_launch_failed' WHERE id = ?",
            )
            .bind(group_session_id)
            .execute(&state.db)
            .await;
        }
    } else {
        // Show "Waiting for friends..." on the validated member's pod
        let pod_id: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND driver_id = ?",
        )
        .bind(group_session_id)
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((Some(pod_id),)) = pod_id {
            let driver_name = get_driver_name(state, driver_id).await;
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::ShowAssistanceScreen {
                        driver_name,
                        message: format!(
                            "Waiting for friends... ({}/{} checked in)",
                            validated_total, accepted_total
                        ),
                    }))
                    .await;
            }
        }
    }

    // Broadcast
    let _ = state.dashboard_tx.send(DashboardEvent::GroupMemberUpdate {
        group_session_id: group_session_id.to_string(),
        driver_id: driver_id.to_string(),
        status: "validated".to_string(),
        pod_id: None,
    });

    Ok(all_validated)
}

/// Get the active group session for a driver (as host or accepted invitee).
pub async fn get_active_group_session(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<Option<GroupSessionInfo>, String> {
    // Find group session where driver is a member with an active status
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT gsm.group_session_id FROM group_session_members gsm
         INNER JOIN group_sessions gs ON gs.id = gsm.group_session_id
         WHERE gsm.driver_id = ? AND gsm.status IN ('pending', 'accepted', 'validated')
           AND gs.status IN ('forming', 'ready', 'active', 'all_validated')
         ORDER BY gs.created_at DESC
         LIMIT 1",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    match row {
        Some((group_session_id,)) => {
            let info = build_group_session_info(state, &group_session_id).await?;
            Ok(Some(info))
        }
        None => Ok(None),
    }
}

/// Check if an auth token belongs to a group session member.
/// Returns (group_session_id, driver_id) if found.
pub async fn find_group_session_for_token(
    state: &Arc<AppState>,
    auth_token_id: &str,
) -> Option<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT group_session_id, driver_id FROM group_session_members
         WHERE auth_token_id = ? AND status = 'accepted'",
    )
    .bind(auth_token_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
}
