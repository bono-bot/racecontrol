//! Billing recovery — agent shutdown, interrupted sessions, lap rejection, grace hydration.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! DEPLOY-02 (agent shutdown), DEPLOY-04 (interrupted sessions),
//! GLD-C-04 (lap rejection + grace window hydration).

use std::sync::Arc;

use rc_common::types::BillingSessionStatus;

use crate::billing::{BillingManager, end_billing_session_public};
use crate::billing_pricing::compute_refund;
use crate::state::AppState;

/// DEPLOY-02: Handle agent graceful shutdown notification.
/// Called by the pod agent during its shutdown sequence when a billing session is active.
/// Ends the session with EndedEarly status so the partial refund logic fires.
/// This endpoint is idempotent — if the session was already ended, returns Ok with refund_paise=0.
/// The endpoint is in public_routes, gated by the agent service key header.
pub async fn handle_agent_shutdown(
    state: &Arc<AppState>,
    session_id: &str,
    pod_id: &str,
    shutdown_reason: &str,
) -> serde_json::Value {
    tracing::info!(
        "DEPLOY-02: Agent shutdown for session {} (pod={}, reason={})",
        session_id, pod_id, shutdown_reason
    );

    // Record shutdown_at timestamp (idempotent — only sets if NULL, since session may already be ended)
    let _ = sqlx::query(
        "UPDATE billing_sessions SET shutdown_at = datetime('now') WHERE id = ? AND shutdown_at IS NULL AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')"
    )
    .bind(session_id)
    .execute(&state.db)
    .await;

    // Fetch current wallet debit before end for refund calculation
    let wallet_info = sqlx::query_as::<_, (String, i64, i64, Option<i64>)>(
        "SELECT driver_id, allocated_seconds, COALESCE(driving_seconds, 0), wallet_debit_paise FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let ended = end_billing_session_public(
        state,
        session_id,
        BillingSessionStatus::EndedEarly,
        Some(&format!("agent_shutdown:{}", shutdown_reason)),
    )
    .await;

    if !ended {
        // Session was already ended (idempotent — return 409 body with ended indicator)
        return serde_json::json!({ "status": "already_ended", "refund_paise": 0 });
    }

    // Calculate approximate refund for response (actual credit applied in end_billing_session)
    let refund_paise = if let Some((_driver_id, allocated, driving, Some(debit))) = wallet_info {
        compute_refund(allocated, driving, debit)
    } else {
        0
    };

    serde_json::json!({
        "status": "ended",
        "session_id": session_id,
        "pod_id": pod_id,
        "refund_paise": refund_paise,
        "ended_at": chrono::Utc::now().to_rfc3339(),
    })
}

/// DEPLOY-04: Check for interrupted sessions for a given pod.
/// Called by rc-agent on startup to detect and clean up sessions that appear interrupted
/// (shutdown_at is set but no ended_at, or still active with a stale last heartbeat).
/// Auto-ends any such sessions so the customer receives a partial refund.
pub async fn handle_interrupted_sessions_check(
    state: &Arc<AppState>,
    pod_id: &str,
) -> serde_json::Value {
    // Find sessions that were interrupted: shutdown_at set but still active/paused
    let interrupted = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, driver_id, COALESCE(driving_seconds, 0) FROM billing_sessions \
         WHERE pod_id = ? AND shutdown_at IS NOT NULL \
         AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')"
    )
    .bind(pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut ended_sessions = Vec::new();
    for (session_id, _driver_id, _driving_seconds) in interrupted {
        let ended = end_billing_session_public(
            state,
            &session_id,
            BillingSessionStatus::EndedEarly,
            Some("interrupted_session_recovery"),
        )
        .await;
        if ended {
            tracing::info!("DEPLOY-04: Auto-ended interrupted session {} for pod {}", session_id, pod_id);
            ended_sessions.push(session_id);
        }
    }

    serde_json::json!({
        "pod_id": pod_id,
        "ended_sessions": ended_sessions,
        "count": ended_sessions.len(),
    })
}

