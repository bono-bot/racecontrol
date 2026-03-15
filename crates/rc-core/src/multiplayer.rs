use std::sync::Arc;

use rand::Rng;

use crate::ac_server;
use crate::auth;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::{CoreToAgentMessage, DashboardEvent};
use rc_common::types::{GroupMemberInfo, GroupSessionInfo};

/// Find N idle pods, preferring adjacent (consecutive pod numbers).
/// Falls back to nearest available pods if adjacency isn't possible.
async fn find_adjacent_idle_pods(
    state: &Arc<AppState>,
    count: usize,
) -> Result<Vec<String>, String> {
    if count == 0 {
        return Ok(vec![]);
    }

    // Get all idle pods sorted by pod_number
    let pods = state.pods.read().await;
    let mut idle_pods: Vec<(String, u32)> = pods
        .values()
        .filter(|p| {
            p.status == rc_common::types::PodStatus::Idle && p.billing_session_id.is_none()
        })
        .map(|p| (p.id.clone(), p.number))
        .collect();
    drop(pods);

    // Filter out pods with active reservations
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
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'))",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(game)
        .bind(track)
        .bind(car)
        .bind(duration_minutes)
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
        .bind(&a)
        .bind(&b)
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

        if let Some((pres,)) = presence {
            if pres.as_deref() != Some("online") {
                return Err(format!("Friend {} is not online", friend_id));
            }
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

    // Generate shared 4-digit PIN
    let shared_pin: u32 = rand::thread_rng().gen_range(1000..=9999);
    let shared_pin_str = format!("{:04}", shared_pin);

    // Create group session
    let group_session_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, created_at)
         VALUES (?, ?, ?, ?, ?, 'forming', ?, datetime('now'))",
    )
    .bind(&group_session_id)
    .bind(host_id)
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind(&shared_pin_str)
    .bind(total_members as i64)
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
        "multiplayer_booking",
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
                .send(CoreToAgentMessage::ShowPinLockScreen {
                    token_id: host_token.id.clone(),
                    driver_name: host_name.clone(),
                    pricing_tier_name: tier_name.clone(),
                    allocated_seconds: duration_minutes as u32 * 60,
                })
                .await;
        }
        drop(agent_senders);

        // Create host member record
        let host_member_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, reservation_id, auth_token_id, wallet_txn_id, invited_at, accepted_at)
             VALUES (?, ?, ?, 'host', 'accepted', ?, ?, ?, ?, datetime('now'), datetime('now'))",
        )
        .bind(&host_member_id)
        .bind(&group_session_id)
        .bind(host_id)
        .bind(host_pod_id)
        .bind(&host_reservation_id)
        .bind(&host_token.id)
        .bind(&wallet_txn_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Create invitee records (pending — they need to accept + pay)
        for (i, friend_id) in friend_ids.iter().enumerate() {
            let member_id = uuid::Uuid::new_v4().to_string();
            let friend_pod_id = &pod_ids[i + 1]; // host gets first pod

            // Pre-assign pod but don't reserve yet (reserve on accept)
            sqlx::query(
                "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, invited_at)
                 VALUES (?, ?, ?, 'invitee', 'pending', ?, datetime('now'))",
            )
            .bind(&member_id)
            .bind(&group_session_id)
            .bind(friend_id)
            .bind(friend_pod_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;
        }

        // Build response
        let info = build_group_session_info(state, &group_session_id).await?;

        // MULTI-01: Auto-start AC server for multiplayer booking
        // Build AcLanSessionConfig from the experience/custom booking data
        let (game, track, car) = if let Some(ref c) = custom {
            (c.0.clone(), c.1.clone(), c.2.clone())
        } else {
            // Resolve from experience
            let exp = sqlx::query_as::<_, (String, String, String)>(
                "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
            )
            .bind(&experience_id_resolved)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("DB error resolving experience: {}", e))?
            .ok_or("Experience not found for AC server config")?;
            exp
        };

        if game == "assetto_corsa" {
            let ac_config = rc_common::types::AcLanSessionConfig {
                name: format!("Multiplayer: {}", experience_name),
                track: track.clone(),
                track_config: String::new(),
                cars: vec![car.clone()],
                max_clients: total_members as u32 + 2, // players + margin
                password: shared_pin_str.clone(),
                ..Default::default()
            };

            match ac_server::start_ac_server(state, ac_config, pod_ids.clone(), None).await {
                Ok(ac_session_id) => {
                    // Store AC session ID on group_session for later stop
                    let _ = sqlx::query(
                        "UPDATE group_sessions SET ac_session_id = ? WHERE id = ?",
                    )
                    .bind(&ac_session_id)
                    .bind(&group_session_id)
                    .execute(&state.db)
                    .await;
                    tracing::info!(
                        "MULTI-01: AC server {} started for multiplayer group {}",
                        ac_session_id, group_session_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to start AC server for multiplayer group {}: {}",
                        group_session_id, e
                    );
                    // Don't fail the booking — server can be started manually
                }
            }
        }

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
            let _ = wallet::credit(
                state,
                host_id,
                price_paise,
                "refund_session",
                Some(&group_session_id),
                Some("Multiplayer booking failed - auto refund"),
                None,
            )
            .await;
            Err(e)
        }
    }
}

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
        "multiplayer_booking",
        Some(group_session_id),
        Some("Multiplayer session invite accepted"),
    )
    .await?;

    // Reserve pod
    let reservation_id = pod_reservation::create_reservation(state, driver_id, &pod_id).await?;

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
            .send(CoreToAgentMessage::ShowPinLockScreen {
                token_id: token.id.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: tier_name,
                allocated_seconds: duration_minutes as u32 * 60,
            })
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

        // Auto-start AC LAN session
        let _ = start_ac_lan_for_group(state, group_session_id).await;
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
                    .send(CoreToAgentMessage::ShowAssistanceScreen {
                        driver_name,
                        message: format!(
                            "Waiting for friends... ({}/{} checked in)",
                            validated_total, accepted_total
                        ),
                    })
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

