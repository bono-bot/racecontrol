use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

use rc_common::protocol::{
    AgentMessage, CoreMessage, DashboardEvent, LaunchState,
};
use rc_common::types::GameLaunchInfo;
use rc_common::types::{AiDebugSuggestion, BillingSessionStatus, GameState, LapData, TelemetryFrame};

use crate::activity_log::log_pod_activity;
use crate::state::{AppState, CachedAssistState};
use crate::{billing, game_launcher};

/// Handle AgentMessage::Telemetry.
pub(crate) async fn handle_telemetry(
    state: &Arc<AppState>,
    frame: &TelemetryFrame,
    registered_pod_id: &Option<String>,
) {
    let mut frame = frame.clone();
    if let Some(expected) = registered_pod_id {
        if frame.pod_id != *expected {
            tracing::warn!("Telemetry pod_id spoof: conn={} frame={} — overriding", expected, frame.pod_id);
            frame.pod_id = expected.clone();
        }
    }
    crate::ac_camera::on_telemetry(state, &frame).await;
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::Telemetry(frame.clone()));
    if let Some(ref tx) = state.telemetry_writer_tx {
        let _ = tx.try_send(frame.clone());
    }
    // GLD-C-02: Record 1s coverage bucket.
    if let Ok(mut timers) = state.billing.active_timers.try_write() {
        if let Some(timer) = timers.get_mut(&frame.pod_id) {
            let elapsed = timer.elapsed_seconds;
            timer.telemetry_seconds_covered.insert(elapsed);
        }
    }
}

/// Handle AgentMessage::LapCompleted.
pub(crate) async fn handle_lap_completed(
    state: &Arc<AppState>,
    lap: &LapData,
) {
    let mut lap = lap.clone();
    if let Some((driver_id, session_id)) =
        crate::lap_tracker::resolve_driver_for_pod(state, &lap.pod_id).await
    {
        lap.driver_id = driver_id;
        lap.session_id = session_id;
    }
    tracing::info!(
        "Lap completed: {} - {}ms on {}",
        lap.driver_id, lap.lap_time_ms, lap.track
    );
    crate::lap_tracker::persist_lap(state, &lap).await;
    if lap.valid {
        crate::lap_consistency::check_lap_consistency(state, &lap).await;
    }
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::LapCompleted(lap));
}

/// Handle AgentMessage::DrivingStateUpdate.
pub(crate) async fn handle_driving_state_update(
    state: &Arc<AppState>,
    pod_id: &str,
    driving_state: rc_common::types::DrivingState,
) {
    tracing::debug!("Pod {} driving state: {:?}", pod_id, driving_state);
    if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
        pod.driving_state = Some(driving_state);
    }
    billing::update_driving_state(state, pod_id, driving_state).await;
}

/// Handle AgentMessage::GameStateUpdate.
pub(crate) async fn handle_game_state_update(
    state: &Arc<AppState>,
    info: &GameLaunchInfo,
) {
    tracing::info!(
        "Pod {} game state: {:?} ({:?})",
        info.pod_id, info.game_state, info.sim_type
    );
    let gs_action = match info.game_state {
        GameState::Running => "Game Running",
        GameState::Loading => "Game Loading",
        GameState::Error => "Game Crashed",
        GameState::Idle => "Game Stopped",
        GameState::Launching => "Game Launching",
        GameState::Stopping => "Game Stopping",
        GameState::InLobby => "Game In Lobby",
    };
    let gs_details = match &info.error_message {
        Some(err) => format!("{}: {}", info.sim_type, err),
        None => format!("{}", info.sim_type),
    };
    log_pod_activity(state, &info.pod_id, "game", gs_action, &gs_details, "agent", None);
    game_launcher::handle_game_state_update(state, info.clone()).await;

    // Phase 317 (LAUNCH-04): Chain failure detection
    {
        let sim_key = format!("{}:{:?}", info.pod_id, info.sim_type);
        match info.game_state {
            GameState::Error => {
                let should_escalate = {
                    let mut tracker = state.chain_failure_tracker.write().await;
                    let entry = tracker.entry(sim_key.clone()).or_default();
                    if entry.is_window_expired() {
                        entry.reset();
                    }
                    if entry.window_start.is_none() {
                        entry.window_start = Some(std::time::Instant::now());
                    }
                    entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
                    let should = entry.consecutive_failures >= 3 && !entry.alerted;
                    if should {
                        entry.alerted = true;
                    }
                    (should, entry.consecutive_failures)
                };
                if should_escalate.0 {
                    let escalation = state.whatsapp_escalation.clone();
                    let pod_id_esc = info.pod_id.clone();
                    let sim_type_str = format!("{}", info.sim_type);
                    let count = should_escalate.1;
                    let incident_id = format!("chain_fail_{}_{:?}", pod_id_esc, info.sim_type);
                    tokio::spawn(async move {
                        escalation.handle_escalation(rc_common::protocol::EscalationPayload {
                            pod_id: pod_id_esc.clone(),
                            incident_id,
                            severity: "critical".to_string(),
                            trigger: "ChainLaunchFailure".to_string(),
                            summary: format!(
                                "Chain failure: {} on {} failed {} times in 10 min",
                                sim_type_str, pod_id_esc, count
                            ),
                            actions_tried: vec!["auto_relaunch_attempted".to_string()],
                            impact: format!("{} is unlaunchable on {} — customers cannot start sessions", sim_type_str, pod_id_esc),
                            dashboard_url: "http://192.168.31.23:3201/fleet".to_string(),
                            timestamp: crate::whatsapp_alerter::ist_now_string(),
                        }).await;
                    });
                }
            }
            GameState::Running => {
                let mut tracker = state.chain_failure_tracker.write().await;
                if let Some(entry) = tracker.get_mut(&sim_key) {
                    entry.reset();
                }
            }
            _ => {}
        }
    }
}