/// GLD-C-04 D-12: Record a lap rejection in the lap_rejections table with grace-window awareness.
///
/// Called when a lap is invalidated/rejected post-session. Computes `grace_window_caught`
/// by checking whether the associated billing session's grace window is still active.
///
/// Column name is `session_id` per CONTEXT.md D-12 (holds billing_session_id at runtime,
/// consistent with laps.session_id).
pub async fn record_lap_rejection(
    state: &std::sync::Arc<crate::state::AppState>,
    billing_session_id: &str,
    lap_number: u32,
    reason: &str,
) {
    // Compute grace_window_caught by checking active_timers (lock dropped before .await)
    let grace_window_caught: bool = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .find(|t| t.session_id == billing_session_id)
            .and_then(|t| t.lap_reject_grace_until)
            .map(|grace_until| chrono::Utc::now() < grace_until)
            .unwrap_or(false)
    }; // guard dropped

    let rejection_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO lap_rejections (id, session_id, lap_number, reason, grace_window_caught)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&rejection_id)
    .bind(billing_session_id)
    .bind(lap_number as i64)
    .bind(reason)
    .bind(grace_window_caught)
    .execute(&state.db)
    .await;

    if grace_window_caught {
        tracing::info!(
            session_id = %billing_session_id,
            lap_number,
            "GLD-C-04 D-12: lap reject caught within grace window — logged to lap_rejections"
        );
    } else {
        tracing::info!(
            session_id = %billing_session_id,
            lap_number,
            "GLD-C-04 D-12: lap reject outside grace window — logged to lap_rejections"
        );
    }
}

/// GLD-C-04 / Phase 363 (P0-3 fix): Patch grace-window fields onto existing timers.
///
/// MUST run AFTER `recover_active_sessions()`. recover populates all 30+ BillingTimer
/// fields correctly (driver_id, driving_seconds, rate, status, etc.). This function
/// only patches `lap_reject_grace_until` + `pending_end_status` on timers that are
/// already present in `active_timers`.
///
/// Original `hydrate_active_timers_from_db` (pre-P0-3) created full BillingTimer
/// instances via `..Default::default()`, which left 25+ fields zeroed/empty. That was
/// then clobbered by recover_active_sessions running second and clearing grace fields.
/// See .planning/audits/PHASE-363-MMA-SUMMARY-2026-04-10.md P0-3 for full root cause.
///
/// For sessions with `lap_reject_grace_until IS NOT NULL` that were NOT picked up by
/// recover (e.g., terminal status rows where grace was set at crash time), we clear
/// the stale grace column in DB so it doesn't confuse future restarts.
pub async fn hydrate_grace_fields_from_db(
    billing: &BillingManager,
    pool: &sqlx::SqlitePool,
) -> anyhow::Result<()> {
    // Fetch only rows with pending grace windows — recover already handled the rest.
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, pod_id, lap_reject_grace_until
         FROM billing_sessions
         WHERE lap_reject_grace_until IS NOT NULL"
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        tracing::info!("GLD-C-04: no pending grace windows found on startup");
        return Ok(());
    }

    let mut patched = 0u32;
    let mut cleared_stale = 0u32;

    // Collect stale session IDs outside the lock to avoid holding lock across .await
    let mut stale_session_ids: Vec<String> = Vec::new();

    {
        let mut timers = billing.active_timers.write().await;
        for (sid, pod_id, grace_str) in &rows {
            let grace_until = chrono::DateTime::parse_from_rfc3339(grace_str)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc));

            if let Some(grace_until) = grace_until {
                if let Some(timer) = timers.get_mut(pod_id) {
                    // Timer exists (recover populated it) — patch grace fields only.
                    timer.lap_reject_grace_until = Some(grace_until);
                    // Conservative default: Completed. The actual end_status was not persisted
                    // (tracked as P1 in MMA audit — add pending_end_status column in future).
                    timer.pending_end_status = Some(BillingSessionStatus::Completed);
                    patched += 1;
                    tracing::info!(
                        session_id = %sid, pod_id = %pod_id,
                        "GLD-C-04: patched grace window onto recovered timer"
                    );
                } else {
                    // Timer NOT in active_timers — recover didn't pick it up. This means
                    // the session reached a status recover doesn't handle (e.g., terminal)
                    // while a grace window was still set. Clear the stale grace column.
                    stale_session_ids.push(sid.clone());
                    cleared_stale += 1;
                }
            }
        }
    } // write guard dropped before any .await

    // Clear stale grace columns outside the lock
    for sid in &stale_session_ids {
        if let Err(e) = sqlx::query(
            "UPDATE billing_sessions SET lap_reject_grace_until = NULL WHERE id = ?"
        )
        .bind(sid)
        .execute(pool)
        .await
        {
            tracing::warn!(session_id = %sid, error = %e, "failed to clear stale grace column");
        } else {
            tracing::info!(session_id = %sid, "GLD-C-04: cleared stale grace column (session not in active_timers)");
        }
    }

    tracing::info!(patched, cleared_stale, "GLD-C-04: grace field hydration complete");
    Ok(())
}
