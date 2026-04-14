//! Billing timeout handlers — pause timeout, offline auto-end, launch timeout.
//!
//! Extracted from billing_timer_expiry.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use crate::activity_log::log_pod_activity;
use crate::billing_pricing::{compute_refund, compute_per_minute_refund};
use crate::state::AppState;

/// Handle pause timeout auto-end with partial refund.
pub(crate) async fn handle_pause_timeouts(
    state: &Arc<AppState>,
    pause_timeout_end: Vec<(String, String, u32, String)>,
) {
    for (pod_id, session_id, driving_seconds, driver_id) in pause_timeout_end {
        log_pod_activity(state, &pod_id, "billing", "Session Auto-Ended",
            "Disconnect pause timeout (10min) — auto-ended with partial refund", "race_engineer", Some(&session_id));

        // Calculate partial refund
        let session_info = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
            "SELECT allocated_seconds, wallet_debit_paise, wallet_owner_id, \
             COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let mut refund_paise: i64 = 0;
        if let Some((allocated, Some(debit), wallet_owner, billing_mode, total_debited, rate_per_min)) = session_info {
            refund_paise = if billing_mode == "per_minute" {
                compute_per_minute_refund(debit, total_debited.unwrap_or(0), rate_per_min.unwrap_or(2500), driving_seconds as i64)
            } else {
                compute_refund(allocated, driving_seconds as i64, debit)
            };
            if refund_paise > 0 {
                let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                // L2-01 fix: handle refund failure explicitly (not let _ =)
                match crate::wallet::refund(
                    state,
                    refund_target,
                    refund_paise,
                    Some(&session_id),
                    Some("Auto-refund: disconnect pause timeout"),
                )
                .await
                {
                    Ok(_) => tracing::info!("BILLING: disconnect timeout refund {}p for session {}", refund_paise, session_id),
                    Err(e) => tracing::error!("CRITICAL: disconnect timeout refund FAILED for session {} ({}p): {}", session_id, refund_paise, e),
                }
            }
        }

        // FATM-04: CAS guard — only update if session is still active/paused_disconnect
        // Prevents double-refund if end_billing_session also races to close this session
        let cas_result = sqlx::query(
            "UPDATE billing_sessions SET status = 'ended_early', driving_seconds = ?, ended_at = datetime('now'),
             refund_paise = ?, notes = 'Auto-ended: disconnect pause timeout (10min)'
             WHERE id = ? AND status IN ('active', 'paused_disconnect')",
        )
        .bind(driving_seconds as i64)
        .bind(refund_paise)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        match cas_result {
            Ok(result) if result.rows_affected() == 0 => {
                tracing::warn!("BILLING: CAS rejected disconnect-timeout end for session {} — already finalized (double-end prevented)", session_id);
            }
            Err(e) => {
                tracing::error!("Failed to update billing session {} on disconnect timeout: {}", session_id, e);
            }
            _ => {}
        }

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'pause_timeout_ended', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .bind(format!("{{\"refund_paise\":{}}}", refund_paise))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        // Clear pod billing reference and restore idle state
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&pod_id) {
                pod.billing_session_id = None;
                pod.current_driver = None;
                pod.status = rc_common::types::PodStatus::Idle;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }

        // Notify agent: session ended — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(&pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::HidePauseOverlay {
                session_id: session_id.clone(),
            })).await;
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                    billing_session_id: session_id.clone(),
                    driver_name: "".to_string(), // Name not needed for timeout end
                    total_laps: 0,
                    best_lap_ms: None,
                    driving_seconds,
                }))
                .await;
        }

        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id,
            pod_id,
            remaining_seconds: 0,
        });
    }
}

