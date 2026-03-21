use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use futures_util::SinkExt;
use tokio::sync::{RwLock, Semaphore};
use tokio_tungstenite::tungstenite::Message;

use crate::ac_launcher;
use crate::ai_debugger::PodStateSnapshot;
use crate::app_state::AppState;
use crate::ffb_controller;
use crate::game_process;
use crate::kiosk;
use crate::pre_flight;
use crate::self_monitor;
use crate::self_test;
use crate::event_loop::{ConnectionState, CrashRecoveryState, LaunchState};
use rc_common::protocol::{AgentMessage, CoreToAgentMessage};
use rc_common::types::*;

/// Type alias for the WebSocket send half.
pub type WsTx = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    tokio_tungstenite::tungstenite::Message,
>;

/// Result returned by handle_ws_message to signal loop control.
pub enum HandleResult {
    Continue,
    Break,
}

/// Independent semaphore for WS command execution (WSEX-02).
pub(crate) const WS_MAX_CONCURRENT_EXECS: usize = 4;
pub(crate) static WS_EXEC_SEMAPHORE: Semaphore = Semaphore::const_new(WS_MAX_CONCURRENT_EXECS);

/// Handle a WebSocket command request.
pub(crate) async fn handle_ws_exec(
    request_id: String,
    cmd: String,
    timeout_ms: u64,
) -> AgentMessage {
    use tokio::time::{timeout, Duration};

    let permit = match WS_EXEC_SEMAPHORE.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return AgentMessage::ExecResult {
                request_id,
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("WS slots exhausted ({} max)", WS_MAX_CONCURRENT_EXECS),
            };
        }
    };

    let result = timeout(Duration::from_millis(timeout_ms), async {
        let mut cmd_proc = tokio::process::Command::new("cmd");
        cmd_proc.args(["/C", &cmd]).kill_on_drop(true);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd_proc.creation_flags(CREATE_NO_WINDOW);
        }
        cmd_proc.output().await
    })
    .await;

    drop(permit);

    let truncate = |s: String| -> String {
        if s.len() > 65_536 {
            let mut t = s[..65_536].to_string();
            t.push_str("
... [truncated]");
            t
        } else {
            s
        }
    };

    match result {
        Ok(Ok(output)) => AgentMessage::ExecResult {
            request_id,
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: truncate(String::from_utf8_lossy(&output.stdout).to_string()),
            stderr: truncate(String::from_utf8_lossy(&output.stderr).to_string()),
        },
        Ok(Err(e)) => AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to run command: {}", e),
        },
        Err(_) => AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: Some(124),
            stdout: String::new(),
            stderr: format!("Command timed out after {}ms", timeout_ms),
        },
    }
}

