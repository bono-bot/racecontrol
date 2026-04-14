//! Billing session extension and tier upgrade.
//!
//! Extracted from billing_session_lifecycle.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::BillingSessionStatus;

use crate::state::AppState;

/// FATM-07: Atomic extension — wallet debit + time addition in single DB transaction.
/// Returns Ok(()) on success. Returns Err with reason on insufficient balance, session not found, or DB failure.
/// In-memory timer is updated ONLY after successful DB commit.
pub async fn extend_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    additional_seconds: u32,
) -> Result<(), String> {
    // Phase 1: Snapshot timer data without holding lock across .await (standing rule: no RwLock across .await)
    let (pod_id_opt, extension_cost_paise, driving_seconds_snapshot, timer_status) = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let timers = state.billing.active_timers.read().await;

        let entry: Option<(String, i64, u32, BillingSessionStatus)> = timers
            .iter()
            .find(|(_, t)| t.session_id == session_id)
            .map(|(k, t)| {
                let current_cost = t.current_cost(&rate_tiers);
                let ext_rate = current_cost.rate_per_min_paise;
                let cost = (ext_rate * additional_seconds as i64 + 30) / 60;
                (k.clone(), cost, t.driving_seconds, t.status.clone())
            });
        (
            entry.as_ref().map(|(k, _, _, _)| k.clone()),
            entry.as_ref().map(|(_, c, _, _)| *c).unwrap_or(0),
            entry.as_ref().map(|(_, _, d, _)| *d).unwrap_or(0),
            entry.map(|(_, _, _, s)| s),
        )
    }; // rate_tiers and timers guards both dropped here

    let pod_id = match pod_id_opt {
        Some(p) => p,
        None => return Err(format!("Session '{}' not found in active timers", session_id)),
    };

    // BILL-04: Validate session is active before extending
    match timer_status.as_ref() {
        Some(BillingSessionStatus::Completed)
        | Some(BillingSessionStatus::EndedEarly)
        | Some(BillingSessionStatus::Cancelled)
        | Some(BillingSessionStatus::CancelledNoPlayable) => {
            let msg = format!(
                "BILL-04: Extension attempt on non-active session {} (status={:?}) — rejected",
                session_id, timer_status
            );
            tracing::warn!("{}", msg);
            return Err(msg);
        }
        _ => {}
    }

    // Look up driver_id for wallet debit (DB read before transaction)
    let driver_id = sqlx::query_as::<_, (String,)>(
        "SELECT driver_id FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error looking up session: {}", e))?
    .map(|(d,)| d)
    .ok_or_else(|| format!("Session '{}' not found in DB", session_id))?;

    tracing::info!(
        "BILL-04: Extension uses rate {}p/min for {} seconds (extension_rate_policy=current_tier_effective_rate, cost={}p)",
        if additional_seconds > 0 { extension_cost_paise * 60 / additional_seconds as i64 } else { 0 },
        additional_seconds, extension_cost_paise
    );

    // FATM-07: Begin single transaction — wallet debit + allocated_seconds update
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error starting extension transaction: {}", e))?;

    // Step 1: Debit wallet within transaction (FATM-07)
    if extension_cost_paise > 0 {
        let debit_result: Result<(i64, String), String> = crate::wallet::debit_in_tx(
            &mut tx,
            &driver_id,
            extension_cost_paise,
            "extension",
            Some(session_id),
            Some(&format!("Extension {}s", additional_seconds)),
            None,
            &state.config.venue.venue_id,
        )
        .await;
        if let Err(e) = debit_result {
            // tx dropped here, rolls back automatically
            return Err(format!("Insufficient balance for extension: {}", e));
        }
    }

    // Step 2: Update allocated_seconds in SAME transaction (FATM-07)
    sqlx::query(
        "UPDATE billing_sessions SET allocated_seconds = allocated_seconds + ? WHERE id = ?",
    )
    .bind(additional_seconds as i64)
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error updating allocated_seconds: {}", e))?;

    // Step 3: Log extension event in SAME transaction
    let metadata = serde_json::json!({
        "extended_by_seconds": additional_seconds,
        "extension_cost_paise": extension_cost_paise,
    });
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
         VALUES (?, ?, 'extended', ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(driving_seconds_snapshot as i64)
    .bind(metadata.to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    // FATM-07: Commit — if commit fails, BOTH debit and time addition roll back atomically
    tx.commit().await
        .map_err(|e| format!("DB commit failed for extension: {}", e))?;

    // RESIL-05: Post-debit negative wallet balance check (NON-BLOCKING).
    // Read balance AFTER commit (lock already dropped). Alert staff if negative.
    // This check does NOT affect the ongoing session — it is alert-only.
    if extension_cost_paise > 0 {
        let balance_row: Option<(i64,)> = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?"
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((balance,)) = balance_row {
            if balance < 0 {
                tracing::error!(
                    "RESIL-05: Negative wallet balance detected: driver={}, balance={}",
                    driver_id, balance
                );
                let msg = format!(
                    "[BILLING ALERT] Negative wallet balance detected! Driver: {}, Balance: {} paise. {}",
                    driver_id, balance, crate::whatsapp_alerter::ist_now_string()
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // Phase 2: ONLY after successful commit, update in-memory timer
    // Re-acquire write lock to update in-memory state
    let info = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.allocated_seconds += additional_seconds;
            // Reset warnings if we extended past thresholds
            if timer.remaining_seconds() > 300 {
                timer.warning_5min_sent = false;
            }
            if timer.remaining_seconds() > 60 {
                timer.warning_1min_sent = false;
            }
            Some(timer.to_info(&rate_tiers))
        } else {
            None
        }
    }; // rate_tiers and timers guards dropped here

    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));
    }

    tracing::info!(
        "FATM-07: Billing session {} extended by {} seconds (cost={}p, atomic debit+time committed)",
        session_id, additional_seconds, extension_cost_paise
    );

    Ok(())
}