/// Atomically debit multiple wallets: validate ALL balances first, then debit sequentially.
/// If any debit fails after validation, roll back all previous debits via credit.
/// Returns Vec<(driver_id, txn_id)> on success, or error with driver name and details.
pub async fn atomic_multi_debit(
    state: &Arc<AppState>,
    driver_ids: &[String],
    price_paise: i64,
    reference_id: &str,
    notes: &str,
) -> Result<Vec<(String, String)>, String> {
    if driver_ids.is_empty() {
        return Ok(vec![]);
    }

    // Phase 1: Validate all wallet balances
    for driver_id in driver_ids {
        wallet::ensure_wallet(state, driver_id).await?;
        let balance = wallet::get_balance(state, driver_id).await?;
        if balance < price_paise {
            let name = get_driver_name(state, driver_id).await;
            return Err(format!(
                "Insufficient balance for {} ({}): have {}p, need {}p",
                name, driver_id, balance, price_paise
            ));
        }
    }

    // Phase 2: Debit sequentially with rollback on failure
    let mut completed: Vec<(String, String)> = Vec::new();

    for driver_id in driver_ids {
        match wallet::debit(
            state,
            driver_id,
            price_paise,
            "multiplayer_booking",
            Some(reference_id),
            Some(notes),
        )
        .await
        {
            Ok((_balance, txn_id)) => {
                completed.push((driver_id.clone(), txn_id));
            }
            Err(e) => {
                // Rollback: credit back all previously debited wallets
                tracing::warn!(
                    "atomic_multi_debit: debit failed for {}, rolling back {} previous debits: {}",
                    driver_id,
                    completed.len(),
                    e
                );
                for (prev_driver_id, _prev_txn_id) in &completed {
                    let _ = wallet::credit(
                        state,
                        prev_driver_id,
                        price_paise,
                        "refund_session",
                        Some(reference_id),
                        Some("Multiplayer booking failed - auto refund"),
                        None,
                    )
                    .await;
                }
                let name = get_driver_name(state, driver_id).await;
                return Err(format!(
                    "Debit failed for {} ({}): {}. All wallets rolled back.",
                    name, driver_id, e
                ));
            }
        }
    }

    Ok(completed)
}

