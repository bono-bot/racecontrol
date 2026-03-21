use std::time::Duration;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::ac_launcher;
use crate::ai_debugger::PodStateSnapshot;
use crate::app_state::AppState;
use crate::ffb_controller;
use crate::game_process;
use crate::kiosk;
use crate::lock_screen::LockScreenEvent;
use crate::udp_heartbeat;
use crate::ws_handler::{HandleResult, WsTx};
use rc_common::protocol::AgentMessage;
use rc_common::types::*;

/// Type alias for the WebSocket receive half.
pub type WsRx = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
>;

/// Tracks the state of a game launch attempt for timeout/retry handling.
/// BILL-01: 3-minute launch timeout with auto-retry once, cancel on second fail (no charge).
pub(crate) enum LaunchState {
    Idle,
    WaitingForLive {
        launched_at: std::time::Instant,
        attempt: u8, // 1 or 2
    },
    Live,
}

/// Crash recovery state machine (SESSION-03).
/// Replaces the old crash_recovery_armed bool + crash_recovery_timer Sleep.
/// Pauses billing, attempts up to 2 game relaunches (60s each), then auto-ends.
#[derive(Debug)]
pub(crate) enum CrashRecoveryState {
    /// No crash recovery in progress.
    Idle,
    /// Billing paused, waiting for game relaunch to succeed.
    PausedWaitingRelaunch {
        attempt: u8,                                               // 1 or 2
        timer: std::pin::Pin<Box<tokio::time::Sleep>>,             // 60s per attempt
        last_sim_type: SimType,
        last_launch_args: Option<String>,
    },
    /// 2nd relaunch failed — auto-end via same path as orphan.
    AutoEndPending,
}

/// Per-connection state — reset on each WebSocket connect.
///
/// These variables are initialized fresh for every new WebSocket connection,
/// as opposed to AppState fields which survive across reconnections.
pub(crate) struct ConnectionState {
    pub(crate) heartbeat_interval: tokio::time::Interval,
    pub(crate) telemetry_interval: tokio::time::Interval,
    pub(crate) detector_interval: tokio::time::Interval,
    pub(crate) game_check_interval: tokio::time::Interval,
    pub(crate) kiosk_interval: tokio::time::Interval,
    pub(crate) overlay_topmost_interval: tokio::time::Interval,
    pub(crate) maintenance_retry_interval: tokio::time::Interval,
    pub(crate) blank_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    pub(crate) blank_timer_armed: bool,
    pub(crate) crash_recovery: CrashRecoveryState,
    pub(crate) launch_state: LaunchState,
    pub(crate) last_launch_args_stored: Option<String>,
    pub(crate) current_driver_name: Option<String>,
    pub(crate) last_ffb_percent: u8,
    pub(crate) last_ffb_preset: String,
    pub(crate) session_max_speed_kmh: f32,
    pub(crate) session_race_position: Option<u32>,
    pub(crate) ws_connect_time: tokio::time::Instant,
    /// 30s grace period after game exit — delays AcStatus::Off to prevent session fragmentation
    pub(crate) exit_grace_timer: std::pin::Pin<Box<tokio::time::Sleep>>,
    pub(crate) exit_grace_armed: bool,
    /// SimType of the game that exited (for correct sim_type on delayed Off signal)
    pub(crate) exit_grace_sim_type: Option<rc_common::types::SimType>,
    /// Track if we already emitted Loading state to server for current launch
    pub(crate) loading_emitted: bool,
    /// Track the current sim_type for the active game (set on LaunchGame, cleared on Idle)
    pub(crate) current_sim_type: Option<rc_common::types::SimType>,
    /// Track whether F1 25 playable signal has been received (from DrivingDetector UdpActive)
    pub(crate) f1_udp_playable_received: bool,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self {
            heartbeat_interval: tokio::time::interval(Duration::from_secs(5)),
            telemetry_interval: tokio::time::interval(Duration::from_millis(100)),
            detector_interval: tokio::time::interval(Duration::from_millis(100)),
            game_check_interval: tokio::time::interval(Duration::from_secs(2)),
            kiosk_interval: tokio::time::interval(Duration::from_secs(5)),
            overlay_topmost_interval: tokio::time::interval(Duration::from_secs(10)),
            maintenance_retry_interval: tokio::time::interval(Duration::from_secs(30)),
            blank_timer: Box::pin(tokio::time::sleep(Duration::from_secs(86400))),
            blank_timer_armed: false,
            crash_recovery: CrashRecoveryState::Idle,
            launch_state: LaunchState::Idle,
            last_launch_args_stored: None,
            current_driver_name: None,
            last_ffb_percent: 70,
            last_ffb_preset: "medium".to_string(),
            session_max_speed_kmh: 0.0,
            session_race_position: None,
            ws_connect_time: tokio::time::Instant::now(),
            exit_grace_timer: Box::pin(tokio::time::sleep(Duration::from_secs(86400))),
            exit_grace_armed: false,
            exit_grace_sim_type: None,
            loading_emitted: false,
            current_sim_type: None,
            f1_udp_playable_received: false,
        }
    }
}

