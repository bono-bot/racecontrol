//! Billing session end — end_billing_session and end_billing_session_public.
//!
//! Extracted from billing_session_lifecycle.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::BillingSessionStatus;

use crate::activity_log::log_pod_activity;
use crate::billing_hooks::post_session_hooks;
use crate::billing_multiplayer::check_and_stop_multiplayer_server;
use crate::billing_pricing::{compute_refund, compute_per_minute_refund};
use crate::event_archive;
use crate::state::AppState;

/// Public wrapper for ending billing sessions from API routes
pub async fn end_billing_session_public(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
    end_reason: Option<&str>,
) -> bool {
    let ended = end_billing_session(state, session_id, end_status).await;
    if ended
        && let Some(reason) = end_reason {
            let _ = sqlx::query("UPDATE billing_sessions SET end_reason = ? WHERE id = ?")
                .bind(reason)
                .bind(session_id)
                .execute(&state.db)
                .await;
        }
    ended
}

pub(crate) async fn end_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
) -> bool {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;

    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id
        && let Some(timer) = timers.get_mut(&pod_id) {
            // FSM-01: gate every status mutation through validate_transition
            let event = match end_status {
                BillingSessionStatus::Completed => crate::billing_fsm::BillingEvent::End,
                BillingSessionStatus::EndedEarly => crate::billing_fsm::BillingEvent::EndEarly,
                BillingSessionStatus::Cancelled => crate::billing_fsm::BillingEvent::Cancel,
                BillingSessionStatus::CancelledNoPlayable => crate::billing_fsm::BillingEvent::CancelNoPlayable,
                other => {
                    tracing::error!("BILLING: end_billing_session called with non-terminal status {:?} for session {}", other, session_id);
                    return false;
                }
            };
            match crate::billing_fsm::validate_transition(timer.status, event) {
                Ok(new_status) => {
                    timer.status = new_status;
                }
                Err(e) => {
                    tracing::warn!("BILLING: {}", e);
                    return false;
                }
            }
            let info = timer.to_info(&rate_tiers);
            let driving_seconds = timer.driving_seconds;
            // MMA-P2: If cost calculation fails (None = tier lookup error), log error
            // and use 0 as fallback (customer-favorable). Previously silent.
            let final_cost_paise = match info.cost_paise {
                Some(cost) => cost,
                None => {
                    tracing::error!("BILLING: cost_paise is None for session {} on pod {} — tier lookup may have failed. Using 0 (customer-favorable fallback).", info.id, pod_id);
                    0
                }
            };

            let activity_action = match end_status {
                BillingSessionStatus::EndedEarly => "Session Ended",
                BillingSessionStatus::Cancelled => "Session Cancelled",
                _ => "Session Expired",
            };
            log_pod_activity(state, &pod_id, "billing", activity_action, &format!("{} — {}s driven", info.driver_name, driving_seconds), "core", Some(session_id));
            event_archive::append_event(&state.db, "billing.session_ended", "billing", Some(&pod_id), serde_json::json!({
                "driver_id": info.driver_id,
                "driving_seconds": driving_seconds,
                "end_status": activity_action,
            }), &state.config.venue.venue_id);

            // GLD-C-02: Capture telemetry coverage bucket BEFORE timer removal (D-05).
            // The HashSet is lost after remove — capture its length now.
            let seconds_covered_at_end: u32 = timers
                .get(&pod_id)
                .map(|t| t.telemetry_seconds_covered.len() as u32)
                .unwrap_or(0);

            timers.remove(&pod_id);
            drop(timers);

            // Trigger any pending (deferred) rolling deploy for this pod
            crate::deploy::check_and_trigger_pending_deploy(state, &pod_id).await;

            let event_type = match end_status {
                BillingSessionStatus::EndedEarly => "ended_early",
                BillingSessionStatus::Cancelled => "cancelled",
                _ => "ended",
            };

            let status_str = match end_status {
                BillingSessionStatus::EndedEarly => "ended_early",
                BillingSessionStatus::Cancelled => "cancelled",
                _ => "completed",
            };

            // FATM-04: CAS guard — only update if session is still 'active'.
            // If rows_affected() == 0, the session was already finalized by another
            // concurrent request (e.g. disconnect timeout racing with staff end).
            // In that case, skip ALL downstream work (refund, agent notify, broadcast).
            // NOTE: Do NOT overwrite wallet_debit_paise here — it must retain the original
            // pre-session charge for correct refund calculation downstream (F-05 fix).
            // final_cost_paise is stored in end_reason for audit purposes.
            // CRITICAL-1 fix: CAS must match ALL valid pre-terminal states, not just 'active'.
            // FSM allows End/EndEarly/Cancel from paused_manual, paused_game_pause, paused_disconnect.
            // Previously only matched 'active' — paused sessions were silently dropped with no refund.
            let cas_result = sqlx::query(
                "UPDATE billing_sessions SET status = ?, driving_seconds = ?, ended_at = datetime('now'), end_reason = ? WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
            )
            .bind(status_str)
            .bind(driving_seconds as i64)
            .bind(format!("final_cost_paise:{}", final_cost_paise))
            .bind(session_id)
            .execute(&state.db)
            .await;

            match cas_result {
                Err(e) => {
                    tracing::error!("Failed to update billing session {} to {}: {}", session_id, status_str, e);
                }
                Ok(result) if result.rows_affected() == 0 => {
                    tracing::warn!(
                        "BILLING: CAS rejected end for session {} — already finalized (double-end prevented)",
                        session_id
                    );
                    return false;
                }
                _ => {}
            }

            if let Err(e) = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await
            {
                tracing::error!("Failed to log billing event '{}' for session {}: {}", event_type, session_id, e);
            }

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

            // MULTI-02: Check if this pod was part of a multiplayer group
            check_and_stop_multiplayer_server(state, &pod_id).await;

            // Proportional refund for early end with wallet debit
            if end_status == BillingSessionStatus::EndedEarly {
                let wallet_info = sqlx::query_as::<_, (String, i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
                    "SELECT driver_id, allocated_seconds, wallet_debit_paise, wallet_owner_id, \
                     COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
                     FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, _allocated, Some(debit), wallet_owner, billing_mode, total_debited, _rate_per_min)) = wallet_info {
                    // Unified refund: use snap-to-package pricing for all billing modes
                    let total_charged = total_debited.unwrap_or(debit);
                    let refund_amount = compute_per_minute_refund(total_charged, 0, 0, driving_seconds as i64);
                    if refund_amount > 0 {
                        let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                        let refund_note = "Early end — snap pricing refund";
                        match crate::wallet::refund(
                            state,
                            refund_target,
                            refund_amount,
                            Some(session_id),
                            Some(refund_note),
                        )
                        .await
                        {
                            Ok(_) => tracing::info!("BILLING: early-end refund {}p for session {} (mode={})", refund_amount, session_id, billing_mode),
                            Err(e) => tracing::error!("CRITICAL: early-end refund FAILED for session {} ({}p): {}", session_id, refund_amount, e),
                        }
                    }
                }
            }

            // Full refund for cancelled sessions (never drove)
            if end_status == BillingSessionStatus::Cancelled {
                let wallet_info = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
                    "SELECT driver_id, wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, Some(debit), wallet_owner)) = wallet_info
                    && debit > 0 {
                        let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                        // L2-01 fix: handle refund failure explicitly
                        match crate::wallet::refund(
                            state,
                            refund_target,
                            debit,
                            Some(session_id),
                            Some("Cancelled session — full refund"),
                        )
                        .await
                        {
                            Ok(_) => tracing::info!("BILLING: cancel refund {}p for session {}", debit, session_id),
                            Err(e) => tracing::error!("CRITICAL: cancel refund FAILED for session {} ({}p): {}", session_id, debit, e),
                        }
                    }

                // FATM-09: Restore any coupon reserved for this session back to 'available'
                match crate::api::routes::restore_coupon_on_cancel(&state.db, session_id).await {
                    Ok(_) => tracing::info!(
                        "FATM-09: Coupon restored for cancelled session {}",
                        session_id
                    ),
                    Err(e) => tracing::warn!(
                        "FATM-09: Coupon restore failed for session {} (non-critical): {}",
                        session_id, e
                    ),
                }
            }

            // Notify agent: stop game and show session summary
            let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, &pod_id)
                .await
                .is_some();

            // Snapshot sender to avoid holding lock across .await
            let sender_clone = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender_clone {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;

                if has_reservation && end_status != BillingSessionStatus::Cancelled {
                    let wallet_balance = crate::wallet::get_balance(state, &info.driver_id)
                        .await
                        .unwrap_or(0);
                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SubSessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                            wallet_balance_paise: wallet_balance,
                            current_split_number: info.current_split_number,
                            total_splits: info.split_count,
                        }))
                        .await;
                } else {
                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                        }))
                        .await;

                    // BlankScreen is handled by rc-agent after showing session summary
                }
            }

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info.clone()));

            tracing::info!("Billing session {} ended ({})", session_id, status_str);

            // Post-session hooks (fire-and-forget)
            if end_status != BillingSessionStatus::Cancelled {
                let state_clone = state.clone();
                let session_id_clone = session_id.to_string();
                let driver_id_clone = info.driver_id.clone();
                let pod_id_clone = pod_id.clone();
                tokio::spawn(async move {
                    post_session_hooks(
                        &state_clone,
                        &session_id_clone,
                        &driver_id_clone,
                        seconds_covered_at_end,
                        &pod_id_clone,
                    )
                    .await;
                });
            }
            return true;
        }

    // ─── Fallback: orphaned session in DB but no in-memory timer ─────────
    // This happens when racecontrol restarts while a session was active.
    drop(timers);
    // Match all pre-terminal states (consistent with CRITICAL-1 CAS fix)
    let orphan = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT bs.id, bs.pod_id, COALESCE(d.name, 'Unknown') FROM billing_sessions bs LEFT JOIN drivers d ON bs.driver_id = d.id WHERE bs.id = ? AND bs.status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to check for orphaned billing session {}: {}", session_id, e);
            return false;
        }
    };

    if let Some((sid, pod_id, driver_name)) = orphan {
        tracing::warn!("Force-ending orphaned billing session {} on {} (no in-memory timer)", sid, pod_id);

        let status_str = match end_status {
            BillingSessionStatus::EndedEarly => "ended_early",
            BillingSessionStatus::Cancelled => "cancelled",
            _ => "completed",
        };

        if let Err(e) = sqlx::query(
            "UPDATE billing_sessions SET status = ?, ended_at = datetime('now') WHERE id = ?",
        )
        .bind(status_str)
        .bind(session_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("Failed to end orphaned billing session {}: {}", session_id, e);
        }

        // CRITICAL-3 fix: issue refund for orphaned sessions (previously skipped entirely)
        let refund_info = sqlx::query_as::<_, (String, i64, Option<i64>, Option<i64>, Option<String>)>(
            "SELECT driver_id, allocated_seconds, wallet_debit_paise, driving_seconds, wallet_owner_id FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((driver_id, allocated, Some(debit), driving_secs, wallet_owner)) = refund_info {
            let driven = driving_secs.unwrap_or(0);
            let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
            let refund_amount = if end_status == BillingSessionStatus::Cancelled {
                debit // full refund for cancellation
            } else {
                compute_refund(allocated, driven, debit)
            };
            if refund_amount > 0 {
                match crate::wallet::refund(state, refund_target, refund_amount, Some(session_id),
                    Some("Orphaned session refund after restart")).await {
                    Ok(_) => tracing::info!("BILLING: orphaned session {} refund {}p to {}", session_id, refund_amount, driver_id),
                    Err(e) => tracing::error!("CRITICAL: orphaned session {} refund FAILED for {}: {}", session_id, driver_id, e),
                }
            }
        }

        log_pod_activity(state, &pod_id, "billing", "Orphaned Session Ended", &format!("{} — force-ended after racecontrol restart", driver_name), "race_engineer", Some(session_id));

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

        // MULTI-02: Check if this orphaned pod was part of a multiplayer group
        check_and_stop_multiplayer_server(state, &pod_id).await;

        // Notify agent to deactivate overlay and show blank — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(&pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            // Stop the game first (matches normal end path behavior)
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                billing_session_id: session_id.to_string(),
                driver_name,
                total_laps: 0,
                best_lap_ms: None,
                driving_seconds: 0,
            })).await;
        }

        return true;
    }

    false
}
