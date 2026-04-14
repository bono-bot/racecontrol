//! Billing timer expiry and timeout handlers — post-lock-release deferred work.
//!
//! Extracted from billing_timer.rs (Phase 385, v49.0 Architecture Completion).
//! Contains: expired session processing, warning broadcasts, pause timeout auto-end,
//! H11 offline auto-end, and launch timeout handling.

use std::sync::Arc;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

use crate::activity_log::log_pod_activity;
use crate::billing_multiplayer::check_and_stop_multiplayer_server;
use crate::state::AppState;

/// Send StopGame + SessionEnded/SubSessionEnded to agents for expired sessions.
/// Also clears pod billing references and checks multiplayer group membership.
pub(crate) async fn handle_expired_sessions(
    state: &Arc<AppState>,
    expired_sessions: &[(String, String, u32, String)],
) {
    if expired_sessions.is_empty() {
        return;
    }

    // Log activity for expired sessions
    for (pod_id, _, driving_seconds, driver_name) in expired_sessions {
        log_pod_activity(state, pod_id, "billing", "Session Expired", &format!("{} — {}s driven", driver_name, driving_seconds), "core", None);
    }

    // Snapshot senders to avoid holding lock across .await (standing rule)
    let sender_snapshot: Vec<_> = {
        let agent_senders = state.agent_senders.read().await;
        expired_sessions.iter().filter_map(|(pod_id, _, _, _)| {
            agent_senders.get(pod_id).map(|s| (pod_id.clone(), s.clone()))
        }).collect()
    }; // lock dropped here
    for (pod_id, session_id, driving_seconds, driver_name) in expired_sessions {
        // Check if pod has active reservation (multi-sub-session support)
        let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, pod_id)
            .await
            .is_some();

        if let Some((_, sender)) = sender_snapshot.iter().find(|(id, _)| id == pod_id) {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;

            if has_reservation {
                // Sub-session ended — pod stays reserved, customer picks next race
                let driver_id_for_wallet = sqlx::query_as::<_, (String,)>(
                    "SELECT driver_id FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
                .map(|r| r.0)
                .unwrap_or_default();

                let wallet_balance = crate::wallet::get_balance(state, &driver_id_for_wallet)
                    .await
                    .unwrap_or(0);

                // Look up split info to determine current/total
                let split_info = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
                    "SELECT split_count, reservation_id FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                let (current_split, total_splits) = if let Some((Some(sc), Some(res_id))) = &split_info {
                    let completed = sqlx::query_as::<_, (i64,)>(
                        "SELECT COUNT(*) FROM billing_sessions WHERE reservation_id = ? AND status IN ('completed', 'ended_early')",
                    )
                    .bind(res_id)
                    .fetch_one(&state.db)
                    .await
                    .map(|r| r.0)
                    .unwrap_or(1);
                    (completed as u32, *sc as u32)
                } else {
                    (1, 1)
                };

                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::SubSessionEnded {
                        billing_session_id: session_id.clone(),
                        driver_name: driver_name.clone(),
                        total_laps: 0,
                        best_lap_ms: None,
                        driving_seconds: *driving_seconds,
                        wallet_balance_paise: wallet_balance,
                        current_split_number: current_split,
                        total_splits,
                    }))
                    .await;

                // If this was the last split, end the reservation
                if current_split >= total_splits {
                    if let Some((_, Some(res_id))) = &split_info {
                        let _ = crate::pod_reservation::end_reservation(state, res_id).await;
                        tracing::info!("Last split completed — reservation {} ended", res_id);
                    }
                }
            } else {
                // Full session ended — pod returns to idle
                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                        billing_session_id: session_id.clone(),
                        driver_name: driver_name.clone(),
                        total_laps: 0,
                        best_lap_ms: None,
                        driving_seconds: *driving_seconds,
                    }))
                    .await;

                // BlankScreen is handled by rc-agent after showing session summary
            }
        }

        // Clear pod billing reference
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(pod_id) {
                pod.billing_session_id = None;
                if has_reservation {
                    // Pod stays reserved for next sub-session — keep driver name visible
                } else {
                    pod.current_driver = None;
                    pod.status = rc_common::types::PodStatus::Idle;
                }
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }
    }

    // MULTI-02: Check if any expired pod was part of a multiplayer group
    for (pod_id, _, _, _) in expired_sessions {
        check_and_stop_multiplayer_server(state, pod_id).await;
    }
}

/// Broadcast billing countdown warnings to dashboards and agents, log to DB.
pub(crate) async fn broadcast_warnings(
    state: &Arc<AppState>,
    warnings: Vec<(String, String, u32, u32)>,
) {
    for (session_id, pod_id, remaining, driving_seconds) in warnings {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id.clone(),
            pod_id: pod_id.clone(),
            remaining_seconds: remaining,
        });

        // BILL-02: Send countdown warning to agent for persistent overlay on customer screen
        let level = if remaining <= 60 { "red" } else { "yellow" };
        tracing::info!("BILL-02: Sending {} countdown warning to pod {} ({}s remaining)", level, pod_id, remaining);
        {
            let sender_clone = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender_clone {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BillingCountdownWarning {
                    remaining_secs: remaining,
                    level: level.to_string(),
                })).await;
            }
        } // agent_senders lock dropped

        // Log warning event to DB
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(if remaining <= 60 {
            "warning_1min"
        } else {
            "warning_5min"
        })
        .bind(driving_seconds as i64)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
    }
}

/// Persist expired sessions to DB (mark completed + insert billing event).
pub(crate) async fn persist_expired_sessions(
    state: &Arc<AppState>,
    expired_sessions: Vec<(String, String, u32, String)>,
) {
    for (_, session_id, driving_seconds, _) in expired_sessions {
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'completed', driving_seconds = ?, ended_at = datetime('now')
             WHERE id = ?",
        )
        .bind(driving_seconds as i64)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
             VALUES (?, ?, 'time_expired', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
    }
}

/// Persist new disconnect pauses to DB + broadcast + send overlay to agent.
pub(crate) async fn persist_new_pauses(
    state: &Arc<AppState>,
    new_pauses: &[(String, String, u32)],
) {
    for (pod_id, session_id, pause_count) in new_pauses {
        log_pod_activity(state, pod_id, "billing", "Session Paused (Disconnect)",
            &format!("Pod offline — pause {}/3", pause_count), "race_engineer", Some(session_id));
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'paused_disconnect', pause_count = ?, last_paused_at = datetime('now')
             WHERE id = ?",
        )
        .bind(*pause_count as i64)
        .bind(session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'paused_disconnect', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(0i64) // driving_seconds not incremented during pause
        .bind(format!("{{\"pause_count\":{},\"reason\":\"disconnect\"}}", pause_count))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        // Broadcast SessionPaused to dashboards
        let _ = state.dashboard_tx.send(DashboardEvent::SessionPaused {
            pod_id: pod_id.clone(),
            session_id: session_id.clone(),
            reason: "disconnect".to_string(),
            pause_count: *pause_count,
        });

        // Send ShowPauseOverlay to agent — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ShowPauseOverlay {
                session_id: session_id.clone(),
                remaining_seconds: 600, // max pause duration
                pause_count: *pause_count,
            })).await;
        }
    }
}

// Timeout handlers extracted to billing_timer_expiry_timeout.rs (Phase 385, v49.0).
// Re-exported so callers don't need to change their import paths.
pub(crate) use crate::billing_timer_expiry_timeout::{
    handle_pause_timeouts, handle_offline_auto_end, handle_launch_timeouts,
};