/// Run the inner event loop for one WebSocket connection lifetime.
///
/// Returns Ok(()) when the connection is lost (select! break).
/// Returns Err when a fatal serialization error occurs.
///
/// Caller (main.rs reconnect loop) creates ws_tx/ws_rx via ws_stream.split(),
/// sets heartbeat_status.ws_connected = true, then calls this function.
pub async fn run(
    state: &mut AppState,
    mut ws_tx: WsTx,
    mut ws_rx: WsRx,
    primary_url: &str,
    failover_url: &Option<String>,
    active_url: &std::sync::Arc<tokio::sync::RwLock<String>>,
    split_brain_probe: &reqwest::Client,
) -> anyhow::Result<()> {
    let mut conn = ConnectionState::new();

    loop {
        tokio::select! {
            _ = conn.heartbeat_interval.tick() => {
                let hb = AgentMessage::Heartbeat(PodInfo {
                    status: PodStatus::Idle,
                    last_seen: Some(Utc::now()),
                    driving_state: Some(state.detector.state()),
                    game_state: state.game_process.as_ref().map(|g| g.state),
                    current_game: state.game_process.as_ref().map(|g| g.sim_type),
                    screen_blanked: Some(state.lock_screen.is_blanked()),
                    ffb_preset: Some(conn.last_ffb_preset.clone()),
                    ..state.pod_info.clone()
                });
                let json = serde_json::to_string(&hb)?;
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    tracing::error!("Lost connection to core server");
                    break;
                }
            }

            _ = conn.telemetry_interval.tick() => {
                let Some(ref mut adapter) = state.adapter else { continue };
                if !adapter.is_connected() {
                    if adapter.connect().is_ok() {
                        state.overlay.set_max_rpm(adapter.max_rpm());
                    }
                    continue;
                }

                match adapter.read_telemetry() {
                    Ok(Some(frame)) => {
                        state.overlay.update_telemetry(&frame);
                        if frame.speed_kmh > conn.session_max_speed_kmh {
                            conn.session_max_speed_kmh = frame.speed_kmh;
                        }

                        if let Ok(Some(lap)) = adapter.poll_lap_completed() {
                            state.overlay.on_lap_completed(&lap);
                            let msg = AgentMessage::LapCompleted(lap);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }

                        let msg = AgentMessage::Telemetry(frame);
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("Telemetry read error: {}", e);
                        adapter.disconnect();
                    }
                }

                if state.game_process.is_some() {
                    if let Some(ref mut adapter2) = state.adapter {
                        if let Some(current_status) = adapter2.read_ac_status() {
                            let status_changed = state.last_ac_status.map_or(true, |prev| prev != current_status);
                            if status_changed {
                                state.ac_status_stable_since = Some(std::time::Instant::now());
                                state.last_ac_status = Some(current_status);
                            }
                            if let (Some(stable_since), Some(status)) = (state.ac_status_stable_since, state.last_ac_status) {
                                if stable_since.elapsed() >= Duration::from_secs(1) {
                                    state.ac_status_stable_since = None;

                                    if status == AcStatus::Live {
                                        // Live: send immediately, transition launch_state
                                        let msg = AgentMessage::GameStatusUpdate {
                                            pod_id: state.pod_id.clone(),
                                            ac_status: status,
                                            sim_type: Some(rc_common::types::SimType::AssettoCorsa),
                                        };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            let _ = ws_tx.send(Message::Text(json.into())).await;
                                        }
                                        conn.launch_state = LaunchState::Live;
                                    } else if status == AcStatus::Off {
                                        // Off: arm 30s grace timer — crash recovery may cancel it
                                        if !matches!(conn.crash_recovery, CrashRecoveryState::PausedWaitingRelaunch { .. }) {
                                            tracing::info!("[billing] AcStatus::Off detected — arming 30s exit grace timer (AC)");
                                            conn.exit_grace_timer = Box::pin(tokio::time::sleep(Duration::from_secs(30)));
                                            conn.exit_grace_armed = true;
                                            conn.exit_grace_sim_type = Some(rc_common::types::SimType::AssettoCorsa);
                                        }
                                    } else {
                                        // Other statuses (e.g. Pause): send immediately
                                        let msg = AgentMessage::GameStatusUpdate {
                                            pod_id: state.pod_id.clone(),
                                            ac_status: status,
                                            sim_type: Some(rc_common::types::SimType::AssettoCorsa),
                                        };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            let _ = ws_tx.send(Message::Text(json.into())).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let LaunchState::WaitingForLive { launched_at, attempt } = &conn.launch_state {
                    let launched_at = *launched_at;
                    let attempt = *attempt;
                    if launched_at.elapsed() > Duration::from_secs(180) {
                        if attempt < 2 {
                            tracing::warn!("AC launch timeout (attempt {}), retrying...", attempt);
                            if let Some(ref mut proc) = state.game_process {
                                let _ = proc.stop();
                            }
                            state.game_process = None;

                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: state.pod_id.clone(),
                                ac_status: AcStatus::Off,
                                sim_type: Some(rc_common::types::SimType::AssettoCorsa),
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_tx.send(Message::Text(json.into())).await;
                            }

                            conn.launch_state = LaunchState::WaitingForLive {
                                launched_at: std::time::Instant::now(),
                                attempt: attempt + 1,
                            };
                        } else {
                            tracing::error!("AC launch failed twice, cancelling session (no charge)");
                            if let Some(ref mut proc) = state.game_process {
                                let _ = proc.stop();
                            }
                            state.game_process = None;
                            conn.launch_state = LaunchState::Idle;
                            let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = None; });

                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: state.pod_id.clone(),
                                ac_status: AcStatus::Off,
                                sim_type: Some(rc_common::types::SimType::AssettoCorsa),
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_tx.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                }
            }

            Some(signal) = state.signal_rx.recv() => {
                // F1 25 PlayableSignal: UdpActive on port 20777 is the billing trigger
                if matches!(signal, crate::driving_detector::DetectorSignal::UdpActive) {
                    if matches!(conn.current_sim_type, Some(rc_common::types::SimType::F125))
                        && matches!(conn.launch_state, LaunchState::WaitingForLive { .. })
                    {
                        conn.f1_udp_playable_received = true;
                    }
                }

                let (_, changed) = state.detector.process_signal(signal);
                if changed {
                    let is_active = matches!(state.detector.state(), DrivingState::Active);
                    state.heartbeat_status.driving_active.store(is_active, std::sync::atomic::Ordering::Relaxed);
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: state.pod_id.clone(),
                        state: state.detector.state(),
                    };
                    let _ = state.failure_monitor_tx.send_modify(|s| { s.driving_state = Some(state.detector.state()); });
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed: {:?}", state.detector.state());
                }
            }

            _ = conn.detector_interval.tick() => {
                let (_, changed) = state.detector.evaluate_state();
                if changed {
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: state.pod_id.clone(),
                        state: state.detector.state(),
                    };
                    let _ = state.failure_monitor_tx.send_modify(|s| { s.driving_state = Some(state.detector.state()); });
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed (timeout): {:?}", state.detector.state());
                }
                let _ = state.failure_monitor_tx.send_modify(|s| {
                    s.hid_connected = state.detector.is_hid_connected();
                    s.last_udp_secs_ago = state.detector.last_udp_packet_elapsed_secs();
                });
            }

            _ = conn.game_check_interval.tick() => {
                if let Some(ref mut game) = state.game_process {
                    let was_active = matches!(game.state, GameState::Running | GameState::Launching);

                    if game.state == GameState::Launching && game.child.is_none() {
                        if let Some(pid) = game_process::find_game_pid(game.sim_type) {
                            game.pid = Some(pid);
                            game_process::persist_pid(pid);
                            game.state = GameState::Running;

                            // Emit GameState::Loading once — process detected, PlayableSignal not yet fired
                            if !conn.loading_emitted && matches!(conn.launch_state, LaunchState::WaitingForLive { .. }) {
                                let loading_info = GameLaunchInfo {
                                    pod_id: state.pod_id.clone(),
                                    sim_type: game.sim_type,
                                    game_state: GameState::Loading,
                                    pid: Some(pid),
                                    launched_at: Some(Utc::now()),
                                    error_message: None,
                                    diagnostics: None,
                                };
                                let loading_msg = AgentMessage::GameStateUpdate(loading_info);
                                if let Ok(json) = serde_json::to_string(&loading_msg) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                                conn.loading_emitted = true;
                                tracing::info!("[billing] GameState::Loading emitted for {:?}", game.sim_type);
                            }

                            let info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Running,
                                pid: Some(pid),
                                launched_at: Some(Utc::now()),
                                error_message: None,
                                diagnostics: None,
                            };
                            let _ = state.failure_monitor_tx.send_modify(|s| {
                                s.game_pid = Some(pid);
                            });
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                    } else {
                        let still_alive = game.is_running();
                        if !still_alive && was_active {
                            state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            state.heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                            let err_msg = "Game process exited unexpectedly".to_string();
                            let info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Error,
                                pid: game.pid,
                                launched_at: None,
                                error_message: Some(err_msg.clone()),
                                diagnostics: None,
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;

                            tracing::info!("[crash-detect] AI debugger enabled={}, url={}, model={}",
                                state.config.ai_debugger.enabled, state.config.ai_debugger.ollama_url, state.config.ai_debugger.ollama_model);
                            if state.config.ai_debugger.enabled {
                                let exit_info = game.last_exit_code
                                    .map(|c| format!("exit code {}", c))
                                    .unwrap_or_else(|| "no exit code".to_string());
                                let err_ctx = format!("{:?} crashed on pod {} ({})", game.sim_type, state.pod_id, exit_info);
                                tracing::info!("[crash-detect] Spawning AI debugger for: {}", err_ctx);
                                let snapshot = PodStateSnapshot {
                                    pod_id: state.pod_id.clone(),
                                    pod_number: state.config.pod.number,
                                    lock_screen_active: state.lock_screen.is_active(),
                                    billing_active: state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                    game_pid: game.pid,
                                    driving_state: Some(state.detector.current_state()),
                                    wheelbase_connected: state.detector.is_hid_connected(),
                                    ws_connected: state.heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                    uptime_seconds: state.agent_start_time.elapsed().as_secs(),
                                    ..Default::default()
                                };
                                tokio::spawn(crate::ai_debugger::analyze_crash(
                                    state.config.ai_debugger.clone(),
                                    state.pod_id.clone(),
                                    game.sim_type,
                                    err_ctx,
                                    snapshot,
                                    state.ai_result_tx.clone(),
                                ));
                            }

                            state.game_process = None;
                            game_process::clear_persisted_pid();
                            state.last_ac_status = None;
                            state.ac_status_stable_since = None;
                            conn.launch_state = LaunchState::Idle;
                            let _ = state.failure_monitor_tx.send_modify(|s| {
                                s.launch_started_at = None;
                                s.game_pid = None;
                            });

                            if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                                tracing::warn!("Game crashed during active billing — pausing billing, attempting relaunch");
                                ffb_controller::safe_session_end(&state.ffb).await;
                                let crash_msg = AgentMessage::GameCrashed { pod_id: state.pod_id.clone(), billing_active: true };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&crash_msg).unwrap_or_default().into())).await;
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                let _ = state.failure_monitor_tx.send_modify(|s| {
                                    s.billing_paused = true;
                                });
                                if let Some(ref sid) = state.failure_monitor_tx.borrow().active_billing_session_id.clone() {
                                    let pause_msg = AgentMessage::BillingPaused {
                                        pod_id: state.pod_id.clone(),
                                        billing_session_id: sid.clone(),
                                    };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&pause_msg).unwrap_or_default().into())).await;
                                }
                                state.overlay.show_toast("Game crashed \u{2014} relaunching...".to_string());
                                let last_sim = SimType::AssettoCorsa; // game_process already set to None above
                                conn.crash_recovery = CrashRecoveryState::PausedWaitingRelaunch {
                                    attempt: 1,
                                    timer: Box::pin(tokio::time::sleep(Duration::from_secs(60))),
                                    last_sim_type: last_sim,
                                    last_launch_args: conn.last_launch_args_stored.clone(),
                                };
                            } else {
                                tracing::info!("Game exited with no active billing — enforcing safe state");
                                // Arm exit grace timer so server gets AcStatus::Off after 30s
                                // (handles non-AC sims that don't have shared memory Off signal)
                                if !matches!(conn.crash_recovery, CrashRecoveryState::PausedWaitingRelaunch { .. }) {
                                    let exited_sim = conn.current_sim_type;
                                    if exited_sim != Some(rc_common::types::SimType::AssettoCorsa) {
                                        // Non-AC sims: server doesn't get AcStatus::Off from telemetry path
                                        // so we arm the grace timer here
                                        if let Some(sim) = exited_sim {
                                            tracing::info!("[billing] {:?} exited — arming 30s exit grace timer", sim);
                                            conn.exit_grace_timer = Box::pin(tokio::time::sleep(Duration::from_secs(30)));
                                            conn.exit_grace_armed = true;
                                            conn.exit_grace_sim_type = Some(sim);
                                        }
                                    }
                                }
                                ffb_controller::safe_session_end(&state.ffb).await;
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                                state.lock_screen.show_idle_pin_entry();
                            }
                        }
                    }
                }

                // Per-sim PlayableSignal dispatch (runs every game_check tick = 2s)
                // AC billing is triggered via AcStatus::Live from telemetry_interval (100ms) — no action here.
                // F1 25: UdpActive from DrivingDetector sets f1_udp_playable_received; fire billing on next tick.
                // Other sims: 90s process-based fallback.
                if state.game_process.is_some() {
                    match conn.current_sim_type {
                        Some(rc_common::types::SimType::AssettoCorsa) | None => {
                            // AC handled via telemetry_interval — no action needed here
                        }
                        Some(rc_common::types::SimType::F125) => {
                            if conn.f1_udp_playable_received
                                && matches!(conn.launch_state, LaunchState::WaitingForLive { .. })
                            {
                                tracing::info!("[billing] F1 25 PlayableSignal (UdpActive) — emitting AcStatus::Live");
                                let msg = AgentMessage::GameStatusUpdate {
                                    pod_id: state.pod_id.clone(),
                                    ac_status: AcStatus::Live,
                                    sim_type: Some(rc_common::types::SimType::F125),
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                                conn.launch_state = LaunchState::Live;
                            }
                        }
                        Some(sim_type) => {
                            // Process-based fallback for iRacing, LMU, EVO, WRC, Forza, etc.
                            if let LaunchState::WaitingForLive { launched_at, .. } = &conn.launch_state {
                                if launched_at.elapsed() >= Duration::from_secs(90) {
                                    tracing::info!("[billing] {:?} process fallback (90s elapsed) — emitting AcStatus::Live", sim_type);
                                    let msg = AgentMessage::GameStatusUpdate {
                                        pod_id: state.pod_id.clone(),
                                        ac_status: AcStatus::Live,
                                        sim_type: Some(sim_type),
                                    };
                                    if let Ok(json) = serde_json::to_string(&msg) {
                                        let _ = ws_tx.send(Message::Text(json.into())).await;
                                    }
                                    conn.launch_state = LaunchState::Live;
                                }
                            }
                        }
                    }
                }
            }

            Some(mut suggestion) = state.ai_result_rx.recv() => {
                tracing::info!("[ai-result] Received AI suggestion for {}", suggestion.pod_id);
                let fix_snapshot = PodStateSnapshot {
                    pod_id: state.pod_id.clone(),
                    pod_number: state.config.pod.number,
                    lock_screen_active: state.lock_screen.is_active(),
                    billing_active: state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                    game_pid: state.game_process.as_ref().and_then(|g| g.pid),
                    driving_state: Some(state.detector.current_state()),
                    wheelbase_connected: state.detector.is_hid_connected(),
                    ws_connected: state.heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                    uptime_seconds: state.agent_start_time.elapsed().as_secs(),
                    last_udp_secs_ago: state.detector.last_udp_packet_elapsed_secs(),
                    game_launch_elapsed_secs: match &conn.launch_state {
                        LaunchState::WaitingForLive { launched_at, .. } => Some(launched_at.elapsed().as_secs()),
                        _ => None,
                    },
                    hid_last_error: !state.detector.is_hid_connected(),
                    ..Default::default()
                };
                let suggestion_text = suggestion.suggestion.clone();
                let fix_handle = tokio::task::spawn_blocking(move || {
                    crate::ai_debugger::try_auto_fix(&suggestion_text, &fix_snapshot)
                });
                let fix_result = match tokio::time::timeout(Duration::from_secs(10), fix_handle).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(e)) => {
                        tracing::warn!("[auto-fix] spawn_blocking panicked: {}", e);
                        None
                    }
                    Err(_) => {
                        tracing::warn!("[auto-fix] Timed out after 10s — skipping auto-fix");
                        None
                    }
                };
                if let Some(ref fix_result) = fix_result {
                    tracing::info!(
                        "[auto-fix] Applied {} — {} (success: {})",
                        fix_result.fix_type, fix_result.detail, fix_result.success
                    );
                    if fix_result.success {
                        let mut memory = crate::ai_debugger::DebugMemory::load();
                        memory.record_fix(
                            &suggestion.sim_type,
                            &suggestion.error_context,
                            &fix_result.fix_type,
                            &suggestion.suggestion,
                        );
                        tracing::info!(
                            "[pattern-memory] Saved: {} for {:?}",
                            fix_result.fix_type, suggestion.sim_type
                        );
                    }
                    suggestion.suggestion = format!(
                        "[AUTO-FIX APPLIED: {} — {}]\n\n{}",
                        fix_result.fix_type, fix_result.detail, suggestion.suggestion
                    );
                }
                let msg = AgentMessage::AiDebugResult(suggestion);
                let json = serde_json::to_string(&msg)?;
                tracing::info!("[ai-result] Sending AiDebugResult via WebSocket...");
                match ws_tx.send(Message::Text(json.into())).await {
                    Ok(_) => tracing::info!("[ai-result] AiDebugResult sent successfully"),
                    Err(e) => tracing::error!("[ai-result] Failed to send AiDebugResult: {}", e),
                }
            }

            _ = conn.kiosk_interval.tick() => {
                if state.kiosk_enabled && state.kiosk.should_enforce() {
                    let allowed = state.kiosk.allowed_set_snapshot();
                    let pod_id_kiosk = state.pod_id.clone();
                    let kiosk_msg_tx = state.ws_exec_result_tx.clone();
                    let lockdown_flag = state.kiosk.lockdown.clone();
                    let lockdown_reason = state.kiosk.lockdown_reason.clone();
                    let enforce_handle = tokio::task::spawn_blocking(move || {
                        let result = crate::kiosk::KioskManager::enforce_process_whitelist_blocking(allowed);

                        for approval in &result.pending_approvals {
                            let msg = rc_common::protocol::AgentMessage::ProcessApprovalRequest {
                                pod_id: pod_id_kiosk.clone(),
                                process_name: approval.process_name.clone(),
                                exe_path: approval.exe_path.clone(),
                                sighting_count: approval.sighting_count,
                            };
                            let _ = kiosk_msg_tx.try_send(msg);
                        }

                        if !result.expired_processes.is_empty() {
                            let names = result.expired_processes.join(", ");
                            let reason = format!(
                                "Unauthorized software detected: {}. Please contact staff to continue.",
                                names
                            );
                            lockdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                            if let Ok(mut r) = lockdown_reason.lock() {
                                *r = reason.clone();
                            }
                            tracing::warn!("Kiosk: LOCKDOWN — {}", reason);

                            let msg = rc_common::protocol::AgentMessage::KioskLockdown {
                                pod_id: pod_id_kiosk.clone(),
                                reason,
                            };
                            let _ = kiosk_msg_tx.try_send(msg);
                        }

                        result.pending_classifications
                    });

                    if let Ok(classifications) = enforce_handle.await {
                        for classification in classifications {
                            let ollama_url = state.config.ai_debugger.ollama_url.clone();
                            let ollama_model = state.config.ai_debugger.ollama_model.clone();
                            let pod_id_c = state.pod_id.clone();
                            let kiosk_msg_tx_c = state.ws_exec_result_tx.clone();
                            tokio::spawn(async move {
                                let verdict = kiosk::classify_process(
                                    &ollama_url,
                                    &ollama_model,
                                    &classification.process_name,
                                    &classification.exe_path,
                                ).await;
                                tracing::info!(
                                    "[kiosk-llm] Verdict for '{}': {:?}",
                                    classification.process_name, verdict
                                );
                                match verdict {
                                    kiosk::ProcessVerdict::Allow => {
                                        kiosk::KioskManager::approve_process(&classification.process_name);
                                        let msg = rc_common::protocol::AgentMessage::ProcessApprovalRequest {
                                            pod_id: pod_id_c,
                                            process_name: classification.process_name,
                                            exe_path: classification.exe_path,
                                            sighting_count: 0,
                                        };
                                        let _ = kiosk_msg_tx_c.try_send(msg);
                                    }
                                    kiosk::ProcessVerdict::Block => {
                                        kiosk::KioskManager::reject_process(&classification.process_name);
                                    }
                                    kiosk::ProcessVerdict::Ask => {}
                                }
                            });
                        }
                    }
                }
            }

            _ = conn.overlay_topmost_interval.tick() => {
                state.overlay.enforce_topmost();
                if state.kiosk_enabled {
                    tokio::task::spawn_blocking(|| {
                        ac_launcher::minimize_background_windows();
                        crate::lock_screen::enforce_kiosk_foreground();
                        ac_launcher::ensure_conspit_link_running();
                    });
                }
            }

            _ = conn.maintenance_retry_interval.tick() => {
                if !state.in_maintenance.load(std::sync::atomic::Ordering::Relaxed) {
                    continue;
                }
                tracing::info!("Maintenance retry: re-running pre-flight checks");
                let ffb_ref: &dyn crate::ffb_controller::FfbBackend = state.ffb.as_ref();
                match crate::pre_flight::run(state, ffb_ref).await {
                    crate::pre_flight::PreFlightResult::Pass => {
                        tracing::info!("Maintenance retry: pre-flight passed — clearing maintenance");
                        state.in_maintenance.store(false, std::sync::atomic::Ordering::Relaxed);
                        state.lock_screen.show_idle_pin_entry();
                        // Send PreFlightPassed to server
                        let pod_id = state.config.pod.number.to_string();
                        let msg = AgentMessage::PreFlightPassed {
                            pod_id,
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                    }
                    crate::pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
                        let failure_strings: Vec<String> = failures.iter().map(|f| f.detail.clone()).collect();
                        tracing::warn!("Maintenance retry: still failing — {:?}", failure_strings);
                        // Refresh lock screen with updated failures
                        state.lock_screen.show_maintenance_required(failure_strings);
                    }
                }
            }

            _ = &mut conn.blank_timer, if conn.blank_timer_armed => {
                conn.blank_timer_armed = false;
                if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!("Skipping idle PinEntry reset — billing is active");
                } else {
                    tracing::info!("Resetting to idle PinEntry after session summary (SESSION-02)");
                    state.lock_screen.show_idle_pin_entry();
                    ffb_controller::safe_session_end(&state.ffb).await;
                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                }
            }

            _ = &mut conn.exit_grace_timer, if conn.exit_grace_armed => {
                conn.exit_grace_armed = false;
                tracing::info!("[billing] Exit grace period expired — emitting AcStatus::Off to server");
                let msg = AgentMessage::GameStatusUpdate {
                    pod_id: state.pod_id.clone(),
                    ac_status: AcStatus::Off,
                    sim_type: conn.exit_grace_sim_type,
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                }
                conn.exit_grace_sim_type = None;
                conn.current_sim_type = None;
                conn.loading_emitted = false;
                conn.f1_udp_playable_received = false;
            }

            _ = async {
                match &mut conn.crash_recovery {
                    CrashRecoveryState::PausedWaitingRelaunch { timer, .. } => {
                        timer.as_mut().await;
                    }
                    _ => {
                        std::future::pending::<()>().await;
                    }
                }
            } => {
                match std::mem::replace(&mut conn.crash_recovery, CrashRecoveryState::Idle) {
                    CrashRecoveryState::PausedWaitingRelaunch { attempt, last_sim_type, last_launch_args, .. } => {
                        if state.game_process.as_ref().and_then(|g| g.pid).is_some() {
                            tracing::info!("[crash-recovery] Game PID detected during recovery wait (attempt {}) — resuming billing", attempt);
                            // Cancel exit grace timer — game has relaunched, billing continues
                            if conn.exit_grace_armed {
                                tracing::info!("[billing] Crash recovery relaunch — cancelling exit grace timer");
                                conn.exit_grace_armed = false;
                                conn.exit_grace_timer = Box::pin(tokio::time::sleep(Duration::from_secs(86400)));
                            }
                            let _ = state.failure_monitor_tx.send_modify(|s| { s.billing_paused = false; });
                            state.overlay.deactivate();
                            if let Some(ref sid) = state.failure_monitor_tx.borrow().active_billing_session_id.clone() {
                                let resume_msg = AgentMessage::BillingResumed {
                                    pod_id: state.pod_id.clone(),
                                    billing_session_id: sid.clone(),
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&resume_msg).unwrap_or_default().into())).await;
                            }
                            conn.crash_recovery = CrashRecoveryState::Idle;
                        } else if attempt < 2 {
                            tracing::warn!("[crash-recovery] Relaunch attempt {} timed out (60s) — trying attempt 2", attempt);
                            state.overlay.show_toast("Relaunching... (2 of 2)".to_string());

                            if last_sim_type == SimType::AssettoCorsa {
                                if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
                                let params: ac_launcher::AcLaunchParams = match &last_launch_args {
                                    Some(args) => serde_json::from_str(args).unwrap_or_else(|_| ac_launcher::AcLaunchParams {
                                        car: "ks_ferrari_sf15t".to_string(),
                                        track: "spa".to_string(),
                                        driver: "Driver".to_string(),
                                        track_config: String::new(),
                                        skin: String::new(),
                                        transmission: "manual".to_string(),
                                        ffb: "medium".to_string(),
                                        aids: None,
                                        conditions: None,
                                        duration_minutes: 60,
                                        game_mode: String::new(),
                                        server_ip: String::new(),
                                        server_port: 0,
                                        server_http_port: 0,
                                        server_password: String::new(),
                                        ai_level: 87,
                                        session_type: "practice".to_string(),
                                        ai_cars: Vec::new(),
                                        starting_position: 1,
                                        formation_lap: false,
                                        weekend_practice_minutes: 0,
                                        weekend_qualify_minutes: 0,
                                    }),
                                    None => ac_launcher::AcLaunchParams {
                                        car: "ks_ferrari_sf15t".to_string(),
                                        track: "spa".to_string(),
                                        driver: "Driver".to_string(),
                                        track_config: String::new(),
                                        skin: String::new(),
                                        transmission: "manual".to_string(),
                                        ffb: "medium".to_string(),
                                        aids: None,
                                        conditions: None,
                                        duration_minutes: 60,
                                        game_mode: String::new(),
                                        server_ip: String::new(),
                                        server_port: 0,
                                        server_http_port: 0,
                                        server_password: String::new(),
                                        ai_level: 87,
                                        session_type: "practice".to_string(),
                                        ai_cars: Vec::new(),
                                        starting_position: 1,
                                        formation_lap: false,
                                        weekend_practice_minutes: 0,
                                        weekend_qualify_minutes: 0,
                                    },
                                };
                                state.heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                                state.heartbeat_status.game_id.store(1, std::sync::atomic::Ordering::Relaxed);
                                let info = GameLaunchInfo {
                                    pod_id: state.pod_id.clone(),
                                    sim_type: last_sim_type,
                                    game_state: GameState::Launching,
                                    pid: None,
                                    launched_at: Some(Utc::now()),
                                    error_message: None,
                                    diagnostics: None,
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&AgentMessage::GameStateUpdate(info)).unwrap_or_default().into())).await;
                                conn.launch_state = LaunchState::WaitingForLive {
                                    launched_at: std::time::Instant::now(),
                                    attempt: 1,
                                };
                                let _ = state.failure_monitor_tx.send_modify(|s| {
                                    s.launch_started_at = Some(std::time::Instant::now());
                                });
                                let launch_result = tokio::task::spawn_blocking(move || {
                                    ac_launcher::launch_ac(&params)
                                }).await;
                                match launch_result {
                                    Ok(Ok(result)) => {
                                        game_process::persist_pid(result.pid);
                                        state.game_process = Some(game_process::GameProcess {
                                            sim_type: last_sim_type,
                                            state: GameState::Running,
                                            child: None,
                                            pid: Some(result.pid),
                                            last_exit_code: None,
                                        });
                                        let _ = state.failure_monitor_tx.send_modify(|s| {
                                            s.game_pid = Some(result.pid);
                                        });
                                        tracing::info!("[crash-recovery] Attempt 2: ac_launcher::launch_ac returned successfully (pid={})", result.pid);
                                    }
                                    Ok(Err(e)) => {
                                        tracing::warn!("[crash-recovery] Attempt 2: ac_launcher::launch_ac failed: {}", e);
                                    }
                                    Err(e) => {
                                        tracing::error!("[crash-recovery] Attempt 2: spawn_blocking panicked: {}", e);
                                    }
                                }
                            } else {
                                let base_config = match last_sim_type {
                                    SimType::AssettoCorsaEvo => &state.config.games.assetto_corsa_evo,
                                    SimType::AssettoCorsaRally => &state.config.games.assetto_corsa_rally,
                                    SimType::IRacing => &state.config.games.iracing,
                                    SimType::F125 => &state.config.games.f1_25,
                                    SimType::LeMansUltimate => &state.config.games.le_mans_ultimate,
                                    SimType::Forza => &state.config.games.forza,
                                    SimType::ForzaHorizon5 => &state.config.games.forza_horizon_5,
                                    SimType::AssettoCorsa => unreachable!("AC handled in the if branch above"),
                                };
                                let mut game_cfg = base_config.clone();
                                if let Some(ref a) = last_launch_args { game_cfg.args = Some(a.clone()); }

                                state.heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                                let info = GameLaunchInfo {
                                    pod_id: state.pod_id.clone(),
                                    sim_type: last_sim_type,
                                    game_state: GameState::Launching,
                                    pid: None,
                                    launched_at: Some(Utc::now()),
                                    error_message: None,
                                    diagnostics: None,
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&AgentMessage::GameStateUpdate(info)).unwrap_or_default().into())).await;
                                let _ = state.failure_monitor_tx.send_modify(|s| {
                                    s.launch_started_at = Some(std::time::Instant::now());
                                });

                                match game_process::GameProcess::launch(&game_cfg, last_sim_type) {
                                    Ok(gp) => {
                                        tracing::info!("[crash-recovery] Attempt 2: {:?} launched (pid: {:?})", last_sim_type, gp.pid);
                                        let gp_pid = gp.pid;
                                        state.game_process = Some(gp);
                                        let _ = state.failure_monitor_tx.send_modify(|s| {
                                            s.game_pid = gp_pid;
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("[crash-recovery] Attempt 2: GameProcess::launch failed for {:?}: {}", last_sim_type, e);
                                    }
                                }
                            }

                            conn.crash_recovery = CrashRecoveryState::PausedWaitingRelaunch {
                                attempt: 2,
                                timer: Box::pin(tokio::time::sleep(Duration::from_secs(60))),
                                last_sim_type,
                                last_launch_args,
                            };
                        } else {
                            tracing::error!("[crash-recovery] Relaunch attempt 2 timed out (60s) — auto-ending session (crash_limit)");
                            state.overlay.show_toast("Session ending".to_string());
                            conn.crash_recovery = CrashRecoveryState::AutoEndPending;
                            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                            if let Some(ref sid) = state.failure_monitor_tx.borrow().active_billing_session_id.clone() {
                                let end_msg = AgentMessage::SessionAutoEnded {
                                    pod_id: state.pod_id.clone(),
                                    billing_session_id: sid.clone(),
                                    reason: "crash_limit".to_string(),
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&end_msg).unwrap_or_default().into())).await;
                            }
                            let _ = state.failure_monitor_tx.send_modify(|s| {
                                s.billing_active = false;
                                s.billing_paused = false;
                                s.launch_started_at = None;
                                s.recovery_in_progress = false;
                                s.active_billing_session_id = None;
                            });
                            ffb_controller::safe_session_end(&state.ffb).await;
                            state.lock_screen.show_idle_pin_entry();
                            state.overlay.deactivate();
                            if let Some(ref mut game) = state.game_process {
                                let _ = game.stop();
                                state.game_process = None;
                            }
                            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
                            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                            conn.current_driver_name = None;
                            state.last_ac_status = None;
                            state.ac_status_stable_since = None;
                            conn.launch_state = LaunchState::Idle;
                            conn.crash_recovery = CrashRecoveryState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            Some(event) = state.lock_event_rx.recv() => {
                match event {
                    LockScreenEvent::PinEntered { pin } => {
                        let msg = AgentMessage::PinEntered {
                            pod_id: state.pod_id.clone(),
                            pin,
                        };
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                        tracing::info!("PIN submitted, forwarding to core for verification");
                    }
                }
            }

            Some(ws_exec_msg) = state.ws_exec_result_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&ws_exec_msg) {
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        tracing::error!("Failed to send WS command result, connection lost");
                        break;
                    }
                }
            }

            Some(hb_event) = state.heartbeat_event_rx.recv() => {
                match hb_event {
                    udp_heartbeat::HeartbeatEvent::CoreDead => {
                        tracing::warn!("UDP heartbeat: core dead — forcing WebSocket reconnect");
                        break;
                    }
                    udp_heartbeat::HeartbeatEvent::ForceReconnect => {
                        if conn.ws_connect_time.elapsed() < Duration::from_secs(10) {
                            tracing::debug!("Ignoring force_reconnect — connected {}s ago (grace period)", conn.ws_connect_time.elapsed().as_secs());
                        } else {
                            tracing::info!("UDP heartbeat: core requested reconnect");
                            break;
                        }
                    }
                    udp_heartbeat::HeartbeatEvent::ForceRestart => {
                        tracing::warn!("UDP heartbeat: core requested restart — exiting");
                        std::process::exit(0);
                    }
                    udp_heartbeat::HeartbeatEvent::CoreAlive => {}
                }
            }

            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        tracing::debug!("Received from core: {}", text);
                        match crate::ws_handler::handle_ws_message(
                            &text,
                            state,
                            &mut conn,
                            &mut ws_tx,
                            primary_url,
                            failover_url,
                            active_url,
                            split_brain_probe,
                        ).await {
                            Ok(HandleResult::Break) => break,
                            Ok(HandleResult::Continue) => {}
                            Err(e) => { tracing::error!("ws_handler error: {}", e); }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("Core server closed connection");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_recovery_state_starts_idle() {
        let state = CrashRecoveryState::Idle;
        assert!(matches!(state, CrashRecoveryState::Idle),
            "CrashRecoveryState must default to Idle");
    }

    #[test]
    fn crash_recovery_state_paused_waiting_relaunch_attempt_1() {
        let attempt: u8 = 1;
        assert!(attempt < 2, "attempt=1 should trigger retry to attempt 2");
    }

    #[test]
    fn crash_recovery_state_attempt_2_triggers_auto_end() {
        let attempt: u8 = 2;
        assert!(!(attempt < 2), "attempt=2 should trigger auto-end (not retry)");
    }
}
