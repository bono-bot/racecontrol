//! Game launcher operations — launch, relaunch, and stop game flows.
//!
//! Extracted from game_launcher.rs (Phase 385, v49.0 Architecture Completion).
//! Contains launch_game (468 lines, the core launch orchestration with semaphore,
//! billing gate, and verification loop), relaunch_game, and stop_game.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::catalog;
use crate::game_launcher::{GameTracker, GameManager, GameLauncherImpl, LaunchSpanSnapshot, launcher_for};
use crate::game_launcher_support::{extract_launch_fields, log_game_event};
use crate::metrics;
use crate::state::AppState;
use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{BillingSessionStatus, GameLaunchInfo, GameState, SimType};

pub async fn launch_game(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: SimType,
    launch_args: Option<String>,
) -> Result<(), String> {
    // RESIL-06: Acquire launch semaphore (max 4 concurrent launches) with 30s timeout.
    // Prevents port starvation when 8 pods launch simultaneously.
    // Timeout prevents deadlock if a launch hangs (pod offline mid-launch, WS never returns).
    // Without timeout: 4 hung launches = no more launches possible, entire venue blocked.
    let _permit = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        state.game_launcher.launch_semaphore.acquire(),
    ).await {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) => return Err("Launch semaphore closed".to_string()),
        Err(_) => {
            tracing::warn!("RESIL-06: Launch semaphore timeout (30s) for pod {} — 4 other launches in progress", pod_id);
            return Err("Server busy — too many concurrent game launches. Please wait 30 seconds.".to_string());
        }
    };

    // Normalize pod_id to canonical form (pod_N) at function entry
    let pod_id_owned = normalize_pod_id(pod_id).map_err(|e| format!("Invalid pod ID: {}", e))?;
    let pod_id = pod_id_owned.as_str();

    // LAUNCH-06: Validate launch args via per-game launcher (rejects invalid JSON)
    let launcher = launcher_for(sim_type);
    launcher.validate_args(launch_args.as_deref())
        .map_err(|e| { tracing::warn!("Launch rejected for pod {}: {}", pod_id, e); e })?;

    // Validate launch combo against pod's content manifest (only if JSON parsed OK)
    if let Some(ref args_json) = launch_args {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(args_json) {
            let car = args.get("car").and_then(|v| v.as_str()).unwrap_or("");
            let track = args.get("track").and_then(|v| v.as_str()).unwrap_or("");
            let session_type = args.get("session_type").and_then(|v| v.as_str()).unwrap_or("");
            let manifest = state.pod_manifests.read().await.get(pod_id).cloned();
            if let Err(reason) = catalog::validate_launch_combo(manifest.as_ref(), car, track, session_type) {
                tracing::warn!("Launch rejected for pod {}: {}", pod_id, reason);
                return Err(reason);
            }
        }
    }

    // STATE-03: Feature flag check — game_launch must be enabled (default: enabled)
    {
        let flags = state.feature_flags.read().await;
        let game_launch_enabled = flags.get("game_launch")
            .map(|f| f.enabled)
            .unwrap_or(true); // Default enabled if flag not configured (Pitfall 6 prevention)
        if !game_launch_enabled {
            tracing::warn!("Launch rejected for pod {}: game_launch feature flag disabled", pod_id);
            return Err("game_launch feature disabled".to_string());
        }
    }

    // Phase 368 D-01: Mint launch_id HERE — before any error-return path.
    // This guarantees a launch_id exists for billing-rejection cards AND the success path.
    // Must be above the billing gate so rejection paths can emit NeedsManualIntervention cards.
    let launch_id = uuid::Uuid::new_v4().to_string();

    // LAUNCH-02/03/04 / FSM-03: Billing gate — check active_timers + waiting_for_game, reject paused.
    // FSM-03: Free gaming guard — LaunchGame is rejected if no active billing session exists.
    // TODO: FSM-03 exception for free trials (no trial concept exists yet)
    //
    // Phase 368: Billing-reject paths emit NeedsManualIntervention card (D-15 hard constraint).
    // Lock-and-drop: capture rejection reason under the read lock, then drop locks BEFORE
    // any async card emission (.await). CLAUDE.md: never hold lock across .await.
    let billing_rejection: Option<String> = {
        let timers = state.billing.active_timers.read().await;
        let waiting = state.billing.waiting_for_game.read().await;
        let has_active = timers.contains_key(pod_id);
        let has_deferred = waiting.contains_key(pod_id);
        if !has_active && !has_deferred {
            Some(format!("FSM-03: Pod {} has no active billing session (free gaming guard)", pod_id))
        } else if let Some(timer) = timers.get(pod_id) {
            // LAUNCH-03: Reject paused sessions
            if matches!(timer.status,
                BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
            ) {
                Some(format!("Pod {} billing session is paused", pod_id))
            } else {
                None
            }
        } else {
            None
        }
        // Guards dropped here — safe to .await below
    };
    if let Some(err_msg) = billing_rejection {
        tracing::warn!("Launch rejected for pod {}: {}", pod_id, err_msg);
        // D-15 hard constraint: NeedsManualIntervention card with sanitized detail.
        // NEVER include customer_id, driver name, balance, tier, wallet state in this string.
        let _billing_card = state.launch_state_machine.start_launch(
            launch_id.clone(),
            pod_id.to_string(),
            sim_type.to_string(),
            rc_common::protocol::LaunchOrigin::Customer,
        ).await;
        if let Some(card) = state.launch_state_machine.transition(
            &launch_id,
            rc_common::protocol::LaunchState::NeedsManualIntervention,
            Some("Launch blocked — billing not ready".to_string()),
            None,
            None,
        ).await {
            let _ = state.dashboard_tx.send(
                rc_common::protocol::DashboardEvent::LaunchStatusChanged(card)
            );
        }
        return Err(err_msg);
    }

    // FSM-08: DB-before-launch guard for split sessions.
    // For split 2+ launches, the split record MUST be persisted as 'active' in
    // split_sessions before any launch command is sent to the agent.
    // This prevents orphaned launches with no billing record.
    {
        let (is_split, session_id, split_number) = {
            let timers = state.billing.active_timers.read().await;
            if let Some(timer) = timers.get(pod_id) {
                let is_multi = timer.split_count > 1 && timer.current_split_number > 1;
                (is_multi, timer.session_id.clone(), timer.current_split_number)
            } else {
                (false, String::new(), 0)
            }
        };

        if is_split {
            let split_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM split_sessions \
                 WHERE parent_session_id = ? AND split_number = ? AND status = 'active'",
            )
            .bind(&session_id)
            .bind(split_number as i64)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

            if split_exists != 1 {
                tracing::error!(
                    "FSM-08: Rejecting launch for pod {} — split {} for session {} is not persisted as active in DB (split_exists={})",
                    pod_id, split_number, session_id, split_exists
                );
                return Err(format!(
                    "FSM-08: Cannot launch split {} on pod {} — not persisted in DB (orphaned launch prevention)",
                    split_number, pod_id
                ));
            }
            tracing::info!(
                "FSM-08: Split {} for session {} verified as active in DB — proceeding with launch on pod {}",
                split_number, session_id, pod_id
            );
        }
    }

    // LIFE-04/LAUNCH-05: Check if a game is currently launching, running, or stopping (avoid double-launch)
    {
        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get(pod_id) {
            if matches!(tracker.game_state, GameState::Launching | GameState::Running | GameState::Stopping) {
                let msg = if tracker.game_state == GameState::Stopping {
                    format!("game still stopping on pod {}", pod_id)
                } else {
                    format!("Pod {} already has a game active", pod_id)
                };
                return Err(msg);
            }
        }
    }

    log_pod_activity(state, pod_id, "game", "Game Launching", &format!("{}", sim_type), "core", None);

    // LAUNCH-08: Query dynamic timeout from historical launch data
    let default_timeout_secs: u64 = match sim_type {
        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => 120,
        _ => 90,
    };
    let (car_for_timeout, track_for_timeout, _, _) = extract_launch_fields(&launch_args);
    let dynamic_timeout = metrics::query_dynamic_timeout(
        &state.db,
        &sim_type.to_string(),
        car_for_timeout.as_deref(),
        track_for_timeout.as_deref(),
        default_timeout_secs,
    ).await;

    // INTEL-05: Query combo reliability to set max_auto_relaunch cap.
    // < 50% reliability with >= 5 launches → 3 attempts. Otherwise → 2 (default).
    let reliability = metrics::query_combo_reliability(
        &state.db,
        pod_id,
        &sim_type.to_string(),
        car_for_timeout.as_deref(),
        track_for_timeout.as_deref(),
    ).await;
    let max_relaunch_cap: u32 = match &reliability {
        Some(r) if r.success_rate < 0.50 && r.total_launches >= 5 => 3,
        _ => 2,
    };

    // Phase 368 Step B: Emit LaunchStarted card BEFORE acquiring the tracker write lock.
    // CLAUDE.md rule: no lock held across .await — start_launch uses its own internal lock-and-drop.
    // Derive origin: initial impl defaults to Customer; Plan 04 / follow-up plumbs per-caller origin.
    // Per D-04 LaunchOrigin enum.
    let launch_origin = rc_common::protocol::LaunchOrigin::Customer; // TODO(368-follow-up): plumb from caller
    let launch_card = state.launch_state_machine.start_launch(
        launch_id.clone(),
        pod_id.to_string(),
        sim_type.to_string(),
        launch_origin,
    ).await;
    let _ = state.dashboard_tx.send(
        rc_common::protocol::DashboardEvent::LaunchStatusChanged(launch_card)
    );

    // Create tracker + insert with TOCTOU re-check (LAUNCH-04)
    let info = {
        let mut games = state.game_launcher.active_games.write().await;
        // TOCTOU re-check: billing must still be present + capture session_id (Phase 310)
        let timers = state.billing.active_timers.read().await;
        let waiting = state.billing.waiting_for_game.read().await;
        if !timers.contains_key(pod_id) && !waiting.contains_key(pod_id) {
            drop(waiting);
            drop(timers);
            drop(games);
            tracing::warn!("Launch rejected for pod {}: billing session expired (TOCTOU)", pod_id);
            // D-15: Emit NeedsManualIntervention for TOCTOU billing expiry.
            // launch_started was already emitted above; transition it to terminal state.
            if let Some(card) = state.launch_state_machine.transition(
                &launch_id,
                rc_common::protocol::LaunchState::NeedsManualIntervention,
                Some("Launch blocked — billing not ready".to_string()),
                None,
                None,
            ).await {
                let _ = state.dashboard_tx.send(
                    rc_common::protocol::DashboardEvent::LaunchStatusChanged(card)
                );
            }
            return Err(format!("Pod {} billing session expired during launch", pod_id));
        }
        let billing_session_id = timers.get(pod_id).map(|t| t.session_id.clone());
        drop(waiting);
        drop(timers);

        let tracker = GameTracker {
            pod_id: pod_id.to_string(),
            sim_type,
            game_state: GameState::Launching,
            pid: None,
            launched_at: Some(Utc::now()),
            error_message: None,
            launch_args: launch_args.clone(),
            auto_relaunch_count: 0,
            externally_tracked: false,
            dynamic_timeout_secs: Some(dynamic_timeout as i64),
            exit_codes: Vec::new(),
            max_auto_relaunch: max_relaunch_cap,
            playable_at: None,
            ready_delay_ms: None,
            billing_session_id,
            launch_id: launch_id.clone(),
        };
        let info = tracker.to_info();
        games.insert(pod_id.to_string(), tracker);
        info
    };

    // Extract launch fields for metrics BEFORE consuming launch_args in the send
    let (launch_car, launch_track, launch_session_type, launch_args_hash) =
        extract_launch_fields(&launch_args);

    // Send command to agent with 1 retry (GAP-1 fix: fire-and-forget → retry-once)
    // Phase 312: Wrap with CoreMessage to get a known command_id for ACK correlation.
    // Extract duration_minutes from billing timer for SessionEnforcer (Forza GAME-03 fix)
    let duration_minutes = {
        let timers = state.billing.active_timers.read().await;
        timers.get(pod_id).map(|t| (t.remaining_seconds() / 60).max(1) as u32)
    };
    let launch_inner = launcher.make_launch_message(sim_type, launch_args, duration_minutes, launch_id.clone());
    let launch_msg = CoreMessage::wrap(launch_inner);
    let command_id = launch_msg.command_id.clone().unwrap_or_default();

    // Register ACK waiter BEFORE sending (WSCMD-01)
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
    state.pending_command_acks.write().await.insert(command_id.clone(), ack_tx);

    let mut send_ok = false;
    for attempt in 1..=2 {
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(pod_id) {
            match tx.send(launch_msg.clone()).await {
                Ok(_) => { send_ok = true; break; }
                Err(e) => {
                    tracing::warn!("LaunchGame send attempt {}/2 failed for pod {}: {}", attempt, pod_id, e);
                    drop(senders);
                    if attempt == 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    }
                }
            }
        } else {
            drop(senders);
            if attempt == 1 {
                tracing::warn!("No agent connected for pod {} (attempt 1/2), retrying in 3s", pod_id);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
            break;
        }
    }
    if !send_ok {
        tracing::warn!("No agent connected for pod {} after 2 attempts", pod_id);
        // Clean up pending ACK waiter since we never sent
        state.pending_command_acks.write().await.remove(&command_id);
        // Update tracker to error
        if let Some(tracker) = state
            .game_launcher
            .active_games
            .write()
            .await
            .get_mut(pod_id)
        {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some("No agent connected".to_string());
            // Re-capture info AFTER updating to Error state so dashboard gets correct state
            let error_info = tracker.to_info();
            if let Err(e) = state
                .dashboard_tx
                .send(DashboardEvent::GameStateChanged(error_info))
            {
                tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
            }
        }
        return Err(format!("No agent connected for pod {}", pod_id));
    }

    // Phase 312 (WSCMD-01/03): Wait for agent ACK with 5s timeout
    match tokio::time::timeout(std::time::Duration::from_secs(5), ack_rx).await {
        Ok(Ok(result)) => {
            if !result.success {
                tracing::warn!("Agent ACK failure for launch on pod {}: {:?}", pod_id, result.error);
                return Err(format!(
                    "Agent failed to launch on pod {}: {}",
                    pod_id,
                    result.error.unwrap_or_else(|| "unknown error".to_string())
                ));
            }
            tracing::info!("Launch ACK received for pod {} (command_id={})", pod_id, command_id);
        }
        Ok(Err(_)) => {
            // Oneshot dropped — agent disconnected
            tracing::warn!("ACK channel dropped for launch on pod {} (agent disconnected?)", pod_id);
            return Err(format!("Agent disconnected before acknowledging launch on pod {}", pod_id));
        }
        Err(_) => {
            // Timeout — WSCMD-03: old agent or WS drop
            tracing::warn!("WSCMD-03: No ACK within 5s for launch on pod {} (command_id={})", pod_id, command_id);
            state.pending_command_acks.write().await.remove(&command_id);
            return Err(format!("Agent did not acknowledge launch command within 5s (pod {})", pod_id));
        }
    }

    // Clone session_id before moving info into broadcast
    let launch_session_id = info.session_id.clone();

    // Broadcast to dashboards (only reached if agent IS connected)
    if let Err(e) = state
        .dashboard_tx
        .send(DashboardEvent::GameStateChanged(info))
    {
        tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id, e);
    }

    // CLOSED-LOOP VERIFICATION: Wait up to 20s for the game process to actually start.
    // The agent ACK above only proves the message was received — NOT that the game launched.
    // Poll the game tracker for state transition from Launching → Loading/Running.
    // This makes the API response truthful: ok=true means the game IS running.
    let mut verified = false;
    let verify_start = std::time::Instant::now();
    let verify_timeout = std::time::Duration::from_secs(20);
    while verify_start.elapsed() < verify_timeout {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let games = state.game_launcher.active_games.read().await;
        if let Some(tracker) = games.get(pod_id) {
            match tracker.game_state {
                GameState::Running | GameState::Loading => {
                    verified = true;
                    tracing::info!(
                        "CLOSED-LOOP: Game verified running on pod {} ({:?}) in {:.1}s",
                        pod_id, tracker.game_state, verify_start.elapsed().as_secs_f64()
                    );
                    break;
                }
                GameState::Error => {
                    tracing::warn!(
                        "CLOSED-LOOP: Game failed on pod {}: {:?}",
                        pod_id, tracker.error_message
                    );
                    break;
                }
                GameState::Idle => {
                    // Agent reported game stopped before it started — launch failed
                    tracing::warn!("CLOSED-LOOP: Game immediately went to Idle on pod {}", pod_id);
                    break;
                }
                _ => {
                    // Still Launching — keep waiting
                }
            }
        } else {
            // Tracker removed — something went wrong
            break;
        }
    }

    if !verified {
        tracing::warn!(
            "CLOSED-LOOP: Game launch NOT verified on pod {} after {:.1}s — process may not have started",
            pod_id, verify_start.elapsed().as_secs_f64()
        );
    }

    // Store verification result so the API can return it
    // (accessed by the route handler after this function returns)
    if let Some(tracker) = state.game_launcher.active_games.write().await.get_mut(pod_id) {
        // Tag the tracker with verification status for API response
        if verified {
            tracker.error_message = None; // Clear any transient messages
        }
    }

    // Record verification in launch metrics
    state.game_launcher.last_launch_verified.store(verified, std::sync::atomic::Ordering::Relaxed);

    // Log event to DB (legacy table, backward compat)
    log_game_event(state, pod_id, &sim_type.to_string(), "launched", None, None).await;

    // Rich launch event recording to new launch_events table + JSONL
    {
        let launch_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: launch_car,
            track: launch_track,
            session_type: launch_session_type,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome: metrics::LaunchOutcome::Success,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: None,
            launch_args_hash,
            attempt_number: 1,
            db_fallback: None,
            session_id: launch_session_id.clone(),
        };
        metrics::record_launch_event(&state.db, &launch_event, &state.config.venue.venue_id).await;
    }
    Ok(())
}

