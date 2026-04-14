use std::sync::Arc;

use rand::Rng;

use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::DashboardEvent;
use rc_common::types::GroupSessionInfo;

use super::ac_launch::start_ac_lan_for_group;
use super::helpers::{build_group_session_info, get_driver_name};

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
            "debit_session",
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

    let (_tier_name, price_paise, duration_minutes) = tier;

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
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'), ?)",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(g)
        .bind(t)
        .bind(c)
        .bind(duration_minutes)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error creating ad-hoc experience: {}", e))?;
        (adhoc_id, adhoc_name)
    } else {
        return Err("Must provide experience_id or game/track/car".to_string());
    };

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
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, validated_count, created_at, started_at, venue_id)
         VALUES (?, ?, ?, ?, ?, 'all_validated', ?, ?, datetime('now'), datetime('now'), ?)",
    )
    .bind(&group_session_id)
    .bind(&driver_ids[0]) // host = first driver
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind(&shared_pin_str)
    .bind(driver_ids.len() as i64)
    .bind(driver_ids.len() as i64) // all pre-validated
    .bind(&state.config.venue.venue_id)
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
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, wallet_txn_id, invited_at, accepted_at, validated_at, venue_id)
             VALUES (?, ?, ?, 'staff_assigned', 'validated', ?, ?, datetime('now'), datetime('now'), datetime('now'), ?)",
        )
        .bind(&member_id)
        .bind(&group_session_id)
        .bind(driver_id)
        .bind(pod_id)
        .bind(wallet_txn_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    }

    // Start AC LAN or launch individual games — surface errors to caller
    if let Err(e) = start_ac_lan_for_group(state, &group_session_id).await {
        tracing::error!("AC LAN launch failed for staff group {}: {}", group_session_id, e);
        // Mark session as failed
        let _ = sqlx::query(
            "UPDATE group_sessions SET status = 'ac_launch_failed' WHERE id = ?",
        )
        .bind(&group_session_id)
        .execute(&state.db)
        .await;
        // Don't return error — booking succeeded, billing was charged.
        // The error is recorded in session status for the dashboard.
        // Staff can retry or refund.
    }

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
