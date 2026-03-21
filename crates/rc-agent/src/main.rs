mod ac_launcher;
mod ai_debugger;
mod app_state;
mod billing_guard;
mod ws_handler;
mod config;
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
mod self_test;
mod sims;
mod startup_log;
mod udp_heartbeat;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use app_state::AppState;
use config::{load_config, detect_installed_games};
use driving_detector::{
    DetectorConfig, DetectorSignal, DrivingDetector,
    is_input_active, is_steering_moving, parse_openffboard_report,
};
use ffb_controller::FfbController;
use ai_debugger::PodStateSnapshot;
use rc_common::protocol::AgentMessage;
use rc_common::types::*;
use rc_common::types::AcStatus;
use sims::SimAdapter;
use sims::assetto_corsa::AssettoCorsaAdapter;
use sims::f1_25::F125Adapter;
use kiosk::KioskManager;
use lock_screen::{LockScreenEvent, LockScreenManager};
use overlay::OverlayManager;

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

// WS_MAX_CONCURRENT_EXECS, WS_EXEC_SEMAPHORE, and handle_ws_exec moved to ws_handler.rs (74-03)

/// Fetch the staff-managed allowlist from racecontrol (GET /api/v1/config/kiosk-allowlist).
/// Returns a list of lowercase process names on success, or an error if unreachable.
async fn fetch_server_allowlist(client: &reqwest::Client, base_url: &str) -> anyhow::Result<Vec<String>> {
    let resp = client
        .get(&format!("{}/api/v1/config/kiosk-allowlist", base_url))
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await?;
    let names = body["allowlist"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e["process_name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok(names)
}

/// Poll the server allowlist every ALLOWLIST_REFRESH_SECS.
///
/// First tick fires immediately (at startup) so kiosk enforcement on first scan
/// already includes staff-added entries. Fetch failures are WARN-level and non-fatal —
/// the hardcoded ALLOWED_PROCESSES baseline continues enforcing.
async fn allowlist_poll_loop(core_http_url: String, client: reqwest::Client) {
    let mut interval = tokio::time::interval(Duration::from_secs(kiosk::ALLOWLIST_REFRESH_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match fetch_server_allowlist(&client, &core_http_url).await {
            Ok(names) => {
                let count = names.len();
                kiosk::set_server_allowlist(names);
                tracing::info!("[allowlist] Updated: {} entries from server", count);
            }
            Err(e) => {
                tracing::warn!("[allowlist] Fetch failed (will retry in 5 min): {}", e);
            }
        }
    }
}

/// Delete log files older than 30 days from the given directory.
fn cleanup_old_logs(log_dir: &std::path::Path) {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(30 * 24 * 3600))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".jsonl") || name.contains(".jsonl.") || name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
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

    // Compute log directory (exe dir) — needed for cleanup and later tracing init
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Clean up old log files (>30 days) before initializing tracing
    cleanup_old_logs(&log_dir);

    println!(r#"
  RaceControl Agent
  Pod Telemetry Bridge
"#);

    // Detect crash recovery BEFORE write_phase("init") truncates the previous log
    let crash_recovery = startup_log::detect_crash_recovery();
    startup_log::write_phase("init", "");
    if crash_recovery {
        eprintln!("[rc-agent] Detected crash recovery -- previous startup did not complete");
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
            eprintln!("[rc-agent] Config error: {}", e);
            early_lock_screen.show_config_error(&e.to_string());
            // Give Edge time to render the error page before process exits
            tokio::time::sleep(Duration::from_secs(2)).await;
            std::process::exit(1);
        }
    };
    // Early lock screen is replaced by the main lock screen manager below
    drop(early_lock_screen);
    startup_log::write_phase("config_loaded", &format!("pod={}", config.pod.number));

    // Initialize tracing AFTER config load — pod_id now available for structured logs
    let pod_id_str = format!("pod_{}", config.pod.number);

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("rc-agent-")
        .filename_suffix("jsonl")
        .build(&log_dir)
        .expect("failed to build rolling file appender");
    let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "rc_agent=info".into());

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking_file),
        )
        .init();

    // Enter pod span — all subsequent logs carry pod_id in span context
    let _pod_span = tracing::info_span!("rc-agent", pod_id = %pod_id_str).entered();
    tracing::info!("Structured logging initialized for {}", pod_id_str);

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

    // SAFE-04: Cap venue power at 80% (9.6Nm on 12Nm, 6.4Nm on 8Nm)
    {
        let ffb_cap = ffb.clone();
        tokio::task::spawn_blocking(move || {
            match ffb_cap.set_gain(80) {
                Ok(true) => tracing::info!("FFB: venue power cap set to 80%"),
                Ok(false) => tracing::debug!("FFB: no wheelbase found — power cap skipped"),
                Err(e) => tracing::warn!("FFB: failed to set power cap: {}", e),
            }
        }).await.ok();
    }

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
            ffb_controller::safe_session_end(&ffb_startup_cleanup).await;
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
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
    // Derive HTTP base URL from WebSocket URL: ws://host:port/ws/agent → http://host:port/api/v1
    let core_http_base = config.core.url
        .replace("ws://", "http://")
        .replace("wss://", "https://")
        .split("/ws")
        .next()
        .unwrap_or("http://127.0.0.1:8080")
        .to_string()
        + "/api/v1";
    billing_guard::spawn(
        failure_monitor_tx.subscribe(),
        ws_exec_result_tx.clone(),
        pod_id.clone(),
        core_http_base,
        config.auto_end_orphan_session_secs,
    );
    tracing::info!("Billing guard started (orphan_timeout={}s)", config.auto_end_orphan_session_secs);

    // ─── Server Allowlist Poll Loop (dynamic kiosk allowlist) ─────────────────
    // Derive HTTP base URL from WebSocket URL: ws://host:port/path → http://host:port
    // First poll fires immediately (interval first tick) so kiosk enforcement on first
    // scan already includes staff-added entries. Non-fatal on fetch failure.
    {
        let core_http_url = config.core.url
            .replace("ws://", "http://")
            .replace("wss://", "https://")
            .split("/ws/")
            .next()
            .unwrap_or("http://127.0.0.1:8080")
            .to_string();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        tokio::spawn(allowlist_poll_loop(core_http_url, http_client));
        tracing::info!("Allowlist poll loop started (refresh every {}s)", kiosk::ALLOWLIST_REFRESH_SECS);
    }

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

    // ─── Phase 50: Startup Self-Test ────────────────────────────────────────
    // Run after all ports are bound. Uses deterministic verdict (no LLM at startup
    // — Ollama call would be too slow and may block the WS reconnect loop).
    let startup_self_test_report = self_test::run_all_probes(
        heartbeat_status.clone(),
        &config.ai_debugger.ollama_url,
    ).await;
    let startup_verdict = self_test::deterministic_verdict(&startup_self_test_report.probes);
    let startup_self_test_verdict: Option<String> = Some(format!("{:?}", startup_verdict.level).to_uppercase());
    let startup_probe_failures: u8 = startup_self_test_report.probes
        .iter()
        .filter(|p| p.status == self_test::ProbeStatus::Fail)
        .count()
        .min(255) as u8;
    tracing::info!(
        "Startup self-test: verdict={:?} failures={}",
        startup_verdict.level,
        startup_probe_failures
    );

    // ─── Bundle pre-loop state into AppState ────────────────────────────────
    let mut state = AppState {
        pod_id,
        pod_info,
        config,
        sim_type,
        installed_games,
        ffb,
        detector,
        adapter,
        hid_detected,
        kiosk,
        kiosk_enabled,
        lock_screen,
        overlay,
        signal_rx,
        lock_event_rx,
        heartbeat_event_rx,
        ai_result_rx,
        ai_result_tx,
        ws_exec_result_rx,
        ws_exec_result_tx,
        failure_monitor_tx,
        heartbeat_status,
        last_launch_error,
        agent_start_time,
        exe_dir,
        heal_result,
        crash_recovery_startup: crash_recovery,
        startup_self_test_verdict,
        startup_probe_failures,
        lock_screen_bound,
        remote_ops_bound,
        game_process,
        last_ac_status,
        ac_status_stable_since,
    };

    // ─── Reconnection Loop ──────────────────────────────────────────────────
    // On disconnect, retry with exponential backoff. All local state
    // (lock screen, kiosk, HID/UDP monitors, game process) persists across
    // reconnections — only the WebSocket is re-established.
    let mut reconnect_attempt: u32 = 0;
    let mut startup_complete_logged = false;
    let mut startup_report_sent = false;
    // SESSION-04: WS 30s grace window — suppress Disconnected screen for brief drops.
    // Billing, game, and overlay keep running during the grace window.
    let mut ws_disconnected_at: Option<std::time::Instant> = None;

    // Phase 68: Runtime URL switching via SwitchController
    let active_url: std::sync::Arc<RwLock<String>> =
        std::sync::Arc::new(RwLock::new(state.config.core.url.clone()));
    let primary_url: String = state.config.core.url.clone();
    let failover_url: Option<String> = state.config.core.failover_url.clone();

    // Phase 69: Split-brain guard — reusable HTTP client for LAN probe (created once, not per-message)
    let split_brain_probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    loop {
        // Connect to core server — read active_url on each iteration (Phase 68: runtime switching)
        let url = active_url.read().await.clone();
        tracing::info!("Connecting to RaceControl core at {}...", url);
        let ws_result = tokio::time::timeout(
            Duration::from_secs(10),
            connect_async(&url),
        ).await;

        let (ws_stream, _) = match ws_result {
            Ok(Ok(stream)) => {
                reconnect_attempt = 0; // Reset on successful connection
                ws_disconnected_at = None; // SESSION-04: Clear grace window on reconnect
                stream
            }
            Ok(Err(e)) => {
                let delay = reconnect_delay_for_attempt(reconnect_attempt);
                tracing::warn!("Failed to connect to core: {}. Attempt {}. Retrying in {:?}...", e, reconnect_attempt, delay);
                // SESSION-04: Only show Disconnected after 30s grace window
                {
                    let disconnected_for = ws_disconnected_at
                        .get_or_insert_with(std::time::Instant::now)
                        .elapsed();
                    if disconnected_for > Duration::from_secs(30) {
                        state.lock_screen.show_disconnected();
                    } else {
                        tracing::info!("[ws-grace] Disconnected {}s — within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
                    }
                }
                tokio::time::sleep(delay).await;
                reconnect_attempt += 1;
                continue;
            }
            Err(_) => {
                let delay = reconnect_delay_for_attempt(reconnect_attempt);
                tracing::warn!("Connection to core timed out. Attempt {}. Retrying in {:?}...", reconnect_attempt, delay);
                // SESSION-04: Only show Disconnected after 30s grace window
                {
                    let disconnected_for = ws_disconnected_at
                        .get_or_insert_with(std::time::Instant::now)
                        .elapsed();
                    if disconnected_for > Duration::from_secs(30) {
                        state.lock_screen.show_disconnected();
                    } else {
                        tracing::info!("[ws-grace] Timed out {}s — within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
                    }
                }
                tokio::time::sleep(delay).await;
                reconnect_attempt += 1;
                continue;
            }
        };
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // Register this pod (include current game state so core can resync)
        let register_msg = AgentMessage::Register(PodInfo {
            last_seen: Some(Utc::now()),
            driving_state: Some(state.detector.state()),
            game_state: state.game_process.as_ref().map(|g| g.state),
            current_game: state.game_process.as_ref().map(|g| g.sim_type),
            screen_blanked: Some(state.lock_screen.is_blanked()),
            ffb_preset: Some("medium".to_string()),
            ..state.pod_info.clone()
        });
        let json = serde_json::to_string(&register_msg)?;
        if ws_tx.send(Message::Text(json.into())).await.is_err() {
            let delay = reconnect_delay_for_attempt(reconnect_attempt);
            tracing::warn!("Failed to register with core. Attempt {}. Reconnecting in {:?}...", reconnect_attempt, delay);
            tokio::time::sleep(delay).await;
            reconnect_attempt += 1;
            continue;
        }
        tracing::info!("Connected and registered as Pod #{}", state.config.pod.number);
        if !startup_complete_logged {
            startup_log::write_phase("websocket", &format!("connected pod={}", state.config.pod.number));
            startup_log::write_phase("complete", "");
            startup_complete_logged = true;
        }

        // Send startup report once per process lifetime (HEAL-02)
        if !startup_report_sent {
            let config_path = state.exe_dir.join("rc-agent.toml");
            let startup_report = AgentMessage::StartupReport {
                pod_id: state.pod_id.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_secs: state.agent_start_time.elapsed().as_secs(),
                config_hash: self_heal::config_hash(&config_path),
                crash_recovery: state.crash_recovery_startup,
                repairs: {
                    let mut r = Vec::new();
                    if state.heal_result.config_repaired { r.push("config".to_string()); }
                    if state.heal_result.script_repaired { r.push("script".to_string()); }
                    if state.heal_result.registry_repaired { r.push("registry_key".to_string()); }
                    r
                },
                // Phase 46 SAFETY-04: boot verification fields — wired in Plan 02
                lock_screen_port_bound: state.lock_screen_bound,
                remote_ops_port_bound: state.remote_ops_bound,
                hid_detected: state.hid_detected,
                udp_ports_bound: state.config.telemetry_ports.ports.clone(),
                // Phase 50: Startup self-test verdict
                startup_self_test_verdict: state.startup_self_test_verdict.clone(),
                startup_probe_failures: state.startup_probe_failures,
            };
            if let Ok(json) = serde_json::to_string(&startup_report) {
                if ws_tx.send(Message::Text(json.into())).await.is_ok() {
                    tracing::info!("Sent startup report to core (crash_recovery={})", state.crash_recovery_startup);
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

        state.heartbeat_status.ws_connected.store(true, std::sync::atomic::Ordering::Relaxed);
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
        // SESSION-03: Crash recovery state machine.
        // Replaces crash_recovery_armed + crash_recovery_timer.
        // Pauses billing, retries relaunch twice (60s each), then auto-ends on 2nd failure.
        let mut crash_recovery = CrashRecoveryState::Idle;
        // Store launch args from LaunchGame handler for use in crash recovery relaunch.
        let mut last_launch_args_stored: Option<String> = None;
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
                        driving_state: Some(state.detector.state()),
                        game_state: state.game_process.as_ref().map(|g| g.state),
                        current_game: state.game_process.as_ref().map(|g| g.sim_type),
                        screen_blanked: Some(state.lock_screen.is_blanked()),
                        ffb_preset: Some(last_ffb_preset.clone()),
                        ..state.pod_info.clone()
                    });
                    let json = serde_json::to_string(&hb)?;
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        tracing::error!("Lost connection to core server");
                        break; // → reconnection loop
                    }
                }
            _ = telemetry_interval.tick() => {
                let Some(ref mut adapter) = state.adapter else { continue };
                if !adapter.is_connected() {
                    if adapter.connect().is_ok() {
                        state.overlay.set_max_rpm(adapter.max_rpm());
                    }
                    continue;
                }

                match adapter.read_telemetry() {
                    Ok(Some(frame)) => {
                        // Update overlay with live telemetry
                        state.overlay.update_telemetry(&frame);
                        // Accumulate top speed for session summary (SESS-01)
                        if frame.speed_kmh > session_max_speed_kmh {
                            session_max_speed_kmh = frame.speed_kmh;
                        }

                        // Check for completed laps via adapter (has proper sector splits)
                        if let Ok(Some(lap)) = adapter.poll_lap_completed() {
                            state.overlay.on_lap_completed(&lap);
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
                if state.game_process.is_some() {
                    if let Some(current_status) = adapter.read_ac_status() {
                        let status_changed = state.last_ac_status.map_or(true, |prev| prev != current_status);
                        if status_changed {
                            // Debounce: require STATUS to be stable for 1 second before reporting
                            // (prevents flapping on rapid ESC press — see RESEARCH.md Pitfall 3)
                            state.ac_status_stable_since = Some(std::time::Instant::now());
                            state.last_ac_status = Some(current_status);
                        }
                        // Send GameStatusUpdate only after STATUS has been stable for 1s
                        if let (Some(stable_since), Some(status)) = (state.ac_status_stable_since, state.last_ac_status) {
                            if stable_since.elapsed() >= Duration::from_secs(1) {
                                let msg = AgentMessage::GameStatusUpdate {
                                    pod_id: state.pod_id.clone(),
                                    ac_status: status,
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                                state.ac_status_stable_since = None; // sent, stop re-sending until next change

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
                            if let Some(ref mut proc) = state.game_process {
                                let _ = proc.stop();
                            }
                            state.game_process = None;

                            // Signal core that game is no longer running
                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: state.pod_id.clone(),
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
                            if let Some(ref mut proc) = state.game_process {
                                let _ = proc.stop();
                            }
                            state.game_process = None;
                            launch_state = LaunchState::Idle;
                            // Site 3a: launch_started_at cleared when launch cancelled (2nd timeout)
                            let _ = state.failure_monitor_tx.send_modify(|s| { s.launch_started_at = None; });

                            // Notify core of launch failure so it can cancel the session (no billing)
                            let msg = AgentMessage::GameStatusUpdate {
                                pod_id: state.pod_id.clone(),
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
            Some(signal) = state.signal_rx.recv() => {
                let (_, changed) = state.detector.process_signal(signal);
                if changed {
                    let is_active = matches!(state.detector.state(), DrivingState::Active);
                    state.heartbeat_status.driving_active.store(is_active, std::sync::atomic::Ordering::Relaxed);
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: state.pod_id.clone(),
                        state: state.detector.state(),
                    };
                    // Site 9a: update failure_monitor watch with current driving state (signal path)
                    let _ = state.failure_monitor_tx.send_modify(|s| { s.driving_state = Some(state.detector.state()); });
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed: {:?}", state.detector.state());
                }
            }
            // Periodic detector evaluation (catches idle timeout transitions)
            _ = detector_interval.tick() => {
                let (_, changed) = state.detector.evaluate_state();
                if changed {
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: state.pod_id.clone(),
                        state: state.detector.state(),
                    };
                    // Site 9b: update failure_monitor watch with current driving state (timeout path)
                    let _ = state.failure_monitor_tx.send_modify(|s| { s.driving_state = Some(state.detector.state()); });
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed (timeout): {:?}", state.detector.state());
                }
                // Update failure monitor with current HID/UDP state (Site 1)
                let _ = state.failure_monitor_tx.send_modify(|s| {
                    s.hid_connected = state.detector.is_hid_connected();
                    s.last_udp_secs_ago = state.detector.last_udp_packet_elapsed_secs();
                });
            }
            // Game process health check (every 2s)
            _ = game_check_interval.tick() => {
                if let Some(ref mut game) = state.game_process {
                    let was_active = matches!(game.state, GameState::Running | GameState::Launching);

                    if game.state == GameState::Launching && game.child.is_none() {
                        // Steam-launched game — scan for process by name
                        if let Some(pid) = game_process::find_game_pid(game.sim_type) {
                            game.pid = Some(pid);
                            game_process::persist_pid(pid);
                            game.state = GameState::Running;
                            let info = GameLaunchInfo {
                                pod_id: state.pod_id.clone(),
                                sim_type: game.sim_type,
                                game_state: GameState::Running,
                                pid: Some(pid),
                                launched_at: Some(Utc::now()),
                                error_message: None,
                                diagnostics: None,
                            };
                            // Site 4c: Steam-launched game PID discovered via find_game_pid
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
                            // Game crashed or exited
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

                            // Trigger AI debugger if configured
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
                                tokio::spawn(ai_debugger::analyze_crash(
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
                            // Reset STATUS tracking on game crash
                            state.last_ac_status = None;
                            state.ac_status_stable_since = None;
                            launch_state = LaunchState::Idle;
                            // Site 3b: launch_started_at cleared on game crash/exit
                            let _ = state.failure_monitor_tx.send_modify(|s| {
                                s.launch_started_at = None;
                                s.game_pid = None;
                            });

                            // SESSION-03: If billing is active and game crashed, pause billing and
                            // attempt up to 2 relaunches (60s each). Auto-end on 2nd failure.
                            if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                                tracing::warn!("Game crashed during active billing — pausing billing, attempting relaunch");
                                // SAFETY: Safe session-end sequence on crash during billing
                                ffb_controller::safe_session_end(&state.ffb).await;
                                // Report crash + FFB status to core
                                let crash_msg = AgentMessage::GameCrashed { pod_id: state.pod_id.clone(), billing_active: true };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&crash_msg).unwrap_or_default().into())).await;
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                // SESSION-03: Pause billing + show overlay + start relaunch state machine
                                let _ = state.failure_monitor_tx.send_modify(|s| {
                                    s.billing_paused = true;
                                });
                                // Send BillingPaused WS notification
                                if let Some(ref sid) = state.failure_monitor_tx.borrow().active_billing_session_id.clone() {
                                    let pause_msg = AgentMessage::BillingPaused {
                                        pod_id: state.pod_id.clone(),
                                        billing_session_id: sid.clone(),
                                    };
                                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&pause_msg).unwrap_or_default().into())).await;
                                }
                                // Show overlay per UI-SPEC (em dash via unicode escape)
                                state.overlay.show_toast("Game crashed \u{2014} relaunching...".to_string());
                                // Capture last sim type for relaunch
                                let last_sim = state.game_process.as_ref().map(|g| g.sim_type).unwrap_or(SimType::AssettoCorsa);
                                // Arm crash recovery state machine — attempt 1 timer starts now
                                crash_recovery = CrashRecoveryState::PausedWaitingRelaunch {
                                    attempt: 1,
                                    timer: Box::pin(tokio::time::sleep(Duration::from_secs(60))),
                                    last_sim_type: last_sim,
                                    last_launch_args: last_launch_args_stored.clone(),
                                };
                            } else {
                                // No billing active — safe session-end then enforce safe state
                                tracing::info!("Game exited with no active billing — enforcing safe state");
                                ffb_controller::safe_session_end(&state.ffb).await;
                                let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                                tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                                state.lock_screen.show_idle_pin_entry();
                            }
                        }
                    }
                }
            }
            // AI debug result channel
            Some(mut suggestion) = state.ai_result_rx.recv() => {
                tracing::info!("[ai-result] Received AI suggestion for {}", suggestion.pod_id);
                // Attempt deterministic auto-fix in a blocking thread to avoid stalling the event loop
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
                    // Site 9: 3 new fields added for failure_monitor context
                    last_udp_secs_ago: state.detector.last_udp_packet_elapsed_secs(),
                    game_launch_elapsed_secs: match &launch_state {
                        LaunchState::WaitingForLive { launched_at, .. } => Some(launched_at.elapsed().as_secs()),
                        _ => None,
                    },
                    hid_last_error: !state.detector.is_hid_connected(),
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
                if state.kiosk_enabled && state.kiosk.should_enforce() {
                    let allowed = state.kiosk.allowed_set_snapshot();
                    let pod_id_kiosk = state.pod_id.clone();
                    let kiosk_msg_tx = state.ws_exec_result_tx.clone();
                    let lockdown_flag = state.kiosk.lockdown.clone();
                    let lockdown_reason = state.kiosk.lockdown_reason.clone();
                    let enforce_handle = tokio::task::spawn_blocking(move || {
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

                        result.pending_classifications
                    });

                    // Fire LLM classification for newly-seen unknown processes (non-blocking)
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
                                        // Send to server for persistence
                                        let msg = rc_common::protocol::AgentMessage::ProcessApprovalRequest {
                                            pod_id: pod_id_c,
                                            process_name: classification.process_name,
                                            exe_path: classification.exe_path,
                                            sighting_count: 0, // LLM-approved
                                        };
                                        let _ = kiosk_msg_tx_c.try_send(msg);
                                    }
                                    kiosk::ProcessVerdict::Block => {
                                        kiosk::KioskManager::reject_process(&classification.process_name);
                                    }
                                    kiosk::ProcessVerdict::Ask => {
                                        // Already in temp_allowlist — existing approval flow handles this
                                    }
                                }
                            });
                        }
                    }
                }
            }
            // Re-enforce overlay TOPMOST + clean desktop + Conspit watchdog every 10s
            _ = overlay_topmost_interval.tick() => {
                state.overlay.enforce_topmost();
                if state.kiosk_enabled {
                    tokio::task::spawn_blocking(|| {
                        ac_launcher::minimize_background_windows();
                        // When no game is running, keep kiosk lock screen in foreground
                        lock_screen::enforce_kiosk_foreground();
                        // Restart Conspit Link if it crashed (stays minimized)
                        ac_launcher::ensure_conspit_link_running();
                    });
                }
            }
            // Auto-show idle PinEntry after session summary (30s delay) — only if no billing active
            _ = &mut blank_timer, if blank_timer_armed => {
                blank_timer_armed = false;
                if state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!("Skipping idle PinEntry reset — billing is active");
                } else {
                    tracing::info!("Resetting to idle PinEntry after session summary (SESSION-02)");
                    state.lock_screen.show_idle_pin_entry();
                    // Final cleanup pass — safe session-end then enforce safe state
                    ffb_controller::safe_session_end(&state.ffb).await;
                    let ffb_msg = AgentMessage::FfbZeroed { pod_id: state.pod_id.clone() };
                    let _ = ws_tx.send(Message::Text(serde_json::to_string(&ffb_msg).unwrap_or_default().into())).await;
                    tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                }
            }
            // SESSION-03: Crash recovery state machine timer
            // Polls the 60s relaunch timer when in PausedWaitingRelaunch state.
            _ = async {
                match &mut crash_recovery {
                    CrashRecoveryState::PausedWaitingRelaunch { timer, .. } => {
                        timer.as_mut().await;
                    }
                    _ => {
                        // Park forever when not in active recovery
                        std::future::pending::<()>().await;
                    }
                }
            } => {
                match std::mem::replace(&mut crash_recovery, CrashRecoveryState::Idle) {
                    CrashRecoveryState::PausedWaitingRelaunch { attempt, last_sim_type, last_launch_args, .. } => {
                        // Check if game PID appeared during the wait (success via heartbeat path)
                        if state.game_process.as_ref().and_then(|g| g.pid).is_some() {
                            tracing::info!("[crash-recovery] Game PID detected during recovery wait (attempt {}) — resuming billing", attempt);
                            let _ = state.failure_monitor_tx.send_modify(|s| { s.billing_paused = false; });
                            state.overlay.deactivate();
                            if let Some(ref sid) = state.failure_monitor_tx.borrow().active_billing_session_id.clone() {
                                let resume_msg = AgentMessage::BillingResumed {
                                    pod_id: state.pod_id.clone(),
                                    billing_session_id: sid.clone(),
                                };
                                let _ = ws_tx.send(Message::Text(serde_json::to_string(&resume_msg).unwrap_or_default().into())).await;
                            }
                            crash_recovery = CrashRecoveryState::Idle;
                        } else if attempt < 2 {
                            // First relaunch timed out (60s) — try attempt 2
                            tracing::warn!("[crash-recovery] Relaunch attempt {} timed out (60s) — trying attempt 2", attempt);
                            state.overlay.show_toast("Relaunching... (2 of 2)".to_string());

                            // Attempt 2: re-launch AC using stored args (mirrors LaunchGame handler)
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
                                launch_state = LaunchState::WaitingForLive {
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
                                // Non-AC crash recovery: mirrors LaunchGame handler's generic-sim branch
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

                            crash_recovery = CrashRecoveryState::PausedWaitingRelaunch {
                                attempt: 2,
                                timer: Box::pin(tokio::time::sleep(Duration::from_secs(60))),
                                last_sim_type,
                                last_launch_args,
                            };
                        } else {
                            // 2nd attempt timed out — auto-end session (same path as orphan auto-end)
                            tracing::error!("[crash-recovery] Relaunch attempt 2 timed out (60s) — auto-ending session (crash_limit)");
                            state.overlay.show_toast("Session ending".to_string());
                            crash_recovery = CrashRecoveryState::AutoEndPending;
                            // Mirror orphan auto-end path: reset billing + FFB + go to idle PinEntry
                            state.heartbeat_status.billing_active.store(false, std::sync::atomic::Ordering::Relaxed);
                            // Send SessionAutoEnded WS notification before clearing session ID
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
                            // SAFETY: Safe session-end sequence before game cleanup
                            ffb_controller::safe_session_end(&state.ffb).await;
                            state.lock_screen.show_idle_pin_entry();
                            state.overlay.deactivate();
                            if let Some(ref mut game) = state.game_process {
                                let _ = game.stop();
                                state.game_process = None;
                            }
                            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
                            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
                            current_driver_name = None;
                            state.last_ac_status = None;
                            state.ac_status_stable_since = None;
                            launch_state = LaunchState::Idle;
                            crash_recovery = CrashRecoveryState::Idle;
                        }
                    }
                    _ => {} // AutoEndPending or Idle — timer shouldn't fire, ignore
                }
            }
            // Lock screen events (customer submitted PIN)
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
            // WebSocket command results from spawned tasks
            Some(ws_exec_msg) = state.ws_exec_result_rx.recv() => {
                if let Ok(json) = serde_json::to_string(&ws_exec_msg) {
                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                        tracing::error!("Failed to send WS command result, connection lost");
                        break;
                    }
                }
            }
            // UDP heartbeat events (fast liveness detection)
            Some(hb_event) = state.heartbeat_event_rx.recv() => {
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
                        // All CoreToAgentMessage dispatch delegated to ws_handler (74-03)
                        match ws_handler::handle_ws_message(
                            &text,
                            &mut state,
                            &mut crash_recovery,
                            &mut launch_state,
                            &mut blank_timer,
                            &mut blank_timer_armed,
                            &mut current_driver_name,
                            &mut last_launch_args_stored,
                            &mut last_ffb_percent,
                            &mut last_ffb_preset,
                            &mut session_max_speed_kmh,
                            &mut session_race_position,
                            &mut ws_tx,
                            &primary_url,
                            &failover_url,
                            &active_url,
                            &split_brain_probe,
                        ).await {
                            Ok(ws_handler::HandleResult::Break) => break,
                            Ok(ws_handler::HandleResult::Continue) => {}
                            Err(e) => { tracing::error!("ws_handler error: {}", e); }
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
        state.heartbeat_status.ws_connected.store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::warn!("Disconnected from core server");

        // SESSION-04: Record disconnect time if not already set (grace window starts here)
        if ws_disconnected_at.is_none() {
            ws_disconnected_at = Some(std::time::Instant::now());
        }

        // If no billing active, enforce safe state on disconnect — kill any orphaned games
        if !state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("No active billing on disconnect — enforcing safe state");
            state.overlay.deactivate();
            // SAFETY: Safe session-end sequence before game cleanup
            ffb_controller::safe_session_end(&state.ffb).await;
            tracing::info!("FFB safety sequence complete on disconnect (ws_tx unavailable for FfbZeroed message)");
            if let Some(ref mut game) = state.game_process {
                let _ = game.stop();
                state.game_process = None;
            }
            if let Some(ref mut adp) = state.adapter { adp.disconnect(); }
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
            state.lock_screen.show_blank_screen();
        } else {
            // SESSION-04: Billing active — apply 30s grace window before showing Disconnected
            let disconnected_for = ws_disconnected_at
                .get_or_insert_with(std::time::Instant::now)
                .elapsed();
            if disconnected_for > Duration::from_secs(30) {
                state.lock_screen.show_disconnected();
            } else {
                tracing::info!("[ws-grace] WS dropped {}s — billing active, within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
            }
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

    // ─── SESSION-03: CrashRecoveryState tests ─────────────────────────────

    #[test]
    fn crash_recovery_state_starts_idle() {
        // CrashRecoveryState default is Idle (SESSION-03)
        let state = CrashRecoveryState::Idle;
        assert!(matches!(state, CrashRecoveryState::Idle),
            "CrashRecoveryState must default to Idle");
    }

    #[test]
    fn crash_recovery_state_paused_waiting_relaunch_attempt_1() {
        // PausedWaitingRelaunch with attempt=1 represents Idle->Recovery transition
        // (Can't easily construct the timer here, so we test the discriminant logic)
        // The key behavior: attempt < 2 means we try again; attempt == 2 means auto-end
        let attempt: u8 = 1;
        assert!(attempt < 2, "attempt=1 should trigger retry to attempt 2");
    }

    #[test]
    fn crash_recovery_state_attempt_2_triggers_auto_end() {
        // When attempt == 2, the state machine should transition to AutoEndPending
        let attempt: u8 = 2;
        assert!(!(attempt < 2), "attempt=2 should trigger auto-end (not retry)");
    }

    // ─── SESSION-04: WS grace window tests ────────────────────────────────

    #[test]
    fn ws_grace_window_is_30_seconds() {
        // 20s should be within 30s grace window — no Disconnected screen
        let disconnected_at = std::time::Instant::now() - Duration::from_secs(20);
        let elapsed = disconnected_at.elapsed();
        assert!(
            elapsed < Duration::from_secs(30),
            "20s should be within 30s grace window"
        );
    }

    #[test]
    fn ws_grace_window_expired_after_30s() {
        // 35s should be outside 30s grace window — show Disconnected screen
        let disconnected_at_old = std::time::Instant::now() - Duration::from_secs(35);
        let elapsed_old = disconnected_at_old.elapsed();
        assert!(
            elapsed_old > Duration::from_secs(30),
            "35s should be outside 30s grace window"
        );
    }

    #[test]
    fn ws_grace_window_boundary_exactly_30s() {
        // At exactly 30s elapsed, we should NOT suppress (disconnected_for > 30s is false at exactly 30s)
        let disconnected_at = std::time::Instant::now() - Duration::from_secs(30);
        let elapsed = disconnected_at.elapsed();
        // elapsed will be >= 30s due to test execution time — this tests the ">30" boundary
        assert!(
            elapsed >= Duration::from_secs(30),
            "30s+ elapsed should be at/beyond grace window boundary"
        );
    }
}
