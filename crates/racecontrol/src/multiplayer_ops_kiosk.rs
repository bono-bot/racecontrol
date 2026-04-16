//! Kiosk self-service multiplayer booking and stale invite cleanup.
//!
//! Contains the kiosk booking flow (host pays for all pods, each gets unique PIN)
//! and periodic cleanup of stale forming sessions.
//!
//! Extracted from multiplayer_ops.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use crate::auth;
use crate::multiplayer::find_adjacent_idle_pods;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use super::{build_group_session_info, get_driver_name};

/// Result returned from kiosk self-service multiplayer booking.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KioskMultiplayerResult {
    pub group_session_id: String,
    pub experience_name: String,
    pub tier_name: String,
    pub allocated_seconds: u32,
    pub assignments: Vec<KioskMultiplayerAssignment>,
}

/// Per-pod assignment in a kiosk multiplayer booking.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KioskMultiplayerAssignment {
    pub pin: String,
    pub pod_id: String,
    pub pod_number: u32,
    pub role: String,
}

/// Kiosk self-service multiplayer booking.
/// Unlike book_multiplayer(), this doesn't require friends to be pre-registered.
/// Host pays for all pods. Each participant gets a unique PIN.
///
/// Returns a list of (pin, pod_number) pairs — one per participant.
pub async fn book_multiplayer_kiosk(
    state: &Arc<AppState>,
    host_id: &str,
    pricing_tier_id: &str,
    pod_count: usize,
    experience_id: Option<&str>,
    custom: Option<(String, String, String)>,
) -> Result<KioskMultiplayerResult, String> {
    if !(2..=8).contains(&pod_count) {
        return Err("Pod count must be between 2 and 8".to_string());
    }

    // Verify pricing tier
    let tier = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, price_paise, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Pricing tier not found")?;

    let (tier_name, price_per_pod_paise, duration_minutes) = tier;
    let total_price = price_per_pod_paise * pod_count as i64;

    // Resolve experience
    let (experience_id_resolved, experience_name) = if let Some(eid) = experience_id {
        let exp = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM kiosk_experiences WHERE id = ?",
        )
        .bind(eid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or("Experience not found")?;
        (eid.to_string(), exp.0)
    } else if let Some((ref game, ref track, ref car)) = custom {
        let adhoc_id = uuid::Uuid::new_v4().to_string();
        let adhoc_name = format!("Custom: {} @ {}", car, track);
        sqlx::query(
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'), ?)",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(game)
        .bind(track)
        .bind(car)
        .bind(duration_minutes)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
        (adhoc_id, adhoc_name)
    } else {
        return Err("Must provide experience_id or custom booking payload".to_string());
    };

    // Validate host wallet
    wallet::ensure_wallet(state, host_id).await?;
    let host_balance = wallet::get_balance(state, host_id).await?;
    if host_balance < total_price {
        return Err(format!(
            "Insufficient wallet balance: have {}p, need {}p ({}p x {} pods)",
            host_balance, total_price, price_per_pod_paise, pod_count
        ));
    }

    // Find adjacent idle pods
    let pod_ids = find_adjacent_idle_pods(state, pod_count).await?;

    // Debit host wallet for all pods
    wallet::ensure_wallet(state, host_id).await?;
    let (_, wallet_txn_id) = wallet::debit(
        state,
        host_id,
        total_price,
        "debit_session",
        None,
        Some(&format!("Kiosk multiplayer: {} x {} pods", experience_name, pod_count)),
    )
    .await?;

    // Ensure ac_session_id column exists
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN ac_session_id TEXT")
        .execute(&state.db)
        .await;

    // Create group session
    let group_session_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, 'active', ?, datetime('now'), ?)",
    )
    .bind(&group_session_id)
    .bind(host_id)
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind("0000") // Placeholder — each participant gets unique PIN
    .bind(pod_count as i64)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Create auth token + reservation for each pod (each gets unique PIN)
    let mut assignments: Vec<KioskMultiplayerAssignment> = Vec::new();
    let host_name = get_driver_name(state, host_id).await;

    for (i, pod_id) in pod_ids.iter().enumerate() {
        let role = if i == 0 { "host" } else { "invitee" };

        // Create reservation
        let reservation_id = pod_reservation::create_reservation(state, host_id, pod_id).await?;

        // Create auth token (generates unique PIN)
        let token = auth::create_auth_token(
            state,
            pod_id.clone(),
            host_id.to_string(),
            pricing_tier_id.to_string(),
            "pin".to_string(),
            None,
            Some(duration_minutes as u32),
            Some(experience_id_resolved.clone()),
            None,
        )
        .await?;

        // The token field IS the PIN for pin-type auth tokens
        let pin = token.token.clone();

        // Get pod number
        let pod_number = {
            let pods = state.pods.read().await;
            pods.get(pod_id).map(|p| p.number).unwrap_or(0)
        };

        // Create group member record
        let member_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, reservation_id, auth_token_id, wallet_txn_id, invited_at, accepted_at, venue_id)
             VALUES (?, ?, ?, ?, 'accepted', ?, ?, ?, ?, datetime('now'), datetime('now'), ?)",
        )
        .bind(&member_id)
        .bind(&group_session_id)
        .bind(host_id)
        .bind(role)
        .bind(pod_id)
        .bind(&reservation_id)
        .bind(&token.id)
        .bind(&wallet_txn_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Send lock screen to pod
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::ShowPinLockScreen {
                    token_id: token.id.clone(),
                    driver_name: host_name.clone(),
                    pricing_tier_name: tier_name.clone(),
                    allocated_seconds: duration_minutes as u32 * 60,
                }))
                .await;
        }

        assignments.push(KioskMultiplayerAssignment {
            pin,
            pod_id: pod_id.clone(),
            pod_number,
            role: role.to_string(),
        });
    }

    // GROUP-01: AC server start deferred to on_member_validated() -> start_ac_lan_for_group()
    // Server starts only when ALL members have validated their PINs (coordinated launch).
    // The ac_session_id will be set on group_sessions by start_ac_lan_for_group().

    // Broadcast to dashboard
    if let Ok(info) = build_group_session_info(state, &group_session_id).await {
        let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionCreated(info));
    }

    tracing::info!(
        "Kiosk multiplayer group {} created: {} pods, host {}",
        group_session_id, pod_count, host_id
    );

    Ok(KioskMultiplayerResult {
        group_session_id,
        experience_name,
        tier_name,
        allocated_seconds: duration_minutes as u32 * 60,
        assignments,
    })
}

