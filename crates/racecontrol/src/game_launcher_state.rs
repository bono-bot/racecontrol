//! Game launcher state machine — handle_game_state_update.
//! Extracted from game_launcher.rs (Phase 385). Processes game state transitions
//! reported by rc-agent: Loading -> Running -> Stopped.
//! Handles crash recovery, error taxonomy, auto-relaunch, billing completion.
use std::sync::Arc;
use chrono::Utc;
use crate::{activity_log::log_pod_activity, metrics, state::AppState};
use crate::game_launcher::{GameTracker, is_stop_guarded};
use crate::game_launcher_support::{classify_error_taxonomy, extract_launch_fields, log_game_event, send_staff_launch_alert};
use rc_common::{pod_id::normalize_pod_id, protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent}, types::{BillingSessionStatus, GameLaunchInfo, GameState, SimType}};

/// Called when agent reports a game state update
pub async fn handle_game_state_update(state: &Arc<AppState>, info: GameLaunchInfo) {
    let pod_id_normalized = normalize_pod_id(&info.pod_id).unwrap_or_else(|_| info.pod_id.clone());
    let pod_id = &pod_id_normalized;

    // STOP-GUARD: Suppress zombie non-Idle updates within 10s of StopGame dispatch.
    // Idle updates pass through — they're exactly what we want after a stop.
    if info.game_state != GameState::Idle
        && is_stop_guarded(state, pod_id.as_str(), 10).await
    {
        tracing::info!(
            "STOP-GUARD: dropped zombie {:?} update for pod {} (stop dispatched within last 10s)",
            info.game_state,
            pod_id
        );
        return;
    }

    // Phase 368: Declare outside lock so IssueFixed emission can happen after lock drops.
    let mut playable_launch_id: Option<String> = None;
    // Update in-memory tracker
    {
        let mut games = state.game_launcher.active_games.write().await;
        match info.game_state {
            GameState::Idle => {
                // Game stopped normally — remove tracker
                games.remove(pod_id);
            }
            _ => {
                if let Some(tracker) = games.get_mut(pod_id) {
                    tracker.game_state = info.game_state;
                    tracker.pid = info.pid.or(tracker.pid);
                    tracker.error_message = info.error_message.clone();
                    if info.game_state == GameState::Running && tracker.launched_at.is_none() {
                        tracker.launched_at = Some(Utc::now());
                    }
                    // Phase 282: Record playable_at and ready_delay_ms when game becomes Running
                    // Phase 368: Capture launch_id for IssueFixed emission AFTER lock drops.
                    // lock-and-drop pattern: games guard released at end of this {} block,
                    // BEFORE any .await on launch_state_machine.transition.
                    if info.game_state == GameState::Running && tracker.playable_at.is_none() {
                        let now = Utc::now();
                        tracker.playable_at = Some(now);
                        if let Some(launched) = tracker.launched_at {
                            let delay = now.signed_duration_since(launched).num_milliseconds();
                            tracker.ready_delay_ms = Some(delay);
                            tracing::info!(
                                "Phase 282: pod {} ready_delay_ms={} (launch→playable)",
                                tracker.pod_id, delay
                            );
                        }
                        // Capture launch_id now — games lock will drop at end of outer { } block
                        // before the IssueFixed emission .await below.
                        playable_launch_id = Some(tracker.launch_id.clone());
                    }
                } else {
                    // Agent reported state for a game we don't have tracked — create tracker
                    games.insert(
                        pod_id.to_string(),
                        GameTracker {
                            pod_id: pod_id.to_string(),
                            sim_type: info.sim_type,
                            game_state: info.game_state,
                            pid: info.pid,
                            launched_at: info.launched_at,
                            error_message: info.error_message.clone(),
                            launch_args: None,
                            auto_relaunch_count: 0,
                            externally_tracked: true,
                            dynamic_timeout_secs: None,
                            exit_codes: Vec::new(),
                            max_auto_relaunch: 2,
                            playable_at: None,
                            ready_delay_ms: None,
                            billing_session_id: None,
                            launch_id: uuid::Uuid::new_v4().to_string(),
                        },
                    );
                }
            }
        }
    }
    // games lock dropped here — safe to .await

    // Phase 368 Step E: Emit IssueFixed at playable_at transition (LLS-03).
    // Only fires on first Running event. Terminal-state guard makes this idempotent.
    if let Some(ref lid) = playable_launch_id
        && let Some(card) = state.launch_state_machine.transition(
            lid,
            rc_common::protocol::LaunchState::IssueFixed,
            None,
            None,
            None,
        ).await {
            let _ = state.dashboard_tx.send(
                rc_common::protocol::DashboardEvent::LaunchStatusChanged(card)
            );
            // D-11: Schedule 5-min auto-dismiss
            let sm = state.launch_state_machine.clone();
            let lid_owned = lid.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                sm.dismiss(&lid_owned).await;
            });
        }

    // Update pod info
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(pod_id) {
            pod.game_state = Some(info.game_state);
            pod.current_game = if info.game_state == GameState::Idle {
                None
            } else {
                Some(info.sim_type)
            };
        }
    }

    // Determine event type for logging
    let event_type = match info.game_state {
        GameState::Running => "running",
        GameState::Loading => "loading",
        GameState::Error => "crashed",
        GameState::Idle => "stopped",
        GameState::Launching => "launched",
        GameState::Stopping => "stopping",
        GameState::InLobby => "in_lobby",
    };

    // Log to DB (legacy table)
    log_game_event(
        state,
        pod_id,
        &info.sim_type.to_string(),
        event_type,
        info.pid,
        info.error_message.as_deref(),
    )
    .await;

    // Rich state update event recording (only for Error/Crash states — informational states covered by launch/relaunch)
    if matches!(info.game_state, GameState::Error) {
        let (outcome, taxonomy) = if info.game_state == GameState::Error {
            let tax = classify_error_taxonomy(info.error_message.as_deref(), info.exit_code);
            (metrics::LaunchOutcome::Crash, Some(tax))
        } else {
            (metrics::LaunchOutcome::Error, None)
        };
        let state_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: info.sim_type.to_string(),
            car: None,
            track: None,
            session_type: None,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome,
            error_taxonomy: taxonomy,
            duration_to_playable_ms: None,
            error_details: info.error_message.clone(),
            launch_args_hash: None,
            attempt_number: 1,
            db_fallback: None,
            session_id: None,
        };
        metrics::record_launch_event(&state.db, &state_event, &state.config.venue.venue_id).await;
    }

    // Broadcast to dashboards
    if let Err(e) = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info.clone()))
    {
        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
    }

    // Phase 282: Update launch_events with ready_delay when game becomes Running
    if info.game_state == GameState::Running {
        let ready_delay = {
            let games = state.game_launcher.active_games.read().await;
            games.get(pod_id).and_then(|t| t.ready_delay_ms)
        };
        if let Some(delay_ms) = ready_delay {
            let _ = sqlx::query(
                "UPDATE launch_events SET duration_to_playable_ms = ? WHERE pod_id = ? AND duration_to_playable_ms IS NULL ORDER BY created_at DESC LIMIT 1"
            )
            .bind(delay_ms)
            .bind(pod_id)
            .execute(&state.db)
            .await;
        }
    }

    // ─── AC Timer Sync: reset billing timer on initial game start ────
    // When AC reports Running, reset billing driving_seconds to 0 so both
    // timers (AC practice + billing) start at the same moment. Only applies
    // during initial launch (driving_seconds < 120) — crash relaunches skip.
    if info.game_state == GameState::Running && info.sim_type == SimType::AssettoCorsa {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id)
            && timer.status == BillingSessionStatus::Active && timer.driving_seconds < 120 {
                tracing::info!(
                    "AC timer sync: resetting billing timer for session {} on pod {} (was {}s)",
                    timer.session_id, pod_id, timer.driving_seconds
                );
                timer.driving_seconds = 0;
                timer.started_at = Some(Utc::now());
                // Sync to DB immediately
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET driving_seconds = 0, started_at = ? WHERE id = ?"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(&timer.session_id)
                .execute(&state.db)
                .await;
            }
    }

    // ─── Race Engineer: Auto-relaunch on crash if billing is active ────
    // LAUNCH-17: Atomic check+increment under single write lock (no TOCTOU)
    if info.game_state == GameState::Error {
        // RECOVER-02 SLA tracking: capture crash detection time at start of Error branch
        let crash_detected_at = std::time::Instant::now();
        let current_exit_code = info.exit_code;
        let error_taxonomy = classify_error_taxonomy(info.error_message.as_deref(), current_exit_code);
        let failure_mode_str = format!("{:?}", error_taxonomy);

        let has_billing = state
            .billing
            .active_timers
            .read()
            .await
            .contains_key(pod_id);

        if has_billing {
            // LAUNCH-17: Single write lock — read AND increment atomically to prevent duplicate relaunches
            // from rapid duplicate Error events on the same pod.
            // Also push exit_code and extract car/track in same lock.
            let should_relaunch: Option<(u32, u32, SimType, Option<String>, Option<String>, Option<String>)> = {
                let mut games = state.game_launcher.active_games.write().await;
                if let Some(tracker) = games.get_mut(pod_id) {
                    // RECOVER-05: Accumulate exit codes for staff alert diagnostics
                    tracker.exit_codes.push(current_exit_code);

                    // Extract car/track for enriched recovery events
                    let (car, track, _, _) = extract_launch_fields(&tracker.launch_args);

                    // LAUNCH-16: Guard null args — externally tracked games have no original launch args
                    if tracker.externally_tracked || tracker.launch_args.is_none() {
                        tracing::warn!(
                            "Race Engineer: skipping relaunch for pod {} — original launch args unavailable, please relaunch from kiosk",
                            pod_id
                        );
                        // RECOVER-04: Broadcast dashboard notification for null-args guard
                        let null_info = {
                            let mut null_tracker_info = tracker.to_info();
                            null_tracker_info.error_message = Some(
                                "Cannot auto-relaunch: no launch args. Manual relaunch required from kiosk.".to_string()
                            );
                            null_tracker_info
                        };
                        let _ = state.dashboard_tx.send(DashboardEvent::GameStateChanged(null_info));
                        None
                    } else if tracker.auto_relaunch_count < tracker.max_auto_relaunch {
                        tracker.auto_relaunch_count += 1;
                        let attempt = tracker.auto_relaunch_count;
                        let max_cap = tracker.max_auto_relaunch;
                        Some((attempt, max_cap, tracker.sim_type, tracker.launch_args.clone(), car, track))
                    } else {
                        None // exhausted
                    }
                } else {
                    None // no tracker — don't relaunch
                }
            };

            if let Some((attempt, max_cap, sim_type, launch_args, car, track)) = should_relaunch {
                let pod_id_owned = pod_id.to_string();
                let state_clone = state.clone();
                let sim_name = format!("{}", sim_type);
                let failure_mode_clone = failure_mode_str.clone();
                let car_clone = car.clone();
                let track_clone = track.clone();

                // RECOVER-03: Query historical recovery data for best action
                let (best_action, success_rate) = metrics::query_best_recovery_action(
                    &state.db,
                    pod_id,
                    &sim_name,
                    &failure_mode_str,
                ).await;
                tracing::info!(
                    "recovery action selected: {} ({:.0}% historical success) for pod {} attempt {}/{}",
                    best_action, success_rate * 100.0, pod_id, attempt, max_cap
                );

                let action_label = format!("{}_attempt_{}", best_action, attempt);

                log_pod_activity(
                    state,
                    pod_id,
                    "race_engineer",
                    "Auto-Relaunching Game",
                    &format!("Race Engineer relaunching {} after crash (attempt {}/{}) — action: {}", sim_name, attempt, max_cap, best_action),
                    "race_engineer",
                    None,
                );

                // Delayed relaunch (5s) — verify billing still active + game still in Error
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                    // Re-check billing still active
                    let still_billing = state_clone
                        .billing
                        .active_timers
                        .read()
                        .await
                        .contains_key(pod_id_owned.as_str());

                    let still_error = state_clone
                        .game_launcher
                        .active_games
                        .read()
                        .await
                        .get(&pod_id_owned)
                        .map(|t| t.game_state == GameState::Error)
                        .unwrap_or(false);

                    if still_billing && still_error {
                        tracing::info!(
                            "Race Engineer: relaunching {} on pod {} (attempt {}/2)",
                            sim_name, pod_id_owned, attempt
                        );

                        // BUG-01 fix: Recalculate remaining duration from DB at relaunch time.
                        // The stored launch_args contain the duration from ORIGINAL launch — stale
                        // after crash + elapsed time. Without this, AC sessions can outlast billing.
                        let fresh_duration: Option<u32> = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                            "SELECT allocated_seconds, driving_seconds, split_duration_minutes FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
                        )
                        .bind(&pod_id_owned)
                        .fetch_optional(&state_clone.db)
                        .await
                        .ok()
                        .flatten()
                        .map(|(allocated, driven, split_mins)| {
                            if let Some(sm) = split_mins {
                                sm as u32
                            } else {
                                let remaining = (allocated as u32).saturating_sub(driven as u32);
                                remaining.div_ceil(60) // ceiling division
                            }
                        });

                        // Inject fresh duration into launch_args JSON
                        let updated_args = launch_args.map(|args_str| {
                            if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&args_str) {
                                if let Some(dur) = fresh_duration {
                                    parsed["duration_minutes"] = serde_json::json!(dur);
                                }
                                parsed.to_string()
                            } else {
                                args_str
                            }
                        });

                        let senders = state_clone.agent_senders.read().await;
                        if let Some(tx) = senders.get(&pod_id_owned) {
                            let _ = tx
                                .send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                                    sim_type,
                                    launch_args: updated_args,
                                    force_clean: true,
                                    duration_minutes: fresh_duration,
                                    launch_id: None,
                                }))
                                .await;
                        }
                        drop(senders);

                        // RECOVER-02: Record enriched recovery event with actual ErrorTaxonomy,
                        // car/track from launch_args, exit_code in details, and SLA duration
                        let recovery_duration_ms = crash_detected_at.elapsed().as_millis() as i64;
                        let recovery_event = metrics::RecoveryEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            pod_id: pod_id_owned.clone(),
                            sim_type: Some(sim_name.clone()),
                            car: car_clone,
                            track: track_clone,
                            failure_mode: failure_mode_clone,
                            recovery_action_tried: action_label,
                            recovery_outcome: metrics::RecoveryOutcome::Attempted,
                            recovery_duration_ms: Some(recovery_duration_ms),
                            error_details: Some(format!("exit_code: {:?}", current_exit_code)),
                        };
                        metrics::record_recovery_event(&state_clone.db, &recovery_event, &state_clone.config.venue.venue_id).await;
                    }
                });
            } else {
                // Check if exhausted (auto_relaunch_count >= max_auto_relaunch) — send staff alert + pause billing
                let (is_exhausted, tracker_exit_codes, tracker_launch_args) = {
                    let games = state.game_launcher.active_games.read().await;
                    games.get(pod_id).map(|t| (
                        t.auto_relaunch_count >= t.max_auto_relaunch,
                        t.exit_codes.clone(),
                        t.launch_args.clone(),
                    )).unwrap_or((false, Vec::new(), None))
                };

                if is_exhausted {
                    // Extract car/track for enriched exhausted event
                    let (car, track, _, _) = extract_launch_fields(&tracker_launch_args);

                    // LAUNCH-03: All relaunch attempts exhausted — pause billing
                    log_pod_activity(
                        state,
                        pod_id,
                        "race_engineer",
                        "Relaunch Limit Reached",
                        &format!(
                            "Race Engineer: max relaunch attempts (2) reached for {}. Billing paused — staff action required.",
                            info.sim_type
                        ),
                        "race_engineer",
                        None,
                    );

                    // Pause billing so customer doesn't pay for downtime
                    let mut timers = state.billing.active_timers.write().await;
                    if let Some(timer) = timers.get_mut(pod_id)
                        && timer.status == BillingSessionStatus::Active {
                            timer.status = BillingSessionStatus::PausedCrashRecovery;
                            timer.pause_reason = crate::billing::PauseReason::CrashRecovery;
                            timer.last_paused_at = Some(Utc::now());
                            timer.pause_seconds = 0;
                            tracing::info!(
                                "LAUNCH-03: Billing paused (crash recovery) on pod {} — launch failed after max auto-relaunch attempts",
                                pod_id
                            );
                        }
                    drop(timers);

                    // LAUNCH-15: WhatsApp staff alert — notify staff that automation failed
                    // RECOVER-05: Include exit_codes and suggested action in alert
                    let best_action_for_alert = {
                        let sim_name_alert = info.sim_type.to_string();
                        let (action, _) = metrics::query_best_recovery_action(
                            &state.db, pod_id, &sim_name_alert, &failure_mode_str
                        ).await;
                        action
                    };
                    send_staff_launch_alert(
                        state,
                        pod_id,
                        &info.sim_type.to_string(),
                        &failure_mode_str,
                        &tracker_exit_codes,
                        &best_action_for_alert,
                    ).await;

                    // RECOVER-02: Record enriched exhausted recovery event
                    let recovery_duration_ms = crash_detected_at.elapsed().as_millis() as i64;
                    let recovery_event = metrics::RecoveryEvent {
                        id: uuid::Uuid::new_v4().to_string(),
                        pod_id: pod_id.to_string(),
                        sim_type: Some(format!("{}", info.sim_type)),
                        car,
                        track,
                        failure_mode: failure_mode_str,
                        recovery_action_tried: "auto_relaunch_exhausted".to_string(),
                        recovery_outcome: metrics::RecoveryOutcome::Failed,
                        recovery_duration_ms: Some(recovery_duration_ms),
                        error_details: Some(format!(
                            "Max relaunch attempts (2) reached. Billing paused. exit_code: {:?}",
                            current_exit_code
                        )),
                    };
                    metrics::record_recovery_event(&state.db, &recovery_event, &state.config.venue.venue_id).await;
                }
            }
        }
    }
}
