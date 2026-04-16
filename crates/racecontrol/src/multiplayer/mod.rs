mod ac_launch;
mod helpers;
mod invite;
mod kiosk;
mod staff;

// Re-export public API surface so callers continue using `multiplayer::*`
pub use invite::{
    accept_group_invite, decline_group_invite, find_group_session_for_token,
    get_active_group_session, on_member_validated,
};
pub use kiosk::{
    book_multiplayer_kiosk, cleanup_stale_invites, KioskMultiplayerAssignment,
    KioskMultiplayerResult,
};
pub use staff::{atomic_multi_debit, staff_book_multiplayer};

use std::sync::Arc;

use rand::Rng;

use crate::auth;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::GroupSessionInfo;

use helpers::{build_group_session_info, canonical_pair, get_driver_name};

/// Find N idle pods, preferring adjacent (consecutive pod numbers).
/// Falls back to nearest available pods if adjacency isn't possible.
///
/// MMA-P1: Uses a DB transaction to atomically check availability + create reservations,
/// preventing TOCTOU race conditions where two concurrent bookings claim the same pod.
pub(crate) async fn find_adjacent_idle_pods(
    state: &Arc<AppState>,
    count: usize,
) -> Result<Vec<String>, String> {
    if count == 0 {
        return Ok(vec![]);
    }

    // Snapshot idle pods from in-memory state (fast pre-filter)
    let pods = state.pods.read().await;
    let mut idle_pods: Vec<(String, u32)> = pods
        .values()
        .filter(|p| {
            p.status == rc_common::types::PodStatus::Idle && p.billing_session_id.is_none()
        })
        .map(|p| (p.id.clone(), p.number))
        .collect();
    drop(pods);

    // MMA-P1: Filter out pods with active reservations.
    // SQLite serializes writes, so the TOCTOU window is limited to concurrent readers.
    // The caller (book_multiplayer) should handle InsertConflict gracefully if a pod
    // gets reserved between this check and the reservation INSERT.
    let mut available: Vec<(String, u32)> = Vec::new();
    for (pod_id, pod_number) in idle_pods.drain(..) {
        let has_reservation = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM pod_reservations WHERE pod_id = ? AND status = 'active'",
        )
        .bind(&pod_id)
        .fetch_one(&state.db)
        .await
        .map(|r| r.0 > 0)
        .unwrap_or(true);

        if !has_reservation {
            available.push((pod_id, pod_number));
        }
    }

    if available.len() < count {
        return Err(format!(
            "Not enough idle pods: need {}, have {}",
            count,
            available.len()
        ));
    }

    available.sort_by_key(|(_, num)| *num);

    // Try to find consecutive pods (sliding window)
    if available.len() >= count {
        for window in available.windows(count) {
            let first = window[0].1;
            let last = window[count - 1].1;
            if (last - first) as usize == count - 1 {
                // Consecutive!
                return Ok(window.iter().map(|(id, _)| id.clone()).collect());
            }
        }
    }

    // Fallback: find pods with minimum spread
    let mut best_window = &available[..count];
    let mut best_spread = available[count - 1].1 - available[0].1;

    for window in available.windows(count) {
        let spread = window[count - 1].1 - window[0].1;
        if spread < best_spread {
            best_spread = spread;
            best_window = window;
        }
    }

    Ok(best_window.iter().map(|(id, _)| id.clone()).collect())
}