/// H11: Auto-end sessions where pod is offline with all pauses exhausted.
pub(crate) async fn handle_offline_auto_end(
    state: &Arc<AppState>,
    sessions_to_auto_end: Vec<(String, String, String)>,
) {
    for (pod_id, session_id, reason) in sessions_to_auto_end {
        tracing::warn!("Auto-ending session {} on pod {} — {}", session_id, pod_id, reason);
        log_pod_activity(state, &pod_id, "billing", "Session Auto-Ended (Offline)",
            &reason, "race_engineer", Some(&session_id));

        // H11-REFUND: Calculate partial refund (same as pause_timeout path)
        let session_info = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
            "SELECT allocated_seconds, driving_seconds, wallet_debit_paise, wallet_owner_id, \
             COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let mut refund_paise: i64 = 0;
        if let Some((allocated, driving_seconds, Some(debit), wallet_owner, billing_mode, total_debited, rate_per_min)) = session_info {
            refund_paise = if billing_mode == "per_minute" {
                compute_per_minute_refund(debit, total_debited.unwrap_or(0), rate_per_min.unwrap_or(2500), driving_seconds)
            } else {
                compute_refund(allocated, driving_seconds, debit)
            };
            if refund_paise > 0 {
                let driver_id_row = sqlx::query_as::<_, (String,)>(
                    "SELECT driver_id FROM billing_sessions WHERE id = ?",
                )
                .bind(&session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                let driver_id = driver_id_row.map(|(d,)| d).unwrap_or_default();
                let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                match crate::wallet::refund(
                    state,
                    refund_target,
                    refund_paise,
                    Some(&session_id),
                    Some("Auto-refund: offline auto-end (H11)"),
                )
                .await
                {
                    Ok(_) => tracing::info!("BILLING: H11 offline refund {}p for session {}", refund_paise, session_id),
                    Err(e) => tracing::error!("CRITICAL: H11 offline refund FAILED for session {} ({}p): {}", session_id, refund_paise, e),
                }
            }
        }

        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'ended_early', ended_at = datetime('now'),
             refund_paise = ?, notes = ? WHERE id = ? AND status IN ('active', 'paused_disconnect')",
        )
        .bind(refund_paise)
        .bind(&reason)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'offline_auto_ended', 0, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(format!("{{\"reason\":\"{}\",\"refund_paise\":{}}}", reason.replace('"', "\\\""), refund_paise))
        .execute(&state.db)
        .await;

        // Remove the timer
        {
            let mut timers = state.billing.active_timers.write().await;
            timers.remove(&pod_id);
        }

        // Reset pod state
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&pod_id) {
                pod.billing_session_id = None;
                pod.current_driver = None;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }

        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id,
            pod_id,
            remaining_seconds: 0,
        });
    }
}

