use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use futures_util::SinkExt;
use tokio::sync::{RwLock, Semaphore};
use tokio_tungstenite::tungstenite::Message;

use crate::ac_launcher;
#[cfg(feature = "ai-debugger")]
use crate::ai_debugger::PodStateSnapshot;
use crate::app_state::AppState;
use crate::ffb_controller;
use crate::game_process;
use crate::kiosk;
use crate::diagnostic_engine;
use crate::pre_flight;
use crate::self_monitor;
use crate::self_test;
use crate::session_enforcer::{ProcessMonitor, SessionEnforcer};
use crate::event_loop::{ConnectionState, CrashRecoveryState, LaunchState};
use rc_common::protocol::{AgentMessage, CoreMessage, CoreToAgentMessage};
use rc_common::types::*;

const LOG_TARGET: &str = "ws";

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
    #[cfg(feature = "http-client")]
    split_brain_probe: &reqwest::Client,
    #[cfg(not(feature = "http-client"))]
    split_brain_probe: &(),
) -> Result<HandleResult> {
    // DEPLOY-05: Parse via CoreMessage wrapper for command_id deduplication.
    // Falls back to legacy bare CoreToAgentMessage if wrapper parse fails
    // (backward-compat for old server versions during rolling deploy).
    let (command_id, core_msg) = match serde_json::from_str::<CoreMessage>(text) {
        Ok(wrapped) => (wrapped.command_id, wrapped.inner),
        Err(_) => {
            // Try legacy bare format (no wrapper)
            match serde_json::from_str::<CoreToAgentMessage>(text) {
                Ok(m) => (None, m),
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "Failed to parse CoreToAgentMessage: {} -- raw: {}", e, text);
                    return Ok(HandleResult::Continue);
                }
            }
        }
    };

    // DEPLOY-05: Deduplicate commands by command_id within 5-minute TTL window.
    // This prevents stale commands from WS reconnect replaying (e.g. duplicate BillingStarted).
    if let Some(ref cid) = command_id {
        let five_min = std::time::Duration::from_secs(300);
        if let Some(seen_at) = conn.seen_command_ids.get(cid) {
            if seen_at.elapsed() < five_min {
                tracing::debug!(target: LOG_TARGET, "DEPLOY-05: Duplicate command_id {} (age={:?}) — silently acking", cid, seen_at.elapsed());
                return Ok(HandleResult::Continue);
            }
        }
        conn.seen_command_ids.insert(cid.clone(), std::time::Instant::now());

        // Periodic cleanup every 60s (counted by heartbeat ticks in event_loop, approximated here)
        conn.dedup_cleanup_ticks += 1;
        if conn.dedup_cleanup_ticks >= 60 {
            conn.dedup_cleanup_ticks = 0;
            let five_min = std::time::Duration::from_secs(300);
            conn.seen_command_ids.retain(|_, ts| ts.elapsed() < five_min);
            tracing::debug!(target: LOG_TARGET, "DEPLOY-05: Pruned seen_command_ids, {} entries remain", conn.seen_command_ids.len());
        }
    }

    match core_msg {
        CoreToAgentMessage::BillingStarted {
            billing_session_id, driver_name, allocated_seconds, ..
        } => {
            tracing::info!(target: LOG_TARGET, "Billing started: {} for {} ({}s)", billing_session_id, driver_name, allocated_seconds);

            // Pre-flight gate (PF-01): check hardware before starting session
            if state.config.preflight.enabled {
                let ffb_ref: &dyn crate::ffb_controller::FfbBackend = state.ffb.as_ref();
                let ws_elapsed = conn.ws_connect_time.elapsed().as_secs();
                match pre_flight::run(state, ffb_ref, ws_elapsed).await {
                    pre_flight::PreFlightResult::Pass => {
                        tracing::info!(target: LOG_TARGET, "Pre-flight passed, proceeding with session");
                        // STAFF-04: Reset cooldown so next failure sends alert immediately
                        state.last_preflight_alert = None;
                    }
                    pre_flight::PreFlightResult::MaintenanceRequired { failures } => {
                        tracing::warn!(target: LOG_TARGET, "Pre-flight FAILED: {:?}", failures.iter().map(|f| &f.detail).collect::<Vec<_>>());
                        let failure_strings: Vec<String> = failures.iter().map(|f| f.detail.clone()).collect();

                        // STAFF-04: Rate-limit PreFlightFailed alerts (60s cooldown)
                        let should_alert = state.last_preflight_alert
                            .map(|t| t.elapsed() > std::time::Duration::from_secs(60))
                            .unwrap_or(true); // None = never alerted, send it

                        if should_alert {
                            let pod_id = state.config.pod.number.to_string();
                            let msg = AgentMessage::PreFlightFailed {
                                pod_id,
                                failures: failure_strings.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_tx.send(Message::Text(json.into())).await;
                            }
                            state.last_preflight_alert = Some(std::time::Instant::now());
                            tracing::warn!(target: LOG_TARGET, "Pre-flight FAILED — alert sent to racecontrol");
                        } else {
                            tracing::info!(
                                target: LOG_TARGET,
                                "Pre-flight FAILED — alert suppressed (cooldown active, {}s since last)",
                                state.last_preflight_alert.map(|t| t.elapsed().as_secs()).unwrap_or(0)
                            );
                        }

                        // ── MESH-INTEGRATION: Emit DiagnosticEvent for each failed check ──
                        // This bridges pre-flight failures into the Meshed Intelligence tier engine,
                        // enabling autonomous healing via Tier 1 deterministic fixes or AI diagnosis.
                        let pod_state = state.failure_monitor_tx.borrow().clone();
                        for failure in &failures {
                            diagnostic_engine::emit_external_event(
                                &state.diagnostic_event_tx,
                                diagnostic_engine::DiagnosticTrigger::PreFlightFailed {
                                    check_name: failure.name.to_string(),
                                    detail: failure.detail.clone(),
                                },
                                &pod_state,
                            );
                        }

                        // Do NOT set billing_active, do NOT show active session
                        // PF-04: show maintenance required lock screen (always fires, not rate-limited)
                        state.lock_screen.show_maintenance_required(failure_strings);
                        // PF-06: arm maintenance flag so retry loop fires
                        state.in_maintenance.store(true, std::sync::atomic::Ordering::Relaxed);
                        return Ok(HandleResult::Continue);
                    }
                }
            }

            // --- All code below only runs on Pass (or preflight disabled) ---
            state.heartbeat_status.billing_active.store(true, std::sync::atomic::Ordering::Release);
            // MMA-Iter2: Also update the static BILLING_ACTIVE flag used by remote_ops
            // exec_command's RCAGENT_SELF_RESTART guard. Without this, the guard was dead code.
            crate::remote_ops::BILLING_ACTIVE.store(true, std::sync::atomic::Ordering::Release);
            conn.blank_timer_armed = false;
            let billing_session_id_clone = billing_session_id.clone();
            let _ = state.failure_monitor_tx.send_modify(|s| {
                s.billing_active = true;
                s.active_billing_session_id = Some(billing_session_id_clone);
            });
            conn.current_driver_name = Some(driver_name.clone());
            conn.session_max_speed_kmh = 0.0;
            conn.session_race_position = None;
            // BILL-01: Start inactivity monitor for this billing session (default 10 min threshold)
            conn.inactivity_monitor = Some(crate::inactivity_monitor::InactivityMonitor::new(600));
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

        CoreToAgentMessage::BillingCountdownWarning { remaining_secs, level } => {
            // BILL-02: Show persistent countdown warning overlay on customer screen
            let billing_on = state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed);
            if billing_on {
                state.lock_screen.show_countdown_warning(remaining_secs, level.as_str());
            }
        }

        CoreToAgentMessage::BillingStopped { billing_session_id } => {
            tracing::info!(target: LOG_TARGET, "Billing stopped: {}", billing_session_id);
            // BILL-01: Clear inactivity monitor on session end
            if let Some(ref mut inact) = conn.inactivity_monitor { inact.reset(); }
            conn.inactivity_monitor = None;
            // BILL-02: Dismiss countdown warning overlay on session end
            state.lock_screen.dismiss_countdown_warning();
            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Release);
            crate::remote_ops::BILLING_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
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
            tracing::info!(target: LOG_TARGET, "Session ended: {} -- {} laps, best: {:?}, {}s", billing_session_id, total_laps, best_lap_ms, driving_seconds);
            // BILL-01: Clear inactivity monitor on session end
            if let Some(ref mut inact) = conn.inactivity_monitor { inact.reset(); }
            conn.inactivity_monitor = None;
            // BILL-02: Dismiss countdown warning overlay on session end
            state.lock_screen.dismiss_countdown_warning();
            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Release);
            crate::remote_ops::BILLING_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
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

        CoreToAgentMessage::LaunchGame { sim_type: launch_sim, launch_args, force_clean, duration_minutes } => {
            // SEC-10: Acquire game launch mutex before any launch-related work.
            // This serializes concurrent LaunchGame commands and ensures clean_state_reset
            // (a spawn_blocking call that can take 5+ seconds) completes before a second
            // LaunchGame proceeds. Without this, two concurrent launches can both pass the
            // force_clean step, spawning two game instances that fight over resources.
            let _game_launch_guard = state.game_launch_mutex.lock().await;
            tracing::debug!(target: LOG_TARGET, "SEC-10: game_launch_mutex acquired");

            // RECOVER-01: Race Engineer requested clean state reset before relaunch
            if force_clean {
                let killed = tokio::task::spawn_blocking(crate::game_process::clean_state_reset).await.unwrap_or(0);
                tracing::info!(target: LOG_TARGET, "RECOVER-01: clean_state_reset before relaunch — killed {} processes", killed);
            }
            // v22.0 Phase 178: Feature flag gate — check if game launch is enabled
            {
                let flags = state.flags.read().await;
                if !flags.flag_enabled("game_launch") {
                    tracing::warn!(target: LOG_TARGET, "LaunchGame blocked by feature flag 'game_launch'");
                    // Do not launch — silently ignore (server should not send LaunchGame if flag is off,
                    // but this is a safety net)
                    return Ok(HandleResult::Continue);
                }
            }
            // LAUNCH-10: Pre-launch health checks — sentinel files, orphan processes, disk space
            {
                let check_result = tokio::task::spawn_blocking(|| {
                    crate::game_process::pre_launch_checks()
                }).await;

                match check_result {
                    Ok(Ok(())) => {
                        tracing::info!(target: LOG_TARGET, "Pre-launch checks passed for {:?}", launch_sim);
                    }
                    Ok(Err(reason)) => {
                        tracing::error!(target: LOG_TARGET, "Pre-launch check FAILED: {}", reason);
                        let error_info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(),
                            sim_type: launch_sim,
                            game_state: GameState::Error,
                            pid: None,
                            launched_at: Some(Utc::now()),
                            error_message: Some(format!("Pre-launch check failed: {}", reason)),
                            diagnostics: None,
                            exit_code: None,
                        };
                        if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                            let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        }
                        return Ok(HandleResult::Continue);
                    }
                    Err(e) => {
                        tracing::error!(target: LOG_TARGET, "Pre-launch check panicked (non-fatal): {}", e);
                        // Proceed — don't block launch on check panic
                    }
                }
            }

            tracing::info!(target: LOG_TARGET, "Launching game: {:?} (args: {:?})", launch_sim, launch_args);
            conn.last_launch_args_stored = launch_args.clone();
            // Track current sim_type for per-sim PlayableSignal dispatch
            conn.current_sim_type = Some(launch_sim);
            conn.loading_emitted = false;
            conn.f1_udp_playable_received = false;

            // ─── Safe Mode: enter before game spawn (zero delay) ──────────────
            // SAFE-01: protected games require scan suppression from first instruction.
            // SAFE-02: entry happens here, before any game process is created.
            if crate::safe_mode::is_protected_game(launch_sim) {
                state.safe_mode.enter(launch_sim);
                state.safe_mode_active.store(true, std::sync::atomic::Ordering::SeqCst);
                // Disarm cooldown if another game launched during cooldown window
                state.safe_mode_cooldown_armed = false;
            }

            // ─── Phase 60: Pre-launch FFB preset loading ──────────────────────
            // Ensure correct wheelbase preset BEFORE game process spawns.
            // Non-fatal: errors are logged, game launch proceeds regardless.
            {
                let pre_load_sim = launch_sim;
                let pre_load_result = tokio::task::spawn_blocking(move || {
                    crate::ffb_controller::pre_load_game_preset(pre_load_sim, None)
                }).await;
                match pre_load_result {
                    Ok(Ok(())) => tracing::info!(target: LOG_TARGET, "pre_load_game_preset: ok for {:?}", launch_sim),
                    Ok(Err(e)) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset failed (non-fatal): {}", e),
                    Err(e) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset panicked (non-fatal): {}", e),
                }
            }

            if launch_sim == SimType::AssettoCorsa {
                if let Some(ref mut adp) = state.adapter { adp.disconnect(); }

                // Game Doctor: pre-launch validation — catch config problems BEFORE 90s timeout.
                // Validates: AC installed, car exists, track exists, track config exists, Steam running.
                // Fails fast with specific error instead of waiting 90s and killing everything.
                let pre_validate_args = launch_args.clone();
                if let Some(ref args_str) = pre_validate_args {
                    // Quick parse to extract car/track for validation (before full params parse)
                    // Fail-closed: malformed JSON = reject launch (don't silently skip validation)
                    match serde_json::from_str::<serde_json::Value>(args_str) {
                    Err(parse_err) => {
                        tracing::error!(target: LOG_TARGET, "Game Doctor pre-launch: malformed launch_args JSON: {}", parse_err);
                        let error_info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(),
                            sim_type: launch_sim,
                            game_state: GameState::Error,
                            pid: None,
                            launched_at: Some(Utc::now()),
                            error_message: Some(format!("Pre-launch validation: malformed launch args: {}", parse_err)),
                            diagnostics: None,
                            exit_code: None,
                        };
                        if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                            let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        }
                        return Ok(HandleResult::Continue);
                    }
                    Ok(quick_params) => {
                        let car = quick_params.get("car").and_then(|v| v.as_str()).unwrap_or("");
                        let track = quick_params.get("track").and_then(|v| v.as_str()).unwrap_or("");
                        let track_config = quick_params.get("track_config").and_then(|v| v.as_str()).unwrap_or("");
                        if let Err(validation_error) = crate::game_doctor::pre_launch_validate(car, track, track_config) {
                            tracing::error!(target: LOG_TARGET, "Game Doctor pre-launch FAIL: {}", validation_error);
                            let error_info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: launch_sim,
                                game_state: GameState::Error,
                                pid: None,
                                launched_at: Some(Utc::now()),
                                error_message: Some(format!("Pre-launch validation: {}", validation_error)),
                                diagnostics: None,
                                exit_code: None,
                            };
                            if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                            }
                            return Ok(HandleResult::Continue);
                        }
                    } // Ok(quick_params)
                    } // match
                }

                let params: ac_launcher::AcLaunchParams = match &launch_args {
                    Some(args) => serde_json::from_str(args).unwrap_or_else(|e| {
                        tracing::warn!(target: LOG_TARGET, "Failed to parse AC launch_args, using defaults (car=ks_ferrari_sf15t, track=spa): {}", e);
                        ac_launcher::AcLaunchParams {
                        car: "ks_ferrari_sf15t".to_string(), track: "spa".to_string(),
                        driver: "Driver".to_string(), track_config: String::new(), skin: String::new(),
                        transmission: "manual".to_string(), ffb: "medium".to_string(),
                        aids: None, conditions: None, duration_minutes: 60, game_mode: String::new(),
                        server_ip: String::new(), server_port: 0, server_http_port: 0,
                        server_password: String::new(), ai_level: 87,
                        session_type: "practice".to_string(), ai_cars: Vec::new(),
                        starting_position: 1, formation_lap: false,
                        weekend_practice_minutes: 0, weekend_qualify_minutes: 0,
                        ai_count: None,
                    }}),
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
                        ai_count: None,
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
 exit_code: None,
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
                    Err(e) => { tracing::error!(target: LOG_TARGET, "AC launch task panicked: {}", e); Err(anyhow::anyhow!("Launch task panicked: {}", e)) }
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
                            exit_code: None,
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
                            tracing::error!(target: LOG_TARGET, "CM failure on {}: {}", pod_id_clone, cm_err);
                            if let Ok(mut err_slot) = state.last_launch_error.lock() { *err_slot = Some(cm_err.clone()); }
                            #[cfg(feature = "ai-debugger")]
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
                                Ok(()) => tracing::info!(target: LOG_TARGET, "Reconnected to AC telemetry"),
                                Err(e) => tracing::warn!(target: LOG_TARGET, "Could not reconnect telemetry: {}", e),
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(target: LOG_TARGET, "AC launch failed: {}", e);
                        if let Ok(mut err_slot) = state.last_launch_error.lock() {
                            *err_slot = Some(format!("Launch failed: {}", e));
                        }
                        let info = GameLaunchInfo {
                            pod_id: pod_id_clone.clone(), sim_type: launch_sim,
                            game_state: GameState::Error, pid: None, launched_at: None,
                            error_message: Some(e.to_string()), diagnostics: None,
 exit_code: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json_str = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        #[cfg(feature = "ai-debugger")]
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

                // GAME-01: Steam readiness check — blocks launch if Steam is not running or updating.
                // Skips for AC (Game Doctor handles it) and non-Steam games.
                {
                    let steam_sim = launch_sim;
                    let steam_config = game_config.clone();
                    let steam_result = tokio::task::spawn_blocking(move || {
                        crate::steam_checks::check_steam_ready(steam_sim, &steam_config)
                    }).await;
                    match steam_result {
                        Ok(Ok(())) => {
                            tracing::info!(target: LOG_TARGET, "GAME-01: Steam readiness check passed for {:?}", launch_sim);
                        }
                        Ok(Err(reason)) => {
                            tracing::error!(target: LOG_TARGET, "GAME-01: Steam readiness check FAILED for {:?}: {}", launch_sim, reason);
                            let error_info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: launch_sim,
                                game_state: GameState::Error,
                                pid: None,
                                launched_at: Some(Utc::now()),
                                error_message: Some(format!("Steam readiness check failed: {}", reason)),
                                diagnostics: None,
                                exit_code: None,
                            };
                            state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            conn.launch_state = LaunchState::Idle;
                            if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                            }
                            return Ok(HandleResult::Continue);
                        }
                        Err(e) => {
                            tracing::warn!(target: LOG_TARGET, "GAME-01: Steam readiness check panicked (non-fatal): {}", e);
                            // Proceed — don't block launch on check panic
                        }
                    }
                }

                // GAME-06: DLC/content availability check — blocks launch if required content is missing.
                // Prevents billing from starting for sessions that cannot load the requested content.
                {
                    let dlc_sim = launch_sim;
                    let dlc_args = game_config.args.clone().unwrap_or_default();
                    let dlc_config = game_config.clone();
                    let dlc_result = tokio::task::spawn_blocking(move || {
                        crate::steam_checks::check_dlc_installed(dlc_sim, &dlc_args, &dlc_config)
                    }).await;
                    match dlc_result {
                        Ok(Ok(())) => {
                            tracing::info!(target: LOG_TARGET, "GAME-06: DLC check passed for {:?}", launch_sim);
                        }
                        Ok(Err(reason)) => {
                            tracing::error!(target: LOG_TARGET, "GAME-06: DLC check FAILED for {:?}: {}", launch_sim, reason);
                            let error_info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: launch_sim,
                                game_state: GameState::Error,
                                pid: None,
                                launched_at: Some(Utc::now()),
                                error_message: Some(format!("Content not installed: {}", reason)),
                                diagnostics: None,
                                exit_code: None,
                            };
                            state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            conn.launch_state = LaunchState::Idle;
                            if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                            }
                            return Ok(HandleResult::Continue);
                        }
                        Err(e) => {
                            tracing::warn!(target: LOG_TARGET, "GAME-06: DLC check panicked (non-fatal): {}", e);
                            // Proceed — don't block launch on check panic
                        }
                    }
                }

                // GAME-04: AC EVO config adapter — write Unreal GameUserSettings.ini before launch.
                // AC EVO uses Unreal Engine, NOT race.ini format. launch_args fields are applied
                // via GameUserSettings.ini so the game picks up car/track/weather selections.
                // Failure is non-fatal: game launches with default config if write fails.
                if launch_sim == SimType::AssettoCorsaEvo {
                    if let Some(ref args) = game_config.args {
                        let evo_dir = crate::sims::assetto_corsa_evo::find_evo_install_dir(&game_config);
                        if let Some(dir) = evo_dir {
                            let evo_args = args.clone();
                            let evo_write_result = tokio::task::spawn_blocking(move || {
                                crate::sims::assetto_corsa_evo::write_evo_config(&evo_args, &dir)
                            }).await;
                            match evo_write_result {
                                Ok(Ok(())) => tracing::info!(target: LOG_TARGET, "GAME-04: AC EVO GameUserSettings.ini written successfully"),
                                Ok(Err(e)) => tracing::warn!(target: LOG_TARGET, "GAME-04: AC EVO config write failed (non-fatal): {}", e),
                                Err(e) => tracing::warn!(target: LOG_TARGET, "GAME-04: AC EVO config write panicked (non-fatal): {}", e),
                            }
                        } else {
                            tracing::warn!(target: LOG_TARGET, "GAME-04: AC EVO install dir not found — launching with default config");
                        }
                    }
                }

                // GAME-05: iRacing subscription/launch check — blocks billing for inactive accounts.
                // iRacing shows a login/subscription dialog if account is inactive. Without this check
                // the customer would be billed for a session they cannot actually play.
                if launch_sim == SimType::IRacing {
                    let iracing_check = tokio::task::spawn_blocking(|| {
                        crate::iracing_checks::check_iracing_ready()
                    }).await;
                    match iracing_check {
                        Ok(Ok(())) => {
                            tracing::info!(target: LOG_TARGET, "GAME-05: iRacing readiness check passed");
                        }
                        Ok(Err(reason)) => {
                            tracing::error!(target: LOG_TARGET, "GAME-05: iRacing readiness check FAILED: {}", reason);
                            let error_info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: launch_sim,
                                game_state: GameState::Error,
                                pid: None,
                                launched_at: Some(Utc::now()),
                                error_message: Some(format!("iRacing readiness check failed: {}", reason)),
                                diagnostics: None,
                                exit_code: None,
                            };
                            state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            conn.launch_state = LaunchState::Idle;
                            if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                            }
                            return Ok(HandleResult::Continue);
                        }
                        Err(e) => {
                            tracing::warn!(target: LOG_TARGET, "GAME-05: iRacing readiness check panicked (non-fatal, proceeding): {}", e);
                            // Proceed — spawn panic is not a billing reason to block
                        }
                    }
                }

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
 exit_code: None,
                };
                let msg = AgentMessage::GameStateUpdate(launching_info);
                let json_str = serde_json::to_string(&msg)?;
                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                // GAME-07: Detect Steam URL launches by checking if this is a Steam game with no direct pid.
                // For steam:// URL launches, GameProcess::launch returns immediately with pid=None.
                // We need to poll for the actual game window after Steam processes the launch request.
                let is_steam_url_launch = game_config.use_steam || game_config.steam_app_id.is_some()
                    || game_config.args.as_deref().map(|a| a.starts_with("steam://")).unwrap_or(false);

                match game_process::GameProcess::launch(&game_config, launch_sim) {
                    Ok(gp) => {
                        tracing::info!(target: LOG_TARGET, "Generic sim {:?} launched (pid: {:?})", launch_sim, gp.pid);
                        let gp_pid = gp.pid;
                        state.game_process = Some(gp);
                        let _ = state.failure_monitor_tx.send_modify(|s| { s.game_pid = gp_pid; });

                        if let Some(pid) = gp_pid {
                            // Direct exe launch — we have the pid immediately.
                            // GAME-08: Create ProcessMonitor for crash detection on all non-AC sims.
                            conn.process_monitor = Some(ProcessMonitor::new(pid, launch_sim));
                            tracing::info!(
                                target: LOG_TARGET,
                                "GAME-08: ProcessMonitor created for {:?} (pid {})",
                                launch_sim, pid
                            );

                            // GAME-03: Create SessionEnforcer for ForzaHorizon5/Forza if duration provided.
                            if matches!(launch_sim, SimType::ForzaHorizon5 | SimType::Forza) {
                                if let Some(duration_mins) = duration_minutes {
                                    let duration_secs = (duration_mins as u64) * 60;
                                    conn.session_enforcer = Some(SessionEnforcer::new(launch_sim, pid, duration_secs));
                                    tracing::info!(
                                        target: LOG_TARGET,
                                        "GAME-03: SessionEnforcer created for {:?} (pid {}, {}min)",
                                        launch_sim, pid, duration_mins
                                    );
                                }
                            }

                            let info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(), sim_type: launch_sim,
                                game_state: GameState::Running, pid: Some(pid),
                                launched_at: Some(Utc::now()), error_message: None, diagnostics: None,
                                exit_code: None,
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json_str = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json_str.into())).await;
                        } else if is_steam_url_launch {
                            // GAME-07: Steam URL launch — pid is None until Steam passes control to the game.
                            // Spawn a background task to wait for the game window to appear.
                            // On success: game is confirmed running. On timeout: report Error to server.
                            // Uses ws_exec_result_tx to route AgentMessage back to the event loop → WS send.
                            let pod_id_clone = state.pod_id.clone();
                            let failure_tx = state.failure_monitor_tx.clone();
                            let ws_result_tx = state.ws_exec_result_tx.clone();
                            tracing::info!(
                                target: LOG_TARGET,
                                "GAME-07: Steam URL launch for {:?} — waiting for game window (60s timeout)",
                                launch_sim
                            );
                            tokio::spawn(async move {
                                let window_result = tokio::task::spawn_blocking(move || {
                                    crate::steam_checks::wait_for_game_window(launch_sim, 60)
                                }).await;
                                match window_result {
                                    Ok(Ok(pid)) => {
                                        tracing::info!(
                                            target: LOG_TARGET,
                                            "GAME-07: Game window confirmed for {:?} (PID {})",
                                            launch_sim, pid
                                        );
                                        let _ = failure_tx.send_modify(|s| { s.game_pid = Some(pid); });
                                        let info = GameLaunchInfo {
                                            pod_id: pod_id_clone.clone(), sim_type: launch_sim,
                                            game_state: GameState::Running, pid: Some(pid),
                                            launched_at: Some(Utc::now()), error_message: None,
                                            diagnostics: None, exit_code: None,
                                        };
                                        let _ = ws_result_tx.send(AgentMessage::GameStateUpdate(info)).await;
                                    }
                                    Ok(Err(reason)) => {
                                        tracing::error!(
                                            target: LOG_TARGET,
                                            "GAME-07: Game window not detected for {:?} after timeout: {}",
                                            launch_sim, reason
                                        );
                                        // Report error to server — Steam showed a dialog instead of launching
                                        let info = GameLaunchInfo {
                                            pod_id: pod_id_clone.clone(), sim_type: launch_sim,
                                            game_state: GameState::Error, pid: None,
                                            launched_at: None,
                                            error_message: Some(format!("Game window not detected: {}", reason)),
                                            diagnostics: None, exit_code: None,
                                        };
                                        let _ = ws_result_tx.send(AgentMessage::GameStateUpdate(info)).await;
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            target: LOG_TARGET,
                                            "GAME-07: wait_for_game_window task panicked (non-fatal): {}",
                                            e
                                        );
                                    }
                                }
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!(target: LOG_TARGET, "Failed to launch {:?}: {}", launch_sim, e);
                        state.heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                        state.heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                        conn.launch_state = LaunchState::Idle;
                        let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = None; });
                        let info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(), sim_type: launch_sim,
                            game_state: GameState::Error, pid: None, launched_at: None,
                            error_message: Some(e.to_string()), diagnostics: None,
 exit_code: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json_str = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json_str.into())).await;
                    }
                }
            }
        }

        CoreToAgentMessage::StopGame => {
            // FSM-05: StopGame must be handled in every CrashRecoveryState variant.
            // If StopGame arrives during crash recovery, the recovery timer must be cancelled
            // to prevent a relaunch AFTER the session is already stopped.
            match &conn.crash_recovery {
                CrashRecoveryState::PausedWaitingRelaunch { attempt, .. } => {
                    tracing::info!(target: LOG_TARGET, "StopGame received during crash recovery (attempt {}) — cancelling relaunch", attempt);
                }
                CrashRecoveryState::AutoEndPending => {
                    tracing::info!(target: LOG_TARGET, "StopGame received during AutoEndPending — clearing");
                }
                CrashRecoveryState::Idle => {} // Normal case
            }
            conn.crash_recovery = CrashRecoveryState::Idle;

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
                tracing::info!(target: LOG_TARGET, "Stopping game: {:?}", game.sim_type);
                let sim = game.sim_type;
                match game.stop() {
                    Ok(()) => {
                        let info = GameLaunchInfo {
                            pod_id: state.pod_id.clone(), sim_type: sim, game_state: GameState::Idle,
                            pid: None, launched_at: None, error_message: None, diagnostics: None,
 exit_code: None,
                        };
                        let msg = AgentMessage::GameStateUpdate(info);
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                    Err(e) => { tracing::error!(target: LOG_TARGET, "Failed to stop game: {}", e); }
                }
                state.game_process = None;
            }
            // GAME-08/GAME-03: Clear process monitor and session enforcer on controlled stop.
            // This prevents the crash detection path from firing after a clean StopGame.
            conn.process_monitor = None;
            conn.session_enforcer = None;
        }

        CoreToAgentMessage::ShowPinLockScreen {
            token_id, driver_name, pricing_tier_name, allocated_seconds
        } => {
            tracing::info!(target: LOG_TARGET, "Lock screen: PIN entry for {}", driver_name);
            state.lock_screen.show_pin_screen(token_id, driver_name, pricing_tier_name, allocated_seconds);
        }

        CoreToAgentMessage::ShowQrLockScreen {
            token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds
        } => {
            tracing::info!(target: LOG_TARGET, "Lock screen: QR display for {}", driver_name);
            state.lock_screen.show_qr_screen(token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds);
        }

        CoreToAgentMessage::ClearLockScreen => {
            tracing::info!(target: LOG_TARGET, "Lock screen cleared");
            state.overlay.deactivate();
            state.lock_screen.clear();
        }

        CoreToAgentMessage::BlankScreen => {
            if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                tracing::warn!(target: LOG_TARGET, "Ignoring BlankScreen -- billing is active");
            } else {
                tracing::info!(target: LOG_TARGET, "Screen blanked via direct command");
                state.overlay.deactivate();
                state.lock_screen.show_blank_screen();
            }
        }

        CoreToAgentMessage::SubSessionEnded {
            billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
            wallet_balance_paise, current_split_number, total_splits,
        } => {
            tracing::info!(
                target: LOG_TARGET,
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
            tracing::info!(target: LOG_TARGET, "Assistance screen for {}: {}", driver_name, message);
            state.lock_screen.show_assistance(driver_name, message);
        }

        CoreToAgentMessage::EnterDebugMode { employee_name } => {
            tracing::info!(target: LOG_TARGET, "Employee debug mode activated by {}", employee_name);
            state.kiosk.enter_debug_mode();
            state.lock_screen.clear();
        }

        CoreToAgentMessage::EnterFreedomMode => {
            tracing::info!(target: LOG_TARGET, "Freedom mode activated — all restrictions lifted, monitoring active");
            state.kiosk.exit_debug_mode();
            state.kiosk.enter_freedom_mode();
            state.lock_screen.clear();
        }

        CoreToAgentMessage::ExitFreedomMode => {
            tracing::info!(target: LOG_TARGET, "Freedom mode deactivated — re-engaging kiosk restrictions");
            state.kiosk.exit_freedom_mode();
            if state.kiosk_enabled {
                state.kiosk.activate();
            }
        }

        CoreToAgentMessage::SettingsUpdated { settings } => {
            tracing::info!(target: LOG_TARGET, "Kiosk settings updated: {:?}", settings);
            if let Some(v) = settings.get("kiosk_lockdown_enabled") {
                if v == "true" && !state.kiosk.is_active() && !state.kiosk.is_debug_mode() {
                    state.kiosk.activate();
                    tracing::info!(target: LOG_TARGET, "Kiosk lockdown ENABLED via remote settings");
                } else if v == "false" && state.kiosk.is_active() {
                    state.kiosk.deactivate();
                    tracing::info!(target: LOG_TARGET, "Kiosk lockdown DISABLED via remote settings");
                }
            }
            if let Some(v) = settings.get("screen_blanking_enabled") {
                tracing::info!(target: LOG_TARGET, "Screen blanking set to: {}", v);
                let billing_on = state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed);
                if v == "true" && state.lock_screen.is_idle_or_blanked() && !billing_on {
                    state.lock_screen.show_blank_screen();
                    tracing::info!(target: LOG_TARGET, "Screen blanking ENABLED -- screen blanked");
                } else if v == "false" {
                    state.lock_screen.clear();
                    tracing::info!(target: LOG_TARGET, "Screen blanking DISABLED -- screen restored");
                }
            }
            if let Some(v) = settings.get("freedom_mode_enabled") {
                if v == "true" && !state.kiosk.is_freedom_mode() {
                    state.kiosk.exit_debug_mode();
                    state.kiosk.enter_freedom_mode();
                    state.lock_screen.clear();
                    tracing::info!(target: LOG_TARGET, "Freedom mode ENABLED via remote settings");
                } else if v == "false" && state.kiosk.is_freedom_mode() {
                    state.kiosk.exit_freedom_mode();
                    if state.kiosk_enabled {
                        state.kiosk.activate();
                    }
                    tracing::info!(target: LOG_TARGET, "Freedom mode DISABLED via remote settings");
                }
            }
            if let Some(url) = settings.get("lock_screen_wallpaper_url") {
                let url_opt = if url.is_empty() { None } else { Some(url.clone()) };
                state.lock_screen.set_wallpaper_url(url_opt);
                tracing::info!(target: LOG_TARGET, "Lock screen wallpaper URL updated");
            }
        }

        CoreToAgentMessage::SetTransmission { transmission } => {
            tracing::info!(target: LOG_TARGET, "Setting transmission to '{}' mid-session (SendInput)", transmission);
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
            tracing::info!(target: LOG_TARGET, "Setting FFB to '{}' mid-session", preset);
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
                    Ok(false) => tracing::warn!(target: LOG_TARGET, "FFB: wheelbase not found for SetFfb"),
                    Err(e) => tracing::error!(target: LOG_TARGET, "FFB gain error: {}", e),
                }
            } else {
                if let Err(e) = ac_launcher::set_ffb(&preset) {
                    tracing::error!(target: LOG_TARGET, "Failed to set FFB (legacy): {}", e);
                }
            }
        }

        CoreToAgentMessage::SetAssist { assist_type, enabled } => {
            tracing::info!(target: LOG_TARGET, "SetAssist: type={}, enabled={}", assist_type, enabled);
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
                other => { tracing::warn!(target: LOG_TARGET, "Unknown assist type: {}", other); }
            }
        }

        CoreToAgentMessage::SetFfbGain { percent } => {
            tracing::info!(target: LOG_TARGET, "SetFfbGain: {}%", percent);
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
                Ok(false) => tracing::warn!(target: LOG_TARGET, "FFB: wheelbase not found for SetFfbGain"),
                Err(e) => tracing::error!(target: LOG_TARGET, "FFB gain set error: {}", e),
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
            tracing::warn!(target: LOG_TARGET, "PIN failed: {}", reason);
            state.lock_screen.show_pin_error(&reason);
        }

        CoreToAgentMessage::Ping { id } => {
            let received_at = std::time::Instant::now();
            let pong = AgentMessage::Pong { id, agent_delay_us: None };
            if let Ok(json) = serde_json::to_string(&pong) {
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    tracing::error!(target: LOG_TARGET, "Failed to send Pong, connection lost");
                    return Ok(HandleResult::Break);
                }
            }
            let process_us = received_at.elapsed().as_micros() as u64;
            if process_us > 5000 {
                tracing::warn!(target: LOG_TARGET, "Pong send took {}us (>5ms)", process_us);
            }
        }

        CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } => {
            // BOOT-04: Intercept GUARD_CONFIRMED before generic exec dispatch
            if cmd.trim() == "GUARD_CONFIRMED" {
                state.guard_confirmed.store(true, std::sync::atomic::Ordering::Relaxed);
                tracing::warn!(
                    target: "state",
                    prev = "report_only_pending",
                    next = "guard_confirmed",
                    "GUARD_CONFIRMED received — process guard may now escalate to kill_and_report"
                );
                // Restore kill_and_report on the whitelist if it was downgraded
                {
                    let mut wl = state.guard_whitelist.write().await;
                    wl.violation_action = "kill_and_report".to_string();
                }
                crate::startup_log::write_phase(
                    "GUARD_CONFIRMED",
                    "Operator confirmed allowlist — process guard escalated to kill_and_report",
                );
                let _ = state.ws_exec_result_tx.send(AgentMessage::ExecResult {
                    request_id,
                    success: true,
                    exit_code: Some(0),
                    stdout: "Process guard confirmed — kill_and_report mode activated".to_string(),
                    stderr: String::new(),
                }).await;
            } else {
                tracing::info!(target: LOG_TARGET, "WS command request {}: {}", request_id, cmd);
                let result_tx = state.ws_exec_result_tx.clone();
                tokio::spawn(async move {
                    let result = handle_ws_exec(request_id, cmd, timeout_ms).await;
                    let _ = result_tx.send(result).await;
                });
            }
        }

        CoreToAgentMessage::ApproveProcess { process_name } => {
            tracing::info!(target: LOG_TARGET, "Server APPROVED process: {}", process_name);
            kiosk::KioskManager::approve_process(&process_name);
            if state.kiosk.is_locked_down() {
                state.kiosk.exit_lockdown();
                state.lock_screen.show_idle_pin_entry();
            }
        }

        CoreToAgentMessage::RejectProcess { process_name } => {
            tracing::warn!(target: LOG_TARGET, "Server REJECTED process: {}", process_name);
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
            tracing::info!(target: LOG_TARGET, "RunSelfTest request_id={}", request_id);
            let status_clone = state.heartbeat_status.clone();
            #[cfg(feature = "ai-debugger")]
            let ollama_url = state.config.ai_debugger.ollama_url.clone();
            #[cfg(not(feature = "ai-debugger"))]
            let ollama_url = String::new();
            #[cfg(feature = "ai-debugger")]
            let ollama_model = state.config.ai_debugger.ollama_model.clone();
            #[cfg(not(feature = "ai-debugger"))]
            let ollama_model = String::new();
            let result_tx = state.ws_exec_result_tx.clone();
            let pod_id_clone = state.pod_id.clone();
            tokio::spawn(async move {
                let mut report = self_test::run_all_probes(status_clone, &ollama_url).await;
                #[cfg(feature = "ai-debugger")]
                let verdict = self_test::get_llm_verdict(&ollama_url, &ollama_model, &report.probes).await;
                #[cfg(not(feature = "ai-debugger"))]
                let verdict = self_test::deterministic_verdict(&report.probes);
                let _ = (&ollama_model, &ollama_url); // suppress unused warnings when ai-debugger off
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
                    target: LOG_TARGET,
                    "[switch] Rejected SwitchController -- target_url {:?} does not match primary ({:?}) or failover ({:?})",
                    target_url, primary_url, failover_url
                );
            } else {
                // Phase 69: Split-brain guard -- verify .23 is actually unreachable before switching
                #[cfg(feature = "http-client")]
                let server_reachable = match split_brain_probe
                    .get("http://192.168.31.23:8090/ping")
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => true,
                    _ => false,
                };
                // When http-client (reqwest) is not available, skip split-brain guard
                #[cfg(not(feature = "http-client"))]
                let server_reachable = false;

                if server_reachable {
                    tracing::warn!(
                        target: LOG_TARGET,
                        "[switch] split-brain guard: .23 still reachable, ignoring SwitchController to {}",
                        target_url
                    );
                    // Do NOT switch -- server is still up from this pod's perspective
                } else {
                    tracing::info!(
                        target: LOG_TARGET,
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

        CoreToAgentMessage::ClearMaintenance => {
            tracing::info!(target: LOG_TARGET, "ClearMaintenance received from server — clearing maintenance state");
            state.in_maintenance.store(false, std::sync::atomic::Ordering::Relaxed);
            state.lock_screen.show_idle_pin_entry();
        }

        CoreToAgentMessage::UpdateProcessWhitelist { whitelist } => {
            let mut wl = state.guard_whitelist.write().await;
            *wl = whitelist;
            tracing::info!(target: LOG_TARGET, "Process guard: whitelist updated via WS push ({} processes)", wl.processes.len());
        }

        // v22.0 Phase 178: Feature flag sync from server
        CoreToAgentMessage::FlagSync(payload) => {
            let mut flags = state.flags.write().await;
            let count = payload.flags.len();
            flags.apply_sync(&payload);
            tracing::info!(target: LOG_TARGET, "Feature flags synced: {} flags, version {}", count, payload.version);
        }

        // v22.0 Phase 178: Kill switch from server — emergency halt capability
        CoreToAgentMessage::KillSwitch(payload) => {
            let mut flags = state.flags.write().await;
            tracing::warn!(target: LOG_TARGET, "Kill switch: {} = {} (reason: {:?})", payload.flag_name, payload.active, payload.reason);
            flags.apply_kill_switch(&payload);
        }

        // v22.0 Phase 178: ConfigPush — hot-reload selected fields without restart.
        // Non-reloadable fields (port, ws_url, pod_number, pod_id) are logged and ignored.
        // ConfigAck is queued in pending_acks to be drained by the event loop after handling.
        CoreToAgentMessage::ConfigPush(payload) => {
            const HOT_RELOAD_FIELDS: &[&str] = &["billing_rates", "game_limits", "process_guard_whitelist", "debug_verbosity"];
            const NON_RELOAD_FIELDS: &[&str] = &["port", "ws_url", "pod_number", "pod_id"];

            let mut accepted = true;
            for (field, value) in &payload.fields {
                if NON_RELOAD_FIELDS.iter().any(|f| field.contains(f)) {
                    tracing::warn!(target: LOG_TARGET, "ConfigPush: ignoring non-reloadable field '{}' (requires restart)", field);
                    continue;
                }
                if HOT_RELOAD_FIELDS.iter().any(|f| field.contains(f)) {
                    if field.contains("process_guard_whitelist") {
                        // Update the existing guard_whitelist via its Arc<RwLock>
                        if let Ok(wl) = serde_json::from_value::<MachineWhitelist>(value.clone()) {
                            let mut guard = state.guard_whitelist.write().await;
                            *guard = wl;
                            tracing::info!(target: LOG_TARGET, "ConfigPush: updated process_guard_whitelist");
                        } else {
                            tracing::warn!(target: LOG_TARGET, "ConfigPush: invalid process_guard_whitelist value");
                            accepted = false;
                        }
                    } else {
                        // Future hot-reload fields (billing_rates, game_limits, debug_verbosity)
                        // will be wired here as their Arc<RwLock> state containers are created
                        tracing::info!(target: LOG_TARGET, "ConfigPush: accepted field '{}' = {}", field, value);
                    }
                } else {
                    tracing::warn!(target: LOG_TARGET, "ConfigPush: unknown field '{}' — ignored", field);
                }
            }
            // Queue ConfigAck to be drained by event loop after this handler returns
            let ack = AgentMessage::ConfigAck(rc_common::types::ConfigAckPayload {
                pod_id: state.pod_id.clone(),
                sequence: payload.sequence,
                accepted,
            });
            conn.pending_acks.push(ack);
            tracing::info!(target: LOG_TARGET, "ConfigPush applied (seq={}), ack queued", payload.sequence);
        }

        CoreToAgentMessage::ForceRelaunchBrowser { pod_id: _ } => {
            // Phase 139: Server-initiated lock screen recovery.
            // Guard: never relaunch during an active billing session (standing rule #10).
            if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                tracing::warn!(
                    target: LOG_TARGET,
                    "Ignoring ForceRelaunchBrowser -- billing is active, skipping relaunch"
                );
            } else {
                tracing::info!(
                    target: LOG_TARGET,
                    "ForceRelaunchBrowser received -- relaunching Edge lock screen"
                );
                state.lock_screen.close_browser();
                state.lock_screen.launch_browser();
                tracing::info!(
                    target: LOG_TARGET,
                    "ForceRelaunchBrowser: close_browser + launch_browser complete"
                );
            }
        }

        // ─── Staff Diagnostic Bridge (v27.0) ─────────────────────────────────

        CoreToAgentMessage::DiagnosticRequest {
            correlation_id,
            incident_id,
            description,
            category,
            requested_by,
        } => {
            tracing::info!(
                target: LOG_TARGET,
                correlation_id = %correlation_id,
                incident_id = %incident_id,
                category = %category,
                requested_by = %requested_by,
                "DiagnosticRequest received — dispatching to tier engine"
            );

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();
            let req = crate::tier_engine::StaffDiagnosticRequest {
                correlation_id: correlation_id.clone(),
                incident_id,
                description,
                category,
                response_tx,
            };

            // Send to tier engine via the staff channel
            match state.staff_diagnostic_tx.try_send(req) {
                Ok(()) => {
                    // Spawn a task to await the result and send it back via WS
                    let ws_result_tx = state.ws_exec_result_tx.clone();
                    let _pod_id = state.pod_id.clone();
                    let cid = correlation_id.clone();
                    tokio::spawn(async move {
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            response_rx,
                        ).await {
                            Ok(Ok(result)) => {
                                let msg = AgentMessage::DiagnosticResult {
                                    correlation_id: result.correlation_id,
                                    tier: result.tier,
                                    outcome: result.outcome,
                                    root_cause: result.root_cause,
                                    fix_action: result.fix_action,
                                    fix_type: result.fix_type,
                                    confidence: result.confidence,
                                    fix_applied: result.fix_applied,
                                    problem_hash: result.problem_hash,
                                    summary: result.summary,
                                };
                                let _ = ws_result_tx.send(msg).await;
                            }
                            Ok(Err(_)) => {
                                tracing::warn!(target: "ws-handler", cid = %cid, "Staff diagnostic: tier engine dropped response channel");
                            }
                            Err(_) => {
                                tracing::warn!(target: "ws-handler", cid = %cid, "Staff diagnostic: tier engine timed out (30s)");
                                // Send timeout result
                                let msg = AgentMessage::DiagnosticResult {
                                    correlation_id: cid,
                                    tier: 0,
                                    outcome: "timeout".to_string(),
                                    root_cause: String::new(),
                                    fix_action: String::new(),
                                    fix_type: "none".to_string(),
                                    confidence: 0.0,
                                    fix_applied: false,
                                    problem_hash: String::new(),
                                    summary: "Tier engine did not respond within 10 seconds".to_string(),
                                };
                                let _ = ws_result_tx.send(msg).await;
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, correlation_id = %correlation_id, "Staff diagnostic channel full or closed: {}", e);
                    // MMA R4-1 fix: send error result back so staff UI doesn't hang
                    let ws_result_tx = state.ws_exec_result_tx.clone();
                    let cid = correlation_id.clone();
                    tokio::spawn(async move {
                        let msg = AgentMessage::DiagnosticResult {
                            correlation_id: cid,
                            tier: 0,
                            outcome: "queue_full".to_string(),
                            root_cause: String::new(),
                            fix_action: String::new(),
                            fix_type: "none".to_string(),
                            confidence: 0.0,
                            fix_applied: false,
                            problem_hash: String::new(),
                            summary: "Diagnostic engine is busy. Please try again in a few seconds.".to_string(),
                        };
                        let _ = ws_result_tx.send(msg).await;
                    });
                }
            }
        }

        CoreToAgentMessage::StaffActionNotify {
            action,
            reason,
            correlation_id,
        } => {
            tracing::info!(
                target: LOG_TARGET,
                action = %action,
                correlation_id = %correlation_id,
                "StaffActionNotify: staff performed '{}' — resetting tier engine dedup window",
                action
            );
            // Log to diagnostic log so /events/recent shows staff actions
            let entry = crate::diagnostic_log::DiagnosticLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                trigger: format!("StaffAction({})", action),
                tier: 0,
                outcome: "manual".to_string(),
                action: action.clone(),
                root_cause: reason,
                fix_type: "staff_manual".to_string(),
                confidence: 1.0,
                fix_applied: true,
                problem_hash: String::new(),
                correlation_id: Some(correlation_id),
                source: "staff".to_string(),
            };
            state.diagnostic_log.push(entry).await;
        }

        other => {
            tracing::warn!(target: LOG_TARGET, "Unhandled CoreToAgentMessage: {:?}", other);
        }
    }

    Ok(HandleResult::Continue)
}

#[cfg(test)]
mod tests {
    #[test]
    fn force_relaunch_browser_variant_exists() {
        // Verify the variant deserializes correctly from JSON
        // (tests protocol contract without spawning browser)
        let json = r#"{"type":"force_relaunch_browser","data":{"pod_id":"pod-1"}}"#;
        let msg: rc_common::protocol::CoreToAgentMessage = serde_json::from_str(json).unwrap();
        match msg {
            rc_common::protocol::CoreToAgentMessage::ForceRelaunchBrowser { pod_id } => {
                assert_eq!(pod_id, "pod-1");
            }
            _ => panic!("expected ForceRelaunchBrowser"),
        }
    }
}