/// Staff-initiated multiplayer booking. Bypasses friendship checks and invite flow.
/// All drivers are immediately validated and the game is launched.
pub async fn staff_book_multiplayer(
    state: &Arc<AppState>,
    driver_ids: Vec<String>,
    pod_ids: Vec<String>,
    experience_id: Option<&str>,
    pricing_tier_id: &str,
    game: Option<&str>,
    track: Option<&str>,
    car: Option<&str>,
) -> Result<GroupSessionInfo, String> {
    // Validate driver_ids.len() == pod_ids.len()
    if driver_ids.len() != pod_ids.len() {
        return Err(format!(
            "driver_ids count ({}) must equal pod_ids count ({})",
            driver_ids.len(),
            pod_ids.len()
        ));
    }

    if driver_ids.is_empty() {
        return Err("Must provide at least one driver".to_string());
    }

    // Validate all pods exist and are not in active sessions
    for pod_id in &pod_ids {
        let pods = state.pods.read().await;
        let pod = pods.get(pod_id);
        match pod {
            None => return Err(format!("Pod {} not found", pod_id)),
            Some(p) => {
                if p.billing_session_id.is_some() {
                    return Err(format!("Pod {} is already in an active session", pod_id));
                }
            }
        }
    }

    // Look up pricing tier to get price_paise
    let tier = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, price_paise, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Pricing tier not found")?;

    let (tier_name, price_paise, duration_minutes) = tier;

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
    } else if let (Some(g), Some(t), Some(c)) = (game, track, car) {
        // Create ad-hoc experience
        let adhoc_id = uuid::Uuid::new_v4().to_string();
        let adhoc_name = format!("Custom: {} @ {}", c, t);
        sqlx::query(
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'))",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(g)
        .bind(t)
        .bind(c)
        .bind(duration_minutes)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error creating ad-hoc experience: {}", e))?;
        (adhoc_id, adhoc_name)
    } else {
        return Err("Must provide experience_id or game/track/car".to_string());
    };

    // Generate shared PIN
    let shared_pin: u32 = rand::thread_rng().gen_range(1000..=9999);
    let shared_pin_str = format!("{:04}", shared_pin);

    // Create group session ID first for reference_id
    let group_session_id = uuid::Uuid::new_v4().to_string();

    // Atomic multi-debit all drivers
    let debit_results = atomic_multi_debit(
        state,
        &driver_ids,
        price_paise,
        &group_session_id,
        &format!("Staff multiplayer: {}", experience_name),
    )
    .await?;

    // Create group_sessions row with status 'all_validated'
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, validated_count, created_at, started_at)
         VALUES (?, ?, ?, ?, ?, 'all_validated', ?, ?, datetime('now'), datetime('now'))",
    )
    .bind(&group_session_id)
    .bind(&driver_ids[0]) // host = first driver
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind(&shared_pin_str)
    .bind(driver_ids.len() as i64)
    .bind(driver_ids.len() as i64) // all pre-validated
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Create group_session_members rows (all status 'validated', role 'staff_assigned')
    for (i, driver_id) in driver_ids.iter().enumerate() {
        let member_id = uuid::Uuid::new_v4().to_string();
        let pod_id = &pod_ids[i];
        let wallet_txn_id = debit_results
            .iter()
            .find(|(did, _)| did == driver_id)
            .map(|(_, tid)| tid.clone());

        sqlx::query(
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, wallet_txn_id, invited_at, accepted_at, validated_at)
             VALUES (?, ?, ?, 'staff_assigned', 'validated', ?, ?, datetime('now'), datetime('now'), datetime('now'))",
        )
        .bind(&member_id)
        .bind(&group_session_id)
        .bind(driver_id)
        .bind(pod_id)
        .bind(wallet_txn_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    }

    // Start AC LAN or launch individual games
    let _ = start_ac_lan_for_group(state, &group_session_id).await;

    // Build and return response
    let info = build_group_session_info(state, &group_session_id).await?;

    // Broadcast
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::GroupSessionCreated(info.clone()));

    tracing::info!(
        "Staff multiplayer session {} created with {} drivers, PIN: {}",
        group_session_id,
        driver_ids.len(),
        shared_pin_str
    );

    Ok(info)
}

// ─── Internal Helpers ──────────────────────────────────────────────────────

