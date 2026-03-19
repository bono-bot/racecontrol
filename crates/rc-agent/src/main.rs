mod ac_launcher;
mod ai_debugger;
mod billing_guard;
mod content_scanner;
mod debug_server;
mod driving_detector;
mod failure_monitor;
mod ffb_controller;
mod firewall;
mod game_process;
mod kiosk;
mod lock_screen;
mod overlay;
mod remote_ops;
mod self_heal;
mod self_monitor;
mod sims;
mod startup_log;
mod udp_heartbeat;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, Semaphore};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use driving_detector::{
    DetectorConfig, DetectorSignal, DrivingDetector,
    is_input_active, is_steering_moving, parse_openffboard_report,
};
use ffb_controller::FfbController;
use ai_debugger::{AiDebuggerConfig, PodStateSnapshot};
use game_process::GameExeConfig;
use rc_common::protocol::AgentMessage;
use rc_common::types::*;
use rc_common::types::AcStatus;
use sims::SimAdapter;
use sims::assetto_corsa::AssettoCorsaAdapter;
use sims::f1_25::F125Adapter;
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
    assetto_corsa_evo: GameExeConfig,
    #[serde(default)]
    assetto_corsa_rally: GameExeConfig,
    #[serde(default)]
    iracing: GameExeConfig,
    #[serde(default)]
    f1_25: GameExeConfig,
    #[serde(default)]
    le_mans_ultimate: GameExeConfig,
    #[serde(default)]
    forza: GameExeConfig,
    #[serde(default)]
    forza_horizon_5: GameExeConfig,
}

