//! Game stop operations — extracted from game_launcher_ops.rs (v49.0 Architecture Completion).

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::game_launcher::{LaunchSpanSnapshot};
use crate::game_launcher_support::log_game_event;
use crate::metrics;
use crate::state::AppState;
use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::GameState;

pub async fn stop_game(state: &Arc<AppState>, pod_id: &str) {
    // Normalize pod_id to canonical form at function entry
    let pod_id_owned = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_owned.as_str();

    // STOP-GUARD (2026-04-12): Record stop timestamp BEFORE dispatching the command,
    // so any zombie GameStateUpdate from rc-agent's sim polling loop that arrives
    // after this point gets suppressed in handle_game_state_update. See Issue 4 RCA.
    state
        .game_launcher
        .recent_stops
        .write()
        .await
        .insert(pod_id.to_string(), Instant::now());

    // Update tracker to Stopping — and snapshot it for the launch_timeline_spans record.
    // Capturing here (not inside the ACK block) guarantees we have the full tracker
    // context (launch_id, sim_type, launched_at, playable_at) even if the ACK times out
    // and the tracker is later auto-transitioned to Error by the STATE-01 30s watcher.
    let (info, span_snapshot) = {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.game_state = GameState::Stopping;
            let snap = LaunchSpanSnapshot {
                launch_id: tracker.launch_id.clone(),
                pod_id: tracker.pod_id.clone(),
                sim_type: tracker.sim_type.to_string(),
                billing_session_id: tracker.billing_session_id.clone(),
                launched_at: tracker.launched_at,
                playable_at: tracker.playable_at,
                // Determine outcome: if game had reached playable state, the user
                // got value from the launch (stopped_after_playable). If not, it
                // was a pre-playable cancel (e.g. staff aborted a stuck launch).
                outcome: if tracker.playable_at.is_some() {
                    "stopped_after_playable"
                } else {
                    "stopped_pre_playable"
                }
                .to_string(),
            };
            (Some(tracker.to_info()), Some(snap))
        } else {
            (None, None)
        }
    };

    if let Some(info) = info {
        // LAUNCH-19: Log actual sim_type, not empty string
        log_pod_activity(state, pod_id, "game", "Game Stopping", &info.sim_type.to_string(), "core", None);

        // Phase 312: Send StopGame with ACK correlation (WSCMD-02/03)
        let stop_msg = CoreMessage::wrap(CoreToAgentMessage::StopGame);
        let stop_command_id = stop_msg.command_id.clone().unwrap_or_default();
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
        state.pending_command_acks.write().await.insert(stop_command_id.clone(), ack_tx);

        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(pod_id) {
            if let Err(e) = tx.send(stop_msg).await {
                tracing::error!("Failed to send StopGame to pod {}: {}", pod_id, e);
                state.pending_command_acks.write().await.remove(&stop_command_id);
            }
        } else {
            state.pending_command_acks.write().await.remove(&stop_command_id);
        }
        drop(senders);

        // Wait for ACK (5s timeout, non-blocking for caller — log but don't propagate)
        match tokio::time::timeout(std::time::Duration::from_secs(5), ack_rx).await {
            Ok(Ok(result)) => {
                if result.success {
                    tracing::info!("Stop ACK received for pod {} (command_id={})", pod_id, stop_command_id);
                    // GSTATE-03: On successful ACK, remove tracker immediately (don't wait for agent Idle report)
                    {
                        let mut games = state.game_launcher.active_games.write().await;
                        if let Some(tracker) = games.get(&*pod_id_owned) {
                            if tracker.game_state == GameState::Stopping {
                                games.remove(&*pod_id_owned);
                                tracing::info!("GSTATE-03: Cleared GameTracker for pod {} after successful stop ACK", pod_id);
                            }
                        }
                    }
                    // GSTATE-03: Update pod info to reflect no active game
                    {
                        let mut pods = state.pods.write().await;
                        if let Some(pod) = pods.get_mut(&*pod_id_owned) {
                            pod.game_state = Some(GameState::Idle);
                            pod.current_game = None;
                        }
                    }
                    // STOP-GUARD (2026-04-12): Re-insert the stop timestamp AFTER tracker
                    // cleanup. The initial insert at function entry covers the window
                    // during the 5-second ACK wait; this re-insert restarts the 10s
                    // phantom-prevention window from the ACK point, which is when any
                    // in-flight zombie updates from the agent are MOST likely to arrive
                    // (agent's polling loop had 5+s to queue one up while waiting).
                    state
                        .game_launcher
                        .recent_stops
                        .write()
                        .await
                        .insert(pod_id_owned.to_string(), Instant::now());
                    // LAUNCH-TIMELINE-STOPPED (2026-04-12): Persist a launch_timeline_spans
                    // row for this stopped launch. Uses INSERT OR IGNORE so that if the
                    // agent later sends a LaunchTimelineReport with the same launch_id
                    // (e.g. stop arrived after BillingStarted fired), the agent's row
                    // — which is authoritative, with full event details — is preserved.
                    // This closes the Issue 5 gap where staff-initiated stops of pre-
                    // playable launches were invisible in /launch-timeline/recent.
                    if let Some(ref snap) = span_snapshot {
                        let db = state.db.clone();
                        let snap_clone = LaunchSpanSnapshot {
                            launch_id: snap.launch_id.clone(),
                            pod_id: snap.pod_id.clone(),
                            sim_type: snap.sim_type.clone(),
                            billing_session_id: snap.billing_session_id.clone(),
                            launched_at: snap.launched_at,
                            playable_at: snap.playable_at,
                            outcome: snap.outcome.clone(),
                        };
                        tokio::spawn(async move {
                            // Compute total_duration_ms from launched_at (or 0 if never launched).
                            let started_at_str = snap_clone
                                .launched_at
                                .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
                                .unwrap_or_else(|| {
                                    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
                                });
                            let total_duration_ms: i64 = snap_clone
                                .launched_at
                                .map(|start| {
                                    Utc::now().signed_duration_since(start).num_milliseconds()
                                })
                                .unwrap_or(0)
                                .max(0);
                            // Minimal event trail — a single "stopped" marker with the
                            // playable_at delta (if any) so dashboards can distinguish
                            // pre- vs post-playable stops at a glance.
                            let events_json = {
                                let playable_delta_ms = snap_clone
                                    .playable_at
                                    .and_then(|p| {
                                        snap_clone.launched_at.map(|l| {
                                            p.signed_duration_since(l).num_milliseconds()
                                        })
                                    })
                                    .unwrap_or(-1);
                                format!(
                                    r#"[{{"kind":"stopped","elapsed_ms":{},"playable_delta_ms":{},"timestamp":"{}"}}]"#,
                                    total_duration_ms,
                                    playable_delta_ms,
                                    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                )
                            };
                            let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
                            let result = sqlx::query(
                                "INSERT OR IGNORE INTO launch_timeline_spans
                                 (launch_id, pod_id, sim_type, preset_id, billing_session_id,
                                  outcome, total_duration_ms, started_at, events_json, created_at)
                                 VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?)",
                            )
                            .bind(&snap_clone.launch_id)
                            .bind(&snap_clone.pod_id)
                            .bind(&snap_clone.sim_type)
                            .bind(&snap_clone.billing_session_id)
                            .bind(&snap_clone.outcome)
                            .bind(total_duration_ms)
                            .bind(&started_at_str)
                            .bind(&events_json)
                            .bind(&created_at)
                            .execute(&db)
                            .await;
                            match result {
                                Ok(r) if r.rows_affected() > 0 => tracing::info!(
                                    "LAUNCH-TIMELINE-STOPPED: Persisted span for {} (pod={}, outcome={}, duration={}ms)",
                                    snap_clone.launch_id,
                                    snap_clone.pod_id,
                                    snap_clone.outcome,
                                    total_duration_ms
                                ),
                                Ok(_) => tracing::debug!(
                                    "LAUNCH-TIMELINE-STOPPED: {} already recorded by agent — skipped",
                                    snap_clone.launch_id
                                ),
                                Err(e) => tracing::error!(
                                    "LAUNCH-TIMELINE-STOPPED: Failed to persist span for {}: {}",
                                    snap_clone.launch_id,
                                    e
                                ),
                            }
                        });
                    }
                } else {
                    tracing::warn!("Agent ACK failure for stop on pod {}: {:?}", pod_id, result.error);
                }
            }
            Ok(Err(_)) => {
                tracing::warn!("ACK channel dropped for stop on pod {} (agent disconnected?)", pod_id);
            }
            Err(_) => {
                tracing::warn!("WSCMD-03: No ACK within 5s for stop on pod {} (command_id={})", pod_id, stop_command_id);
                state.pending_command_acks.write().await.remove(&stop_command_id);
            }
        }

        // Broadcast to dashboards
        let sim_type_str = info.sim_type.to_string();
        if let Err(e) = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info))
        {
            tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
        }

        // Log event (legacy table)
        log_game_event(state, pod_id, &sim_type_str, "stopping", None, None).await;

        // Rich stop event recording
        {
            let stop_event = metrics::LaunchEvent {
                id: uuid::Uuid::new_v4().to_string(),
                pod_id: pod_id.to_string(),
                sim_type: sim_type_str,
                car: None,
                track: None,
                session_type: None,
                timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                outcome: metrics::LaunchOutcome::Success,
                error_taxonomy: None,
                duration_to_playable_ms: None,
                error_details: Some("Graceful stop".to_string()),
                launch_args_hash: None,
                attempt_number: 1,
                db_fallback: None,
                session_id: None,
            };
            metrics::record_launch_event(&state.db, &stop_event, &state.config.venue.venue_id).await;
        }

        // STATE-01: Spawn 30s Stopping timeout — auto-transitions to Error if game doesn't stop
        let state_clone = state.clone();
        let pod_id_timeout = pod_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let mut games = state_clone.game_launcher.active_games.write().await;
            if let Some(tracker) = games.get_mut(&pod_id_timeout) {
                if tracker.game_state == GameState::Stopping {
                    tracker.game_state = GameState::Error;
                    tracker.error_message = Some("Stop timed out (30s)".to_string());
                    let info = tracker.to_info();
                    drop(games);
                    if let Err(e) = state_clone.dashboard_tx.send(DashboardEvent::GameStateChanged(info)) {
                        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id_timeout, e);
                    }
                    tracing::warn!("game state: Stopping timed out on pod {}", pod_id_timeout);
                }
            }
        });
    }
}