/// Act 2: Upgrade a package billing session to a higher tier (e.g. 30min -> 60min).
/// Only allows upgrading to a tier with longer duration. Charges the price difference only.
/// Per-minute sessions cannot be upgraded to packages (and vice versa).
pub async fn upgrade_billing_tier(
    state: &Arc<AppState>,
    session_id: &str,
    new_tier_id: &str,
) -> Result<(), String> {
    // Look up current session
    let session = sqlx::query_as::<_, (String, String, String, i64, i64, String)>(
        "SELECT bs.id, bs.driver_id, bs.pricing_tier_id, bs.allocated_seconds, bs.wallet_debit_paise, \
         COALESCE(bs.billing_mode, 'package') \
         FROM billing_sessions bs WHERE bs.id = ? AND bs.status IN ('active', 'paused_manual', 'paused_game_pause', 'waiting_for_game')",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Active session '{}' not found", session_id))?;

    let (_sid, driver_id, current_tier_id, current_allocated, current_debit, billing_mode) = session;

    // Only package sessions can be upgraded
    if billing_mode != "package" {
        return Err("Per-minute sessions cannot be upgraded to a package tier".to_string());
    }

    // Look up new tier
    let new_tier = sqlx::query_as::<_, (String, i64, i64, String)>(
        "SELECT name, duration_minutes, price_paise, COALESCE(billing_mode, 'package') FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(new_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Tier '{}' not found or inactive", new_tier_id))?;

    let (new_tier_name, new_duration_min, new_price_paise, new_billing_mode) = new_tier;

    // New tier must also be a package
    if new_billing_mode != "package" {
        return Err("Cannot upgrade to a per-minute tier".to_string());
    }

    // New tier must have longer duration (upgrade only, no downgrade)
    let new_allocated = new_duration_min * 60;
    if new_allocated <= current_allocated {
        return Err(format!(
            "New tier '{}' ({}min) is not longer than current ({}min) — upgrade only",
            new_tier_name, new_duration_min, current_allocated / 60
        ));
    }

    // Charge the difference only
    let difference_paise = new_price_paise - current_debit;
    if difference_paise < 0 {
        return Err("New tier is cheaper — use refund instead".to_string());
    }

    // Resolve wallet owner (linked racers)
    let wallet_owner = crate::wallet::resolve_wallet_owner(state, &driver_id).await?;

    // Atomic transaction: debit wallet + update session
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error: {}", e))?;

    if difference_paise > 0 {
        crate::wallet::debit_in_tx(
            &mut tx,
            &wallet_owner,
            difference_paise,
            "tier_upgrade",
            Some(session_id),
            Some(&format!("Upgrade to {}", new_tier_name)),
            None,
            &state.config.venue.venue_id,
        )
        .await
        .map_err(|e| format!("Insufficient balance for upgrade: {}", e))?;
    }

    sqlx::query(
        "UPDATE billing_sessions SET pricing_tier_id = ?, allocated_seconds = ?, wallet_debit_paise = ? WHERE id = ?",
    )
    .bind(new_tier_id)
    .bind(new_allocated)
    .bind(new_price_paise)
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error updating session: {}", e))?;

    // Log upgrade event
    let metadata = serde_json::json!({
        "from_tier": current_tier_id,
        "to_tier": new_tier_id,
        "difference_paise": difference_paise,
        "new_allocated_seconds": new_allocated,
    });
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, metadata, venue_id)
         VALUES (?, ?, 'tier_upgrade', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(metadata.to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    tx.commit().await.map_err(|e| format!("DB commit failed: {}", e))?;

    // Update in-memory timer
    let info = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let mut timers = state.billing.active_timers.write().await;
        let pod_id = timers.iter().find(|(_, t)| t.session_id == session_id).map(|(k, _)| k.clone());
        if let Some(pod_id) = pod_id {
            if let Some(timer) = timers.get_mut(&pod_id) {
                timer.allocated_seconds = new_allocated as u32;
                timer.warning_5min_sent = false;
                timer.warning_1min_sent = false;
                Some(timer.to_info(&rate_tiers))
            } else { None }
        } else { None }
    };

    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));
    }

    tracing::info!(
        "Tier upgrade: session {} from {} to {} (difference={}p, new_allocated={}s)",
        session_id, current_tier_id, new_tier_id, difference_paise, new_allocated
    );

    Ok(())
}