/// Dispatch a decoded WebSocket text frame to the appropriate handler.
///
/// Per-connection locals are bundled into ConnectionState (event_loop.rs).
/// SwitchController uses outer-loop URL state for the split-brain guard.
pub async fn handle_ws_message(
    text: &str,
    state: &mut AppState,
    conn: &mut ConnectionState,
    ws_tx: &mut WsTx,
    primary_url: &str,
    failover_url: &Option<String>,
    active_url: &Arc<RwLock<String>>,
    split_brain_probe: &reqwest::Client,
) -> Result<HandleResult> {
    let core_msg = match serde_json::from_str::<CoreToAgentMessage>(text) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to parse CoreToAgentMessage: {} -- raw: {}", e, text);
            return Ok(HandleResult::Continue);
        }
    };

    match core_msg {
        CoreToAgentMessage::BillingStarted {
            billing_session_id, driver_name, allocated_seconds, ..
        } => {
            tracing::info!("Billing started: {} for {} ({}s)", billing_session_id, driver_name, allocated_seconds);

            // Pre-flight gate (PF-01): check hardware before starting session
            if state.config.preflight.enabled {
                let ffb_ref: &dyn crate::ffb_controller::FfbBackend = state.ffb.as_ref();
                match pre_flight::run(state, ffb_ref).await {
                    pre_flight::PreFlightResult::Pass => {
                        tracing::info!("Pre-flight passed, proceeding with session");
                    }
                    pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
                        tracing::warn!("Pre-flight FAILED: {:?}", failures.iter().map(|f| &f.detail).collect::<Vec<_>>());
                        let failure_strings: Vec<String> = failures.iter().map(|f| f.detail.clone()).collect();
                        let pod_id = state.config.pod.number.to_string();
                        let msg = AgentMessage::PreFlightFailed {
                            pod_id,
                            failures: failure_strings,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                        // Do NOT set billing_active, do NOT show active session
                        // Phase 98 will add MaintenanceRequired lock screen state here
                        return Ok(HandleResult::Continue);
                    }
                }
            }

            // --- All code below only runs on Pass (or preflight disabled) ---
            state.heartbeat_status.billing_active.store(true, std::sync::atomic::Ordering::Relaxed);
            conn.blank_timer_armed = false;
            let billing_session_id_clone = billing_session_id.clone();
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.billing_active = true;
                s.active_billing_session_id = Some(billing_session_id_clone);
            });
            conn.current_driver_name = Some(driver_name.clone());
            conn.session_max_speed_kmh = 0.0;
            conn.session_race_position = None;
            if allocated_seconds == 0 || allocated_seconds >= 10800 {
                state.overlay.activate_v2(driver_name.clone());
            } else {
                state.overlay.activate(driver_name.clone(), allocated_seconds);
            }
            state.lock_screen.show_active_session(driver_name, allocated_seconds, allocated_seconds);
            tokio::task::spawn_blocking(|| ac_launcher::minimize_background_windows());
        }

        CoreToAgentMessage::BillingTick {
            remaining_seconds, allocated_seconds: _, driver_name: _,
            elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_next_tier, ..
        } => {
            state.lock_screen.update_remaining(remaining_seconds);
            if let (Some(elapsed), Some(cost), Some(rate)) = (elapsed_seconds, cost_paise, rate_per_min_paise) {
                state.overlay.update_billing_v2(elapsed, cost, rate, paused.unwrap_or(false), minutes_to_next_tier);
            } else {
                state.overlay.update_billing(remaining_seconds);
            }
        }

        CoreToAgentMessage::BillingStopped { billing_session_id } => {
            tracing::info!("Billing stopped: {}", billing_session_id);
            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
            state.overlay.deactivate();
            state.last_ac_status = None;
            state.ac_status_stable_since = None;
            conn.launch_state = LaunchState::Idle;
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.billing_active = false;
                s.active_billing_session_id = None;
                s.billing_paused = false;
                s.launch_started_at = None;
            });
            ffb_controller::safe_session_end(&state.ffb).await;
            state.lock_screen.show_active_session("Session Complete!".to_string(), 0, 0);
            if let Some(ref mut game) = state.game_process { let _ = game.stop(); state.game_process = None; }
            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
            let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
            let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
            conn.current_driver_name = None;
        }

        CoreToAgentMessage::SessionEnded {
            billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
        } => {
            tracing::info!("Session ended: {} -- {} laps, best: {:?}, {}s", billing_session_id, total_laps, best_lap_ms, driving_seconds);
            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
            conn.crash_recovery = CrashRecoveryState::Idle;
            state.overlay.deactivate();
            state.last_ac_status = None;
            state.ac_status_stable_since = None;
            conn.launch_state = LaunchState::Idle;
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.billing_active = false;
                s.active_billing_session_id = None;
                s.billing_paused = false;
                s.launch_started_at = None;
                s.recovery_in_progress = false;
            });
            ffb_controller::safe_session_end(&state.ffb).await;
            state.lock_screen.show_session_summary(
                driver_name, total_laps, best_lap_ms, driving_seconds,
                if conn.session_max_speed_kmh > 0.0 { Some(conn.session_max_speed_kmh) } else { None },
                conn.session_race_position,
            );
            if let Some(ref mut game) = state.game_process { let _ = game.stop(); state.game_process = None; }
            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
            let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
            let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
            conn.current_driver_name = None;
            conn.blank_timer.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_secs(30));
            conn.blank_timer_armed = true;
        }

        CoreToAgentMessage::LaunchGame { sim_type: launch_sim, launch_args } => {
            tracing::info!("Launching game: {:?} (args: {:?})", launch_sim, launch_args);
            conn.last_launch_args_stored = launch_args.clone();
            // Track current sim_type for per-sim PlayableSignal dispatch
            conn.current_sim_type = Some(launch_sim);
            conn.loading_emitted = false;
            conn.f1_udp_playable_received = false;

            if launch_sim == SimType::AssettoCorsa {
                if let Some(ref mut adp) = state.adapter { adp.disconnect(); }

                let params: ac_launcher::AcLaunchParams = match &launch_args {
                    Some(args) => serde_json::from_str(args).unwrap_or_else(|_| ac_launcher::AcLaunchParams {
                        car: "ks_ferrari_sf15t".to_string(), track: "spa".to_string(),
                        driver: "Driver".to_string(), track_config: String::new(), skin: String::new(),
                        transmission: "manual".to_string(), ffb: "medium".to_string(),
                        aids: None, conditions: None, duration_minutes: 60, game_mode: String::new(),
                        server_ip: String::new(), server_port: 0, server_http_port: 0,
                        server_password: String::new(), ai_level: 87,
                        session_type: "practice".to_string(), ai_cars: Vec::new(),
                        starting_position: 1, formation_lap: false,
                        weekend_practice_minutes: 0, weekend_qualify_minutes: 0,
                    }),
                    None => ac_launcher::AcLaunchParams {
                        car: "ks_ferrari_sf15t".to_string(), track: "spa".to_string(),
                        driver: "Driver".to_string(), track_config: String::new(), skin: String::new(),
                        transmission: "manual".to_string(), ffb: "medium".to_string(),
                        aids: None, conditions: None, duration_minutes: 60, game_mode: String::new(),
                        server_ip: String::new(), server_port: 0, server_http_port: 0,
                        server_password: String::new(), ai_level: 87,
                        session_type: "practice".to_string(), ai_cars: Vec::new(),
                        starting_position: 1, formation_lap: false,
                        weekend_practice_minutes: 0, weekend_qualify_minutes: 0,
                    },
                };

                state.heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                state.heartbeat_status.game_id.store(match launch_sim {
                    SimType::AssettoCorsa => 1, SimType::F125 => 2, SimType::IRacing => 3,
                    SimType::LeMansUltimate => 4, SimType::Forza => 5, SimType::AssettoCorsaEvo => 6,
                    SimType::AssettoCorsaRally => 7, SimType::ForzaHorizon5 => 8,
                }, std::sync::atomic::Ordering::Relaxed);

                let splash_name = conn.current_driver_name.clone().unwrap_or_else(|| "Driver".to_string());
                state.lock_screen.show_launch_splash(splash_name);

                let info = GameLaunchInfo {
                    pod_id: state.pod_id.clone(), sim_type: launch_sim,
                    game_state: GameState::Launching, pid: None,
                    launched_at: Some(Utc::now()), error_message: None, diagnostics: None,
                };
                let msg = AgentMessage::GameStateUpdate(info);
                let json_str = serde_json::to_string(&msg)?;
                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                conn.launch_state = LaunchState::WaitingForLive { launched_at: std::time::Instant::now(), attempt: 1 };
                let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = Some(std::time::Instant::now()); });

                let pod_id_clone = state.pod_id.clone();
                let launch_result = tokio::task::spawn_blocking(move || ac_launcher::launch_ac(&params)).await;
                let launch_result = match launch_result {
                    Ok(r) => r,
                    Err(e) => { tracing::error!("AC launch task panicked: {}", e); Err(anyhow::anyhow!("Launch task panicked: {}", e)) }
                };

                match launch_result {
                    Ok(result) => {
                        if let Ok(mut err_slot) = state.last_launch_error.lock() { *err_slot = result.cm_error.clone(); }
                        let info = GameLaunchInfo {
                            pod_id: pod_id_clone.clone(), sim_type: launch_sim,
                            game_state: GameState::Running, pid: Some(result.pid),
                            launched_at: Some(Utc::now()), error_message: result.cm_error.clone(),
                            diagnostics: Some(rc_common::types::LaunchDiagnostics {
                                cm_attempted: result.diagnostics.cm_attempted,
                                cm_exit_code: result.diagnostics.cm_exit_code,
                                cm_log_errors: result.diagnostics.cm_log_errors.clone(),
                                fallback_used: result.diagnostics.fallback_used,
                                direct_exit_code: result.diagnostics.direct_exit_code,
                            }),
                        };
                        game_process::persist_pid(result.pid);
                        state.game_process = Some(game_process::GameProcess {
                            sim_type: launch_sim, state: GameState::Running,
                            child: None, pid: Some(result.pid), last_exit_code: None,
                        });
                        let _ = state.failure_monitor_tx.send_modify(|s| { s.game_pid = Some(result.pid); });
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json_str = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json_str.into())).await;

                        if let Some(ref cm_err) = result.cm_error {
                            tracing::error!("[CM_ERROR] CM failure on {}: {}", pod_id_clone, cm_err);
                            if let Ok(mut err_slot) = state.last_launch_error.lock() { *err_slot = Some(cm_err.clone()); }
                            if state.config.ai_debugger.enabled {
                                let err_ctx = format!(
                                    "Content Manager multiplayer launch failed on pod {}. {}. Fell back to direct acs.exe launch.",
                                    pod_id_clone, cm_err
                                );
                                let snapshot = PodStateSnapshot {
                                    pod_id: pod_id_clone.clone(), pod_number: state.config.pod.number,
                                    lock_screen_active: state.lock_screen.is_active(),
                                    billing_active: state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                    game_pid: None, driving_state: Some(state.detector.current_state()),
                                    wheelbase_connected: state.detector.is_hid_connected(),
                                    ws_connected: state.heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                    uptime_seconds: state.agent_start_time.elapsed().as_secs(),
                                    ..Default::default()
                                };
                                tokio::spawn(crate::ai_debugger::analyze_crash(
                                    state.config.ai_debugger.clone(), pod_id_clone.clone(),
                                    launch_sim, err_ctx, snapshot, state.ai_result_tx.clone(),
                                ));
                            }
                        }

                        if let Some(ref mut adp) = state.adapter {
                            match adp.connect() {
                                Ok(()) => tracing::info!("Reconnected to AC telemetry"),
                                Err(e) => tracing::warn!("Could not reconnect telemetry: {}", e),
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("AC launch failed: {}", e);
                        if let Ok(mut err_slot) = state.last_launch_error.lock() {
                            *err_slot = Some(format!("Launch failed: {}", e));
                        }
                        let info = GameLaunchInfo {
                            pod_id: pod_id_clone.clone(), sim_type: launch_sim,
                            game_state: GameState::Error, pid: None, launched_at: None,
                            error_message: Some(e.to_string()), diagnostics: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json_str = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        if state.config.ai_debugger.enabled {
                            let err_ctx = format!("AC launch completely failed on pod {}: {}", pod_id_clone, e);
                            let snapshot = PodStateSnapshot {
                                pod_id: pod_id_clone.clone(), pod_number: state.config.pod.number,
                                lock_screen_active: state.lock_screen.is_active(),
                                billing_active: state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                game_pid: None, driving_state: Some(state.detector.current_state()),
                                wheelbase_connected: state.detector.is_hid_connected(),
                                ws_connected: state.heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                uptime_seconds: state.agent_start_time.elapsed().as_secs(),
                                ..Default::default()
                            };
                            tokio::spawn(crate::ai_debugger::analyze_crash(
                                state.config.ai_debugger.clone(), pod_id_clone,
                                launch_sim, err_ctx, snapshot, state.ai_result_tx.clone(),
                            ));
                        }
                    }
                }
            } else {
                // Generic launch for other sims (F1 25, iRacing, LMU, Forza, AC Evo/Rally)
                let base_config = match launch_sim {
                    SimType::AssettoCorsa => &state.config.games.assetto_corsa,
                    SimType::AssettoCorsaEvo => &state.config.games.assetto_corsa_evo,
                    SimType::AssettoCorsaRally => &state.config.games.assetto_corsa_rally,
                    SimType::IRacing => &state.config.games.iracing,
                    SimType::F125 => &state.config.games.f1_25,
                    SimType::LeMansUltimate => &state.config.games.le_mans_ultimate,
                    SimType::Forza => &state.config.games.forza,
                    SimType::ForzaHorizon5 => &state.config.games.forza_horizon_5,
                };
                let mut game_config = base_config.clone();
                if let Some(args) = launch_args { game_config.args = Some(args); }

                state.heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                state.heartbeat_status.game_id.store(match launch_sim {
                    SimType::AssettoCorsa => 1, SimType::F125 => 2, SimType::IRacing => 3,
                    SimType::LeMansUltimate => 4, SimType::Forza => 5, SimType::AssettoCorsaEvo => 6,
                    SimType::AssettoCorsaRally => 7, SimType::ForzaHorizon5 => 8,
                }, std::sync::atomic::Ordering::Relaxed);

                let splash_name = conn.current_driver_name.clone().unwrap_or_else(|| "Driver".to_string());
                state.lock_screen.show_launch_splash(splash_name);
                conn.launch_state = LaunchState::WaitingForLive { launched_at: std::time::Instant::now(), attempt: 1 };
                let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = Some(std::time::Instant::now()); });

                let launching_info = GameLaunchInfo {
                    pod_id: state.pod_id.clone(), sim_type: launch_sim,
                    game_state: GameState::Launching, pid: None,
                    launched_at: Some(Utc::now()), error_message: None, diagnostics: None,
                };
                let msg = AgentMessage::GameStateUpdate(launching_info);
                let json_str = serde_json::to_string(&msg)?;
                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                match game_process::GameProcess::launch(&game_config, launch_sim) {
                    Ok(gp) => {
                        tracing::info!("Generic sim {:?} launched (pid: {:?})", launch_sim, gp.pid);
                        let gp_pid = gp.pid;
                        state.game_process = Some(gp);
                        let _ = state.failure_monitor_tx.send_modify(|s| { s.game_pid = gp_pid; });
                        if let Some(pid) = gp_pid {
                            let info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(), sim_type: launch_sim,
                                game_state: GameState::Running, pid: Some(pid),
                                launched_at: Some(Utc::now()), error_message: None, diagnostics: None,
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json_str = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to launch {:?}: {}", launch_sim, e);
                        state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                        state.heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                        conn.launch_state = LaunchState::Idle;
                        let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = None; });
                        let info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(), sim_type: launch_sim,
                            game_state: GameState::Error, pid: None, launched_at: None,
                            error_message: Some(e.to_string()), diagnostics: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json_str = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json_str.into())).await;
                    }
                }
            }
        }

        CoreToAgentMessage::StopGame => {
            state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
            state.heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
            state.last_ac_status = None;
            state.ac_status_stable_since = None;
            conn.launch_state = LaunchState::Idle;
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.recovery_in_progress = true;
                s.launch_started_at = None;
            });
            ffb_controller::safe_session_end(&state.ffb).await;
            let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
            let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
            if let Some(ref mut game) = state.game_process {
                tracing::info!("Stopping game: {:?}", game.sim_type);
                let sim = game.sim_type;
                match game.stop() {
                    Ok(()) => {
                        let info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(), sim_type: sim, game_state: GameState::Idle,
                            pid: None, launched_at: None, error_message: None, diagnostics: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                    Err(e) => { tracing::error!("Failed to stop game: {}", e); }
                }
                state.game_process = None;
            }
        }

        CoreToAgentMessage::ShowPinLockScreen {
            token_id, driver_name, pricing_tier_name, allocated_seconds
        } => {
            tracing::info!("Lock screen: PIN entry for {}", driver_name);
            state.lock_screen.show_pin_screen(token_id, driver_name, pricing_tier_name, allocated_seconds);
        }

        CoreToAgentMessage::ShowQrLockScreen {
            token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds
        } => {
            tracing::info!("Lock screen: QR display for {}", driver_name);
            state.lock_screen.show_qr_screen(token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds);
        }

        CoreToAgentMessage::ClearLockScreen => {
            tracing::info!("Lock screen cleared");
            state.overlay.deactivate();
            state.lock_screen.clear();
        }

        CoreToAgentMessage::BlankScreen => {
            if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                tracing::warn!("Ignoring BlankScreen -- billing is active");
            } else {
                tracing::info!("Screen blanked via direct command");
                state.overlay.deactivate();
                state.lock_screen.show_blank_screen();
            }
        }

        CoreToAgentMessage::SubSessionEnded {
            billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
            wallet_balance_paise, current_split_number, total_splits,
        } => {
            tracing::info!(
                "Sub-session ended: {} -- split {}/{}, {} laps, wallet: {}p",
                billing_session_id, current_split_number, total_splits, total_laps, wallet_balance_paise
            );
            conn.crash_recovery = CrashRecoveryState::Idle;
            state.overlay.deactivate();
            state.last_ac_status = None;
            state.ac_status_stable_since = None;
            conn.launch_state = LaunchState::Idle;
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.launch_started_at = None;
                s.recovery_in_progress = false;
            });
            ffb_controller::safe_session_end(&state.ffb).await;
            state.lock_screen.show_between_sessions(
                driver_name, total_laps, best_lap_ms, driving_seconds,
                wallet_balance_paise, current_split_number, total_splits,
            );
            if let Some(ref mut game) = state.game_process { let _ = game.stop(); state.game_process = None; }
            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
            let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
            let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
        }

        CoreToAgentMessage::ShowAssistanceScreen { driver_name, message } => {
            tracing::info!("Assistance screen for {}: {}", driver_name, message);
            state.lock_screen.show_assistance(driver_name, message);
        }

        CoreToAgentMessage::EnterDebugMode { employee_name } => {
            tracing::info!("Employee debug mode activated by {}", employee_name);
            state.kiosk.enter_debug_mode();
            state.lock_screen.clear();
        }

        CoreToAgentMessage::SettingsUpdated { settings } => {
            tracing::info!("Kiosk settings updated: {:?}", settings);
            if let Some(v) = settings.get("kiosk_lockdown_enabled") {
                if v == "true" && !state.kiosk.is_active() && !state.kiosk.is_debug_mode() {
                    state.kiosk.activate();
                    tracing::info!("Kiosk lockdown ENABLED via remote settings");
                } else if v == "false" && state.kiosk.is_active() {
                    state.kiosk.deactivate();
                    tracing::info!("Kiosk lockdown DISABLED via remote settings");
                }
            }
            if let Some(v) = settings.get("screen_blanking_enabled") {
                tracing::info!("Screen blanking set to: {}", v);
                let billing_on = state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed);
                if v == "true" && state.lock_screen.is_idle_or_blanked() && !billing_on {
                    state.lock_screen.show_blank_screen();
                    tracing::info!("Screen blanking ENABLED -- screen blanked");
                } else if v == "false" {
                    state.lock_screen.clear();
                    tracing::info!("Screen blanking DISABLED -- screen restored");
                }
            }
            if let Some(url) = settings.get("lock_screen_wallpaper_url") {
                let url_opt = if url.is_empty() { None } else { Some(url.clone()) };
                state.lock_screen.set_wallpaper_url(url_opt);
                tracing::info!("Lock screen wallpaper URL updated");
            }
        }

        CoreToAgentMessage::SetTransmission { transmission } => {
            tracing::info!("Setting transmission to '{}' mid-session (SendInput)", transmission);
            ac_launcher::mid_session::toggle_ac_transmission();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let auto_shifter = state.adapter.as_ref()
                .and_then(|a| a.read_assist_state()).map(|(_, _, a)| a).unwrap_or(false);
            state.overlay.show_toast(if auto_shifter { "Transmission: AUTO".into() } else { "Transmission: MANUAL".into() });
            let confirm = AgentMessage::AssistChanged {
                pod_id: state.pod_id.clone(), assist_type: "transmission".into(),
                enabled: auto_shifter, confirmed: true,
            };
            if let Ok(json) = serde_json::to_string(&confirm) {
                let _ = ws_tx.send(Message::Text(json.into())).await;
            }
        }

        CoreToAgentMessage::SetFfb { preset } => {
            tracing::info!("Setting FFB to '{}' mid-session", preset);
            match preset.as_str() {
                "light" | "medium" | "strong" => conn.last_ffb_preset = preset.clone(),
                _ => {}
            }
            if let Ok(percent) = preset.parse::<u8>() {
                match state.ffb.set_gain(percent) {
                    Ok(true) => {
                        let clamped = percent.clamp(10, 100);
                        conn.last_ffb_percent = clamped;
                        state.overlay.show_toast(format!("FFB: {}%", clamped));
                        let confirm = AgentMessage::FfbGainChanged {
                            pod_id: state.pod_id.clone(), percent: clamped
                        };
                        if let Ok(json) = serde_json::to_string(&confirm) {
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                    }
                    Ok(false) => tracing::warn!("FFB: wheelbase not found for SetFfb"),
                    Err(e) => tracing::error!("FFB gain error: {}", e),
                }
            } else {
                if let Err(e) = ac_launcher::set_ffb(&preset) {
                    tracing::error!("Failed to set FFB (legacy): {}", e);
                }
            }
        }

        CoreToAgentMessage::SetAssist { assist_type, enabled } => {
            tracing::info!("SetAssist: type={}, enabled={}", assist_type, enabled);
            match assist_type.as_str() {
                "abs" => {
                    let current = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(abs, _, _)| abs > 0).unwrap_or(false);
                    if current != enabled {
                        if enabled { ac_launcher::mid_session::toggle_ac_abs(); }
                        else {
                            let level = state.adapter.as_ref()
                                .and_then(|a| a.read_assist_state()).map(|(abs, _, _)| abs).unwrap_or(1);
                            for _ in 0..level {
                                ac_launcher::mid_session::send_ctrl_shift_key(0x41);
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let confirmed_abs = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(abs, _, _)| abs > 0).unwrap_or(false);
                    state.overlay.show_toast(if confirmed_abs { "ABS: ON".into() } else { "ABS: OFF".into() });
                    let confirm = AgentMessage::AssistChanged {
                        pod_id: state.pod_id.clone(), assist_type: "abs".into(),
                        enabled: confirmed_abs, confirmed: true,
                    };
                    if let Ok(json) = serde_json::to_string(&confirm) {
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                }
                "tc" => {
                    let current = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(_, tc, _)| tc > 0).unwrap_or(false);
                    if current != enabled {
                        if enabled { ac_launcher::mid_session::toggle_ac_tc(); }
                        else {
                            let level = state.adapter.as_ref()
                                .and_then(|a| a.read_assist_state()).map(|(_, tc, _)| tc).unwrap_or(1);
                            for _ in 0..level {
                                ac_launcher::mid_session::send_ctrl_shift_key(0x54);
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let confirmed_tc = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(_, tc, _)| tc > 0).unwrap_or(false);
                    state.overlay.show_toast(if confirmed_tc { "TC: ON".into() } else { "TC: OFF".into() });
                    let confirm = AgentMessage::AssistChanged {
                        pod_id: state.pod_id.clone(), assist_type: "tc".into(),
                        enabled: confirmed_tc, confirmed: true,
                    };
                    if let Ok(json) = serde_json::to_string(&confirm) {
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                }
                "transmission" => {
                    let current = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(_, _, auto)| auto).unwrap_or(false);
                    if current != enabled { ac_launcher::mid_session::toggle_ac_transmission(); }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let confirmed = state.adapter.as_ref()
                        .and_then(|a| a.read_assist_state()).map(|(_, _, auto)| auto).unwrap_or(false);
                    state.overlay.show_toast(if confirmed { "Transmission: AUTO".into() } else { "Transmission: MANUAL".into() });
                    let confirm = AgentMessage::AssistChanged {
                        pod_id: state.pod_id.clone(), assist_type: "transmission".into(),
                        enabled: confirmed, confirmed: true,
                    };
                    if let Ok(json) = serde_json::to_string(&confirm) {
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                }
                other => { tracing::warn!("Unknown assist type: {}", other); }
            }
        }

        CoreToAgentMessage::SetFfbGain { percent } => {
            tracing::info!("SetFfbGain: {}%", percent);
            match state.ffb.set_gain(percent) {
                Ok(true) => {
                    let clamped = percent.clamp(10, 100);
                    conn.last_ffb_percent = clamped;
                    state.overlay.show_toast(format!("FFB: {}%", clamped));
                    let confirm = AgentMessage::FfbGainChanged {
                        pod_id: state.pod_id.clone(), percent: clamped,
                    };
                    if let Ok(json) = serde_json::to_string(&confirm) {
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                }
                Ok(false) => tracing::warn!("FFB: wheelbase not found for SetFfbGain"),
                Err(e) => tracing::error!("FFB gain set error: {}", e),
            }
        }

        CoreToAgentMessage::QueryAssistState => {
            let (abs, tc, auto_shifter) = state.adapter.as_ref()
                .and_then(|a| a.read_assist_state()).unwrap_or((0, 0, false));
            let assist_msg = AgentMessage::AssistState {
                pod_id: state.pod_id.clone(), abs, tc, auto_shifter, ffb_percent: conn.last_ffb_percent,
            };
            if let Ok(json) = serde_json::to_string(&assist_msg) {
                let _ = ws_tx.send(Message::Text(json.into())).await;
            }
        }

        CoreToAgentMessage::PinFailed { reason } => {
            tracing::warn!("PIN failed: {}", reason);
            state.lock_screen.show_pin_error(&reason);
        }

        CoreToAgentMessage::Ping { id } => {
            let received_at = std::time::Instant::now();
            let pong = AgentMessage::Pong { id, agent_delay_us: None };
            if let Ok(json) = serde_json::to_string(&pong) {
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    tracing::error!("Failed to send Pong, connection lost");
                    return Ok(HandleResult::Break);
                }
            }
            let process_us = received_at.elapsed().as_micros() as u64;
            if process_us > 5000 {
                tracing::warn!("Pong send took {}us (>5ms)", process_us);
            }
        }

        CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } => {
            tracing::info!("WS command request {}: {}", request_id, cmd);
            let result_tx = state.ws_exec_result_tx.clone();
            tokio::spawn(async move {
                let result = handle_ws_exec(request_id, cmd, timeout_ms).await;
                let _ = result_tx.send(result).await;
            });
        }

        CoreToAgentMessage::ApproveProcess { process_name } => {
            tracing::info!("Server APPROVED process: {}", process_name);
            kiosk::KioskManager::approve_process(&process_name);
            if state.kiosk.is_locked_down() {
                state.kiosk.exit_lockdown();
                state.lock_screen.show_idle_pin_entry();
            }
        }

        CoreToAgentMessage::RejectProcess { process_name } => {
            tracing::warn!("Server REJECTED process: {}", process_name);
            kiosk::KioskManager::reject_process(&process_name);
            let reason = format!(
                "Unauthorized software '{}' detected. Please contact staff.",
                process_name
            );
            state.kiosk.enter_lockdown(&reason);
            state.lock_screen.show_lockdown(&reason);
            let lockdown_msg = AgentMessage::KioskLockdown {
                pod_id: state.pod_id.clone(), reason,
            };
            let _ = state.ws_exec_result_tx.try_send(lockdown_msg);
        }

        CoreToAgentMessage::RunSelfTest { request_id } => {
            tracing::info!("[self-test] RunSelfTest request_id={}", request_id);
            let status_clone = state.heartbeat_status.clone();
            let ollama_url = state.config.ai_debugger.ollama_url.clone();
            let ollama_model = state.config.ai_debugger.ollama_model.clone();
            let result_tx = state.ws_exec_result_tx.clone();
            let pod_id_clone = state.pod_id.clone();
            tokio::spawn(async move {
                let mut report = self_test::run_all_probes(status_clone, &ollama_url).await;
                let verdict = self_test::get_llm_verdict(&ollama_url, &ollama_model, &report.probes).await;
                report.verdict = Some(verdict);
                let report_json = serde_json::to_value(&report).unwrap_or_default();
                let msg = AgentMessage::SelfTestResult {
                    pod_id: pod_id_clone, request_id, report: report_json,
                };
                let _ = result_tx.send(msg).await;
            });
        }

        CoreToAgentMessage::SwitchController { target_url } => {
            // Phase 68: Runtime URL switching
            let is_primary = target_url == primary_url;
            let is_failover = failover_url.as_ref().map_or(false, |f| target_url == *f);

            if !is_primary && !is_failover {
                tracing::warn!(
                    "[switch] Rejected SwitchController -- target_url {:?} does not match primary ({:?}) or failover ({:?})",
                    target_url, primary_url, failover_url
                );
            } else {
                // Phase 69: Split-brain guard -- verify .23 is actually unreachable before switching
                let server_reachable = match split_brain_probe
                    .get("http://192.168.31.23:8090/ping")
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => true,
                    _ => false,
                };

                if server_reachable {
                    tracing::warn!(
                        "[switch] split-brain guard: .23 still reachable, ignoring SwitchController to {}",
                        target_url
                    );
                    // Do NOT switch -- server is still up from this pod's perspective
                } else {
                    tracing::info!(
                        "[switch] split-brain guard passed -- .23 unreachable, accepting switch to {}",
                        target_url
                    );
                    *active_url.write().await = target_url.clone();

                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    state.heartbeat_status.last_switch_ms.store(now_ms, std::sync::atomic::Ordering::Relaxed);

                    self_monitor::log_event(&format!("SWITCH: target={}", target_url));

                    let _ = ws_tx.send(tokio_tungstenite::tungstenite::Message::Close(None)).await;
                    return Ok(HandleResult::Break); // -> outer reconnect loop picks up new URL
                }
            }
        }

        other => {
            tracing::warn!("Unhandled CoreToAgentMessage: {:?}", other);
        }
    }

    Ok(HandleResult::Continue)
}