/// Handle AgentMessage::AiDebugResult.
pub(crate) async fn handle_ai_debug_result(
    state: &Arc<AppState>,
    suggestion: &AiDebugSuggestion,
) {
    tracing::info!(
        "AI debug suggestion for pod {}: {}",
        suggestion.pod_id, suggestion.model
    );
    let id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
         VALUES (?, ?, ?, ?, ?, ?, 'crash')"
    )
    .bind(&id)
    .bind(&suggestion.pod_id)
    .bind(format!("{:?}", suggestion.sim_type))
    .bind(&suggestion.error_context)
    .bind(&suggestion.suggestion)
    .bind(&suggestion.model)
    .execute(&state.db)
    .await;
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::AiDebugSuggestion(suggestion.clone()));
}

/// Handle AgentMessage::LaunchStatusUpdate.
pub(crate) async fn handle_launch_status_update(
    state: &Arc<AppState>,
    launch_id: &str,
    new_state: &LaunchState,
    detail: &Option<String>,
    ai_tier: &Option<u8>,
    fix_action: &Option<String>,
) {
    if launch_id.starts_with("rcagent-local-") {
        tracing::error!(
            launch_id = %launch_id,
            ?new_state,
            "split-deploy launch_id received — REQUIRES FLEET UPDATE. \
             rc-agent minted this id because the server was on an older build \
             when the pod booted. Upgrade rc-agent to build_id >= 368-02 to \
             restore D-01 canonical server-minted launch_id."
        );
    }

    let maybe_card = state
        .launch_state_machine
        .transition(launch_id, *new_state, detail.clone(), *ai_tier, fix_action.clone())
        .await;

    match maybe_card {
        Some(card) => {
            if let Err(e) = state.dashboard_tx.send(
                DashboardEvent::LaunchStatusChanged(card)
            ) {
                tracing::debug!(
                    error = %e,
                    "dashboard_tx has no subscribers for LaunchStatusChanged"
                );
            }
        }
        None => {
            tracing::warn!(
                launch_id = %launch_id,
                ?new_state,
                "LaunchStatusUpdate for unknown or terminal launch_id — dropping"
            );
        }
    }
}

/// Handle AgentMessage::Pong (application-level round-trip measurement).
pub(crate) async fn handle_pong(
    pending_ping: &Arc<tokio::sync::Mutex<Option<(u64, Instant)>>>,
    id: u64,
    agent_delay_us: &Option<u64>,
    registered_pod_id: &Option<String>,
    conn_id: u64,
) {
    let mut guard = pending_ping.lock().await;
    if let Some((pending_id, sent_at)) = guard.take() {
        if pending_id == id {
            let elapsed_ms = sent_at.elapsed().as_millis();
            let fallback_label = format!("conn_{}", conn_id);
            let label = registered_pod_id.as_deref().unwrap_or(&fallback_label);
            if elapsed_ms > 600 {
                let agent_info = match agent_delay_us {
                    Some(us) => format!(", agent_process={}us", us),
                    None => String::new(),
                };
                tracing::warn!(
                    "WS round-trip slow: {} took {}ms (threshold 600ms{})",
                    label, elapsed_ms, agent_info
                );
            } else {
                tracing::debug!(
                    "WS round-trip: {}ms ({})",
                    elapsed_ms, label
                );
            }
        } else {
            tracing::debug!(
                "Stale pong id={} (expected {}), discarding",
                id, pending_id
            );
        }
    }
}

/// Handle AgentMessage::GameStatusUpdate.
pub(crate) async fn handle_game_status_update(
    state: &Arc<AppState>,
    pod_id: &str,
    ac_status: rc_common::types::AcStatus,
    sim_type: Option<rc_common::types::SimType>,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) {
    tracing::info!("Pod {} AC STATUS: {:?}", pod_id, ac_status);
    log_pod_activity(state, pod_id, "game", &format!("AC Status: {:?}", ac_status), "", "agent", None);
    billing::handle_game_status_update(state, pod_id, ac_status, sim_type, cmd_tx).await;
}