/// Handle launch timeout — retry on first attempt, cancel with refund on second.
pub(crate) async fn handle_launch_timeouts(
    state: &Arc<AppState>,
    timed_out: Vec<(String, u8)>,
) {
    for (pod_id, attempt) in timed_out {
        if attempt == 1 {
            // First timeout: reset to attempt 2 and allow another 3 minutes.
            // CRITICAL: acquire write lock in a tight block, snapshot retry data, then drop.
            // Previous code held the write lock alive when acquiring a read lock on the same
            // RwLock — tokio::sync::RwLock is not re-entrant, causing a deadlock that froze
            // the entire billing tick loop.
            let (retry_sim, retry_args) = {
                let mut waiting = state.billing.waiting_for_game.write().await;
                if let Some(entry) = waiting.get_mut(&pod_id) {
                    tracing::warn!(
                        "Launch timeout (attempt 1) for pod {} — allowing retry (attempt 2)",
                        pod_id
                    );
                    entry.attempt = 2;
                    entry.waiting_since = std::time::Instant::now();
                    // Snapshot retry data while we have the lock
                    (
                        entry.sim_type.unwrap_or(rc_common::types::SimType::AssettoCorsa),
                        entry.launch_args.clone(),
                    )
                } else {
                    (rc_common::types::SimType::AssettoCorsa, None)
                }
                // write lock dropped here
            };
            log_pod_activity(state, &pod_id, "billing", "Launch Timeout",
                "AC failed to reach LIVE in 3 min — retry allowed", "race_engineer", None);
            // The agent-side LaunchState machine handles the actual retry
            // Send LaunchGame again with the ORIGINAL sim_type and args (not hardcoded AC)
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                    sim_type: retry_sim,
                    launch_args: retry_args,
                    force_clean: false,
                    duration_minutes: None,
                    launch_id: None,
                })).await;
            }
        } else {
            // Second timeout: cancel with no charge.
            // CRITICAL: Remove entry and drop write lock immediately — never hold across .await.
            // Previous code held waiting_for_game.write() across multiple DB queries, wallet
            // credit, and WS sends (~90 lines of async work), blocking ALL billing operations.
            let entry = {
                let mut waiting = state.billing.waiting_for_game.write().await;
                waiting.remove(&pod_id)
                // write lock dropped here
            };
            tracing::error!(
                "Launch timeout (attempt 2) for pod {} — cancelling session (no charge)",
                pod_id
            );
            log_pod_activity(state, &pod_id, "billing", "Launch Failed",
                "AC failed to reach LIVE after 2 attempts (6 min total) — session cancelled, no charge", "race_engineer", None);

            // BILL-06: Cancel session — handle both pre-committed (BILL-13) and PIN-auth paths
            if let Some(ref timed_out_entry) = entry {
                if let Some(ref pre_data) = timed_out_entry.pre_committed {
                    // BILL-13: Pre-committed session already in DB — UPDATE existing record + refund
                    let _ = sqlx::query(
                        "UPDATE billing_sessions SET status = 'cancelled_no_playable', ended_at = datetime('now'), driving_seconds = 0 WHERE id = ?",
                    )
                    .bind(&pre_data.session_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| tracing::error!("Failed to update cancelled_no_playable for session {}: {}", pre_data.session_id, e));
                    // Refund wallet debit
                    let debit_row: Option<(i64, Option<String>)> = sqlx::query_as(
                        "SELECT wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
                    )
                    .bind(&pre_data.session_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten();
                    if let Some((debit_paise, wallet_owner)) = debit_row {
                        if debit_paise > 0 {
                            let refund_target = wallet_owner.as_deref().unwrap_or(&timed_out_entry.driver_id);
                            match crate::wallet::credit(
                                state, refund_target, debit_paise, "refund_session",
                                Some(&pre_data.session_id),
                                Some("Auto-refund: launch timeout (game never reached playable state)"),
                                None,
                            ).await {
                                Ok(_) => tracing::info!(
                                    "Launch timeout refund: {}p for session {} (pod={}, driver={})",
                                    debit_paise, pre_data.session_id, timed_out_entry.pod_id, timed_out_entry.driver_id
                                ),
                                Err(e) => tracing::error!(
                                    "Launch timeout refund FAILED: {}p for session {}: {}",
                                    debit_paise, pre_data.session_id, e
                                ),
                            }
                        }
                    }
                } else {
                    // PIN auth path — no DB record exists yet, create cancelled_no_playable record
                    let session_id = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        "INSERT INTO billing_sessions (id, pod_id, driver_id, pricing_tier_id, allocated_seconds, status, created_at, ended_at, driving_seconds, total_paused_seconds, venue_id)
                         VALUES (?, ?, ?, ?, 0, 'cancelled_no_playable', datetime('now'), datetime('now'), 0, 0, ?)",
                    )
                    .bind(&session_id)
                    .bind(&timed_out_entry.pod_id)
                    .bind(&timed_out_entry.driver_id)
                    .bind(&timed_out_entry.pricing_tier_id)
                    .bind(&state.config.venue.venue_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| tracing::error!("Failed to insert cancelled_no_playable record (launch timeout): {}", e));
                }
                tracing::warn!(
                    "Session cancelled_no_playable: pod={} driver={} (launch timeout attempt 2)",
                    timed_out_entry.pod_id, timed_out_entry.driver_id
                );
            }

            // Send BillingStopped to agent so it shows session cancelled
            // Snapshot sender — don't hold agent_senders lock across .await
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let billing_session_id = entry
                    .map(|e| format!("deferred-{}", e.pod_id))
                    .unwrap_or_default();
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BillingStopped {
                    billing_session_id,
                })).await;
            }

            // Clear pod state back to idle
            {
                let mut pods = state.pods.write().await;
                if let Some(pod) = pods.get_mut(&pod_id) {
                    pod.billing_session_id = None;
                    pod.current_driver = None;
                    pod.status = rc_common::types::PodStatus::Idle;
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }
        }
    }
}