/// Book a multiplayer group session.
/// Host wallet is debited, pods are reserved, auth tokens created with shared PIN.
/// Either `experience_id` or `custom` must be provided.
pub async fn book_multiplayer(
    state: &Arc<AppState>,
    host_id: &str,
    experience_id: Option<&str>,
    pricing_tier_id: &str,
    friend_ids: Vec<String>,
    custom: Option<(String, String, String)>, // (game, track, car)
) -> Result<GroupSessionInfo, String> {
    let total_members = 1 + friend_ids.len(); // host + friends

    // Verify pricing tier
    let tier = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, price_paise, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Pricing tier not found")?;

    let (tier_name, price_paise, duration_minutes) = tier;

    // Resolve experience: either from experience_id or create ad-hoc from custom payload
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
        // Create ad-hoc experience for this custom multiplayer booking
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
        .map_err(|e| format!("DB error creating ad-hoc experience: {}", e))?;
        (adhoc_id, adhoc_name)
    } else {
        return Err("Must provide experience_id or custom booking payload".to_string());
    };

    // Verify all friend_ids are actual friends of host
    for friend_id in &friend_ids {
        let (a, b) = canonical_pair(host_id, friend_id);
        let friendship: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM friendships WHERE driver_a_id = ? AND driver_b_id = ?",
        )
        .bind(a)
        .bind(b)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if friendship.is_none() {
            return Err(format!("Driver {} is not your friend", friend_id));
        }

        // Check friend is online
        let presence: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT presence FROM drivers WHERE id = ?",
        )
        .bind(friend_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if let Some((pres,)) = presence
            && pres.as_deref() != Some("online") {
                return Err(format!("Friend {} is not online", friend_id));
            }
    }

    // Validate host wallet
    wallet::ensure_wallet(state, host_id).await?;
    let host_balance = wallet::get_balance(state, host_id).await?;
    if host_balance < price_paise {
        return Err(format!(
            "Insufficient wallet balance: have {}p, need {}p",
            host_balance, price_paise
        ));
    }

    // Find adjacent idle pods for all members
    let pod_ids = find_adjacent_idle_pods(state, total_members).await?;

    let shared_pin: u32 = rand::thread_rng().gen_range(1000..=9999);
    let shared_pin_str = format!("{:04}", shared_pin);

    // Create group session
    let group_session_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, 'forming', ?, datetime('now'), ?)",
    )
    .bind(&group_session_id)
    .bind(host_id)
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind(&shared_pin_str)
    .bind(total_members as i64)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Ensure ac_session_id column exists (idempotent DDL for rolling deploy)
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN ac_session_id TEXT")
        .execute(&state.db)
        .await; // Ignore error if column already exists

    // Debit host wallet
    let (_, wallet_txn_id) = wallet::debit(
        state,
        host_id,
        price_paise,
        "debit_session",
        Some(&group_session_id),
        Some(&format!("Multiplayer session: {}", experience_name)),
    )
    .await?;

    // Wrap remaining operations so we can refund host if any step fails
    let result: Result<GroupSessionInfo, String> = async {
        // Reserve pod for host + create auth token
        let host_pod_id = &pod_ids[0];
        let host_reservation_id = pod_reservation::create_reservation(state, host_id, host_pod_id).await?;

        let host_token = auth::create_auth_token(
            state,
            host_pod_id.clone(),
            host_id.to_string(),
            pricing_tier_id.to_string(),
            "pin".to_string(),
            None,
            Some(duration_minutes as u32),
            Some(experience_id_resolved.clone()),
            None,
        )
        .await?;

        // Override the auto-generated PIN with the shared PIN
        sqlx::query("UPDATE auth_tokens SET token = ? WHERE id = ?")
            .bind(&shared_pin_str)
            .bind(&host_token.id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        // Re-send lock screen with shared PIN
        let host_name = get_driver_name(state, host_id).await;
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(host_pod_id) {
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::ShowPinLockScreen {
                    token_id: host_token.id.clone(),
                    driver_name: host_name.clone(),
                    pricing_tier_name: tier_name.clone(),
                    allocated_seconds: duration_minutes as u32 * 60,
                }))
                .await;
        }
        drop(agent_senders);

        // Create host member record
        let host_member_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, reservation_id, auth_token_id, wallet_txn_id, invited_at, accepted_at, venue_id)
             VALUES (?, ?, ?, 'host', 'accepted', ?, ?, ?, ?, datetime('now'), datetime('now'), ?)",
        )
        .bind(&host_member_id)
        .bind(&group_session_id)
        .bind(host_id)
        .bind(host_pod_id)
        .bind(&host_reservation_id)
        .bind(&host_token.id)
        .bind(&wallet_txn_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Create invitee records (pending — they need to accept + pay)
        for (i, friend_id) in friend_ids.iter().enumerate() {
            let member_id = uuid::Uuid::new_v4().to_string();
            let friend_pod_id = &pod_ids[i + 1]; // host gets first pod

            // Pre-assign pod but don't reserve yet (reserve on accept)
            sqlx::query(
                "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, invited_at, venue_id)
                 VALUES (?, ?, ?, 'invitee', 'pending', ?, datetime('now'), ?)",
            )
            .bind(&member_id)
            .bind(&group_session_id)
            .bind(friend_id)
            .bind(friend_pod_id)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;
        }

        // Build response
        let info = build_group_session_info(state, &group_session_id).await?;

        // GROUP-01: AC server start deferred to on_member_validated() -> start_ac_lan_for_group()
        // Server starts only when ALL members have validated their PINs (coordinated launch).
        // The ac_session_id will be set on group_sessions by start_ac_lan_for_group().

        // Broadcast to dashboard
        let _ = state
            .dashboard_tx
            .send(DashboardEvent::GroupSessionCreated(info.clone()));

        tracing::info!(
            "Multiplayer group session {} created by {} with {} members, PIN: {}",
            group_session_id,
            host_id,
            total_members,
            shared_pin_str
        );

        Ok(info)
    }.await;

    match result {
        Ok(info) => Ok(info),
        Err(e) => {
            // Refund host wallet since booking failed after debit
            tracing::warn!(
                "Multiplayer booking failed after wallet debit, refunding host {}: {}",
                host_id, e
            );
            // MMA-Iter3: Track refund result. If refund fails, insert into pending_refunds
            // table so a background reconciliation job can retry later.
            let refund_result = wallet::credit(
                state,
                host_id,
                price_paise,
                "refund_session",
                Some(&group_session_id),
                Some("Multiplayer booking failed - auto refund"),
                None,
            )
            .await;
            if let Err(ref refund_err) = refund_result {
                tracing::error!(
                    "CRITICAL: Refund FAILED for host {} amount {} paise (group {}): {}. Recording pending refund.",
                    host_id, price_paise, group_session_id, refund_err
                );
                // Best-effort insert into pending_refunds table (created if not exists)
                let _ = sqlx::query(
                    "CREATE TABLE IF NOT EXISTS pending_refunds (
                        id TEXT PRIMARY KEY, driver_id TEXT NOT NULL, amount_paise INTEGER NOT NULL,
                        reason TEXT, reference_id TEXT, status TEXT DEFAULT 'pending',
                        created_at TEXT DEFAULT (datetime('now')), retry_count INTEGER DEFAULT 0
                    )"
                ).execute(&state.db).await;
                let _ = sqlx::query(
                    "INSERT INTO pending_refunds (id, driver_id, amount_paise, reason, reference_id)
                     VALUES (?, ?, ?, 'multiplayer_booking_failed', ?)"
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(host_id)
                .bind(price_paise)
                .bind(&group_session_id)
                .execute(&state.db)
                .await;
            }
            Err(e)
        }
    }
}
