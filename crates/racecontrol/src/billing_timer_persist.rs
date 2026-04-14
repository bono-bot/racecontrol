//! Billing timer persistence — periodic DB sync of active timer state.
//!
//! Extracted from billing_timer.rs (Phase 385, v49.0 Architecture Completion).
//! Contains sync_timers_to_db (5s interval) and persist_timer_state (staggered per-pod).

use std::sync::Arc;

use rc_common::types::BillingSessionStatus;

use crate::state::AppState;

/// Called every 5 seconds to persist driving_seconds to database
pub async fn sync_timers_to_db(state: &Arc<AppState>) {
    // MMA-P2: Snapshot timer data under lock, then release lock before DB writes.
    // This prevents the read lock from blocking tick_all_timers during DB contention.
    let snapshots: Vec<(String, BillingSessionStatus, u32, u32)> = {
        let timers = state.billing.active_timers.read().await;
        timers.values()
            .filter(|t| matches!(t.status,
                BillingSessionStatus::Active
                | BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
                | BillingSessionStatus::PausedCrashRecovery
            ))
            .map(|t| (t.session_id.clone(), t.status, t.driving_seconds, t.total_paused_seconds))
            .collect()
    }; // lock released here

    for (session_id, status, driving_seconds, total_paused_seconds) in &snapshots {
        let result = if *status == BillingSessionStatus::Active
            || *status == BillingSessionStatus::PausedManual
        {
            sqlx::query("UPDATE billing_sessions SET driving_seconds = ? WHERE id = ?")
                .bind(*driving_seconds as i64)
                .bind(session_id)
                .execute(&state.db)
                .await
        } else {
            // PausedDisconnect or PausedGamePause: also persist pause seconds
            sqlx::query("UPDATE billing_sessions SET driving_seconds = ?, total_paused_seconds = ? WHERE id = ?")
                .bind(*driving_seconds as i64)
                .bind(*total_paused_seconds as i64)
                .bind(session_id)
                .execute(&state.db)
                .await
        };
        // MMA-P2: Log SQLITE_BUSY errors instead of silently dropping them
        if let Err(e) = result {
            tracing::warn!("billing sync_to_db failed for session {}: {} — will retry next cycle", session_id, e);
        }
    }
}

/// Persist billing timer elapsed_seconds to DB for a specific pod index.
/// Called by the staggered timer persistence loop — each pod writes at a different
/// second offset within the minute: Pod N writes at second (N * 7) % 60.
/// This prevents all 8 pods from hitting SQLite simultaneously. (RESIL-02)
pub async fn persist_timer_state(state: &Arc<AppState>, target_pod_number: Option<u32>) {
    let now_str = chrono::Utc::now().to_rfc3339();

    // Snapshot timers under lock, then release before any async work (no RwLock across .await)
    let snapshots: Vec<(String, u32, u32, u32, String, u32)> = {
        let timers = state.billing.active_timers.read().await;
        timers.values()
            .filter(|t| matches!(t.status,
                BillingSessionStatus::Active
                | BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
                | BillingSessionStatus::PausedCrashRecovery
            ))
            .filter(|t| {
                // If target_pod_number specified, only persist that pod's timer
                match target_pod_number {
                    Some(n) => {
                        // Extract pod number from pod_id (e.g., "pod_3" -> 3)
                        t.pod_id.trim_start_matches("pod_").parse::<u32>().unwrap_or(0) == n
                    }
                    None => true, // persist all (used for shutdown/emergency)
                }
            })
            .map(|t| (t.session_id.clone(), t.elapsed_seconds, t.driving_seconds, t.total_paused_seconds, t.pod_id.clone(), t.recovery_pause_seconds))
            .collect()
    }; // lock released here

    for (session_id, elapsed, driving, paused, pod_id, recovery_pause) in &snapshots {
        let result = sqlx::query(
            "UPDATE billing_sessions SET elapsed_seconds = ?, driving_seconds = ?, total_paused_seconds = ?, recovery_pause_seconds = ?, last_timer_sync_at = ? WHERE id = ?"
        )
        .bind(*elapsed as i64)
        .bind(*driving as i64)
        .bind(*paused as i64)
        .bind(*recovery_pause as i64)
        .bind(&now_str)
        .bind(session_id)
        .execute(&state.db)
        .await;

        match result {
            Ok(_) => tracing::debug!("Timer persisted for session {} on {}: elapsed={}s", session_id, pod_id, elapsed),
            Err(e) => tracing::warn!("Timer persist failed for session {} on {}: {} — will retry next cycle", session_id, pod_id, e),
        }
    }
}
