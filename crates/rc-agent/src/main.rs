mod ac_launcher;
mod ai_debugger;
mod debug_server;
mod driving_detector;
mod game_process;
mod kiosk;
mod lock_screen;
mod overlay;
mod sims;
mod udp_heartbeat;

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use driving_detector::{
    DetectorConfig, DetectorSignal, DrivingDetector,
    is_input_active, is_steering_moving, parse_openffboard_report,
};
use ai_debugger::AiDebuggerConfig;
use game_process::GameExeConfig;
use rc_common::protocol::AgentMessage;
use rc_common::types::*;
use sims::SimAdapter;
use sims::assetto_corsa::AssettoCorsaAdapter;
use kiosk::KioskManager;
use lock_screen::{LockScreenEvent, LockScreenManager};
use overlay::OverlayManager;

#[derive(Debug, Deserialize)]
struct AgentConfig {
    pod: PodConfig,
    core: CoreConfig,
    #[serde(default)]
    wheelbase: WheelbaseConfig,
    #[serde(default)]
    telemetry_ports: TelemetryPortsConfig,
    #[serde(default)]
    games: GamesConfig,
    #[serde(default)]
    ai_debugger: AiDebuggerConfig,
    #[serde(default)]
    kiosk: KioskConfig,
}

#[derive(Debug, Deserialize)]
struct KioskConfig {
    #[serde(default = "default_true")]
    enabled: bool,
}

impl Default for KioskConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_true() -> bool { true }

#[derive(Debug, Default, Deserialize)]
struct GamesConfig {
    #[serde(default)]
    assetto_corsa: GameExeConfig,
    #[serde(default)]
    iracing: GameExeConfig,
    #[serde(default)]
    f1_25: GameExeConfig,
    #[serde(default)]
    le_mans_ultimate: GameExeConfig,
    #[serde(default)]
    forza: GameExeConfig,
}

#[derive(Debug, Deserialize)]
struct PodConfig {
    number: u32,
    name: String,
    sim: String,
    #[serde(default = "default_sim_ip")]
    sim_ip: String,
    #[serde(default = "default_sim_port")]
    sim_port: u16,
}

#[derive(Debug, Deserialize)]
struct CoreConfig {
    #[serde(default = "default_core_url")]
    url: String,
}

#[derive(Debug, Deserialize)]
struct WheelbaseConfig {
    #[serde(default = "default_wheelbase_vid")]
    vendor_id: u16,
    #[serde(default = "default_wheelbase_pid")]
    product_id: u16,
}

