//! Stale session cleanup — auto-cancels billing sessions stuck in 'pending' or 'waiting_for_game'.
//!
//! Extracted from billing_timer.rs (Phase 385, v49.0 Architecture Completion).
//! Bug #11 + LBILL: game-aware stale cancel with wallet refund.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use rc_common::types::GameState;

use crate::state::AppState;

/// Bug #11 + LBILL: Auto-cancel DB billing sessions stuck in 'pending' or 'waiting_for_game' for > 5 minutes.
/// BILL-13 FIX: Also refund wallet for pre-committed sessions that were debited but never activated.
/// LBILL-01/02/03: Check GameTracker before cancelling waiting_for_game sessions — game-aware stale cancel.
pub(crate) async fn cleanup_stale_sessions(state: &Arc<AppState>) {
    let stale_sessions: Vec<(String, String, Option<i64>, String, String, String, Option<String>)> = match sqlx::query_as(
        "SELECT id, driver_id, wallet_debit_paise, pod_id, created_at, status, wallet_owner_id FROM billing_sessions \
         WHERE status IN ('pending', 'waiting_for_game') \
         AND created_at < datetime('now', '-5 minutes') \
         AND ended_at IS NULL",
    )
    .fetch_all(&state.db)
    .await {
        Ok(rows) => {
            if !rows.is_empty() {
                tracing::info!("LBILL: found {} stale sessions to evaluate", rows.len());
            }
            rows
        }
        Err(e) => {
            tracing::error!("LBILL: DB query failed: {}", e);
            Vec::new()
        }
    };

    if stale_sessions.is_empty() {
        return;
    }

    // LBILL-01: Snapshot active_games — never hold lock across .await
    let game_snapshot: HashMap<String, GameState> = {
        let games = state.game_launcher.active_games.read().await;
        games.iter().map(|(k, v)| (k.clone(), v.game_state)).collect()
    };

    // (session_id, driver_id, wallet_debit_paise, wallet_owner_id, pod_id)
    let mut sessions_to_cancel: Vec<(String, String, Option<i64>, Option<String>, String)> = Vec::new();

    for (session_id, driver_id, wallet_debit_paise, pod_id, created_at_str, status, wallet_owner_id) in &stale_sessions {
        // Parse created_at to compute age
        let created_at = chrono::NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        let age_minutes = created_at
            .map(|ca| (Utc::now() - ca).num_minutes())
            .unwrap_or(99); // Treat unparseable as very old → cancel

        if status == "pending" {
            // LBILL-03: Pending sessions always cancel — no game launched yet
            tracing::info!(
                "LBILL-03: Cancelling stale pending session {} — no game launched yet (pod {}, age {}min)",
                session_id, pod_id, age_minutes
            );
            sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
        } else {
            // status == "waiting_for_game"
            let game_state = game_snapshot.get(pod_id.as_str()).copied();
            let game_alive = matches!(game_state, Some(GameState::Launching)
                | Some(GameState::Loading)
                | Some(GameState::Running));

            if game_alive && age_minutes < 10 {
                // LBILL-02: Game is alive and under 10 min — extend, don't cancel
                tracing::info!(
                    "LBILL-02: Extending stale session {} — game {:?} on pod {} (age {}min, created {})",
                    session_id, game_state, pod_id, age_minutes, created_at_str
                );
                // Skip — do not add to sessions_to_cancel
            } else if game_alive && age_minutes >= 10 {
                // LBILL-02: Absolute timeout — cancel despite game being alive
                tracing::warn!(
                    "LBILL-02: Absolute timeout — cancelling session {} despite game alive on pod {} ({}min)",
                    session_id, pod_id, age_minutes
                );
                sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
            } else {
                // LBILL-03: Game is dead — cancel with refund
                tracing::info!(
                    "LBILL-03: Cancelling stale session {} — no active game on pod {} (game_state={:?}, age {}min)",
                    session_id, pod_id, game_state, age_minutes
                );
                sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
            }
        }
    }

    // Refund wallet for sessions being cancelled (BILL-13 kiosk path)
    for (session_id, driver_id, wallet_debit_paise, wallet_owner, _pod_id) in &sessions_to_cancel {
        if let Some(debit) = wallet_debit_paise
            && *debit > 0 {
                let refund_target = wallet_owner.as_deref().unwrap_or(driver_id.as_str());
                match crate::wallet::credit(
                    state,
                    refund_target,
                    *debit,
                    "refund_session",
                    Some(session_id.as_str()),
                    Some("Auto-refund: session cancelled (game never reached playable state)"),
                    None,
                ).await {
                    Ok(_) => tracing::info!(
                        "Bug #11: Refunded {}p for stale cancelled session {} (driver={})",
                        debit, session_id, driver_id
                    ),
                    Err(e) => tracing::error!(
                        "Bug #11: Failed to refund {}p for stale session {}: {}",
                        debit, session_id, e
                    ),
                }
            }
    }

    // Cancel only the sessions that were not extended
    for (session_id, _, _, _, cancel_pod_id) in &sessions_to_cancel {
        if let Err(e) = sqlx::query(
            "UPDATE billing_sessions SET status = 'cancelled', ended_at = datetime('now') \
             WHERE id = ? AND ended_at IS NULL",
        )
        .bind(session_id)
        .execute(&state.db)
        .await
        {
            tracing::warn!("Failed to auto-cancel stale billing session {}: {}", session_id, e);
        }

        // CRITICAL FIX: Remove entry from in-memory waiting_for_game map
        // Without this, the per-pod billing lock blocks all future billing/start on this pod
        let normalized = cancel_pod_id.replace('-', "_");
        if state.billing.waiting_for_game.write().await.remove(&normalized).is_some() {
            tracing::info!(
                "Cleared waiting_for_game entry for pod {} (session {} auto-cancelled)",
                cancel_pod_id, session_id
            );
        }
    }
}