/// CRASH-04: Relaunch a crashed game using stored launch_args from GameTracker.
/// Resets auto_relaunch_count so Race Engineer gets fresh attempts.
pub async fn relaunch_game(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), String> {
    // Normalize pod_id to canonical form at function entry
    let pod_id_owned = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_owned.as_str();

    // Get stored launch info from tracker
    let (sim_type, launch_args) = {
        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get(pod_id).ok_or("No game tracker for pod")?;
        if tracker.game_state != GameState::Error {
            return Err(format!("Pod game is {:?}, not Error — cannot relaunch", tracker.game_state));
        }
        // LAUNCH-16: Guard null launch_args — externally tracked games don't have original args
        if tracker.externally_tracked || tracker.launch_args.is_none() {
            return Err(format!(
                "Cannot relaunch pod {} — original launch args unavailable (externally tracked or null). Please relaunch from kiosk.",
                pod_id
            ));
        }
        (tracker.sim_type, tracker.launch_args.clone())
    };

    // Verify billing is still active
    let has_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id);
    if !has_billing {
        return Err("No active billing session — cannot relaunch".into());
    }

    // Reset relaunch counter for fresh Race Engineer attempts
    {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.auto_relaunch_count = 0;
            tracker.game_state = GameState::Launching;
            tracker.error_message = None;
            tracker.launched_at = Some(Utc::now());
        }
    }

    // Extract launch fields for metrics BEFORE consuming launch_args in the send
    let (relaunch_car, relaunch_track, relaunch_session_type, relaunch_args_hash) =
        extract_launch_fields(&launch_args);

    // Send LaunchGame to agent (canonical pod_id guaranteed by normalization at entry)
    let senders = state.agent_senders.read().await;
    let tx = senders.get(pod_id).ok_or("Pod not connected")?;
    tx.send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
        sim_type,
        launch_args,
        force_clean: true,
        duration_minutes: None,
        launch_id: None,
    }))
    .await
    .map_err(|e| format!("Failed to send launch command: {}", e))?;

    // Log + broadcast
    log_game_event(state, pod_id, &sim_type.to_string(), "relaunched", None, Some("Manual relaunch from kiosk")).await;

    // Rich launch event recording for relaunch
    {
        let relaunch_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: relaunch_car,
            track: relaunch_track,
            session_type: relaunch_session_type,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome: metrics::LaunchOutcome::Success,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: Some("Manual relaunch from kiosk".to_string()),
            launch_args_hash: relaunch_args_hash,
            attempt_number: 2,
            db_fallback: None,
            session_id: None,
        };
        metrics::record_launch_event(&state.db, &relaunch_event, &state.config.venue.venue_id).await;
    }
    let info = {
        let games = state.game_launcher.active_games.read().await;
        games.get(pod_id).map(|t| t.to_info())
    };
    if let Some(info) = info {
        let pod_id_for_warn = pod_id.to_string();
        if let Err(e) = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info))
        {
            tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id_for_warn, e);
        }
    }

    log_pod_activity(state, pod_id, "game", "Game Relaunched", "Manual relaunch from kiosk", "core", None);

    Ok(())
}

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