/// Cleanup stale group session invites.
/// Cancels group_sessions with status 'forming' that are older than 5 minutes.
/// Pending members get status 'timeout', pods are released.
/// If no accepted members remain, the session is cancelled.
pub async fn cleanup_stale_invites(state: &Arc<AppState>) {
    // Find stale forming sessions (older than 5 minutes)
    let stale_sessions: Vec<(String,)> = match sqlx::query_as(
        "SELECT id FROM group_sessions
         WHERE status = 'forming'
           AND created_at < datetime('now', '-5 minutes')",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("[cleanup_stale_invites] DB error: {}", e);
            return;
        }
    };

    if stale_sessions.is_empty() {
        return;
    }

    tracing::info!(
        "[cleanup_stale_invites] Found {} stale forming sessions",
        stale_sessions.len()
    );

    for (session_id,) in &stale_sessions {
        // Timeout pending members
        let _ = sqlx::query(
            "UPDATE group_session_members SET status = 'timeout'
             WHERE group_session_id = ? AND status = 'pending'",
        )
        .bind(session_id)
        .execute(&state.db)
        .await;

        // Check if any members have accepted/validated status
        let accepted_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM group_session_members
             WHERE group_session_id = ? AND status IN ('accepted', 'validated')",
        )
        .bind(session_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if accepted_count == 0 {
            // No accepted members — cancel the session
            let _ = sqlx::query(
                "UPDATE group_sessions SET status = 'cancelled' WHERE id = ?",
            )
            .bind(session_id)
            .execute(&state.db)
            .await;

            tracing::info!(
                "[cleanup_stale_invites] Cancelled stale session {} (no accepted members)",
                session_id
            );
        } else {
            tracing::info!(
                "[cleanup_stale_invites] Session {} has {} accepted members — keeping active",
                session_id,
                accepted_count
            );
        }

        // Release pods for timed-out members by clearing pod_id
        let _ = sqlx::query(
            "UPDATE group_session_members SET pod_id = NULL
             WHERE group_session_id = ? AND status = 'timeout'",
        )
        .bind(session_id)
        .execute(&state.db)
        .await;
    }
}