/// Handle AgentMessage::GameCrashed.
pub(crate) async fn handle_game_crashed(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_active: bool,
) {
    tracing::warn!("Pod {} game crashed (billing_active={})", pod_id, billing_active);
    log_pod_activity(state, pod_id, "game", "Game Crashed", &format!("billing_active={}", billing_active), "agent", None);
    // CRASH-02: Auto-pause billing on game crash
    if billing_active {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id) {
            if timer.status == BillingSessionStatus::Active {
                timer.status = BillingSessionStatus::PausedGamePause;
                timer.pause_seconds = 0;
                timer.pause_count += 1;
                tracing::info!("Billing auto-paused on crash for pod {}", pod_id);
            }
        }
    }

    // RESIL-06: Record crash event and check if pod should be flagged for maintenance.
    let crash_id = uuid::Uuid::new_v4().to_string();
    let crash_result = sqlx::query(
        "INSERT INTO pod_crash_events (id, pod_id, crash_type) VALUES (?, ?, 'game_crash')"
    )
    .bind(&crash_id)
    .bind(pod_id)
    .execute(&state.db)
    .await;

    if let Err(e) = crash_result {
        tracing::warn!("RESIL-06: Failed to insert crash event for pod {}: {}", pod_id, e);
    } else {
        let count_result: Result<(i64,), _> = sqlx::query_as(
            "SELECT COUNT(*) FROM pod_crash_events WHERE pod_id = ? AND created_at > datetime('now', '-1 hour')"
        )
        .bind(pod_id)
        .fetch_one(&state.db)
        .await;

        if let Ok((count,)) = count_result {
            let pod_id_owned = pod_id.to_string();
            let count_i32 = count as i32;
            {
                let mut fleet = state.pod_fleet_health.write().await;
                let store = fleet.entry(pod_id_owned.clone()).or_default();
                store.crashes_last_hour = count_i32;
                if count > 3 && !store.maintenance_flag {
                    store.maintenance_flag = true;
                    tracing::error!(
                        "RESIL-06: Pod {} flagged for maintenance — {} crashes in 1 hour",
                        pod_id_owned, count
                    );
                    let alert_msg = format!(
                        "[MAINTENANCE] Pod {} auto-flagged: {} crashes in last hour. Check hardware. {}",
                        pod_id_owned, count,
                        crate::whatsapp_alerter::ist_now_string()
                    );
                    drop(fleet);
                    crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
                }
            }
        }
    }
}

/// Handle AgentMessage::AssistChanged.
pub(crate) async fn handle_assist_changed(
    state: &Arc<AppState>,
    pod_id: &str,
    assist_type: &str,
    enabled: bool,
    confirmed: bool,
) {
    tracing::info!(
        "Pod {} assist changed: {} = {} (confirmed: {})",
        pod_id, assist_type, enabled, confirmed
    );
    log_pod_activity(state, pod_id, "game", "Assist Changed",
        &format!("{} = {} (confirmed: {})", assist_type, enabled, confirmed), "agent", None);
    {
        let mut cache = state.assist_cache.write().await;
        let entry = cache.entry(pod_id.to_string()).or_default();
        match assist_type {
            "abs" => entry.abs = if enabled { 1 } else { 0 },
            "tc" => entry.tc = if enabled { 1 } else { 0 },
            "transmission" => entry.auto_shifter = enabled,
            _ => {}
        }
    }
}

/// Handle AgentMessage::FfbGainChanged.
pub(crate) async fn handle_ffb_gain_changed(
    state: &Arc<AppState>,
    pod_id: &str,
    percent: u8,
) {
    tracing::info!("Pod {} FFB gain changed to {}%", pod_id, percent);
    log_pod_activity(state, pod_id, "game", "FFB Gain Changed",
        &format!("{}%", percent), "agent", None);
    {
        let mut cache = state.assist_cache.write().await;
        let entry = cache.entry(pod_id.to_string()).or_default();
        entry.ffb_percent = percent;
    }
}

/// Handle AgentMessage::AssistState.
pub(crate) async fn handle_assist_state(
    state: &Arc<AppState>,
    pod_id: &str,
    abs: u8,
    tc: u8,
    auto_shifter: bool,
    ffb_percent: u8,
) {
    tracing::info!(
        "Pod {} assist state: ABS={} TC={} auto_shifter={} FFB={}%",
        pod_id, abs, tc, auto_shifter, ffb_percent
    );
    log_pod_activity(state, pod_id, "game", "Assist State",
        &format!("ABS={} TC={} auto_shifter={} FFB={}%", abs, tc, auto_shifter, ffb_percent), "agent", None);
    {
        let mut cache = state.assist_cache.write().await;
        cache.insert(pod_id.to_string(), CachedAssistState {
            abs,
            tc,
            auto_shifter,
            ffb_percent,
        });
    }
}
