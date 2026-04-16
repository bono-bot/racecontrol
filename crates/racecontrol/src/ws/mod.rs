mod dashboard_handler;
mod ai_handler;
mod spectator_handler;
mod agent_auth;
mod agent_register;
mod agent_game;
mod agent_fleet;
mod agent_sync;
mod agent_sync_misc;
mod agent_lifecycle;

pub use dashboard_handler::handle_dashboard;
pub use ai_handler::{ai_ws, ws_exec_on_pod};
pub use spectator_handler::spectator_ws;
pub use agent_auth::WsAuthParams;
pub(crate) use agent_auth::verify_ws_token;

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

use rc_common::protocol::{
    AgentMessage, CoreMessage, CoreToAgentMessage, DashboardEvent,
};
use crate::state::AppState;

use agent_auth::AgentAuthResult;

/// Phase 364 GLD-D-05: Counter for try_send overflow events on hot-path WS channels.
/// Incremented when try_send returns TrySendError::Full (channel full, message dropped).
/// Read by metrics_producers.rs every 30s and flushed to metrics_samples table.
pub static WS_TRY_SEND_OVERFLOWS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// MI Phase 1: Count of currently connected dashboard WebSocket clients.
/// Incremented on WS connect, decremented on disconnect.
/// Used by detect_fleet_anomalies() to detect "kiosk healthy but no WS clients" (DASHBOARD_ORPHAN).
static DASHBOARD_CLIENT_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Rolling count of WS connect events in the last 60s. High churn = stale frontend build.
static DASHBOARD_WS_CONNECTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Rolling count of WS disconnect events in the last 60s.
static DASHBOARD_WS_DISCONNECTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Get the current number of connected dashboard WS clients.
pub fn dashboard_client_count() -> u32 {
    DASHBOARD_CLIENT_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Get WS churn metrics (connects, disconnects in rolling window).
/// Churn > 10/min indicates a client in a connect/disconnect loop (stale frontend build).
pub fn dashboard_ws_churn() -> (u32, u32) {
    (
        DASHBOARD_WS_CONNECTS.load(std::sync::atomic::Ordering::Relaxed),
        DASHBOARD_WS_DISCONNECTS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Reset churn counters (called every 60s by a background task).
pub fn reset_ws_churn_counters() {
    DASHBOARD_WS_CONNECTS.store(0, std::sync::atomic::Ordering::Relaxed);
    DASHBOARD_WS_DISCONNECTS.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// WebSocket endpoint for pod agents
pub async fn agent_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    match agent_auth::authenticate_agent_ws(&state, &params) {
        Ok(auth_result) => Ok(ws.on_upgrade(move |socket| handle_agent(socket, state, auth_result))),
        Err(reason) => {
            tracing::warn!("WS agent connection rejected — {}", reason);
            if params.jwt.as_ref().is_some_and(|j| !j.is_empty()) {
                let state_clone = state.clone();
                let reason_clone = reason.clone();
                tokio::spawn(async move {
                    crate::whatsapp_alerter::send_admin_alert(
                        &state_clone.config,
                        "ws_jwt_rejected",
                        &format!("Pod WS connection rejected: {}", reason_clone),
                    ).await;
                });
            }
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// WebSocket endpoint for dashboard clients.
/// Accepts token via query param (`?token=...`) OR Sec-WebSocket-Protocol header.
pub async fn dashboard_ws(
    Query(params): Query<WsAuthParams>,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut token = params.token.clone();
    if token.is_none() || token.as_deref() == Some("") {
        token = headers
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
    }
    let source = token.as_deref().unwrap_or("no-token").chars().take(8).collect::<String>();
    if agent_auth::ws_auth_locked_out(&source) {
        tracing::warn!("WS dashboard connection rate-limited — source '{}' locked out", source);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if !verify_ws_token(&state, &token) {
        agent_auth::ws_auth_record_failure(&source);
        tracing::warn!("WS dashboard connection rejected — invalid or missing token");
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut response = ws.on_upgrade(|socket| handle_dashboard(socket, state)).into_response();
    if let Some(proto) = headers.get("sec-websocket-protocol") {
        response.headers_mut().insert("sec-websocket-protocol", proto.clone());
    }
    Ok(response)
}

/// Main agent WebSocket handler — sets up channels, then dispatches messages.
async fn handle_agent(socket: WebSocket, state: Arc<AppState>, auth_result: AgentAuthResult) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    static CONN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let conn_id = CONN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    tracing::info!("Pod agent connected (conn_id={}, auth={})", conn_id,
        match &auth_result { AgentAuthResult::PskAuthenticated => "psk", AgentAuthResult::JwtAuthenticated { .. } => "jwt" });

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CoreMessage>(64);
    let mut registered_pod_id: Option<String> = None;

    let jwt_issued_for_conn = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        matches!(auth_result, AgentAuthResult::JwtAuthenticated { .. }),
    ));

    // Shared state for pending application-level ping measurement
    static PING_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pending_ping: Arc<tokio::sync::Mutex<Option<(u64, Instant)>>> = Arc::new(tokio::sync::Mutex::new(None));
    let pending_ping_send = pending_ping.clone();

    // Spawn task to forward commands from mpsc to WebSocket sender + keepalive pings.
    let send_task = tokio::spawn(async move {
        let mut ping_interval = interval(Duration::from_secs(15));
        ping_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut measure_interval = interval(Duration::from_secs(30));
        measure_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        ping_interval.tick().await;
        measure_interval.tick().await;

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(wrapped) => {
                            if let Ok(json) = serde_json::to_string(&wrapped)
                                && let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                                    tracing::warn!("WS send failed (conn_id={}): {} — continuing", conn_id, e);
                                }
                        }
                        None => break,
                    }
                }
                _ = ping_interval.tick() => {
                    tracing::trace!("WS ping sent (conn_id={})", conn_id);
                    if ws_sender.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                _ = measure_interval.tick() => {
                    let ping_id = PING_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let msg = CoreMessage::wrap(CoreToAgentMessage::Ping { id: ping_id });
                    if let Ok(json) = serde_json::to_string(&msg) {
                        *pending_ping_send.lock().await = Some((ping_id, Instant::now()));
                        if ws_sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Phase 306 WSAUTH-02: JWT rotation task
    let jwt_rotation_pod_id: std::sync::Arc<tokio::sync::Mutex<Option<(String, u32)>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(None));
    {
        let rotation_pod_id = jwt_rotation_pod_id.clone();
        let rotation_state = state.clone();
        let rotation_cmd_tx = cmd_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let guard = rotation_pod_id.lock().await;
            let Some((ref pod_id, pod_number)) = *guard else { return };
            let pod_id = pod_id.clone();
            drop(guard);
            tokio::time::sleep(Duration::from_secs(22 * 3600)).await;
            if rotation_cmd_tx.is_closed() { return; }
            match crate::auth::middleware::create_pod_jwt(&rotation_state.config.auth.jwt_secret, &pod_id, pod_number, 24) {
                Ok(token) => {
                    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp();
                    let msg = CoreMessage::wrap(CoreToAgentMessage::RefreshJwt { token, expires_at });
                    if rotation_cmd_tx.try_send(msg).is_ok() {
                        tracing::info!("Phase 306: JWT refreshed for pod {} (expires_at={})", pod_id, expires_at);
                    }
                }
                Err(e) => tracing::error!("Phase 306: JWT refresh failed for {}: {}", pod_id, e),
            }
        });
    }

    // ─── Message dispatch loop ──────────────────────────────────────────────
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            if text.len() > 2_097_152 {
                tracing::warn!("Agent WS message too large ({} bytes, conn_id={}) — dropping", text.len(), conn_id);
                continue;
            }
            match serde_json::from_str::<AgentMessage>(&text) {
                Ok(agent_msg) => {
                    dispatch_agent_message(
                        &state, &agent_msg, &mut registered_pod_id,
                        &jwt_rotation_pod_id, &jwt_issued_for_conn,
                        &cmd_tx, &pending_ping, conn_id,
                    ).await;
                    // Disconnect message handler returns a signal to break
                    if matches!(&agent_msg, AgentMessage::Disconnect { .. }) {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Invalid agent message: {}", e);
                }
            }
        }
    }

    // Cleanup on ungraceful disconnect
    agent_lifecycle::cleanup_on_disconnect(&state, &registered_pod_id, conn_id).await;

    send_task.abort();
    tracing::info!("Pod agent disconnected");
}

/// Dispatch a single agent message to the appropriate handler module.
#[allow(clippy::too_many_arguments)]
async fn dispatch_agent_message(
    state: &Arc<AppState>,
    agent_msg: &AgentMessage,
    registered_pod_id: &mut Option<String>,
    jwt_rotation_pod_id: &Arc<tokio::sync::Mutex<Option<(String, u32)>>>,
    jwt_issued_for_conn: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    cmd_tx: &mpsc::Sender<CoreMessage>,
    pending_ping: &Arc<tokio::sync::Mutex<Option<(u64, Instant)>>>,
    conn_id: u64,
) {
    match agent_msg {
        AgentMessage::Register(pod_info) => {
            agent_register::handle_register(
                state, pod_info, registered_pod_id,
                jwt_rotation_pod_id, jwt_issued_for_conn,
                cmd_tx, conn_id,
            ).await;
        }
        AgentMessage::Heartbeat(pod_info) => {
            agent_register::handle_heartbeat(state, pod_info, registered_pod_id).await;
        }
        AgentMessage::Telemetry(frame) => {
            agent_game::handle_telemetry(state, frame, registered_pod_id).await;
        }
        AgentMessage::LapCompleted(lap) => {
            agent_game::handle_lap_completed(state, lap).await;
        }
        AgentMessage::SessionUpdate(session) => {
            let _ = state.dashboard_tx.send(DashboardEvent::SessionUpdate(session.clone()));
        }
        AgentMessage::DrivingStateUpdate { pod_id, state: driving_state } => {
            agent_game::handle_driving_state_update(state, pod_id, *driving_state).await;
        }
        AgentMessage::GameStateUpdate(info) => {
            agent_game::handle_game_state_update(state, info).await;
        }
        AgentMessage::AiDebugResult(suggestion) => {
            agent_game::handle_ai_debug_result(state, suggestion).await;
        }
        AgentMessage::LaunchStatusUpdate { launch_id, state: new_state, detail, ai_tier, fix_action, origin: _ } => {
            agent_game::handle_launch_status_update(state, launch_id, new_state, detail, ai_tier, fix_action).await;
        }
        AgentMessage::PinEntered { pod_id, pin } => {
            tracing::info!("PIN entered on pod {}", pod_id);
            crate::activity_log::log_pod_activity(state, pod_id, "auth", "PIN Entered", "", "agent", None);
            crate::auth::handle_pin_entered(state, pod_id.clone(), pin.clone()).await;
        }
        AgentMessage::Pong { id, agent_delay_us } => {
            agent_game::handle_pong(pending_ping, *id, agent_delay_us, registered_pod_id, conn_id).await;
        }
        AgentMessage::GameStatusUpdate { pod_id, ac_status, sim_type } => {
            agent_game::handle_game_status_update(state, pod_id, *ac_status, *sim_type, cmd_tx).await;
        }
        AgentMessage::FfbZeroed { pod_id } => {
            tracing::info!("Pod {} FFB zeroed (safety action completed)", pod_id);
            crate::activity_log::log_pod_activity(state, pod_id, "safety", "FFB Zeroed", "Wheelbase torque set to 0", "agent", None);
        }
        AgentMessage::GameCrashed { pod_id, billing_active } => {
            agent_game::handle_game_crashed(state, pod_id, *billing_active).await;
        }
        AgentMessage::AssistChanged { pod_id, assist_type, enabled, confirmed } => {
            agent_game::handle_assist_changed(state, pod_id, assist_type, *enabled, *confirmed).await;
        }
        AgentMessage::FfbGainChanged { pod_id, percent } => {
            agent_game::handle_ffb_gain_changed(state, pod_id, *percent).await;
        }
        AgentMessage::AssistState { pod_id, abs, tc, auto_shifter, ffb_percent } => {
            agent_game::handle_assist_state(state, pod_id, *abs, *tc, *auto_shifter, *ffb_percent).await;
        }
        AgentMessage::ContentManifest(manifest) => {
            agent_sync::handle_content_manifest(state, manifest, registered_pod_id).await;
        }
        AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr } => {
            agent_sync::handle_exec_result(state, request_id, *success, *exit_code, stdout, stderr).await;
        }
        AgentMessage::CommandAck { command_id, success, error } => {
            agent_sync::handle_command_ack(state, command_id, *success, error, registered_pod_id).await;
        }
        AgentMessage::StartupReport {
            pod_id, version, uptime_secs, config_hash,
            crash_recovery, repairs,
            lock_screen_port_bound, remote_ops_port_bound,
            hid_detected, udp_ports_bound,
            windows_session_id, ..
        } => {
            agent_fleet::handle_startup_report(
                state, pod_id, version, *uptime_secs, config_hash,
                *crash_recovery, repairs,
                *lock_screen_port_bound, *remote_ops_port_bound,
                *hid_detected, udp_ports_bound,
                *windows_session_id,
            ).await;
        }
        AgentMessage::HardwareFailure { pod_id, reason, detail } => {
            crate::bot_coordinator::handle_hardware_failure(state, pod_id, reason, detail).await;
        }
        AgentMessage::HardwareDisconnect { pod_id, device, timestamp } => {
            agent_fleet::handle_hardware_disconnect(state, pod_id, device, timestamp).await;
        }
        AgentMessage::TelemetryGap { pod_id, sim_type: _, gap_seconds } => {
            crate::bot_coordinator::handle_telemetry_gap(state, pod_id, *gap_seconds as u64).await;
        }
        AgentMessage::TelemetryQualityGap { pod_id, gap_ms } => {
            crate::bot_coordinator::handle_telemetry_quality_gap(state, pod_id, *gap_ms).await;
        }
        AgentMessage::SessionStalled { pod_id, silence_seconds } => {
            crate::bot_coordinator::handle_session_stalled(state, pod_id, *silence_seconds).await;
        }
        AgentMessage::BillingAnomaly { pod_id, billing_session_id, reason, detail } => {
            crate::bot_coordinator::handle_billing_anomaly(state, pod_id, billing_session_id, *reason, detail).await;
        }
        AgentMessage::LapFlagged { pod_id, lap_id, reason, detail } => {
            tracing::info!("[bot] LapFlagged pod={} lap={} reason={:?}: {}", pod_id, lap_id, reason, detail);
        }
        AgentMessage::MultiplayerFailure { pod_id, reason, session_id } => {
            crate::bot_coordinator::handle_multiplayer_failure(state, pod_id, reason, session_id.as_deref()).await;
        }
        AgentMessage::Disconnect { pod_id } => {
            agent_fleet::handle_disconnect(state, pod_id).await;
            // Caller checks for Disconnect and breaks
        }
        AgentMessage::ProcessApprovalRequest { pod_id, process_name, exe_path, sighting_count } => {
            tracing::warn!(
                "[kiosk] Pod {} requesting approval for '{}' (path: {}, seen {} times)",
                pod_id, process_name, exe_path, sighting_count
            );
            crate::activity_log::log_pod_activity(
                state, pod_id, "kiosk", "Process Approval Request",
                &format!("Process '{}' at '{}' seen {} times — awaiting approval", process_name, exe_path, sighting_count),
                "rc-bot", None,
            );
        }
        AgentMessage::KioskLockdown { pod_id, reason } => {
            agent_fleet::handle_kiosk_lockdown(state, pod_id, reason).await;
        }
        AgentMessage::SessionAutoEnded { pod_id, billing_session_id, reason } => {
            agent_sync::handle_session_auto_ended(state, pod_id, billing_session_id, reason);
        }
        AgentMessage::BillingPaused { pod_id, billing_session_id } => {
            agent_sync::handle_billing_paused(state, pod_id, billing_session_id).await;
        }
        AgentMessage::BillingResumed { pod_id, billing_session_id } => {
            agent_sync::handle_billing_resumed(pod_id, billing_session_id);
        }
        AgentMessage::PreFlightPassed { pod_id } => {
            agent_sync::handle_preflight_passed(state, pod_id).await;
        }
        AgentMessage::PreFlightFailed { pod_id, failures, .. } => {
            agent_sync::handle_preflight_failed(state, pod_id, failures).await;
        }
        AgentMessage::SelfTestResult { pod_id, request_id, report } => {
            agent_sync::handle_self_test_result(state, pod_id, request_id, report).await;
        }
        AgentMessage::ProcessViolation(violation) => {
            agent_fleet::handle_process_violation(state, violation, registered_pod_id).await;
        }
        AgentMessage::ProcessGuardStatus { pod_id, scan_count, violation_count_total, violation_count_last_scan, last_scan_at, guard_active } => {
            agent_sync::handle_process_guard_status(pod_id, *scan_count, *violation_count_total, *violation_count_last_scan, last_scan_at, *guard_active);
        }
        AgentMessage::IdleHealthFailed { pod_id, failures, consecutive_count, timestamp } => {
            agent_sync::handle_idle_health_failed(state, pod_id, failures, *consecutive_count, timestamp).await;
        }
        AgentMessage::FlagCacheSync(payload) => {
            agent_sync::handle_flag_cache_sync(state, payload, cmd_tx).await;
        }
        AgentMessage::ConfigAck(payload) => {
            agent_sync::handle_config_ack(state, payload).await;
        }
        AgentMessage::SentinelChange { pod_id, file, action, timestamp } => {
            agent_fleet::handle_sentinel_change(state, pod_id, file, action, timestamp).await;
        }
        AgentMessage::DiagnosticResult {
            correlation_id, tier, outcome, root_cause,
            fix_action, fix_type, confidence, fix_applied,
            problem_hash, summary,
        } => {
            agent_fleet::handle_diagnostic_result(
                state, correlation_id, *tier, outcome, root_cause,
                fix_action, fix_type, *confidence, *fix_applied,
                problem_hash, summary, registered_pod_id,
            ).await;
        }
        AgentMessage::InactivityAlert { pod_id, idle_seconds } => {
            agent_sync::handle_inactivity_alert(state, pod_id, *idle_seconds).await;
        }
        AgentMessage::ExtendedTelemetry { pod_id, .. } => {
            agent_sync::handle_extended_telemetry(state, pod_id, agent_msg);
        }
        AgentMessage::MeshSolutionAnnounce { .. }
        | AgentMessage::MeshSolutionRequest { .. }
        | AgentMessage::MeshExperimentAnnounce { .. }
        | AgentMessage::MeshHeartbeat { .. } => {
            crate::mesh_handler::handle_mesh_message(state, agent_msg, cmd_tx).await;
        }
        AgentMessage::ModelEvalSync { pod_id, records } => {
            agent_sync::handle_model_eval_sync(state, pod_id, records).await;
        }
        AgentMessage::ModelReputationSync { pod_id, rows } => {
            agent_sync::handle_model_reputation_sync(state, pod_id, rows).await;
        }
        AgentMessage::EscalationRequest(payload) => {
            agent_sync::handle_escalation_request(state, payload);
        }
        AgentMessage::ExperienceScoreReport { pod_id, total_score, status, .. } => {
            agent_sync::handle_experience_score_report(state, pod_id, *total_score, status).await;
        }
        AgentMessage::JwtAck { pod_id } => {
            tracing::info!("Phase 306: JWT ack from pod {}", pod_id);
        }
        AgentMessage::GameInventoryUpdate(inventory) => {
            agent_sync::handle_game_inventory_update(state, inventory);
        }
        AgentMessage::ComboValidationReport { pod_id, results } => {
            agent_sync::handle_combo_validation_report(state, pod_id, results);
        }
        AgentMessage::LaunchTimelineReport(timeline) => {
            agent_sync::handle_launch_timeline_report(state, timeline);
        }
        AgentMessage::ConfigMismatchDetected { pod_id, sim_type, mismatches, timestamp } => {
            agent_sync::handle_config_mismatch_detected(state, pod_id, sim_type, mismatches, timestamp).await;
        }
        _ => { /* catch-all for future protocol additions */ }
    }
}