/// Auto-start AC LAN session when all group members are validated.
async fn start_ac_lan_for_group(
    state: &Arc<AppState>,
    group_session_id: &str,
) -> Result<(), String> {
    // Get group session details
    let session = sqlx::query_as::<_, (String,)>(
        "SELECT experience_id FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let experience_id = session.0;

    // Get experience details
    let exp = sqlx::query_as::<_, (String, String, String)>(
        "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Experience not found")?;

    let (game, track, car) = exp;

    // Get all validated member pod_ids
    let members: Vec<(String, String)> = sqlx::query_as(
        "SELECT driver_id, pod_id FROM group_session_members
         WHERE group_session_id = ? AND status = 'validated' AND pod_id IS NOT NULL",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let pod_ids: Vec<String> = members.iter().map(|(_, pid)| pid.clone()).collect();

    if game == "assetto_corsa" || game == "ac" {
        let human_count = members.len();

        // Query track pit count from any pod's content manifest for AI filler calculation.
        // Default to 24 if not available (reasonable for most tracks).
        let max_pits: usize = {
            let manifests = state.pod_manifests.read().await;
            manifests.values()
                .find_map(|m| {
                    m.tracks.iter()
                        .find(|t| t.id == track)
                        .and_then(|t| t.configs.first())
                        .and_then(|c| c.pit_count)
                        .map(|p| p as usize)
                })
                .unwrap_or(24)
        };

        // Calculate AI filler count: fill remaining pits, cap at 19 (AC 20-slot limit)
        let ai_count = max_pits.saturating_sub(human_count).min(19);

        // Query difficulty tier from experience for AI_LEVEL mapping
        let difficulty_tier: Option<String> = sqlx::query_scalar(
            "SELECT difficulty_tier FROM kiosk_experiences WHERE id = ?",
        )
        .bind(&experience_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        // Map difficulty tier to AI_LEVEL (Phase 2 midpoints)
        let ai_level = match difficulty_tier.as_deref() {
            Some("rookie") => 75,
            Some("amateur") => 82,
            Some("semi_pro") => 87,
            Some("pro") => 93,
            Some("alien") => 98,
            _ => 87, // Default to SemiPro midpoint
        };

        // Build human entry slots
        let mut entry_slots = Vec::new();
        for (_i, (driver_id, pod_id)) in members.iter().enumerate() {
            let (dname, dguid) = get_driver_entry_info(state, driver_id).await;
            entry_slots.push(rc_common::types::AcEntrySlot {
                car_model: car.clone(),
                skin: String::new(),
                driver_name: dname,
                guid: dguid,
                ballast: 0,
                restrictor: 0,
                pod_id: Some(pod_id.clone()),
                ai_mode: None,
            });
        }

        // Add AI fillers (same car as players, AI=fixed for AssettoServer)
        if ai_count > 0 {
            let ai_names = rc_common::ai_names::pick_ai_names(ai_count);
            for name in ai_names {
                entry_slots.push(rc_common::types::AcEntrySlot {
                    car_model: car.clone(),
                    skin: String::new(),
                    driver_name: name,
                    guid: String::new(),
                    ballast: 0,
                    restrictor: 0,
                    pod_id: None,
                    ai_mode: Some("fixed".to_string()),
                });
            }
            tracing::info!(
                "Added {} AI fillers (AI_LEVEL={}) for group session {}",
                ai_count, ai_level, group_session_id
            );
        }

        // Build AC LAN config
        let config = rc_common::types::AcLanSessionConfig {
            name: format!("Multiplayer - Group {}", &group_session_id[..8]),
            track: track.clone(),
            track_config: String::new(),
            cars: vec![car.clone()],
            max_clients: (human_count + ai_count) as u32,
            password: String::new(),
            sessions: vec![rc_common::types::AcSessionBlock {
                name: "Race".to_string(),
                session_type: rc_common::types::SessionType::Race,
                duration_minutes: 0,
                laps: 10,
                wait_time_secs: 10,
            }],
            entries: entry_slots,
            weather: vec![rc_common::types::AcWeatherConfig {
                graphics: "3_clear".to_string(),
                base_temperature_ambient: 26,
                base_temperature_road: 32,
                variation_ambient: 2,
                variation_road: 2,
                wind_base_speed_min: 0,
                wind_base_speed_max: 10,
                wind_base_direction: 0,
                wind_variation_direction: 15,
            }],
            dynamic_track: rc_common::types::AcDynamicTrackConfig {
                session_start: 90,
                randomness: 2,
                session_transfer: 90,
                lap_gain: 30,
            },
            pickup_mode: true,
            udp_port: 0,  // Dynamically assigned by PortAllocator in start_ac_server()
            tcp_port: 0,  // Dynamically assigned by PortAllocator in start_ac_server()
            http_port: 0, // Dynamically assigned by PortAllocator in start_ac_server()
            min_csp_version: 0,
            ..Default::default()
        };

        match crate::ac_server::start_ac_server(state, config, pod_ids.clone(), Some(ai_level)).await {
            Ok(ac_session_id) => {
                // Store AC session ID + track/car/ai_count for lobby enrichment
                sqlx::query("UPDATE group_sessions SET ac_session_id = ?, track = ?, car = ?, ai_count = ? WHERE id = ?")
                    .bind(&ac_session_id)
                    .bind(&track)
                    .bind(&car)
                    .bind(ai_count as i64)
                    .execute(&state.db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;

                // Broadcast
                let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionAllValidated {
                    group_session_id: group_session_id.to_string(),
                    ac_session_id: ac_session_id.clone(),
                    pod_ids: pod_ids.clone(),
                });

                tracing::info!(
                    "AC LAN started for group session {}: ac_session={}",
                    group_session_id,
                    ac_session_id
                );
            }
            Err(e) => {
                tracing::error!("Failed to start AC LAN for group {}: {}", group_session_id, e);
                return Err(e.to_string());
            }
        }
    } else {
        // Non-AC games: launch game on each pod individually
        let sim_type = match game.as_str() {
            "iracing" => rc_common::types::SimType::IRacing,
            "f1_25" | "f1" => rc_common::types::SimType::F125,
            "le_mans_ultimate" | "lmu" => rc_common::types::SimType::LeMansUltimate,
            "forza" => rc_common::types::SimType::Forza,
            _ => rc_common::types::SimType::AssettoCorsa,
        };

        let agent_senders = state.agent_senders.read().await;
        for (driver_id, pod_id) in &members {
            if let Some(sender) = agent_senders.get(pod_id) {
                let driver_name = get_driver_name(state, driver_id).await;
                let launch_args = serde_json::json!({
                    "car": car, "track": track, "driver": driver_name
                })
                .to_string();

                let _ = sender
                    .send(CoreToAgentMessage::LaunchGame {
                        sim_type: sim_type.clone(),
                        launch_args: Some(launch_args),
                    })
                    .await;
            }
        }
        drop(agent_senders);

        // Broadcast
        let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionAllValidated {
            group_session_id: group_session_id.to_string(),
            ac_session_id: String::new(),
            pod_ids,
        });

        tracing::info!(
            "Games launched for group session {} ({})",
            group_session_id,
            game
        );
    }

    Ok(())
}

/// Check if all invitees have responded (accepted or declined). If so, set status to 'ready'.
async fn check_all_responded(state: &Arc<AppState>, group_session_id: &str) {
    let pending: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM group_session_members
         WHERE group_session_id = ? AND status = 'pending'",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if pending.map(|r| r.0).unwrap_or(1) == 0 {
        let _ = sqlx::query(
            "UPDATE group_sessions SET status = 'ready' WHERE id = ? AND status = 'forming'",
        )
        .bind(group_session_id)
        .execute(&state.db)
        .await;
    }
}

/// Build full GroupSessionInfo from DB.
async fn build_group_session_info(
    state: &Arc<AppState>,
    group_session_id: &str,
) -> Result<GroupSessionInfo, String> {
    let session = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT gs.id, gs.host_driver_id, gs.experience_id, gs.shared_pin, gs.status, gs.created_at
         FROM group_sessions gs WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let (id, host_driver_id, experience_id, shared_pin, status, created_at) = session;

    let host_name = get_driver_name(state, &host_driver_id).await;

    let experience_name: String = sqlx::query_scalar(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Get members
    let member_rows: Vec<(String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT gsm.driver_id, gsm.role, gsm.pod_id, gsm.status, d.name
         FROM group_session_members gsm
         INNER JOIN drivers d ON d.id = gsm.driver_id
         WHERE gsm.group_session_id = ?
         ORDER BY gsm.role DESC, gsm.invited_at",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut members = Vec::new();
    for (driver_id, role, pod_id, member_status, name) in member_rows {
        let customer_id = get_customer_id(state, &driver_id).await;
        let pod_number = if let Some(ref pid) = pod_id {
            get_pod_number(state, pid).await
        } else {
            None
        };

        members.push(GroupMemberInfo {
            driver_id,
            driver_name: name.unwrap_or_else(|| "Unknown".to_string()),
            customer_id,
            role,
            status: member_status,
            pod_id,
            pod_number,
        });
    }

    let pricing_tier_name: String = sqlx::query_scalar(
        "SELECT pt.name FROM pricing_tiers pt
         INNER JOIN group_sessions gs ON gs.pricing_tier_id = pt.id
         WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Query enrichment fields from group_sessions (track, car, ai_count added in Phase 9)
    let enrichment: Option<(Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT track, car, ai_count FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (track, car, ai_count) = enrichment.unwrap_or((None, None, None));

    // Query difficulty_tier from experience (if available)
    let difficulty_tier: Option<String> = sqlx::query_scalar(
        "SELECT difficulty_tier FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Ok(GroupSessionInfo {
        id,
        host_driver_id,
        host_name,
        experience_name,
        pricing_tier_name,
        shared_pin,
        status,
        members,
        created_at,
        track,
        car,
        ai_count: ai_count.map(|c| c as u32),
        difficulty_tier,
    })
}

async fn get_driver_name(state: &Arc<AppState>, driver_id: &str) -> String {
    sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Unknown".to_string())
}

async fn get_customer_id(state: &Arc<AppState>, driver_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT customer_id FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
}

async fn get_pod_number(state: &Arc<AppState>, pod_id: &str) -> Option<u32> {
    let pods = state.pods.read().await;
    pods.get(pod_id).map(|p| p.number)
}

fn canonical_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

/// Get driver name and steam_guid for AC entry list population.
async fn get_driver_entry_info(state: &Arc<AppState>, driver_id: &str) -> (String, String) {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT name, steam_guid FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((name, guid)) => (
            name.unwrap_or_else(|| "Driver".to_string()),
            guid.unwrap_or_default(),
        ),
        None => ("Driver".to_string(), String::new()),
    }
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
    if pod_count < 2 || pod_count > 8 {
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
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'))",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(game)
        .bind(track)
        .bind(car)
        .bind(duration_minutes)
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
        "multiplayer_kiosk",
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
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, created_at)
         VALUES (?, ?, ?, ?, ?, 'active', ?, datetime('now'))",
    )
    .bind(&group_session_id)
    .bind(host_id)
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind("0000") // Placeholder — each participant gets unique PIN
    .bind(pod_count as i64)
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
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, reservation_id, auth_token_id, wallet_txn_id, invited_at, accepted_at)
             VALUES (?, ?, ?, ?, 'accepted', ?, ?, ?, ?, datetime('now'), datetime('now'))",
        )
        .bind(&member_id)
        .bind(&group_session_id)
        .bind(host_id)
        .bind(role)
        .bind(pod_id)
        .bind(&reservation_id)
        .bind(&token.id)
        .bind(&wallet_txn_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Send lock screen to pod
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender
                .send(CoreToAgentMessage::ShowPinLockScreen {
                    token_id: token.id.clone(),
                    driver_name: host_name.clone(),
                    pricing_tier_name: tier_name.clone(),
                    allocated_seconds: duration_minutes as u32 * 60,
                })
                .await;
        }

        assignments.push(KioskMultiplayerAssignment {
            pin,
            pod_id: pod_id.clone(),
            pod_number,
            role: role.to_string(),
        });
    }

    // MULTI-01: Auto-start AC server
    let (game, track, car) = if let Some(ref c) = custom {
        (c.0.clone(), c.1.clone(), c.2.clone())
    } else {
        let exp = sqlx::query_as::<_, (String, String, String)>(
            "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
        )
        .bind(&experience_id_resolved)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or("Experience not found")?;
        exp
    };

    if game == "assetto_corsa" {
        let ac_config = rc_common::types::AcLanSessionConfig {
            name: format!("Kiosk Multiplayer: {}", experience_name),
            track: track.clone(),
            track_config: String::new(),
            cars: vec![car.clone()],
            max_clients: pod_count as u32 + 2,
            password: String::new(), // Kiosk multiplayer uses unique PINs per pod, not server password
            ..Default::default()
        };

        match ac_server::start_ac_server(state, ac_config, pod_ids.clone(), None).await {
            Ok(ac_session_id) => {
                let _ = sqlx::query(
                    "UPDATE group_sessions SET ac_session_id = ? WHERE id = ?",
                )
                .bind(&ac_session_id)
                .bind(&group_session_id)
                .execute(&state.db)
                .await;
                tracing::info!(
                    "MULTI-01: AC server {} started for kiosk multiplayer group {}",
                    ac_session_id, group_session_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to start AC server for kiosk multiplayer {}: {}",
                    group_session_id, e
                );
            }
        }
    }

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