impl Default for WheelbaseConfig {
    fn default() -> Self {
        Self {
            vendor_id: default_wheelbase_vid(),
            product_id: default_wheelbase_pid(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TelemetryPortsConfig {
    #[serde(default = "default_telemetry_ports")]
    ports: Vec<u16>,
}

impl Default for TelemetryPortsConfig {
    fn default() -> Self {
        Self {
            ports: default_telemetry_ports(),
        }
    }
}

fn default_sim_ip() -> String { "127.0.0.1".to_string() }
fn default_sim_port() -> u16 { 9996 }
fn default_core_url() -> String { "ws://127.0.0.1:8080/ws/agent".to_string() }
fn default_wheelbase_vid() -> u16 { 0x1209 }
fn default_wheelbase_pid() -> u16 { 0xFFB0 }
fn default_telemetry_ports() -> Vec<u16> { vec![9996, 20777, 5300, 6789, 5555] }

#[tokio::main]
async fn main() -> Result<()> {
    // Single-instance guard: prevent zombie rc-agent processes
    #[cfg(windows)]
    let _mutex_guard = {
        use std::ffi::CString;
        let name = CString::new("Global\\RacingPoint_RCAgent_SingleInstance").unwrap();
        let handle = unsafe {
            winapi::um::synchapi::CreateMutexA(
                std::ptr::null_mut(),
                1, // bInitialOwner = TRUE
                name.as_ptr(),
            )
        };
        if handle.is_null() || unsafe { winapi::um::errhandlingapi::GetLastError() } == 183 {
            // ERROR_ALREADY_EXISTS = 183
            eprintln!("rc-agent is already running. Exiting to prevent zombie.");
            if !handle.is_null() {
                unsafe { winapi::um::handleapi::CloseHandle(handle); }
            }
            std::process::exit(0);
        }
        handle // held until process exits → mutex released automatically
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rc_agent=info".into()),
        )
        .init();

    println!(r#"
  RaceControl Agent
  Pod Telemetry Bridge
"#);

    // Load config
    let config = load_config()?;
    tracing::info!("Pod #{}: {} (sim: {})", config.pod.number, config.pod.name, config.pod.sim);
    tracing::info!("Core server: {}", config.core.url);

    let pod_id = format!("pod_{}", config.pod.number);
    let sim_type = match config.pod.sim.as_str() {
        "assetto_corsa" | "ac" => SimType::AssettoCorsa,
        "iracing" => SimType::IRacing,
        "lmu" | "le_mans_ultimate" => SimType::LeMansUltimate,
        "f1_25" | "f1" => SimType::F125,
        "forza" => SimType::Forza,
        other => {
            tracing::error!("Unknown sim type: {}", other);
            return Ok(());
        }
    };

    // Build pod info
    let pod_info = PodInfo {
        id: pod_id.clone(),
        number: config.pod.number,
        name: config.pod.name.clone(),
        ip_address: local_ip(),
        mac_address: None,
        sim_type,
        status: PodStatus::Idle,
        current_driver: None,
        current_session_id: None,
        last_seen: Some(Utc::now()),
        driving_state: None,
        billing_session_id: None,
        game_state: None,
        current_game: None,
    };

    // Watchdog: ensure pod-agent.exe stays running
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            watchdog_ensure_running("pod-agent.exe").await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    // Create sim adapter (None for unsupported sims — they still run heartbeats)
    let mut adapter: Option<Box<dyn SimAdapter>> = match sim_type {
        SimType::AssettoCorsa => Some(Box::new(AssettoCorsaAdapter::new(
            pod_id.clone(),
            config.pod.sim_ip.clone(),
            config.pod.sim_port,
        ))),
        _ => {
            tracing::warn!("Sim adapter not yet implemented for {:?}, running in heartbeat-only mode", sim_type);
            None
        }
    };

    // Set up driving detector (USB HID + UDP)
    let detector_config = DetectorConfig {
        wheelbase_vid: config.wheelbase.vendor_id,
        wheelbase_pid: config.wheelbase.product_id,
        telemetry_ports: config.telemetry_ports.ports.clone(),
        ..DetectorConfig::default()
    };
    let mut detector = DrivingDetector::new(&detector_config);

    // Channel for detector signals from HID/UDP tasks
    let (signal_tx, mut signal_rx) = mpsc::channel::<DetectorSignal>(256);

    // Spawn USB HID wheelbase monitor (blocking I/O in spawn_blocking)
    let hid_signal_tx = signal_tx.clone();
    let hid_vid = config.wheelbase.vendor_id;
    let hid_pid = config.wheelbase.product_id;
    let hid_pedal_threshold = detector_config.pedal_threshold;
    let hid_steering_deadzone = detector_config.steering_deadzone;
    tokio::spawn(async move {
        run_hid_monitor(hid_vid, hid_pid, hid_pedal_threshold, hid_steering_deadzone, hid_signal_tx).await;
    });

    // Spawn UDP telemetry port listeners
    let udp_signal_tx = signal_tx.clone();
    let udp_ports = config.telemetry_ports.ports.clone();
    tokio::spawn(async move {
        run_udp_monitor(udp_ports, udp_signal_tx).await;
    });

    // Try to connect to sim (for telemetry/laps — separate from billing detection)
    if let Some(ref mut adp) = adapter {
        match adp.connect() {
            Ok(()) => tracing::info!("Connected to {} telemetry", sim_type),
            Err(e) => {
                tracing::warn!("Could not connect to sim: {}. Will retry...", e);
            }
        }
    }

    // Game process state
    let mut game_process: Option<game_process::GameProcess> = None;

    // AI debugger result channel
    let (ai_result_tx, mut ai_result_rx) = mpsc::channel::<AiDebugSuggestion>(16);

    // Kiosk mode — prevent unauthorized desktop access on gaming PCs
    let kiosk_enabled = config.kiosk.enabled;
    let mut kiosk = KioskManager::new();
    if kiosk_enabled {
        kiosk.activate();
        tracing::info!("Kiosk mode ENABLED");
    } else {
        tracing::info!("Kiosk mode DISABLED (set kiosk.enabled=true in config to enable)");
    }

    // Lock screen for customer authentication (PIN / QR)
    // Always start the lock screen server so customers can enter PINs
    let (lock_event_tx, mut lock_event_rx) = mpsc::channel::<LockScreenEvent>(16);
    let mut lock_screen = LockScreenManager::new(lock_event_tx);
    lock_screen.start_server();
    tracing::info!("Lock screen server started on port 18923");

    // Racing HUD overlay for in-session display
    let mut overlay = OverlayManager::new();
    overlay.start_server();
    tracing::info!("Overlay server started on port 18925");

    // Debug server for remote diagnostics (LAN-accessible on port 18924)
    debug_server::spawn(lock_screen.state_handle(), config.pod.name.clone(), config.pod.number);

    // ─── UDP Heartbeat (fast liveness detection alongside WebSocket) ─────────
    let heartbeat_status = std::sync::Arc::new(udp_heartbeat::HeartbeatStatus::new());
    let (heartbeat_event_tx, mut heartbeat_event_rx) = mpsc::channel::<udp_heartbeat::HeartbeatEvent>(16);

    // Parse core IP from WebSocket URL (ws://IP:PORT/path → IP)
    let core_ip = config.core.url
        .replace("ws://", "")
        .replace("wss://", "")
        .split(':')
        .next()
        .unwrap_or("127.0.0.1")
        .to_string();

    {
        let hb_status = heartbeat_status.clone();
        let hb_tx = heartbeat_event_tx.clone();
        let hb_ip = core_ip.clone();
        let hb_pod = config.pod.number as u8;
        tokio::spawn(async move {
            udp_heartbeat::run(hb_ip, hb_pod, hb_status, hb_tx).await;
        });
    }
    tracing::info!("UDP heartbeat started → {}:{}", core_ip, rc_common::udp_protocol::HEARTBEAT_PORT);

    // ─── Reconnection Loop ──────────────────────────────────────────────────
    // On disconnect, retry with exponential backoff. All local state
    // (lock screen, kiosk, HID/UDP monitors, game process) persists across
    // reconnections — only the WebSocket is re-established.
    let mut reconnect_delay = Duration::from_secs(1);

    loop {
        // Connect to core server
        tracing::info!("Connecting to RaceControl core at {}...", config.core.url);
        let ws_result = tokio::time::timeout(
            Duration::from_secs(10),
            connect_async(&config.core.url),
        ).await;

        let (ws_stream, _) = match ws_result {
            Ok(Ok(stream)) => {
                reconnect_delay = Duration::from_secs(1); // Reset backoff on success
                stream
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to connect to core: {}. Retrying in {:?}...", e, reconnect_delay);
                lock_screen.show_disconnected();
                tokio::time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
                continue;
            }
            Err(_) => {
                tracing::warn!("Connection to core timed out. Retrying in {:?}...", reconnect_delay);
                lock_screen.show_disconnected();
                tokio::time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
                continue;
            }
        };
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // Register this pod (include current game state so core can resync)
        let register_msg = AgentMessage::Register(PodInfo {
            last_seen: Some(Utc::now()),
            driving_state: Some(detector.state()),
            game_state: game_process.as_ref().map(|g| g.state),
            current_game: game_process.as_ref().map(|g| g.sim_type),
            ..pod_info.clone()
        });
        let json = serde_json::to_string(&register_msg)?;
        if ws_tx.send(Message::Text(json.into())).await.is_err() {
            tracing::warn!("Failed to register with core, reconnecting...");
            tokio::time::sleep(reconnect_delay).await;
            continue;
        }
        tracing::info!("Connected and registered as Pod #{}", config.pod.number);
        heartbeat_status.ws_connected.store(true, std::sync::atomic::Ordering::Relaxed);

        // Main event loop — runs until connection is lost
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(5));
        let mut telemetry_interval = tokio::time::interval(Duration::from_millis(100));
        let mut detector_interval = tokio::time::interval(Duration::from_millis(100));
        let mut game_check_interval = tokio::time::interval(Duration::from_secs(2));
        let mut kiosk_interval = tokio::time::interval(Duration::from_secs(5));
        let mut overlay_topmost_interval = tokio::time::interval(Duration::from_secs(10));
        // Auto-blank timer: set when session summary is shown, fires after 15s
        let mut blank_timer: std::pin::Pin<Box<tokio::time::Sleep>> =
            Box::pin(tokio::time::sleep(Duration::from_secs(86400))); // dormant
        let mut blank_timer_armed = false;

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    let hb = AgentMessage::Heartbeat(PodInfo {
                        status: PodStatus::Idle, // billing state is managed by rc-core, not agent
                        last_seen: Some(Utc::now()),
                        driving_state: Some(detector.state()),
                        game_state: game_process.as_ref().map(|g| g.state),
                        current_game: game_process.as_ref().map(|g| g.sim_type),
                        ..pod_info.clone()
                    });
                    let json = serde_json::to_string(&hb)?;
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        tracing::error!("Lost connection to core server");
                        break; // → reconnection loop
                    }
                }
            _ = telemetry_interval.tick() => {
                let Some(ref mut adapter) = adapter else { continue };
                if !adapter.is_connected() {
                    let _ = adapter.connect();
                    continue;
                }

                match adapter.read_telemetry() {
                    Ok(Some(frame)) => {
                        // Update overlay with live telemetry
                        overlay.update_telemetry(&frame);

                        // Check for completed laps via adapter (has proper sector splits)
                        if let Ok(Some(lap)) = adapter.poll_lap_completed() {
                            overlay.on_lap_completed(&lap);
                            let msg = AgentMessage::LapCompleted(lap);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }

                        // Send telemetry frame
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
            }
            // Process driving detector signals from HID/UDP tasks
            Some(signal) = signal_rx.recv() => {
                let (_, changed) = detector.process_signal(signal);
                if changed {
                    let is_active = matches!(detector.state(), DrivingState::Active);
                    heartbeat_status.driving_active.store(is_active, std::sync::atomic::Ordering::Relaxed);
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: pod_id.clone(),
                        state: detector.state(),
                    };
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed: {:?}", detector.state());
                }
            }
            // Periodic detector evaluation (catches idle timeout transitions)
            _ = detector_interval.tick() => {
                let (_, changed) = detector.evaluate_state();
                if changed {
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: pod_id.clone(),
                        state: detector.state(),
                    };
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed (timeout): {:?}", detector.state());
                }
            }
            // Game process health check (every 2s)
            _ = game_check_interval.tick() => {
                if let Some(ref mut game) = game_process {
                    let was_active = matches!(game.state, GameState::Running | GameState::Launching);

                    if game.state == GameState::Launching && game.child.is_none() {
                        // Steam-launched game — scan for process by name
                        if let Some(pid) = game_process::find_game_pid(game.sim_type) {
                            game.pid = Some(pid);
                            game.state = GameState::Running;
                            let info = GameLaunchInfo {
                                pod_id: pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Running,
                                pid: Some(pid),
                                launched_at: Some(Utc::now()),
                                error_message: None,
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                    } else {
                        let still_alive = game.is_running();
                        if !still_alive && was_active {
                            // Game crashed or exited
                            heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                            let err_msg = "Game process exited unexpectedly".to_string();
                            let info = GameLaunchInfo {
                                pod_id: pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Error,
                                pid: game.pid,
                                launched_at: None,
                                error_message: Some(err_msg.clone()),
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;

                            // Trigger AI debugger if configured
                            if config.ai_debugger.enabled {
                                let exit_info = game.last_exit_code
                                    .map(|c| format!("exit code {}", c))
                                    .unwrap_or_else(|| "no exit code".to_string());
                                let err_ctx = format!("{:?} crashed on pod {} ({})", game.sim_type, pod_id, exit_info);
                                tokio::spawn(ai_debugger::analyze_crash(
                                    config.ai_debugger.clone(),
                                    pod_id.clone(),
                                    game.sim_type,
                                    err_ctx,
                                    ai_result_tx.clone(),
                                ));
                            }

                            game_process = None;
                        }
                    }
                }
            }
            // AI debug result channel
            Some(suggestion) = ai_result_rx.recv() => {
                let msg = AgentMessage::AiDebugResult(suggestion);
                let json = serde_json::to_string(&msg)?;
                let _ = ws_tx.send(Message::Text(json.into())).await;
            }
            // Kiosk enforcement — kill unauthorized processes
            _ = kiosk_interval.tick() => {
                if kiosk_enabled {
                    kiosk.enforce_process_whitelist();
                }
            }
            // Re-enforce overlay TOPMOST every 10s (survives game focus changes)
            _ = overlay_topmost_interval.tick() => {
                overlay.enforce_topmost();
            }
            // Auto-blank after session summary (15s delay)
            _ = &mut blank_timer, if blank_timer_armed => {
                tracing::info!("Auto-blanking screen after session summary");
                lock_screen.show_blank_screen();
                blank_timer_armed = false;
            }
            // Lock screen events (customer submitted PIN)
            Some(event) = lock_event_rx.recv() => {
                match event {
                    LockScreenEvent::PinEntered { pin } => {
                        let msg = AgentMessage::PinEntered {
                            pod_id: pod_id.clone(),
                            pin,
                        };
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                        tracing::info!("PIN submitted, forwarding to core for verification");
                    }
                }
            }
            // UDP heartbeat events (fast liveness detection)
            Some(hb_event) = heartbeat_event_rx.recv() => {
                match hb_event {
                    udp_heartbeat::HeartbeatEvent::CoreDead => {
                        tracing::warn!("UDP heartbeat: core dead — forcing WebSocket reconnect");
                        break; // → reconnection loop
                    }
                    udp_heartbeat::HeartbeatEvent::ForceReconnect => {
                        tracing::info!("UDP heartbeat: core requested reconnect");
                        break; // → reconnection loop
                    }
                    udp_heartbeat::HeartbeatEvent::ForceRestart => {
                        tracing::warn!("UDP heartbeat: core requested restart — exiting");
                        std::process::exit(0); // Watchdog will restart us
                    }
                    udp_heartbeat::HeartbeatEvent::CoreAlive => {
                        // Informational — core is back after being dead
                    }
                }
            }
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        tracing::debug!("Received from core: {}", text);
                        if let Ok(core_msg) = serde_json::from_str::<rc_common::protocol::CoreToAgentMessage>(&text) {
                            match core_msg {
                                rc_common::protocol::CoreToAgentMessage::BillingStarted { billing_session_id, driver_name, allocated_seconds } => {
                                    tracing::info!("Billing started: {} for {} ({}s)", billing_session_id, driver_name, allocated_seconds);
                                    heartbeat_status.billing_active.store(true, std::sync::atomic::Ordering::Relaxed);
                                    blank_timer_armed = false; // cancel any pending auto-blank
                                    overlay.activate(driver_name.clone(), allocated_seconds);
                                    lock_screen.show_active_session(driver_name, allocated_seconds, allocated_seconds);
                                }
                                rc_common::protocol::CoreToAgentMessage::BillingTick { remaining_seconds, allocated_seconds: _, driver_name: _ } => {
                                    lock_screen.update_remaining(remaining_seconds);
                                    overlay.update_billing(remaining_seconds);
                                }
                                rc_common::protocol::CoreToAgentMessage::BillingStopped { billing_session_id } => {
                                    tracing::info!("Billing stopped: {}", billing_session_id);
                                    overlay.deactivate();
                                    // Fallback — SessionEnded is the preferred message with summary data
                                    lock_screen.show_active_session("Session Complete!".to_string(), 0, 0);
                                }
                                rc_common::protocol::CoreToAgentMessage::SessionEnded {
                                    billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
                                } => {
                                    tracing::info!(
                                        "Session ended: {} — {} laps, best: {:?}, {}s",
                                        billing_session_id, total_laps, best_lap_ms, driving_seconds
                                    );
                                    heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                                    overlay.deactivate();
                                    // Stop the game if still running
                                    if let Some(ref mut game) = game_process {
                                        let _ = game.stop();
                                        game_process = None;
                                    }
                                    // Show session summary, then auto-blank after 15s
                                    lock_screen.show_session_summary(
                                        driver_name, total_laps, best_lap_ms, driving_seconds,
                                    );
                                    blank_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
                                    blank_timer_armed = true;
                                }
                                rc_common::protocol::CoreToAgentMessage::LaunchGame { sim_type: launch_sim, launch_args } => {
                                    tracing::info!("Launching game: {:?} (args: {:?})", launch_sim, launch_args);

                                    // AC gets special handling: kill → write race.ini → launch → restart Conspit
                                    if launch_sim == SimType::AssettoCorsa {
                                        // Disconnect telemetry adapter before killing AC
                                        if let Some(ref mut adp) = adapter { adp.disconnect(); }

                                        // Parse launch params from JSON
                                        let params: ac_launcher::AcLaunchParams = match &launch_args {
                                            Some(args) => serde_json::from_str(args).unwrap_or(ac_launcher::AcLaunchParams {
                                                car: "ks_ferrari_sf15t".to_string(),
                                                track: "spa".to_string(),
                                                driver: "Driver".to_string(),
                                                track_config: String::new(),
                                                skin: "00_default".to_string(),
                                                transmission: "manual".to_string(),
                                                aids: None,
                                                conditions: None,
                                                duration_minutes: 60,
                                            }),
                                            None => ac_launcher::AcLaunchParams {
                                                car: "ks_ferrari_sf15t".to_string(),
                                                track: "spa".to_string(),
                                                driver: "Driver".to_string(),
                                                track_config: String::new(),
                                                skin: "00_default".to_string(),
                                                transmission: "manual".to_string(),
                                                aids: None,
                                                conditions: None,
                                                duration_minutes: 60,
                                            },
                                        };

                                        // Update heartbeat status
                                        heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                                        heartbeat_status.game_id.store(match launch_sim {
                                            SimType::AssettoCorsa => 1,
                                            SimType::F125 => 2,
                                            SimType::IRacing => 3,
                                            SimType::LeMansUltimate => 4,
                                            SimType::Forza => 5,
                                        }, std::sync::atomic::Ordering::Relaxed);

                                        // Send "launching" state
                                        let info = GameLaunchInfo {
                                            pod_id: pod_id.clone(),
                                            sim_type: launch_sim,
                                            game_state: GameState::Launching,
                                            pid: None,
                                            launched_at: Some(Utc::now()),
                                            error_message: None,
                                        };
                                        let msg = AgentMessage::GameStateUpdate(info);
                                        let json_str = serde_json::to_string(&msg)?;
                                        let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                        // Run blocking launch sequence in spawn_blocking
                                        let pod_id_clone = pod_id.clone();
                                        let launch_result = tokio::task::spawn_blocking(move || {
                                            ac_launcher::launch_ac(&params)
                                        }).await;

                                        let launch_result = match launch_result {
                                            Ok(r) => r,
                                            Err(e) => {
                                                tracing::error!("AC launch task panicked: {}", e);
                                                Err(anyhow::anyhow!("Launch task panicked: {}", e))
                                            }
                                        };

                                        match launch_result {
                                            Ok(pid) => {
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id_clone,
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Running,
                                                    pid: Some(pid),
                                                    launched_at: Some(Utc::now()),
                                                    error_message: None,
                                                };
                                                game_process = Some(game_process::GameProcess {
                                                    sim_type: launch_sim,
                                                    state: GameState::Running,
                                                    child: None,
                                                    pid: Some(pid),
                                                    last_exit_code: None,
                                                });
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                                // Reconnect telemetry adapter to new AC instance
                                                if let Some(ref mut adp) = adapter {
                                                    match adp.connect() {
                                                        Ok(()) => tracing::info!("Reconnected to AC telemetry"),
                                                        Err(e) => tracing::warn!("Could not reconnect telemetry: {}", e),
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!("AC launch failed: {}", e);
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id_clone,
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Error,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: Some(e.to_string()),
                                                };
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                                            }
                                        }
                                    } else {
                                        // Generic launch for other sims
                                        let base_config = match launch_sim {
                                            SimType::AssettoCorsa => &config.games.assetto_corsa,
                                            SimType::IRacing => &config.games.iracing,
                                            SimType::F125 => &config.games.f1_25,
                                            SimType::LeMansUltimate => &config.games.le_mans_ultimate,
                                            SimType::Forza => &config.games.forza,
                                        };
                                        let mut game_config = base_config.clone();
                                        if let Some(args) = launch_args {
                                            game_config.args = Some(args);
                                        }
                                        match game_process::GameProcess::launch(&game_config, launch_sim) {
                                            Ok(gp) => {
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id.clone(),
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Launching,
                                                    pid: gp.pid,
                                                    launched_at: Some(Utc::now()),
                                                    error_message: None,
                                                };
                                                game_process = Some(gp);
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to launch {:?}: {}", launch_sim, e);
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id.clone(),
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Error,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: Some(e.to_string()),
                                                };
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;
                                            }
                                        }
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::StopGame => {
                                    heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                                    heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                                    if let Some(ref mut game) = game_process {
                                        tracing::info!("Stopping game: {:?}", game.sim_type);
                                        let sim = game.sim_type;
                                        match game.stop() {
                                            Ok(()) => {
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id.clone(),
                                                    sim_type: sim,
                                                    game_state: GameState::Idle,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: None,
                                                };
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json.into())).await;
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to stop game: {}", e);
                                            }
                                        }
                                        game_process = None;
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::ShowPinLockScreen {
                                    token_id, driver_name, pricing_tier_name, allocated_seconds,
                                } => {
                                    tracing::info!("Lock screen: PIN entry for {}", driver_name);
                                    lock_screen.show_pin_screen(
                                        token_id, driver_name, pricing_tier_name, allocated_seconds,
                                    );
                                }
                                rc_common::protocol::CoreToAgentMessage::ShowQrLockScreen {
                                    token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds,
                                } => {
                                    tracing::info!("Lock screen: QR display for {}", driver_name);
                                    lock_screen.show_qr_screen(
                                        token_id, qr_payload, driver_name, pricing_tier_name, allocated_seconds,
                                    );
                                }
                                rc_common::protocol::CoreToAgentMessage::ClearLockScreen => {
                                    tracing::info!("Lock screen cleared");
                                    overlay.deactivate();
                                    lock_screen.clear();
                                }
                                rc_common::protocol::CoreToAgentMessage::BlankScreen => {
                                    tracing::info!("Screen blanked via direct command");
                                    overlay.deactivate();
                                    lock_screen.show_blank_screen();
                                }
                                rc_common::protocol::CoreToAgentMessage::SubSessionEnded {
                                    billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds, wallet_balance_paise,
                                } => {
                                    tracing::info!(
                                        "Sub-session ended: {} — {} laps, wallet: {}p",
                                        billing_session_id, total_laps, wallet_balance_paise
                                    );
                                    overlay.deactivate();
                                    // Stop the game
                                    if let Some(ref mut game) = game_process {
                                        let _ = game.stop();
                                        game_process = None;
                                    }
                                    // Show between-sessions screen
                                    lock_screen.show_between_sessions(
                                        driver_name, total_laps, best_lap_ms, driving_seconds, wallet_balance_paise,
                                    );
                                }
                                rc_common::protocol::CoreToAgentMessage::ShowAssistanceScreen { driver_name, message } => {
                                    tracing::info!("Assistance screen for {}: {}", driver_name, message);
                                    lock_screen.show_assistance(driver_name, message);
                                }
                                rc_common::protocol::CoreToAgentMessage::EnterDebugMode { employee_name } => {
                                    tracing::info!("Employee debug mode activated by {}", employee_name);
                                    kiosk.enter_debug_mode();
                                    lock_screen.clear();
                                }
                                rc_common::protocol::CoreToAgentMessage::SettingsUpdated { settings } => {
                                    tracing::info!("Kiosk settings updated: {:?}", settings);
                                    if let Some(v) = settings.get("kiosk_lockdown_enabled") {
                                        if v == "true" && !kiosk.is_active() && !kiosk.is_debug_mode() {
                                            kiosk.activate();
                                            tracing::info!("Kiosk lockdown ENABLED via remote settings");
                                        } else if v == "false" && kiosk.is_active() {
                                            kiosk.deactivate();
                                            tracing::info!("Kiosk lockdown DISABLED via remote settings");
                                        }
                                    }
                                    if let Some(v) = settings.get("screen_blanking_enabled") {
                                        tracing::info!("Screen blanking set to: {}", v);
                                        if v == "true" && lock_screen.is_idle_or_blanked() {
                                            lock_screen.show_blank_screen();
                                            tracing::info!("Screen blanking ENABLED — screen blanked");
                                        } else if v == "false" {
                                            lock_screen.clear();
                                            tracing::info!("Screen blanking DISABLED — screen restored");
                                        }
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SetTransmission { transmission } => {
                                    tracing::info!("Setting transmission to '{}' mid-session", transmission);
                                    if let Err(e) = ac_launcher::set_transmission(&transmission) {
                                        tracing::error!("Failed to set transmission: {}", e);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("Core server closed connection");
                        break; // → reconnection loop
                    }
                    _ => {}
                }
            }
        }
        } // end inner event loop

        // Connection lost — update UDP heartbeat status and show disconnected
        heartbeat_status.ws_connected.store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::warn!("Disconnected from core server, will reconnect in {:?}...", reconnect_delay);
        lock_screen.show_disconnected();
        tokio::time::sleep(reconnect_delay).await;
        reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
    } // end reconnection loop
}

fn load_config() -> Result<AgentConfig> {
    let paths = ["rc-agent.toml", "/etc/racecontrol/rc-agent.toml"];
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let config: AgentConfig = toml::from_str(&content)?;
            tracing::info!("Loaded config from {}", path);
            return Ok(config);
        }
    }

    // Default config
    tracing::warn!("No config file found, using defaults");
    Ok(AgentConfig {
        pod: PodConfig {
            number: 1,
            name: "Pod 01".to_string(),
            sim: "assetto_corsa".to_string(),
            sim_ip: default_sim_ip(),
            sim_port: default_sim_port(),
        },
        core: CoreConfig {
            url: default_core_url(),
        },
        wheelbase: WheelbaseConfig::default(),
        telemetry_ports: TelemetryPortsConfig::default(),
        games: GamesConfig::default(),
        ai_debugger: AiDebuggerConfig::default(),
        kiosk: KioskConfig::default(),
    })
}

fn local_ip() -> String {
    // Simple local IP detection
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// USB HID wheelbase monitor — runs in a spawned task.
/// Reads input reports from the Conspit wheelbase and sends signals to the detector.
async fn run_hid_monitor(
    vid: u16,
    pid: u16,
    pedal_threshold: f32,
    steering_deadzone: f32,
    signal_tx: mpsc::Sender<DetectorSignal>,
) {
    let config = DetectorConfig {
        wheelbase_vid: vid,
        wheelbase_pid: pid,
        pedal_threshold,
        steering_deadzone,
        ..DetectorConfig::default()
    };

    loop {
        // Try to open the HID device (blocking operation)
        let result = tokio::task::spawn_blocking(move || {
            hidapi::HidApi::new()
        })
        .await;

        let api = match result {
            Ok(Ok(api)) => api,
            Ok(Err(e)) => {
                tracing::warn!("Failed to initialize HID API: {}", e);
                let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                tracing::error!("HID task panic: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Try to find and open the wheelbase
        let device = api.open(vid, pid);
        match device {
            Ok(dev) => {
                tracing::info!(
                    "Connected to wheelbase HID device (VID:{:#06x} PID:{:#06x})",
                    vid, pid
                );
                dev.set_blocking_mode(false).ok();

                let mut prev_steering: f32 = 0.0;
                let mut buf = [0u8; 64];

                loop {
                    match dev.read_timeout(&mut buf, 10) {
                        Ok(len) if len > 0 => {
                            if let Some(input) = parse_openffboard_report(&buf[..len]) {
                                let pedal_active = is_input_active(&input, &config);
                                let steer_moving =
                                    is_steering_moving(input.steering, prev_steering, 0.005);
                                prev_steering = input.steering;

                                let signal = if pedal_active || steer_moving {
                                    DetectorSignal::HidActive
                                } else {
                                    DetectorSignal::HidIdle
                                };
                                if signal_tx.send(signal).await.is_err() {
                                    return; // Main loop exited
                                }
                            }
                        }
                        Ok(_) => {
                            // No data (non-blocking)
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            tracing::warn!("HID read error: {}", e);
                            let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                tracing::debug!(
                    "Wheelbase HID device not found (VID:{:#06x} PID:{:#06x}), retrying...",
                    vid, pid
                );
                let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
            }
        }

        // Retry after delay
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// UDP telemetry port monitor — listens on multiple game telemetry ports.
/// If any data arrives on any port, signals that a game is actively outputting telemetry.
async fn run_udp_monitor(ports: Vec<u16>, signal_tx: mpsc::Sender<DetectorSignal>) {
    use tokio::net::UdpSocket;

    // Spawn a listener task per port — each sends UdpActive signals independently
    for port in ports {
        let tx = signal_tx.clone();
        tokio::spawn(async move {
            let sock = match UdpSocket::bind(format!("0.0.0.0:{}", port)).await {
                Ok(s) => {
                    tracing::info!("Listening for game telemetry on UDP port {}", port);
                    s
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not bind UDP port {}: {} (game may already be using it)",
                        port, e
                    );
                    return;
                }
            };

            let mut buf = [0u8; 2048];
            loop {
                match sock.recv_from(&mut buf).await {
                    Ok((len, _)) if len > 0 => {
                        if tx.send(DetectorSignal::UdpActive).await.is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("UDP recv error on port {}: {}", port, e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    // Keep this task alive; the per-port tasks do the actual work.
    // Send periodic UdpIdle signals so the detector knows UDP monitoring is running
    // but no data is arriving (if no per-port tasks have sent UdpActive recently).
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if signal_tx.send(DetectorSignal::UdpIdle).await.is_err() {
            return;
        }
    }
}

const WATCHDOG_DIR: &str = r"C:\RacingPoint";

/// Check if an exe is running; if not and it exists on disk, start it.
async fn watchdog_ensure_running(exe_name: &str) {
    let exe_path = format!(r"{}\{}", WATCHDOG_DIR, exe_name);
    if !std::path::Path::new(&exe_path).exists() {
        return;
    }

    let output = tokio::process::Command::new("cmd")
        .args(["/C", &format!("tasklist /NH /FI \"IMAGENAME eq {}\"", exe_name)])
        .kill_on_drop(true)
        .output()
        .await;

    let is_running = match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains(exe_name)
        }
        Err(_) => return,
    };

    if !is_running {
        tracing::warn!("[watchdog] {} not running — restarting", exe_name);
        let _ = tokio::process::Command::new("cmd")
            .args(["/C", &format!("cd /d {} && start /b {}", WATCHDOG_DIR, exe_name)])
            .kill_on_drop(false)
            .spawn();
    }
}