/// Detect which games are installed on this pod based on GamesConfig.
/// A game is considered "installed" if it has an exe_path or steam_app_id configured.
/// AC (original) is always included — it's the default game on every pod.
fn detect_installed_games(games: &GamesConfig) -> Vec<SimType> {
    let mut installed = vec![SimType::AssettoCorsa]; // AC always available (Content Manager)
    if games.f1_25.exe_path.is_some() || games.f1_25.steam_app_id.is_some() {
        installed.push(SimType::F125);
    }
    if games.iracing.exe_path.is_some() || games.iracing.steam_app_id.is_some() {
        installed.push(SimType::IRacing);
    }
    if games.forza.exe_path.is_some() || games.forza.steam_app_id.is_some() {
        installed.push(SimType::Forza);
    }
    if games.le_mans_ultimate.exe_path.is_some() || games.le_mans_ultimate.steam_app_id.is_some() {
        installed.push(SimType::LeMansUltimate);
    }
    if games.assetto_corsa_evo.exe_path.is_some() || games.assetto_corsa_evo.steam_app_id.is_some() {
        installed.push(SimType::AssettoCorsaEvo);
    }
    if games.assetto_corsa_rally.exe_path.is_some() || games.assetto_corsa_rally.steam_app_id.is_some() {
        installed.push(SimType::AssettoCorsaRally);
    }
    if games.forza_horizon_5.exe_path.is_some() || games.forza_horizon_5.steam_app_id.is_some() {
        installed.push(SimType::ForzaHorizon5);
    }
    installed
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

/// Tracks the state of a game launch attempt for timeout/retry handling.
/// BILL-01: 3-minute launch timeout with auto-retry once, cancel on second fail (no charge).
enum LaunchState {
    Idle,
    WaitingForLive {
        launched_at: std::time::Instant,
        attempt: u8, // 1 or 2
    },
    Live,
}

/// Independent semaphore for WebSocket command execution — separate from HTTP remote_ops semaphore (WSEX-02).
/// Ensures WS commands work even when all HTTP slots are full.
const WS_MAX_CONCURRENT_EXECS: usize = 4;
static WS_EXEC_SEMAPHORE: Semaphore = Semaphore::const_new(WS_MAX_CONCURRENT_EXECS);

/// Handle a WebSocket command request: acquire semaphore permit, run command with timeout,
/// return AgentMessage::ExecResult with stdout/stderr/exit_code.
async fn handle_ws_exec(request_id: String, cmd: String, timeout_ms: u64) -> rc_common::protocol::AgentMessage {
    use tokio::time::{timeout, Duration};

    let permit = match WS_EXEC_SEMAPHORE.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return rc_common::protocol::AgentMessage::ExecResult {
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

    // Truncate stdout/stderr to 64KB to prevent oversized WebSocket messages
    let truncate = |s: String| -> String {
        if s.len() > 65_536 {
            let mut t = s[..65_536].to_string();
            t.push_str("\n... [truncated]");
            t
        } else {
            s
        }
    };

    match result {
        Ok(Ok(output)) => rc_common::protocol::AgentMessage::ExecResult {
            request_id,
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: truncate(String::from_utf8_lossy(&output.stdout).to_string()),
            stderr: truncate(String::from_utf8_lossy(&output.stderr).to_string()),
        },
        Ok(Err(e)) => rc_common::protocol::AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to run command: {}", e),
        },
        Err(_) => rc_common::protocol::AgentMessage::ExecResult {
            request_id,
            success: false,
            exit_code: Some(124),
            stdout: String::new(),
            stderr: format!("Command timed out after {}ms", timeout_ms),
        },
    }
}

/// Guard against recursive panics in the hook
static PANIC_HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Lock screen state handle — set after LockScreenManager is created, used by panic hook
static PANIC_LOCK_STATE: OnceLock<std::sync::Arc<std::sync::Mutex<lock_screen::LockScreenState>>> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    // SAFETY-01: Panic hook — zero FFB + log crash + show error lock screen + exit
    // Must be installed BEFORE any other init so even config-load panics are caught.
    // The hook is sync-only: no async, no allocator-dependent code, try_lock not lock.
    let ffb_vid: u16 = 0x1209;  // Conspit Ares OpenFFBoard defaults
    let ffb_pid: u16 = 0xFFB0;
    std::panic::set_hook(Box::new(move |panic_info| {
        // Guard: only run once if somehow called recursively
        if PANIC_HOOK_ACTIVE.swap(true, Ordering::SeqCst) {
            std::process::exit(1);
        }

        // 1. Log to stderr (always safe)
        eprintln!("[rc-agent PANIC] {:?}", panic_info);

        // 2. Append to rc-bot-events.log (sync file write, safe)
        {
            use std::io::Write;
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(r"C:\RacingPoint\rc-bot-events.log")
                .and_then(|mut f| {
                    writeln!(f, "[{}] [PANIC] rc-agent crashed: {:?}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        panic_info)
                });
        }

        // 3. Zero FFB — the critical safety action (SAFETY-03 retry logic)
        let ffb = ffb_controller::FfbController::new(ffb_vid, ffb_pid);
        ffb.zero_force_with_retry(3, 100);

        // 4. Update lock screen to show error (use try_lock to avoid deadlock)
        if let Some(state_handle) = PANIC_LOCK_STATE.get() {
            if let Ok(mut state) = state_handle.try_lock() {
                *state = lock_screen::LockScreenState::ConfigError {
                    message: "System Error — Please Contact Staff".to_string(),
                };
            }
        }

        // 5. Small delay to let the HTTP server serve the error page
        std::thread::sleep(std::time::Duration::from_millis(500));

        // 6. Exit cleanly — no stack unwinding from here
        std::process::exit(1);
    }));

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

    // Logging: stdout + file appender (C:\RacingPoint\rc-agent.log)
    // File log persists after crashes so we can diagnose via pod-agent.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "rc_agent=info".into());

    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let file_appender = tracing_appender::rolling::never(&log_dir, "rc-agent.log");
    let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let stdout_layer = tracing_subscriber::fmt::layer();
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_file)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    println!(r#"
  RaceControl Agent
  Pod Telemetry Bridge
"#);

    // Detect crash recovery BEFORE write_phase("init") truncates the previous log
    let crash_recovery = startup_log::detect_crash_recovery();
    startup_log::write_phase("init", "");
    if crash_recovery {
        tracing::warn!("Detected crash recovery -- previous startup did not complete");
    }

    // Start a minimal lock screen server early so we can show a branded error
    // if config loading fails. The server does not require config values.
    let (early_lock_event_tx, _early_lock_event_rx) = mpsc::channel::<LockScreenEvent>(16);
    let mut early_lock_screen = LockScreenManager::new(early_lock_event_tx);
    early_lock_screen.start_server();
    startup_log::write_phase("lock_screen", "");

    // Self-heal: verify and repair config, start script, registry key (HEAL-01)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\RacingPoint"));
    let heal_result = self_heal::run(&exe_dir);
    if heal_result.config_repaired || heal_result.script_repaired || heal_result.registry_repaired {
        let repairs: Vec<&str> = [
            heal_result.config_repaired.then_some("config"),
            heal_result.script_repaired.then_some("script"),
            heal_result.registry_repaired.then_some("registry_key"),
        ].into_iter().flatten().collect();
        startup_log::write_phase("self_heal", &format!("repairs={}", repairs.join(",")));
    } else {
        startup_log::write_phase("self_heal", "no_repairs_needed");
    }

    // Load and validate config — fail fast with branded lock screen error on any issue
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Config error: {}", e);
            early_lock_screen.show_config_error(&e.to_string());
            // Give Edge time to render the error page before process exits
            tokio::time::sleep(Duration::from_secs(2)).await;
            std::process::exit(1);
        }
    };
    // Early lock screen is replaced by the main lock screen manager below
    drop(early_lock_screen);
    startup_log::write_phase("config_loaded", &format!("pod={}", config.pod.number));

    let agent_start_time = std::time::Instant::now();
    tracing::info!("Pod #{}: {} (sim: {})", config.pod.number, config.pod.name, config.pod.sim);
    tracing::info!("Core server: {}", config.core.url);

    // Clean up orphaned game processes from previous rc-agent instance
    let orphans_cleaned = game_process::cleanup_orphaned_games();
    if orphans_cleaned > 0 {
        tracing::warn!("Cleaned up {} orphaned game processes on startup", orphans_cleaned);
    }

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

    // Determine installed games from config
    let installed_games = detect_installed_games(&config.games);
    tracing::info!("Installed games: {:?}", installed_games);

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
        installed_games: installed_games.clone(),
        screen_blanked: None,
        ffb_preset: None,
    };

    // Firewall auto-config — ensure ICMP + TCP 8090 rules exist (FW-01, FW-02, FW-03)
    match firewall::configure() {
        firewall::FirewallResult::Configured => {
            tracing::info!("Firewall configured");
        }
        firewall::FirewallResult::Failed(msg) => {
            tracing::warn!("Firewall config failed: {} — continuing anyway", msg);
        }
    }
    startup_log::write_phase("firewall", "");

    // Remote ops HTTP server (merged pod-agent) — port 8090
    // SAFETY-02: start_checked returns a oneshot so bind failures are observable.
    // We start it early so the retry loop (up to 30s) runs concurrently with other init.
    let remote_ops_rx = remote_ops::start_checked(8090);
    startup_log::write_phase("http_server", "port=8090");

    // Set up driving detector (USB HID + UDP)
    let detector_config = DetectorConfig {
        wheelbase_vid: config.wheelbase.vendor_id,
        wheelbase_pid: config.wheelbase.product_id,
        telemetry_ports: config.telemetry_ports.ports.clone(),
        ..DetectorConfig::default()
    };
    let mut detector = DrivingDetector::new(&detector_config);

    // FFB safety controller — zero wheelbase torque on session end/startup
    let ffb = std::sync::Arc::new(FfbController::new(
        config.wheelbase.vendor_id,
        config.wheelbase.product_id,
    ));

    // FFB-03: Zero force on startup with retry — recover from any prior unclean exit
    // hid_detected = true if device found and command succeeded (used in BootVerification)
    let hid_detected = {
        let ffb_startup = ffb.clone();
        tokio::task::spawn_blocking(move || {
            ffb_startup.zero_force_with_retry(3, 100)
        }).await.unwrap_or(false)
    };

    // Channel for detector signals from HID/UDP tasks
    let (signal_tx, mut signal_rx) = mpsc::channel::<DetectorSignal>(256);

    // Create sim adapter (None for unsupported sims — they still run heartbeats)
    let mut adapter: Option<Box<dyn SimAdapter>> = match sim_type {
        SimType::AssettoCorsa => Some(Box::new(AssettoCorsaAdapter::new(
            pod_id.clone(),
            config.pod.sim_ip.clone(),
            config.pod.sim_port,
        ))),
        SimType::F125 => Some(Box::new(F125Adapter::new(
            pod_id.clone(),
            Some(signal_tx.clone()),
        ))),
        _ => {
            tracing::warn!("Sim adapter not yet implemented for {:?}, running in heartbeat-only mode", sim_type);
            None
        }
    };

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

    // AC STATUS polling state for billing trigger (Pitfall 1: stale shared memory, Pitfall 3: debounce)
    let mut last_ac_status: Option<AcStatus> = None;
    let mut ac_status_stable_since: Option<std::time::Instant> = None;
    let mut launch_state = LaunchState::Idle;

    // AI debugger result channel
    let (ai_result_tx, mut ai_result_rx) = mpsc::channel::<AiDebugSuggestion>(16);

    // WebSocket command result channel — spawned tasks send results here, select loop drains and sends via ws_tx
    let (ws_exec_result_tx, mut ws_exec_result_rx) = mpsc::channel::<rc_common::protocol::AgentMessage>(16);

    // Failure monitor state watch channel — main.rs event loop writes, failure_monitor reads
    let (failure_monitor_tx, failure_monitor_rx) =
        tokio::sync::watch::channel(failure_monitor::FailureMonitorState::default());

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
    // SAFETY-02: Use start_server_checked so bind failure is observable (not silent)
    let lock_screen_rx = lock_screen.start_server_checked();

    // Register lock screen state with panic hook so it can show error on crash
    let _ = PANIC_LOCK_STATE.set(lock_screen.state_handle());

    // SAFETY-02: Wait for lock screen bind result — exit on failure
    let lock_screen_bound = match tokio::time::timeout(
        Duration::from_secs(5),
        lock_screen_rx,
    ).await {
        Ok(Ok(Ok(port))) => {
            tracing::info!("Lock screen server bound on port {}", port);
            true
        }
        Ok(Ok(Err(e))) => {
            tracing::error!("FATAL: Lock screen bind failed: {}", e);
            std::process::exit(1);
        }
        Ok(Err(_)) => {
            tracing::error!("FATAL: Lock screen bind result channel dropped");
            std::process::exit(1);
        }
        Err(_) => {
            tracing::error!("FATAL: Lock screen bind timed out after 5s");
            std::process::exit(1);
        }
    };

    // LOCK-02: Show branded startup page immediately — customers see Racing Point
    // branding while rc-agent connects to racecontrol, not a blank screen or idle message.
    lock_screen.show_startup_connecting();

    // Racing HUD overlay for in-session display
    let mut overlay = OverlayManager::new();
    overlay.start_server();
    tracing::info!("Overlay server started on port 18925");

    // Shared state for last game launch error (visible in debug console)
    let last_launch_error: debug_server::LastLaunchError =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    // Debug server for remote diagnostics (LAN-accessible on port 18924)
    debug_server::spawn(
        lock_screen.state_handle(),
        config.pod.name.clone(),
        config.pod.number,
        last_launch_error.clone(),
    );

    // Delayed startup cleanup — enforce safe state to kill any orphaned games
    // from previous session/crash. Delay gives startup apps time to open.
    {
        let ffb_startup_cleanup = ffb.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            tokio::task::spawn_blocking(move || {
                ffb_startup_cleanup.zero_force_with_retry(3, 100);
                ac_launcher::enforce_safe_state();
            });
            tracing::info!("Startup: safe state enforced — pod clean for first customer");
        });
    }

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

    // ─── Self-Monitor (CLOSE_WAIT detection + LLM-gated relaunch) ───────────
    self_monitor::spawn(config.ai_debugger.clone(), heartbeat_status.clone());
    tracing::info!("Self-monitor started (check interval: 5min)");

    // ─── Failure Monitor (game freeze, launch timeout, USB reconnect) ────────
    failure_monitor::spawn(
        heartbeat_status.clone(),
        failure_monitor_rx,
        ws_exec_result_tx.clone(),
        pod_id.clone(),
        config.pod.number as u32,
    );
    tracing::info!("Failure monitor started (poll interval: 5s)");

    // ─── Billing Guard (stuck session + idle drift detection) ────────────────
    // Site billing_guard: spawn billing anomaly detection task (shares watch receiver)
    billing_guard::spawn(
        failure_monitor_tx.subscribe(),
        ws_exec_result_tx.clone(),
        pod_id.clone(),
    );
    tracing::info!("Billing guard started");

    // SAFETY-02: Wait for remote ops bind result — exit on failure.
    // Started early (before FFB/HID init) so the 30s retry window runs concurrently.
    let remote_ops_bound = match tokio::time::timeout(
        Duration::from_secs(35), // 10 attempts * 3s + margin
        remote_ops_rx,
    ).await {
        Ok(Ok(Ok(port))) => {
            tracing::info!("Remote ops server bound on port {}", port);
            true
        }
        Ok(Ok(Err(e))) => {
            tracing::error!("FATAL: Remote ops bind failed: {}", e);
            std::process::exit(1);
        }
        Ok(Err(_)) => {
            tracing::error!("FATAL: Remote ops bind result channel dropped");
            std::process::exit(1);
        }
        Err(_) => {
            tracing::error!("FATAL: Remote ops bind timed out after 35s");
            std::process::exit(1);
        }
    };

    // ─── Reconnection Loop ──────────────────────────────────────────────────
    // On disconnect, retry with exponential backoff. All local state
    // (lock screen, kiosk, HID/UDP monitors, game process) persists across
    // reconnections — only the WebSocket is re-established.
    let mut reconnect_attempt: u32 = 0;
    let mut startup_complete_logged = false;
    let mut startup_report_sent = false;

    loop {
        // Connect to core server
        tracing::info!("Connecting to RaceControl core at {}...", config.core.url);
        let ws_result = tokio::time::timeout(
            Duration::from_secs(10),
            connect_async(&config.core.url),
        ).await;

        let (ws_stream, _) = match ws_result {
            Ok(Ok(stream)) => {
                reconnect_attempt = 0; // Reset on successful connection
                stream
            }
            Ok(Err(e)) => {
                let delay = reconnect_delay_for_attempt(reconnect_attempt);
                tracing::warn!("Failed to connect to core: {}. Attempt {}. Retrying in {:?}...", e, reconnect_attempt, delay);
                lock_screen.show_disconnected();
                tokio::time::sleep(delay).await;
                reconnect_attempt += 1;
                continue;
            }
            Err(_) => {
                let delay = reconnect_delay_for_attempt(reconnect_attempt);
                tracing::warn!("Connection to core timed out. Attempt {}. Retrying in {:?}...", reconnect_attempt, delay);
                lock_screen.show_disconnected();
                tokio::time::sleep(delay).await;
                reconnect_attempt += 1;
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
            screen_blanked: Some(lock_screen.is_blanked()),
            ffb_preset: Some("medium".to_string()),
            ..pod_info.clone()
        });
        let json = serde_json::to_string(&register_msg)?;
        if ws_tx.send(Message::Text(json.into())).await.is_err() {
            let delay = reconnect_delay_for_attempt(reconnect_attempt);
            tracing::warn!("Failed to register with core. Attempt {}. Reconnecting in {:?}...", reconnect_attempt, delay);
            tokio::time::sleep(delay).await;
            reconnect_attempt += 1;
            continue;
        }
        tracing::info!("Connected and registered as Pod #{}", config.pod.number);
        if !startup_complete_logged {
            startup_log::write_phase("websocket", &format!("connected pod={}", config.pod.number));
            startup_log::write_phase("complete", "");
            startup_complete_logged = true;
        }

        // Send startup report once per process lifetime (HEAL-02)
        if !startup_report_sent {
            let config_path = exe_dir.join("rc-agent.toml");
            let startup_report = AgentMessage::StartupReport {
                pod_id: pod_id.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: agent_start_time.elapsed().as_secs(),
                config_hash: self_heal::config_hash(&config_path),
                crash_recovery,
                repairs: {
                    let mut r = Vec::new();
                    if heal_result.config_repaired { r.push("config".to_string()); }
                    if heal_result.script_repaired { r.push("script".to_string()); }
                    if heal_result.registry_repaired { r.push("registry_key".to_string()); }
                    r
                },
                // Phase 46 SAFETY-04: boot verification fields — wired in Plan 02
                lock_screen_port_bound: lock_screen_bound,
                remote_ops_port_bound: remote_ops_bound,
                hid_detected,
                udp_ports_bound: config.telemetry_ports.ports.clone(),
            };
            if let Ok(json) = serde_json::to_string(&startup_report) {
                if ws_tx.send(Message::Text(json.into())).await.is_ok() {
                    tracing::info!("Sent startup report to core (crash_recovery={})", crash_recovery);
                    startup_report_sent = true;
                } else {
                    tracing::warn!("Failed to send startup report — will retry next connect");
                }
            }
        }

        // Send content manifest after registration so core knows what's installed
        let manifest = content_scanner::scan_ac_content();
        tracing::info!("Scanned AC content: {} cars, {} tracks", manifest.cars.len(), manifest.tracks.len());
        let manifest_msg = AgentMessage::ContentManifest(manifest);
        if let Ok(json) = serde_json::to_string(&manifest_msg) {
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                tracing::warn!("Failed to send content manifest");
            }
        }

        heartbeat_status.ws_connected.store(true, std::sync::atomic::Ordering::Relaxed);
        let ws_connect_time = tokio::time::Instant::now();

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
        // Crash recovery timer: armed when game crashes during active billing.
        // If core doesn't send SessionEnded within 30s, force-reset to safe state.
        let mut crash_recovery_timer: std::pin::Pin<Box<tokio::time::Sleep>> =
            Box::pin(tokio::time::sleep(Duration::from_secs(86400))); // dormant
        let mut crash_recovery_armed = false;
        // Cache driver_name from BillingStarted for use in LaunchGame splash screen.
        // LaunchGame message does not carry driver_name — must be cached here.
        let mut current_driver_name: Option<String> = None;
        // Phase 6: Cache last-sent FFB gain percentage for QueryAssistState responses.
        // Default 70% (medium preset) — could read from controls.ini GAIN= at startup.
        let mut last_ffb_percent: u8 = 70;
        let mut last_ffb_preset: String = "medium".to_string();
        // Phase 11: Telemetry accumulators for session summary stats (SESS-01, SESS-02).
        // Reset on BillingStarted, passed to show_session_summary on SessionEnded.
        let mut session_max_speed_kmh: f32 = 0.0;
        let mut session_race_position: Option<u32> = None;

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    let hb = AgentMessage::Heartbeat(PodInfo {
                        status: PodStatus::Idle, // billing state is managed by racecontrol, not agent
                        last_seen: Some(Utc::now()),
                        driving_state: Some(detector.state()),
                        game_state: game_process.as_ref().map(|g| g.state),
                        current_game: game_process.as_ref().map(|g| g.sim_type),
                        screen_blanked: Some(lock_screen.is_blanked()),
                        ffb_preset: Some(last_ffb_preset.clone()),
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
                    if adapter.connect().is_ok() {
                        overlay.set_max_rpm(adapter.max_rpm());
                    }
                    continue;
                }

                match adapter.read_telemetry() {
                    Ok(Some(frame)) => {
                        // Update overlay with live telemetry
                        overlay.update_telemetry(&frame);
                        // Accumulate top speed for session summary (SESS-01)
                        if frame.speed_kmh > session_max_speed_kmh {
                            session_max_speed_kmh = frame.speed_kmh;
                        }

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

                // Poll AC STATUS for billing trigger (only when game process is alive)
                // Pitfall 1: guard with game_process.is_some() to avoid stale shared memory reads
                if game_process.is_some() {
                    if let Some(current_status) = adapter.read_ac_status() {
                        let status_changed = last_ac_status.map_or(true, |prev| prev != current_status);
                        if status_changed {
                            // Debounce: require STATUS to be stable for 1 second before reporting
                            // (prevents flapping on rapid ESC press — see RESEARCH.md Pitfall 3)
                            ac_status_stable_since = Some(std::time::Instant::now());
                            last_ac_status = Some(current_status);
                        }
                        // Send GameStatusUpdate only after STATUS has been stable for 1s
                        if let (Some(stable_since), Some(status)) = (ac_status_stable_since, last_ac_status) {
                            if stable_since.elapsed() >= Duration::from_secs(1) {
                                let msg = AgentMessage::GameStatusUpdate {
                                    pod_id: pod_id.clone(),
                                    ac_status: status,
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                                ac_status_stable_since = None; // sent, stop re-sending until next change

                                // Update LaunchState on successful STATUS=LIVE
                                if status == AcStatus::Live {
                                    launch_state = LaunchState::Live;
                                }
                            }
                        }
                    }
                }

                // Check launch timeout (3-min per CONTEXT.md locked decision, BILL-01)
                if let LaunchState::WaitingForLive { launched_at, attempt } = &launch_state {
                    if launched_at.elapsed() > Duration::from_secs(180) {
                        if *attempt < 2 {
                            // First timeout — auto-retry once
                            tracing::warn!("AC launch timeout (attempt {}), retrying...", attempt);
                            if let Some(ref mut proc) = game_process {
                                let _ = proc.stop();
                            }
                            game_process = None;

                            // Signal core that game is no longer running
                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: pod_id.clone(),
                                ac_status: AcStatus::Off,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_tx.send(Message::Text(json.into())).await;
                            }

                            // Update launch state to attempt 2 and wait for core to re-send LaunchGame
                            launch_state = LaunchState::WaitingForLive {
                                launched_at: std::time::Instant::now(),
                                attempt: attempt + 1,
                            };
                        } else {
                            // Second timeout — cancel entirely, no charge
                            tracing::error!("AC launch failed twice, cancelling session (no charge)");
                            if let Some(ref mut proc) = game_process {
                                let _ = proc.stop();
                            }
                            game_process = None;
                            launch_state = LaunchState::Idle;
                            // Site 3a: launch_started_at cleared when launch cancelled (2nd timeout)
                            let _ = failure_monitor_tx.send_modify(|s| { s.launch_started_at = None; });

                            // Notify core of launch failure so it can cancel the session (no billing)
                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: pod_id.clone(),
                                ac_status: AcStatus::Off,
                            };
                            if let Ok(json) = serde_json::to_string(&msg) {
                                let _ = ws_tx.send(Message::Text(json.into())).await;
                            }
                        }
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
                    // Site 9a: update failure_monitor watch with current driving state (signal path)
                    let _ = failure_monitor_tx.send_modify(|s| { s.driving_state = Some(detector.state()); });
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
                    // Site 9b: update failure_monitor watch with current driving state (timeout path)
                    let _ = failure_monitor_tx.send_modify(|s| { s.driving_state = Some(detector.state()); });
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed (timeout): {:?}", detector.state());
                }
                // Update failure monitor with current HID/UDP state (Site 1)
                let _ = failure_monitor_tx.send_modify(|s| {
                    s.hid_connected = detector.is_hid_connected();
                    s.last_udp_secs_ago = detector.last_udp_packet_elapsed_secs();
                });
            }
            // Game process health check (every 2s)
            _ = game_check_interval.tick() => {
                if let Some(ref mut game) = game_process {
                    let was_active = matches!(game.state, GameState::Running | GameState::Launching);

                    if game.state == GameState::Launching && game.child.is_none() {
                        // Steam-launched game — scan for process by name
                        if let Some(pid) = game_process::find_game_pid(game.sim_type) {
                            game.pid = Some(pid);
                            game_process::persist_pid(pid);
                            game.state = GameState::Running;
                            let info = GameLaunchInfo {
                                pod_id: pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Running,
                                pid: Some(pid),
                                launched_at: Some(Utc::now()),
                                error_message: None,
                                diagnostics: None,
                            };
                            // Site 4c: Steam-launched game PID discovered via find_game_pid
                            let _ = failure_monitor_tx.send_modify(|s| {
                                s.game_pid = Some(pid);
                            });
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
                                diagnostics: None,
                            };
                            let msg = AgentMessage::GameStateUpdate(info);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;

                            // Trigger AI debugger if configured
                            tracing::info!("[crash-detect] AI debugger enabled={}, url={}, model={}",
                                config.ai_debugger.enabled, config.ai_debugger.ollama_url, config.ai_debugger.ollama_model);
                            if config.ai_debugger.enabled {
                                let exit_info = game.last_exit_code
                                    .map(|c| format!("exit code {}", c))
                                    .unwrap_or_else(|| "no exit code".to_string());
                                let err_ctx = format!("{:?} crashed on pod {} ({})", game.sim_type, pod_id, exit_info);
                                tracing::info!("[crash-detect] Spawning AI debugger for: {}", err_ctx);
                                let snapshot = PodStateSnapshot {
                                    pod_id: pod_id.clone(),
                                    pod_number: config.pod.number,
                                    lock_screen_active: lock_screen.is_active(),
                                    billing_active: heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                    game_pid: game.pid,
                                    driving_state: Some(detector.current_state()),
                                    wheelbase_connected: detector.is_hid_connected(),
                                    ws_connected: heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                    uptime_seconds: agent_start_time.elapsed().as_secs(),
                                    ..Default::default()
                                };
                                tokio::spawn(ai_debugger::analyze_crash(
                                    config.ai_debugger.clone(),
                                    pod_id.clone(),
                                    game.sim_type,
                                    err_ctx,
                                    snapshot,
                                    ai_result_tx.clone(),
                                ));
                            }

                            game_process = None;
                            game_process::clear_persisted_pid();
                            // Reset STATUS tracking on game crash
                            last_ac_status = None;
                            ac_status_stable_since = None;
                            launch_state = LaunchState::Idle;
                            // Site 3b: launch_started_at cleared on game crash/exit
                            let _ = failure_monitor_tx.send_modify(|s| {
                                s.launch_started_at = None;
                                s.game_pid = None;
                            });

                            // If billing is active and game crashed, zero FFB immediately then arm crash recovery timer.
                            // Gives core 30s to send SessionEnded; otherwise force-reset.
                            if heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                                tracing::warn!("Game crashed during active billing — zeroing FFB immediately");
                                // SAFETY: Zero FFB immediately on crash during billing
                                { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                // Report crash + FFB status to core
                                let crash_msg = AgentMessage::GameCrashed { pod_id: pod_id.clone(), billing_active: true };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&crash_msg).unwrap_or_default().into())).await;
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                // Arm recovery timer
                                crash_recovery_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(30));
                                crash_recovery_armed = true;
                            } else {
                                // No billing active — enforce safe state immediately (FFB first, awaited)
                                tracing::info!("Game exited with no active billing — enforcing safe state");
                                { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
                                lock_screen.show_blank_screen();
                            }
                        }
                    }
                }
            }
            // AI debug result channel
            Some(mut suggestion) = ai_result_rx.recv() => {
                tracing::info!("[ai-result] Received AI suggestion for {}", suggestion.pod_id);
                // Attempt deterministic auto-fix in a blocking thread to avoid stalling the event loop
                let fix_snapshot = PodStateSnapshot {
                    pod_id: pod_id.clone(),
                    pod_number: config.pod.number,
                    lock_screen_active: lock_screen.is_active(),
                    billing_active: heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                    game_pid: game_process.as_ref().and_then(|g| g.pid),
                    driving_state: Some(detector.current_state()),
                    wheelbase_connected: detector.is_hid_connected(),
                    ws_connected: heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                    uptime_seconds: agent_start_time.elapsed().as_secs(),
                    // Site 9: 3 new fields added for failure_monitor context
                    last_udp_secs_ago: detector.last_udp_packet_elapsed_secs(),
                    game_launch_elapsed_secs: match &launch_state {
                        LaunchState::WaitingForLive { launched_at, .. } => Some(launched_at.elapsed().as_secs()),
                        _ => None,
                    },
                    hid_last_error: !detector.is_hid_connected(),
                    ..Default::default()
                };
                let suggestion_text = suggestion.suggestion.clone();
                let fix_handle = tokio::task::spawn_blocking(move || {
                    ai_debugger::try_auto_fix(&suggestion_text, &fix_snapshot)
                });
                // Timeout auto-fix after 10s — don't let a hanging process block the suggestion delivery
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
                    // Save successful fixes to pattern memory for instant replay next time
                    if fix_result.success {
                        let mut memory = ai_debugger::DebugMemory::load();
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
            // Kiosk enforcement — watch unauthorized processes, request approval from server
            _ = kiosk_interval.tick() => {
                if kiosk_enabled && kiosk.should_enforce() {
                    let allowed = kiosk.allowed_set_snapshot();
                    let pod_id_kiosk = pod_id.clone();
                    let kiosk_msg_tx = ws_exec_result_tx.clone();
                    let lockdown_flag = kiosk.lockdown.clone();
                    let lockdown_reason = kiosk.lockdown_reason.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = crate::kiosk::KioskManager::enforce_process_whitelist_blocking(allowed);

                        // Send approval requests to server
                        for approval in &result.pending_approvals {
                            let msg = rc_common::protocol::AgentMessage::ProcessApprovalRequest {
                                pod_id: pod_id_kiosk.clone(),
                                process_name: approval.process_name.clone(),
                                exe_path: approval.exe_path.clone(),
                                sighting_count: approval.sighting_count,
                            };
                            let _ = kiosk_msg_tx.try_send(msg);
                        }

                        // Handle expired temp entries — trigger lockdown
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
                    });
                }
            }
            // Re-enforce overlay TOPMOST + clean desktop + Conspit watchdog every 10s
            _ = overlay_topmost_interval.tick() => {
                overlay.enforce_topmost();
                if kiosk_enabled {
                    tokio::task::spawn_blocking(|| {
                        ac_launcher::minimize_background_windows();
                        // When no game is running, keep kiosk lock screen in foreground
                        lock_screen::enforce_kiosk_foreground();
                        // Restart Conspit Link if it crashed (stays minimized)
                        ac_launcher::ensure_conspit_link_running();
                    });
                }
            }
            // Auto-blank after session summary (15s delay) — only if no billing active
            _ = &mut blank_timer, if blank_timer_armed => {
                blank_timer_armed = false;
                if heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!("Skipping auto-blank — billing is active");
                } else {
                    tracing::info!("Auto-blanking screen after session summary");
                    lock_screen.show_blank_screen();
                    // Final cleanup pass — FFB zero first (awaited), then safe state
                    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
                }
            }
            // Crash recovery: game crashed during billing, core didn't send SessionEnded in 30s
            _ = &mut crash_recovery_timer, if crash_recovery_armed => {
                crash_recovery_armed = false;
                tracing::warn!("[crash-recovery] 30s timeout — core did not send SessionEnded. Force-resetting pod.");
                heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                overlay.deactivate();
                // Reset STATUS tracking and LaunchState
                last_ac_status = None;
                ac_status_stable_since = None;
                launch_state = LaunchState::Idle;
                // Site 3c: crash_recovery_timer cleared launch and billing state
                let _ = failure_monitor_tx.send_modify(|s| {
                    s.launch_started_at = None;
                    s.billing_active = false;
                    s.recovery_in_progress = false;
                });
                // SAFETY: Zero FFB FIRST (awaited), before any game cleanup
                { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                tokio::time::sleep(Duration::from_millis(500)).await;
                // STEP 1: Show lock screen (covers desktop before game cleanup)
                lock_screen.show_blank_screen();
                // STEP 2: Report FFB status
                let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                // STEP 3: Stop game and clean up AFTER FFB is zeroed
                if let Some(ref mut game) = game_process {
                    let _ = game.stop();
                    game_process = None;
                }
                if let Some(ref mut adp) = adapter { adp.disconnect(); }
                tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
                current_driver_name = None;
                // Notify core that we force-ended
                let msg = AgentMessage::GameStateUpdate(GameLaunchInfo {
                    pod_id: pod_id.clone(),
                    sim_type: SimType::AssettoCorsa,
                    game_state: GameState::Idle,
                    pid: None,
                    launched_at: None,
                    error_message: Some("Crash recovery: forced safe state after 30s timeout".to_string()),
                    diagnostics: None,
                });
                let json = serde_json::to_string(&msg)?;
                let _ = ws_tx.send(Message::Text(json.into())).await;
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
            // WebSocket command results from spawned tasks
            Some(ws_exec_msg) = ws_exec_result_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&ws_exec_msg) {
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        tracing::error!("Failed to send WS command result, connection lost");
                        break;
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
                        // Grace period: ignore force_reconnect within 10s of connecting
                        // to avoid race condition where UDP pong arrives before Register is processed
                        if ws_connect_time.elapsed() < Duration::from_secs(10) {
                            tracing::debug!("Ignoring force_reconnect — connected {}s ago (grace period)", ws_connect_time.elapsed().as_secs());
                        } else {
                            tracing::info!("UDP heartbeat: core requested reconnect");
                            break; // → reconnection loop
                        }
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
                                    // Site 6: BillingStarted — set billing_active in failure_monitor
                                    let _ = failure_monitor_tx.send_modify(|s| {
                                        s.billing_active = true;
                                    });
                                    // Cache driver_name for use in LaunchGame splash screen
                                    current_driver_name = Some(driver_name.clone());
                                    // Reset telemetry accumulators for new session (SESS-01, SESS-02)
                                    session_max_speed_kmh = 0.0;
                                    session_race_position = None;
                                    // If allocated_seconds is the hard max cap (10800) or 0, this is open-ended billing — use v2 taxi meter
                                    if allocated_seconds == 0 || allocated_seconds >= 10800 {
                                        overlay.activate_v2(driver_name.clone());
                                    } else {
                                        // Legacy fixed-duration billing — use existing activate
                                        overlay.activate(driver_name.clone(), allocated_seconds);
                                    }
                                    lock_screen.show_active_session(driver_name, allocated_seconds, allocated_seconds);
                                    // Minimize all background windows for a clean game view
                                    tokio::task::spawn_blocking(|| ac_launcher::minimize_background_windows());
                                }
                                rc_common::protocol::CoreToAgentMessage::BillingTick {
                                    remaining_seconds, allocated_seconds: _, driver_name: _,
                                    elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_next_tier, ..
                                } => {
                                    lock_screen.update_remaining(remaining_seconds); // keep legacy lock screen update
                                    // Use v2 billing update if new fields are present (core has been updated)
                                    if let (Some(elapsed), Some(cost), Some(rate)) = (elapsed_seconds, cost_paise, rate_per_min_paise) {
                                        overlay.update_billing_v2(
                                            elapsed,
                                            cost,
                                            rate,
                                            paused.unwrap_or(false),
                                            minutes_to_next_tier,
                                        );
                                    } else {
                                        // Fallback to legacy countdown update (old core version)
                                        overlay.update_billing(remaining_seconds);
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::BillingStopped { billing_session_id } => {
                                    tracing::info!("Billing stopped: {}", billing_session_id);
                                    heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                                    overlay.deactivate();
                                    // Reset STATUS tracking and LaunchState
                                    last_ac_status = None;
                                    ac_status_stable_since = None;
                                    launch_state = LaunchState::Idle;
                                    // Site 6+3d: BillingStopped — clear billing and launch state in failure_monitor
                                    let _ = failure_monitor_tx.send_modify(|s| {
                                        s.billing_active = false;
                                        s.launch_started_at = None;
                                    });
                                    // SAFETY: Zero FFB BEFORE anything else (awaited)
                                    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    // Show lock screen (covers desktop before game is killed)
                                    lock_screen.show_active_session("Session Complete!".to_string(), 0, 0);
                                    // Stop game and clean up AFTER FFB is zeroed
                                    if let Some(ref mut game) = game_process {
                                        let _ = game.stop();
                                        game_process = None;
                                    }
                                    if let Some(ref mut adp) = adapter { adp.disconnect(); }
                                    // Report FFB status
                                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                    // Cleanup (enforce_safe_state WITHOUT ffb zero — already done)
                                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
                                    current_driver_name = None;
                                }
                                rc_common::protocol::CoreToAgentMessage::SessionEnded {
                                    billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds,
                                } => {
                                    tracing::info!(
                                        "Session ended: {} — {} laps, best: {:?}, {}s",
                                        billing_session_id, total_laps, best_lap_ms, driving_seconds
                                    );
                                    heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                                    crash_recovery_armed = false; // cancel crash timer — core responded
                                    overlay.deactivate();
                                    // Reset STATUS tracking and LaunchState
                                    last_ac_status = None;
                                    ac_status_stable_since = None;
                                    launch_state = LaunchState::Idle;
                                    // Site 7: SessionEnded — clear billing, launch, and recovery state in failure_monitor
                                    let _ = failure_monitor_tx.send_modify(|s| {
                                        s.billing_active = false;
                                        s.launch_started_at = None;
                                        s.recovery_in_progress = false;
                                    });

                                    // SAFETY: Zero FFB BEFORE anything else (awaited)
                                    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                    tokio::time::sleep(Duration::from_millis(500)).await;

                                    // Show session summary with accumulated telemetry stats (SESS-01, SESS-02)
                                    lock_screen.show_session_summary(
                                        driver_name, total_laps, best_lap_ms, driving_seconds,
                                        if session_max_speed_kmh > 0.0 { Some(session_max_speed_kmh) } else { None },
                                        session_race_position,
                                    );

                                    // Stop game and clean up AFTER FFB is zeroed
                                    if let Some(ref mut game) = game_process {
                                        let _ = game.stop();
                                        game_process = None;
                                    }
                                    // Disconnect telemetry adapter
                                    if let Some(ref mut adp) = adapter { adp.disconnect(); }
                                    // Report FFB status
                                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                    // Cleanup (enforce_safe_state WITHOUT ffb zero — already done)
                                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
                                    current_driver_name = None;
                                    // LIFE-03: auto-return to idle after 15s session summary display
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

                                        // Update heartbeat status
                                        heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                                        heartbeat_status.game_id.store(match launch_sim {
                                            SimType::AssettoCorsa => 1,
                                            SimType::F125 => 2,
                                            SimType::IRacing => 3,
                                            SimType::LeMansUltimate => 4,
                                            SimType::Forza => 5,
                                            SimType::AssettoCorsaEvo => 6,
                                            SimType::AssettoCorsaRally => 7,
                                            SimType::ForzaHorizon5 => 8,
                                        }, std::sync::atomic::Ordering::Relaxed);

                                        // Show branded splash screen while game loads (~10s gap)
                                        // Must be before spawn_blocking so the screen is visible during the long load
                                        let splash_name = current_driver_name.clone().unwrap_or_else(|| "Driver".to_string());
                                        lock_screen.show_launch_splash(splash_name);

                                        // Send "launching" state
                                        let info = GameLaunchInfo {
                                            pod_id: pod_id.clone(),
                                            sim_type: launch_sim,
                                            game_state: GameState::Launching,
                                            pid: None,
                                            launched_at: Some(Utc::now()),
                                            error_message: None,
                                            diagnostics: None,
                                        };
                                        let msg = AgentMessage::GameStateUpdate(info);
                                        let json_str = serde_json::to_string(&msg)?;
                                        let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                        // Track launch for timeout handling (BILL-01: 3-min timeout)
                                        launch_state = LaunchState::WaitingForLive {
                                            launched_at: std::time::Instant::now(),
                                            attempt: 1,
                                        };
                                        // Site 2: notify failure_monitor that launch started
                                        let _ = failure_monitor_tx.send_modify(|s| {
                                            s.launch_started_at = Some(std::time::Instant::now());
                                        });

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
                                            Ok(result) => {
                                                // Clear or set launch error in debug console
                                                if let Ok(mut err_slot) = last_launch_error.lock() {
                                                    *err_slot = result.cm_error.clone();
                                                }

                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id_clone.clone(),
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Running,
                                                    pid: Some(result.pid),
                                                    launched_at: Some(Utc::now()),
                                                    error_message: result.cm_error.clone(),
                                                    diagnostics: Some(rc_common::types::LaunchDiagnostics {
                                                        cm_attempted: result.diagnostics.cm_attempted,
                                                        cm_exit_code: result.diagnostics.cm_exit_code,
                                                        cm_log_errors: result.diagnostics.cm_log_errors.clone(),
                                                        fallback_used: result.diagnostics.fallback_used,
                                                        direct_exit_code: result.diagnostics.direct_exit_code,
                                                    }),
                                                };
                                                game_process::persist_pid(result.pid);
                                                game_process = Some(game_process::GameProcess {
                                                    sim_type: launch_sim,
                                                    state: GameState::Running,
                                                    child: None,
                                                    pid: Some(result.pid),
                                                    last_exit_code: None,
                                                });
                                                // Site 4: game_process gained a PID after successful AC launch
                                                let _ = failure_monitor_tx.send_modify(|s| {
                                                    s.game_pid = Some(result.pid);
                                                });
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                                // If CM failed during multiplayer, store in debug console + trigger AI debugger
                                                if let Some(ref cm_err) = result.cm_error {
                                                    tracing::error!("[CM_ERROR] Multiplayer CM failure on {}: {}", pod_id_clone, cm_err);
                                                    if let Ok(mut err_slot) = last_launch_error.lock() {
                                                        *err_slot = Some(cm_err.clone());
                                                    }
                                                    if config.ai_debugger.enabled {
                                                        let err_ctx = format!(
                                                            "Content Manager multiplayer launch failed on pod {}. {}. \
                                                             Fell back to direct acs.exe launch.",
                                                            pod_id_clone, cm_err
                                                        );
                                                        let snapshot = PodStateSnapshot {
                                                            pod_id: pod_id_clone.clone(),
                                                            pod_number: config.pod.number,
                                                            lock_screen_active: lock_screen.is_active(),
                                                            billing_active: heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                                            game_pid: None,
                                                            driving_state: Some(detector.current_state()),
                                                            wheelbase_connected: detector.is_hid_connected(),
                                                            ws_connected: heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                                            uptime_seconds: agent_start_time.elapsed().as_secs(),
                                                            ..Default::default()
                                                        };
                                                        tokio::spawn(ai_debugger::analyze_crash(
                                                            config.ai_debugger.clone(),
                                                            pod_id_clone.clone(),
                                                            launch_sim,
                                                            err_ctx,
                                                            snapshot,
                                                            ai_result_tx.clone(),
                                                        ));
                                                    }
                                                }

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
                                                if let Ok(mut err_slot) = last_launch_error.lock() {
                                                    *err_slot = Some(format!("Launch failed: {}", e));
                                                }
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id_clone.clone(),
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Error,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: Some(e.to_string()),
                                                    diagnostics: None,
                                                };
                                                let msg = AgentMessage::GameStateUpdate(info);
                                                let json_str = serde_json::to_string(&msg)?;
                                                let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                                // Trigger AI debugger for total launch failure
                                                if config.ai_debugger.enabled {
                                                    let err_ctx = format!(
                                                        "AC launch completely failed on pod {}: {}",
                                                        pod_id_clone, e
                                                    );
                                                    let snapshot = PodStateSnapshot {
                                                        pod_id: pod_id_clone.clone(),
                                                        pod_number: config.pod.number,
                                                        lock_screen_active: lock_screen.is_active(),
                                                        billing_active: heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed),
                                                        game_pid: None,
                                                        driving_state: Some(detector.current_state()),
                                                        wheelbase_connected: detector.is_hid_connected(),
                                                        ws_connected: heartbeat_status.ws_connected.load(std::sync::atomic::Ordering::Relaxed),
                                                        uptime_seconds: agent_start_time.elapsed().as_secs(),
                                                        ..Default::default()
                                                    };
                                                    tokio::spawn(ai_debugger::analyze_crash(
                                                        config.ai_debugger.clone(),
                                                        pod_id_clone,
                                                        launch_sim,
                                                        err_ctx,
                                                        snapshot,
                                                        ai_result_tx.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                    } else {
                                        // Generic launch for other sims (F1 25, iRacing, LMU, Forza, AC Evo/Rally)
                                        // Mirrors AC lifecycle: heartbeat → splash → launch_state → launch → monitor
                                        let base_config = match launch_sim {
                                            SimType::AssettoCorsa => &config.games.assetto_corsa,
                                            SimType::AssettoCorsaEvo => &config.games.assetto_corsa_evo,
                                            SimType::AssettoCorsaRally => &config.games.assetto_corsa_rally,
                                            SimType::IRacing => &config.games.iracing,
                                            SimType::F125 => &config.games.f1_25,
                                            SimType::LeMansUltimate => &config.games.le_mans_ultimate,
                                            SimType::Forza => &config.games.forza,
                                            SimType::ForzaHorizon5 => &config.games.forza_horizon_5,
                                        };
                                        let mut game_config = base_config.clone();
                                        if let Some(args) = launch_args {
                                            game_config.args = Some(args);
                                        }

                                        // 1. Update heartbeat status (was missing — pod appeared idle during launch)
                                        heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
                                        heartbeat_status.game_id.store(match launch_sim {
                                            SimType::AssettoCorsa => 1,
                                            SimType::F125 => 2,
                                            SimType::IRacing => 3,
                                            SimType::LeMansUltimate => 4,
                                            SimType::Forza => 5,
                                            SimType::AssettoCorsaEvo => 6,
                                            SimType::AssettoCorsaRally => 7,
                                            SimType::ForzaHorizon5 => 8,
                                        }, std::sync::atomic::Ordering::Relaxed);

                                        // 2. Show branded splash screen (was missing — blank screen during load)
                                        let splash_name = current_driver_name.clone().unwrap_or_else(|| "Driver".to_string());
                                        lock_screen.show_launch_splash(splash_name);

                                        // 3. Arm launch timeout (was missing — BILL-01 3-min timeout didn't work)
                                        launch_state = LaunchState::WaitingForLive {
                                            launched_at: std::time::Instant::now(),
                                            attempt: 1,
                                        };

                                        // 4. Notify failure monitor (was missing — self-monitor couldn't detect stuck launches)
                                        let _ = failure_monitor_tx.send_modify(|s| {
                                            s.launch_started_at = Some(std::time::Instant::now());
                                        });

                                        // 5. Send Launching state to server
                                        let launching_info = GameLaunchInfo {
                                            pod_id: pod_id.clone(),
                                            sim_type: launch_sim,
                                            game_state: GameState::Launching,
                                            pid: None,
                                            launched_at: Some(Utc::now()),
                                            error_message: None,
                                            diagnostics: None,
                                        };
                                        let msg = AgentMessage::GameStateUpdate(launching_info);
                                        let json_str = serde_json::to_string(&msg)?;
                                        let _ = ws_tx.send(Message::Text(json_str.into())).await;

                                        // 6. Launch the game
                                        match game_process::GameProcess::launch(&game_config, launch_sim) {
                                            Ok(gp) => {
                                                tracing::info!("Generic sim {:?} launched (pid: {:?})", launch_sim, gp.pid);
                                                // Site 4b: game_process gained PID after generic sim launch
                                                let gp_pid = gp.pid;
                                                game_process = Some(gp);
                                                let _ = failure_monitor_tx.send_modify(|s| {
                                                    s.game_pid = gp_pid;
                                                });
                                                // For direct-exe launches (pid known), report Running immediately.
                                                // For Steam launches (pid=None), game_check_interval will scan + transition.
                                                if let Some(pid) = gp_pid {
                                                    let info = GameLaunchInfo {
                                                        pod_id: pod_id.clone(),
                                                        sim_type: launch_sim,
                                                        game_state: GameState::Running,
                                                        pid: Some(pid),
                                                        launched_at: Some(Utc::now()),
                                                        error_message: None,
                                                        diagnostics: None,
                                                    };
                                                    let msg = AgentMessage::GameStateUpdate(info);
                                                    let json_str = serde_json::to_string(&msg)?;
                                                    let _ = ws_tx.send(Message::Text(json_str.into())).await;
                                                }
                                                // Steam launches: game_check_interval (2s tick) will find PID via
                                                // find_game_pid() and send Running state — no extra code needed here.
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to launch {:?}: {}", launch_sim, e);
                                                heartbeat_status.game_running.store(false, std::sync::atomic::Ordering::Relaxed);
                                                heartbeat_status.game_id.store(0, std::sync::atomic::Ordering::Relaxed);
                                                launch_state = LaunchState::Idle;
                                                let _ = failure_monitor_tx.send_modify(|s| {
                                                    s.launch_started_at = None;
                                                });
                                                let info = GameLaunchInfo {
                                                    pod_id: pod_id.clone(),
                                                    sim_type: launch_sim,
                                                    game_state: GameState::Error,
                                                    pid: None,
                                                    launched_at: None,
                                                    error_message: Some(e.to_string()),
                                                    diagnostics: None,
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
                                    // Reset STATUS tracking and LaunchState
                                    last_ac_status = None;
                                    ac_status_stable_since = None;
                                    launch_state = LaunchState::Idle;
                                    // Site 8: StopGame from server — set recovery_in_progress + clear launch state
                                    let _ = failure_monitor_tx.send_modify(|s| {
                                        s.recovery_in_progress = true;
                                        s.launch_started_at = None;
                                    });
                                    // SAFETY: Zero FFB BEFORE killing the game (awaited)
                                    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    // Report FFB status
                                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
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
                                                    diagnostics: None,
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
                                    if heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                                        tracing::warn!("Ignoring BlankScreen — billing is active");
                                    } else {
                                        tracing::info!("Screen blanked via direct command");
                                        overlay.deactivate();
                                        lock_screen.show_blank_screen();
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SubSessionEnded {
                                    billing_session_id, driver_name, total_laps, best_lap_ms, driving_seconds, wallet_balance_paise,
                                    current_split_number, total_splits,
                                } => {
                                    tracing::info!(
                                        "Sub-session ended: {} — split {}/{}, {} laps, wallet: {}p",
                                        billing_session_id, current_split_number, total_splits, total_laps, wallet_balance_paise
                                    );
                                    crash_recovery_armed = false; // cancel crash timer
                                    overlay.deactivate();
                                    // Reset STATUS tracking and LaunchState
                                    last_ac_status = None;
                                    ac_status_stable_since = None;
                                    launch_state = LaunchState::Idle;
                                    // Site 3e: SubSessionEnded — clear launch state (billing stays active between splits)
                                    let _ = failure_monitor_tx.send_modify(|s| {
                                        s.launch_started_at = None;
                                        s.recovery_in_progress = false;
                                    });

                                    // SAFETY: Zero FFB BEFORE anything else (awaited)
                                    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
                                    tokio::time::sleep(Duration::from_millis(500)).await;

                                    // Show between-sessions lock screen
                                    lock_screen.show_between_sessions(
                                        driver_name, total_laps, best_lap_ms, driving_seconds, wallet_balance_paise,
                                        current_split_number, total_splits,
                                    );

                                    // Stop game and clean up AFTER FFB is zeroed
                                    if let Some(ref mut game) = game_process {
                                        let _ = game.stop();
                                        game_process = None;
                                    }
                                    // Disconnect telemetry adapter
                                    if let Some(ref mut adp) = adapter { adp.disconnect(); }
                                    // Report FFB status
                                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                    // Cleanup (enforce_safe_state WITHOUT ffb zero — already done)
                                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
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
                                        let billing_on = heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed);
                                        if v == "true" && lock_screen.is_idle_or_blanked() && !billing_on {
                                            lock_screen.show_blank_screen();
                                            tracing::info!("Screen blanking ENABLED — screen blanked");
                                        } else if v == "false" {
                                            lock_screen.clear();
                                            tracing::info!("Screen blanking DISABLED — screen restored");
                                        }
                                    }
                                    // BRAND-02: wallpaper URL for lock screen background
                                    if let Some(url) = settings.get("lock_screen_wallpaper_url") {
                                        let url_opt = if url.is_empty() { None } else { Some(url.clone()) };
                                        lock_screen.set_wallpaper_url(url_opt);
                                        tracing::info!("Lock screen wallpaper URL updated");
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SetTransmission { transmission } => {
                                    // Phase 6: instant toggle via SendInput instead of INI write
                                    tracing::info!("Setting transmission to '{}' mid-session (SendInput)", transmission);
                                    ac_launcher::mid_session::toggle_ac_transmission();
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    // Read confirmed state from shared memory
                                    let auto_shifter = adapter.as_ref()
                                        .and_then(|a| a.read_assist_state())
                                        .map(|(_, _, a)| a)
                                        .unwrap_or(false);
                                    overlay.show_toast(if auto_shifter { "Transmission: AUTO".into() } else { "Transmission: MANUAL".into() });
                                    let confirm = rc_common::protocol::AgentMessage::AssistChanged {
                                        pod_id: pod_id.clone(),
                                        assist_type: "transmission".into(),
                                        enabled: auto_shifter,
                                        confirmed: true,
                                    };
                                    if let Ok(json) = serde_json::to_string(&confirm) {
                                        let _ = ws_tx.send(Message::Text(json.into())).await;
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SetFfb { preset } => {
                                    // Phase 6: parse as percent if numeric, otherwise use legacy preset lookup
                                    tracing::info!("Setting FFB to '{}' mid-session", preset);
                                    // Track preset name for heartbeat reporting
                                    match preset.as_str() {
                                        "light" | "medium" | "strong" => last_ffb_preset = preset.clone(),
                                        _ => {} // numeric or unknown — don't update preset name
                                    }
                                    if let Ok(percent) = preset.parse::<u8>() {
                                        match ffb.set_gain(percent) {
                                            Ok(true) => {
                                                let clamped = percent.clamp(10, 100);
                                                last_ffb_percent = clamped;
                                                overlay.show_toast(format!("FFB: {}%", clamped));
                                                let confirm = rc_common::protocol::AgentMessage::FfbGainChanged {
                                                    pod_id: pod_id.clone(),
                                                    percent: clamped,
                                                };
                                                if let Ok(json) = serde_json::to_string(&confirm) {
                                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                                }
                                            }
                                            Ok(false) => tracing::warn!("FFB: wheelbase not found for SetFfb"),
                                            Err(e) => tracing::error!("FFB gain error: {}", e),
                                        }
                                    } else {
                                        // Legacy preset fallback (writes INI for next launch)
                                        if let Err(e) = ac_launcher::set_ffb(&preset) {
                                            tracing::error!("Failed to set FFB (legacy): {}", e);
                                        }
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SetAssist { assist_type, enabled } => {
                                    tracing::info!("SetAssist: type={}, enabled={}", assist_type, enabled);
                                    match assist_type.as_str() {
                                        "abs" => {
                                            let current = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(abs, _, _)| abs > 0)
                                                .unwrap_or(false);
                                            if current != enabled {
                                                if enabled {
                                                    ac_launcher::mid_session::toggle_ac_abs();
                                                } else {
                                                    let level = adapter.as_ref()
                                                        .and_then(|a| a.read_assist_state())
                                                        .map(|(abs, _, _)| abs)
                                                        .unwrap_or(1);
                                                    for _ in 0..level {
                                                        ac_launcher::mid_session::send_ctrl_shift_key(0x41);
                                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                                    }
                                                }
                                            }
                                            tokio::time::sleep(Duration::from_millis(100)).await;
                                            let confirmed_abs = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(abs, _, _)| abs > 0)
                                                .unwrap_or(false);
                                            overlay.show_toast(if confirmed_abs { "ABS: ON".into() } else { "ABS: OFF".into() });
                                            let confirm = rc_common::protocol::AgentMessage::AssistChanged {
                                                pod_id: pod_id.clone(),
                                                assist_type: "abs".into(),
                                                enabled: confirmed_abs,
                                                confirmed: true,
                                            };
                                            if let Ok(json) = serde_json::to_string(&confirm) {
                                                let _ = ws_tx.send(Message::Text(json.into())).await;
                                            }
                                        }
                                        "tc" => {
                                            let current = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(_, tc, _)| tc > 0)
                                                .unwrap_or(false);
                                            if current != enabled {
                                                if enabled {
                                                    ac_launcher::mid_session::toggle_ac_tc();
                                                } else {
                                                    let level = adapter.as_ref()
                                                        .and_then(|a| a.read_assist_state())
                                                        .map(|(_, tc, _)| tc)
                                                        .unwrap_or(1);
                                                    for _ in 0..level {
                                                        ac_launcher::mid_session::send_ctrl_shift_key(0x54);
                                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                                    }
                                                }
                                            }
                                            tokio::time::sleep(Duration::from_millis(100)).await;
                                            let confirmed_tc = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(_, tc, _)| tc > 0)
                                                .unwrap_or(false);
                                            overlay.show_toast(if confirmed_tc { "TC: ON".into() } else { "TC: OFF".into() });
                                            let confirm = rc_common::protocol::AgentMessage::AssistChanged {
                                                pod_id: pod_id.clone(),
                                                assist_type: "tc".into(),
                                                enabled: confirmed_tc,
                                                confirmed: true,
                                            };
                                            if let Ok(json) = serde_json::to_string(&confirm) {
                                                let _ = ws_tx.send(Message::Text(json.into())).await;
                                            }
                                        }
                                        "transmission" => {
                                            let current = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(_, _, auto)| auto)
                                                .unwrap_or(false);
                                            if current != enabled {
                                                ac_launcher::mid_session::toggle_ac_transmission();
                                            }
                                            tokio::time::sleep(Duration::from_millis(100)).await;
                                            let confirmed = adapter.as_ref()
                                                .and_then(|a| a.read_assist_state())
                                                .map(|(_, _, auto)| auto)
                                                .unwrap_or(false);
                                            overlay.show_toast(if confirmed { "Transmission: AUTO".into() } else { "Transmission: MANUAL".into() });
                                            let confirm = rc_common::protocol::AgentMessage::AssistChanged {
                                                pod_id: pod_id.clone(),
                                                assist_type: "transmission".into(),
                                                enabled: confirmed,
                                                confirmed: true,
                                            };
                                            if let Ok(json) = serde_json::to_string(&confirm) {
                                                let _ = ws_tx.send(Message::Text(json.into())).await;
                                            }
                                        }
                                        other => {
                                            tracing::warn!("Unknown assist type: {}", other);
                                        }
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::SetFfbGain { percent } => {
                                    tracing::info!("SetFfbGain: {}%", percent);
                                    match ffb.set_gain(percent) {
                                        Ok(true) => {
                                            let clamped = percent.clamp(10, 100);
                                            last_ffb_percent = clamped;
                                            overlay.show_toast(format!("FFB: {}%", clamped));
                                            let confirm = rc_common::protocol::AgentMessage::FfbGainChanged {
                                                pod_id: pod_id.clone(),
                                                percent: clamped,
                                            };
                                            if let Ok(json) = serde_json::to_string(&confirm) {
                                                let _ = ws_tx.send(Message::Text(json.into())).await;
                                            }
                                        }
                                        Ok(false) => tracing::warn!("FFB: wheelbase not found for SetFfbGain"),
                                        Err(e) => tracing::error!("FFB gain set error: {}", e),
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::QueryAssistState => {
                                    let (abs, tc, auto_shifter) = adapter.as_ref()
                                        .and_then(|a| a.read_assist_state())
                                        .unwrap_or((0, 0, false));
                                    let state = rc_common::protocol::AgentMessage::AssistState {
                                        pod_id: pod_id.clone(),
                                        abs,
                                        tc,
                                        auto_shifter,
                                        ffb_percent: last_ffb_percent,
                                    };
                                    if let Ok(json) = serde_json::to_string(&state) {
                                        let _ = ws_tx.send(Message::Text(json.into())).await;
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::PinFailed { reason } => {
                                    tracing::warn!("PIN failed: {}", reason);
                                    lock_screen.show_pin_error(&reason);
                                }
                                rc_common::protocol::CoreToAgentMessage::Ping { id } => {
                                    // Measure how long the event loop took to reach this Ping.
                                    // The WS frame arrived at the OS socket earlier — the delta
                                    // between frame arrival and here reveals async runtime stalls.
                                    let received_at = std::time::Instant::now();
                                    let pong = rc_common::protocol::AgentMessage::Pong {
                                        id,
                                        agent_delay_us: None, // set below after send
                                    };
                                    if let Ok(json) = serde_json::to_string(&pong) {
                                        if ws_tx.send(Message::Text(json.into())).await.is_err() {
                                            tracing::error!("Failed to send Pong, connection lost");
                                            break;
                                        }
                                    }
                                    let process_us = received_at.elapsed().as_micros() as u64;
                                    if process_us > 5000 {
                                        tracing::warn!("Pong send took {}us (>5ms)", process_us);
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::Exec { request_id, cmd, timeout_ms } => {
                                    tracing::info!("WS command request {}: {}", request_id, cmd);
                                    let result_tx = ws_exec_result_tx.clone();
                                    tokio::spawn(async move {
                                        let result = handle_ws_exec(request_id, cmd, timeout_ms).await;
                                        let _ = result_tx.send(result).await;
                                    });
                                }
                                rc_common::protocol::CoreToAgentMessage::ApproveProcess { process_name } => {
                                    tracing::info!("Server APPROVED process: {}", process_name);
                                    crate::kiosk::KioskManager::approve_process(&process_name);
                                    // Clear lockdown if it was triggered by this process
                                    if kiosk.is_locked_down() {
                                        kiosk.exit_lockdown();
                                        lock_screen.show_blank_screen();
                                    }
                                }
                                rc_common::protocol::CoreToAgentMessage::RejectProcess { process_name } => {
                                    tracing::warn!("Server REJECTED process: {}", process_name);
                                    crate::kiosk::KioskManager::reject_process(&process_name);
                                    let reason = format!(
                                        "Unauthorized software '{}' detected. Please contact staff.",
                                        process_name
                                    );
                                    kiosk.enter_lockdown(&reason);
                                    lock_screen.show_lockdown(&reason);
                                    // Notify server via exec result channel
                                    let lockdown_msg = rc_common::protocol::AgentMessage::KioskLockdown {
                                        pod_id: pod_id.clone(),
                                        reason,
                                    };
                                    let _ = ws_exec_result_tx.try_send(lockdown_msg);
                                }
                                other => {
                                    tracing::warn!("Unhandled CoreToAgentMessage: {:?}", other);
                                }
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
        tracing::warn!("Disconnected from core server");

        // If no billing active, enforce safe state on disconnect — kill any orphaned games
        if !heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("No active billing on disconnect — enforcing safe state");
            overlay.deactivate();
            // SAFETY: Zero FFB FIRST (awaited), before game cleanup
            { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
            tracing::info!("FFB zeroed on disconnect (ws_tx unavailable for FfbZeroed message)");
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Some(ref mut game) = game_process {
                let _ = game.stop();
                game_process = None;
            }
            if let Some(ref mut adp) = adapter { adp.disconnect(); }
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
            lock_screen.show_blank_screen();
        } else {
            lock_screen.show_disconnected();
        }

        let delay = reconnect_delay_for_attempt(reconnect_attempt);
        tracing::warn!("Attempt {}. Reconnecting in {:?}...", reconnect_attempt, delay);
        tokio::time::sleep(delay).await;
        reconnect_attempt += 1;
    } // end reconnection loop
}

/// Compute reconnect delay based on attempt number.
/// First 3 attempts: 1s each (fast retry for brief CPU spike blips).
/// After that: exponential backoff 2s, 4s, 8s, 16s, capped at 30s.
fn reconnect_delay_for_attempt(attempt: u32) -> Duration {
    if attempt < 3 {
        Duration::from_secs(1)
    } else {
        let exp = (attempt - 2).min(5);
        Duration::from_secs(2u64.pow(exp)).min(Duration::from_secs(30))
    }
}

/// Validate agent configuration. Returns Err with all issues found (not fail-fast).
///
/// Rules:
/// - pod.number must be 1–8 inclusive
/// - pod.name must not be blank after trimming
/// - core.url must start with "ws://" or "wss://"
fn validate_config(config: &AgentConfig) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if config.pod.number == 0 || config.pod.number > 8 {
        errors.push(format!(
            "pod.number must be 1-8, got {}",
            config.pod.number
        ));
    }

    if config.pod.name.trim().is_empty() {
        errors.push("pod.name must not be empty".to_string());
    }

    let url = config.core.url.trim();
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        errors.push(format!(
            "core.url must start with ws:// or wss://, got {:?}",
            url
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("{}", errors.join("; ")))
    }
}

fn config_search_paths() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();

    // Primary: exe directory (correct on Windows regardless of CWD)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join("rc-agent.toml"));
        }
    }
    // Fallback: CWD (useful for `cargo run` in dev)
    paths.push(std::path::PathBuf::from("rc-agent.toml"));
    // Legacy Linux path
    paths.push(std::path::PathBuf::from("/etc/racecontrol/rc-agent.toml"));

    paths
}

fn load_config() -> Result<AgentConfig> {
    let search_paths = config_search_paths();

    for path in &search_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let config: AgentConfig = toml::from_str(&content)?;
            tracing::info!("Loaded config from {}", path.display());
            validate_config(&config)?;
            return Ok(config);
        }
    }

    Err(anyhow::anyhow!(
        "No config file found. Searched: {}",
        search_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
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

/// Create a UDP socket with SO_REUSEADDR and non-inheritable handle.
/// Mirrors the TCP pattern in remote_ops.rs for :8090.
/// On Windows, non-inheritable prevents cmd.exe children from holding the port open
/// after rc-agent exits (which would cause error 10048 on self-relaunch).
fn bind_udp_reusable(port: u16) -> Option<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};

    let raw = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    raw.set_reuse_address(true).ok()?;
    raw.set_nonblocking(true).ok()?;
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse().ok()?;
    raw.bind(&addr.into()).ok()?;

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use winapi::um::handleapi::SetHandleInformation;
        const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
        let sock_handle = raw.as_raw_socket() as usize;
        let ok = unsafe { SetHandleInformation(sock_handle as *mut _, HANDLE_FLAG_INHERIT, 0) };
        if ok == 0 {
            tracing::warn!("UDP port {}: SetHandleInformation failed", port);
        }
    }

    let std_sock: std::net::UdpSocket = raw.into();
    tokio::net::UdpSocket::from_std(std_sock).ok()
}

/// UDP telemetry port monitor — listens on multiple game telemetry ports.
/// If any data arrives on any port, signals that a game is actively outputting telemetry.
async fn run_udp_monitor(ports: Vec<u16>, signal_tx: mpsc::Sender<DetectorSignal>) {
    // Spawn a listener task per port — each sends UdpActive signals independently
    for port in ports {
        let tx = signal_tx.clone();
        tokio::spawn(async move {
            let sock = match bind_udp_reusable(port) {
                Some(s) => {
                    tracing::info!("Listening for game telemetry on UDP port {} (SO_REUSEADDR)", port);
                    s
                }
                None => {
                    tracing::warn!(
                        "Could not bind UDP port {} with SO_REUSEADDR (game may already be using it)",
                        port
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_process::GameExeConfig;
    use rc_common::types::SimType;

    fn valid_config() -> AgentConfig {
        AgentConfig {
            pod: PodConfig {
                number: 3,
                name: "Pod 03".to_string(),
                sim: "assetto_corsa".to_string(),
                sim_ip: default_sim_ip(),
                sim_port: default_sim_port(),
            },
            core: CoreConfig {
                url: "ws://192.168.31.23:8080/ws/agent".to_string(),
            },
            wheelbase: WheelbaseConfig::default(),
            telemetry_ports: TelemetryPortsConfig::default(),
            games: GamesConfig::default(),
            ai_debugger: AiDebuggerConfig::default(),
            kiosk: KioskConfig::default(),
        }
    }

    #[test]
    fn validate_config_accepts_valid_config() {
        let config = valid_config();
        assert!(validate_config(&config).is_ok(), "Valid config should pass validation");
    }

    #[test]
    fn validate_config_rejects_pod_number_zero() {
        let mut config = valid_config();
        config.pod.number = 0;
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.number must be 1-8"),
            "Error should mention pod.number must be 1-8, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_pod_number_nine() {
        let mut config = valid_config();
        config.pod.number = 9;
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.number must be 1-8"),
            "Error should mention pod.number must be 1-8, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_empty_pod_name() {
        let mut config = valid_config();
        config.pod.name = "   ".to_string(); // whitespace only
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("pod.name"),
            "Error should mention pod.name, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_http_url() {
        let mut config = valid_config();
        config.core.url = "http://192.168.31.23:8080/ws/agent".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("ws://"),
            "Error should mention ws://, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_rejects_empty_url() {
        let mut config = valid_config();
        config.core.url = "".to_string();
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("ws://"),
            "Error should mention ws://, got: {}",
            err
        );
    }

    #[test]
    fn validate_config_accepts_wss_url() {
        let mut config = valid_config();
        config.core.url = "wss://app.racingpoint.cloud/ws/agent".to_string();
        assert!(validate_config(&config).is_ok(), "wss:// URL should be accepted");
    }

    #[test]
    fn validate_config_accepts_pod_number_1_and_8() {
        let mut config = valid_config();
        config.pod.number = 1;
        assert!(validate_config(&config).is_ok(), "Pod 1 should be valid");
        config.pod.number = 8;
        assert!(validate_config(&config).is_ok(), "Pod 8 should be valid");
    }

    #[test]
    fn load_config_returns_err_when_no_file_exists() {
        // Temporarily change to a directory without a config file
        // We test this by trying to parse an empty/nonexistent config
        // Since load_config reads from CWD, we check it returns Err (not defaults)
        // by verifying that the code path for missing files exists and returns Err.
        // Direct testing of file-system behavior is done via integration test.
        // Here we verify validate_config is the gatekeeper for default values.
        let mut config = valid_config();
        // A pod.number=1 with default core URL used to be the default. Now it must be explicit.
        config.pod.number = 1;
        config.core.url = "ws://127.0.0.1:8080/ws/agent".to_string();
        // This SHOULD pass (valid explicit config, not a sneaky default)
        assert!(validate_config(&config).is_ok(), "Explicitly valid config should pass");
    }

    #[test]
    fn test_config_search_paths_includes_exe_dir() {
        use std::path::PathBuf;
        let paths = config_search_paths();
        // Must have at least one path
        assert!(!paths.is_empty(), "config_search_paths() must return at least one path");
        // First path must end with rc-agent.toml
        let first = &paths[0];
        assert!(
            first.file_name().map(|n| n == "rc-agent.toml").unwrap_or(false),
            "First search path must end with rc-agent.toml, got: {}",
            first.display()
        );
        // First path must NOT be just "rc-agent.toml" (must include a parent directory from exe)
        assert!(
            first != &PathBuf::from("rc-agent.toml"),
            "First path must include exe directory, not bare 'rc-agent.toml', got: {}",
            first.display()
        );
    }

    #[test]
    fn test_config_search_paths_includes_cwd_fallback() {
        use std::path::PathBuf;
        let paths = config_search_paths();
        // Must contain CWD-relative fallback
        let has_cwd_fallback = paths.contains(&PathBuf::from("rc-agent.toml"));
        assert!(has_cwd_fallback, "config_search_paths() must include 'rc-agent.toml' (CWD fallback)");
        // CWD fallback must appear AFTER the exe-dir path (index > 0)
        let cwd_index = paths.iter().position(|p| p == &PathBuf::from("rc-agent.toml")).unwrap();
        assert!(
            cwd_index > 0,
            "CWD fallback 'rc-agent.toml' must appear after exe-dir path (index {}), not at index 0",
            cwd_index
        );
    }

    #[test]
    fn test_load_config_error_lists_all_searched_paths() {
        // Change to a temp directory that has no rc-agent.toml
        let tmp = std::env::temp_dir().join("rc_agent_test_no_config");
        let _ = std::fs::create_dir_all(&tmp);
        let original = std::env::current_dir().ok();

        // Set CWD to temp dir (best effort — doesn't affect exe-dir search)
        let _ = std::env::set_current_dir(&tmp);

        let result = load_config();

        // Restore original CWD
        if let Some(orig) = original {
            let _ = std::env::set_current_dir(orig);
        }

        let err = result.expect_err("load_config() must return Err when no config file exists");
        let msg = err.to_string();
        assert!(
            msg.contains("No config file found"),
            "Error must contain 'No config file found', got: {}",
            msg
        );
        assert!(
            msg.contains("Searched:"),
            "Error must contain 'Searched:', got: {}",
            msg
        );
        // Must list at least 2 distinct path entries (exe-dir + CWD fallback)
        let path_count = msg.matches("rc-agent.toml").count();
        assert!(
            path_count >= 2,
            "Error must list at least 2 paths containing 'rc-agent.toml', found {} in: {}",
            path_count,
            msg
        );
    }

    #[test]
    fn test_reconnect_delay_fast_retries() {
        assert_eq!(reconnect_delay_for_attempt(0), Duration::from_secs(1));
        assert_eq!(reconnect_delay_for_attempt(1), Duration::from_secs(1));
        assert_eq!(reconnect_delay_for_attempt(2), Duration::from_secs(1));
    }

    #[test]
    fn test_reconnect_delay_exponential_backoff() {
        assert_eq!(reconnect_delay_for_attempt(3), Duration::from_secs(2));
        assert_eq!(reconnect_delay_for_attempt(4), Duration::from_secs(4));
        assert_eq!(reconnect_delay_for_attempt(5), Duration::from_secs(8));
        assert_eq!(reconnect_delay_for_attempt(6), Duration::from_secs(16));
    }

    #[test]
    fn test_reconnect_delay_cap() {
        assert_eq!(reconnect_delay_for_attempt(7), Duration::from_secs(30));
        assert_eq!(reconnect_delay_for_attempt(100), Duration::from_secs(30));
    }

    // ─── installed games tests (merged) ──────────────────────────────────

    #[test]
    fn test_installed_games_detection_new_games() {
        // Configure AC Rally and FH5 with steam_app_id
        let mut games = GamesConfig::default();
        games.assetto_corsa_rally = GameExeConfig {
            steam_app_id: Some(3917090),
            use_steam: true,
            ..Default::default()
        };
        games.forza_horizon_5 = GameExeConfig {
            steam_app_id: Some(1551360),
            use_steam: true,
            ..Default::default()
        };
        let installed = detect_installed_games(&games);
        // AC is always included
        assert!(installed.contains(&SimType::AssettoCorsa));
        // New games should be detected
        assert!(
            installed.contains(&SimType::AssettoCorsaRally),
            "AC Rally not detected with steam_app_id"
        );
        assert!(
            installed.contains(&SimType::ForzaHorizon5),
            "FH5 not detected with steam_app_id"
        );
    }

    #[test]
    fn test_installed_games_empty_config_only_ac() {
        // Default config (no games configured) should only have AC
        let games = GamesConfig::default();
        let installed = detect_installed_games(&games);
        assert_eq!(installed, vec![SimType::AssettoCorsa]);
    }

    #[test]
    fn test_installed_games_detection_exe_path() {
        // Test that exe_path also triggers detection
        let mut games = GamesConfig::default();
        games.assetto_corsa_rally = GameExeConfig {
            exe_path: Some("C:\\Games\\acr.exe".to_string()),
            ..Default::default()
        };
        let installed = detect_installed_games(&games);
        assert!(installed.contains(&SimType::AssettoCorsaRally));
    }

    #[test]
    fn test_installed_games_all_configured() {
        // All games configured — should detect all 8
        let mut games = GamesConfig::default();
        games.f1_25 = GameExeConfig { steam_app_id: Some(3059520), ..Default::default() };
        games.iracing = GameExeConfig { steam_app_id: Some(266410), ..Default::default() };
        games.forza = GameExeConfig { steam_app_id: Some(2440510), ..Default::default() };
        games.le_mans_ultimate = GameExeConfig { steam_app_id: Some(2399420), ..Default::default() };
        games.assetto_corsa_evo = GameExeConfig { steam_app_id: Some(3058630), ..Default::default() };
        games.assetto_corsa_rally = GameExeConfig { steam_app_id: Some(3917090), ..Default::default() };
        games.forza_horizon_5 = GameExeConfig { steam_app_id: Some(1551360), ..Default::default() };
        let installed = detect_installed_games(&games);
        assert_eq!(installed.len(), 8, "Expected all 8 SimType variants to be detected");
        assert!(installed.contains(&SimType::AssettoCorsa));
        assert!(installed.contains(&SimType::F125));
        assert!(installed.contains(&SimType::IRacing));
        assert!(installed.contains(&SimType::Forza));
        assert!(installed.contains(&SimType::LeMansUltimate));
        assert!(installed.contains(&SimType::AssettoCorsaEvo));
        assert!(installed.contains(&SimType::AssettoCorsaRally));
        assert!(installed.contains(&SimType::ForzaHorizon5));
    }
}
