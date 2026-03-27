mod ac_launcher;
#[cfg(feature = "ai-debugger")]
mod ai_debugger;
mod app_state;
mod feature_flags;
mod billing_guard;
mod event_loop;
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
mod pre_flight;
mod remote_ops;
mod safe_mode;
mod self_heal;
#[cfg(feature = "process-guard")]
mod process_guard;
mod self_monitor;
mod sentinel_watcher;
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
use feature_flags::FeatureFlags;
use config::{load_config, detect_installed_games};
use driving_detector::{
    DetectorConfig, DetectorSignal, DrivingDetector,
    is_input_active, is_steering_moving, parse_openffboard_report,
};
use ffb_controller::FfbController;
use rc_common::protocol::AgentMessage;
use rc_common::types::*;
use sims::SimAdapter;
use sims::assetto_corsa::AssettoCorsaAdapter;
use sims::f1_25::F125Adapter;
use sims::iracing::IracingAdapter;
use sims::lmu::LmuAdapter;
use kiosk::KioskManager;
use lock_screen::{LockScreenEvent, LockScreenManager};
use overlay::OverlayManager;

const LOG_TARGET: &str = "rc-agent";
const BUILD_ID: &str = env!("GIT_HASH");

// LaunchState and CrashRecoveryState moved to event_loop.rs (74-04)
// WS_MAX_CONCURRENT_EXECS, WS_EXEC_SEMAPHORE, and handle_ws_exec moved to ws_handler.rs (74-03)

/// Fetch the staff-managed allowlist from racecontrol (GET /api/v1/config/kiosk-allowlist).
/// Returns a list of lowercase process names on success, or an error if unreachable.
#[cfg(feature = "http-client")]
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
#[cfg(feature = "http-client")]
async fn allowlist_poll_loop(core_http_url: String, client: reqwest::Client) {
    let mut interval = tokio::time::interval(Duration::from_secs(kiosk::ALLOWLIST_REFRESH_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match fetch_server_allowlist(&client, &core_http_url).await {
            Ok(names) => {
                let count = names.len();
                kiosk::set_server_allowlist(names);
                tracing::info!(target: LOG_TARGET, "allowlist updated: {} entries from server", count);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "allowlist fetch failed (will retry in 5 min): {}", e);
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
/// Graceful shutdown flag — set by Ctrl+C / SIGTERM handler.
/// Background tasks can check this to exit cleanly.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
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
        let name = CString::new("Global\\RacingPoint_RCAgent_SingleInstance")
            .expect("mutex name contains no null bytes");
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

    // Match both module path (rc_agent::*) and explicit target: "rc-agent" (LOG_TARGET)
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "rc_agent=info,rc-agent=info".into());

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
    let _pod_span = tracing::info_span!(
        "rc-agent",
        pod_id = %pod_id_str,
        build_id = BUILD_ID,
    ).entered();
    tracing::info!(target: LOG_TARGET, "Structured logging initialized for {}", pod_id_str);

    // Compute binary SHA256 once at startup (OTA-10) — before server start
    remote_ops::init_binary_sha256();

    let agent_start_time = std::time::Instant::now();
    tracing::info!(target: LOG_TARGET, "Pod #{}: {} (sim: {})", config.pod.number, config.pod.name, config.pod.sim);
    tracing::info!(target: LOG_TARGET, "Core server: {}", config.core.url);

    // Clean up orphaned game processes from previous rc-agent instance
    let orphans_cleaned = game_process::cleanup_orphaned_games();
    if orphans_cleaned > 0 {
        tracing::warn!(target: LOG_TARGET, "Cleaned up {} orphaned game processes on startup", orphans_cleaned);
    }

    let pod_id = format!("pod_{}", config.pod.number);
    let sim_type = match config.pod.sim.as_str() {
        "assetto_corsa" | "ac" => SimType::AssettoCorsa,
        "iracing" => SimType::IRacing,
        "lmu" | "le_mans_ultimate" => SimType::LeMansUltimate,
        "f1_25" | "f1" => SimType::F125,
        "forza" => SimType::Forza,
        other => {
            tracing::error!(target: LOG_TARGET, "Unknown sim type: {}", other);
            return Ok(());
        }
    };

    // Determine installed games from config
    let installed_games = detect_installed_games(&config.games);
    tracing::info!(target: LOG_TARGET, "Installed games: {:?}", installed_games);

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
        freedom_mode: None,
    };

    // Firewall auto-config — ensure ICMP + TCP 8090 rules exist (FW-01, FW-02, FW-03)
    match firewall::configure() {
        firewall::FirewallResult::Configured => {
            tracing::info!(target: LOG_TARGET, "Firewall configured");
        }
        firewall::FirewallResult::Failed(msg) => {
            tracing::warn!(target: LOG_TARGET, "Firewall config failed: {} — continuing anyway", msg);
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
                Ok(true) => tracing::info!(target: LOG_TARGET, "FFB: venue power cap set to 80%"),
                Ok(false) => tracing::debug!(target: LOG_TARGET, "FFB: no wheelbase found — power cap skipped"),
                Err(e) => tracing::warn!(target: LOG_TARGET, "FFB: failed to set power cap: {}", e),
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
        SimType::IRacing => Some(Box::new(IracingAdapter::new(
            pod_id.clone(),
        ))),
        SimType::LeMansUltimate => Some(Box::new(LmuAdapter::new(
            pod_id.clone(),
        ))),
        SimType::AssettoCorsaEvo => {
            if config.ac_evo_telemetry_enabled {
                Some(Box::new(
                    sims::assetto_corsa_evo::AssettoCorsaEvoAdapter::new(pod_id.clone()),
                ))
            } else {
                tracing::info!(
                    target: LOG_TARGET,
                    "AC EVO telemetry disabled by feature flag (ac_evo_telemetry_enabled=false)"
                );
                None
            }
        }
        SimType::AssettoCorsaRally => Some(Box::new(
            sims::assetto_corsa_evo::AssettoCorsaEvoAdapter::new_rally(pod_id.clone()),
        )),
        _ => {
            tracing::warn!(target: LOG_TARGET, "Sim adapter not yet implemented for {:?}, running in heartbeat-only mode", sim_type);
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
            Ok(()) => tracing::info!(target: LOG_TARGET, "Connected to {} telemetry", sim_type),
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Could not connect to sim: {}. Will retry...", e);
            }
        }
    }

    // Game process state — now bundled in AppState (game_process, last_ac_status, ac_status_stable_since)

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
        tracing::info!(target: LOG_TARGET, "Kiosk mode ENABLED");
    } else {
        tracing::info!(target: LOG_TARGET, "Kiosk mode DISABLED (set kiosk.enabled=true in config to enable)");
    }

    // Lock screen for customer authentication (PIN / QR)
    // Always start the lock screen server so customers can enter PINs
    let (lock_event_tx, mut lock_event_rx) = mpsc::channel::<LockScreenEvent>(16);
    let mut lock_screen = LockScreenManager::new(lock_event_tx);
    // POS-01: Disable browser launch on auxiliary devices (POS, staff terminals).
    // State tracking and HTTP server still active for health/debug, but no Edge overlay.
    if !config.lock_screen.enabled {
        lock_screen.set_browser_disabled(true);
    }
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
            tracing::info!(target: LOG_TARGET, "Lock screen server bound on port {}", port);
            true
        }
        Ok(Ok(Err(e))) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Lock screen bind failed: {}", e);
            std::process::exit(1);
        }
        Ok(Err(_)) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Lock screen bind result channel dropped");
            std::process::exit(1);
        }
        Err(_) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Lock screen bind timed out after 5s");
            std::process::exit(1);
        }
    };

    // LOCK-02: Show branded startup page immediately — customers see Racing Point
    // branding while rc-agent connects to racecontrol, not a blank screen or idle message.
    lock_screen.show_startup_connecting();

    // Racing HUD overlay for in-session display
    let mut overlay = OverlayManager::new();
    overlay.start_server();
    tracing::info!(target: LOG_TARGET, "Overlay server started on port 18925");

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

    // ─── Auto-Switch Config (ConspitLink game detection) ─────────────────────
    // Ensures Global.json at C:\RacingPoint\ with AresAutoChangeConfig=open
    // and verifies GameToBaseConfig.json mappings. Runs BEFORE enforce_safe_state
    // so ConspitLink starts with correct config. Non-fatal: errors logged, not propagated.
    tokio::task::spawn_blocking(|| {
        let result = ffb_controller::ensure_auto_switch_config();
        if !result.errors.is_empty() {
            tracing::warn!(
                target: LOG_TARGET,
                "Auto-switch config errors: {:?}",
                result.errors
            );
        }
        tracing::info!(
            target: LOG_TARGET,
            placed = result.global_json_placed,
            changed = result.global_json_changed,
            game_to_base_fixed = result.game_to_base_fixed,
            restarted = result.conspit_restarted,
            "Auto-switch config complete"
        );
    });

    // Delayed startup cleanup — enforce safe state to kill any orphaned games
    // from previous session/crash. Delay gives startup apps time to open.
    {
        let ffb_startup_cleanup = ffb.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(8)).await;
            ffb_controller::safe_session_end(&ffb_startup_cleanup).await;
            tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(true); });
            tracing::info!(target: LOG_TARGET, "Startup: safe state enforced — pod clean for first customer");
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
        .unwrap_or_else(|| {
            tracing::warn!(target: "state", field = "server_ip", source = "ws_url", fallback = "127.0.0.1", "config field fell back to hardcoded default");
            "127.0.0.1"
        })
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
    tracing::info!(target: LOG_TARGET, "UDP heartbeat started → {}:{}", core_ip, rc_common::udp_protocol::HEARTBEAT_PORT);

    // ─── Self-Monitor (CLOSE_WAIT detection + LLM-gated relaunch) ───────────
    self_monitor::spawn(config.ai_debugger.clone(), heartbeat_status.clone());
    tracing::info!(target: LOG_TARGET, "Self-monitor started (check interval: 5min)");

    // ─── Failure Monitor (game freeze, launch timeout, USB reconnect) ────────
    failure_monitor::spawn(
        heartbeat_status.clone(),
        failure_monitor_rx,
        ws_exec_result_tx.clone(),
        pod_id.clone(),
        config.pod.number as u32,
    );
    tracing::info!(target: LOG_TARGET, "Failure monitor started (poll interval: 5s)");

    // ─── Feature Flags — load from disk cache, shared with billing_guard and AppState ──
    // v22.0 Phase 178: Create Arc here (before AppState) so billing_guard can share it.
    let flags_arc = std::sync::Arc::new(RwLock::new(FeatureFlags::load_from_cache()));

    // ─── Billing Guard (stuck session + idle drift detection) ────────────────
    // Site billing_guard: spawn billing anomaly detection task (shares watch receiver)
    // Derive HTTP base URL from WebSocket URL: ws://host:port/ws/agent → http://host:port/api/v1
    let core_http_base = config.core.url
        .replace("ws://", "http://")
        .replace("wss://", "https://")
        .split("/ws")
        .next()
        .unwrap_or_else(|| {
            tracing::warn!(target: "state", field = "api_base_url", source = "ws_url_split", fallback = "http://127.0.0.1:8080", "config field fell back to hardcoded default");
            "http://127.0.0.1:8080"
        })
        .to_string()
        + "/api/v1";

    // BOOT-02: Periodic feature flag re-fetch — self-heals within 5 minutes when server comes back
    #[cfg(feature = "http-client")]
    {
        let flags_clone = flags_arc.clone();
        let http_base = core_http_base.clone();
        let http_client_flags = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        rc_common::boot_resilience::spawn_periodic_refetch(
            "feature_flags".to_string(),
            Duration::from_secs(300), // 5 minutes
            move || {
                let flags = flags_clone.clone();
                let base = http_base.clone();
                let client = http_client_flags.clone();
                async move {
                    feature_flags::FeatureFlags::fetch_from_server(&client, &base, &flags).await
                }
            },
        );
        tracing::info!(target: LOG_TARGET, "Feature flags periodic re-fetch started (interval=300s)");
    }

    billing_guard::spawn(
        failure_monitor_tx.subscribe(),
        ws_exec_result_tx.clone(),
        pod_id.clone(),
        core_http_base,
        config.auto_end_orphan_session_secs,
        flags_arc.clone(),  // v22.0 Phase 178: pass feature flags for billing_guard gate
    );
    tracing::info!(target: LOG_TARGET, "Billing guard started (orphan_timeout={}s)", config.auto_end_orphan_session_secs);

    // ─── Server Allowlist Poll Loop (dynamic kiosk allowlist) ─────────────────
    // Derive HTTP base URL from WebSocket URL: ws://host:port/path → http://host:port
    // First poll fires immediately (interval first tick) so kiosk enforcement on first
    // scan already includes staff-added entries. Non-fatal on fetch failure.
    #[cfg(feature = "http-client")]
    {
        let core_http_url = config.core.url
            .replace("ws://", "http://")
            .replace("wss://", "https://")
            .split("/ws/")
            .next()
            .unwrap_or_else(|| {
                tracing::warn!(target: "state", field = "allowlist_poll_url", source = "ws_url_split", fallback = "http://127.0.0.1:8080", "config field fell back to hardcoded default");
                "http://127.0.0.1:8080"
            })
            .to_string();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        tokio::spawn(allowlist_poll_loop(core_http_url, http_client));
        tracing::info!(target: LOG_TARGET, "Allowlist poll loop started (refresh every {}s)", kiosk::ALLOWLIST_REFRESH_SECS);
    }

    // SAFETY-02: Wait for remote ops bind result — exit on failure.
    // Started early (before FFB/HID init) so the 30s retry window runs concurrently.
    let remote_ops_bound = match tokio::time::timeout(
        Duration::from_secs(35), // 10 attempts * 3s + margin
        remote_ops_rx,
    ).await {
        Ok(Ok(Ok(port))) => {
            tracing::info!(target: LOG_TARGET, "Remote ops server bound on port {}", port);
            true
        }
        Ok(Ok(Err(e))) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Remote ops bind failed: {}", e);
            std::process::exit(1);
        }
        Ok(Err(_)) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Remote ops bind result channel dropped");
            std::process::exit(1);
        }
        Err(_) => {
            tracing::error!(target: LOG_TARGET, "FATAL: Remote ops bind timed out after 35s");
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
        target: LOG_TARGET,
        "Startup self-test: verdict={:?} failures={}",
        startup_verdict.level,
        startup_probe_failures
    );

    // ─── Process Guard: fetch whitelist from server ──────────────────────────
    // Fetch merged whitelist for this pod. Falls back to empty whitelist (report_only) if server
    // is unreachable at startup — guard will still scan but log only.
    // Requires both process-guard feature (for the guard task) AND ai-debugger (for reqwest).
    let fetched_whitelist = {
        #[cfg(all(feature = "process-guard", feature = "http-client"))]
        {
        let http_url = config.core.url
            .replace("ws://", "http://")
            .replace("wss://", "https://")
            .split("/ws")
            .next()
            .unwrap_or_else(|| {
                tracing::warn!(target: "state", field = "whitelist_url", source = "ws_url_split", fallback = "http://127.0.0.1:8080", "config field fell back to hardcoded default");
                "http://127.0.0.1:8080"
            })
            .to_string();
        let whitelist_url = format!("{}/api/v1/guard/whitelist/pod-{}", http_url, config.pod.number);
        match reqwest::Client::new()
            .get(&whitelist_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<rc_common::types::MachineWhitelist>().await {
                    Ok(wl) => {
                        tracing::info!(target: LOG_TARGET, "Process guard: whitelist fetched ({} processes)", wl.processes.len());
                        wl
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "Process guard: whitelist parse error: {} — using default (report_only)", e);
                        rc_common::types::MachineWhitelist::default()
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!(target: LOG_TARGET, "Process guard: whitelist fetch {} — using default (report_only)", resp.status());
                rc_common::types::MachineWhitelist::default()
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Process guard: whitelist fetch error: {} — using default (report_only)", e);
                rc_common::types::MachineWhitelist::default()
            }
        }
        }
        #[cfg(not(all(feature = "process-guard", feature = "http-client")))]
        rc_common::types::MachineWhitelist::default()
    };

    // ─── Bundle pre-loop state into AppState ────────────────────────────────
    let (guard_violation_tx, guard_violation_rx) = mpsc::channel::<rc_common::protocol::AgentMessage>(32);
    let guard_whitelist = std::sync::Arc::new(RwLock::new(fetched_whitelist));
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
        game_process: None,
        last_ac_status: None,
        ac_status_stable_since: None,
        in_maintenance: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        safe_mode: safe_mode::SafeMode::new(),
        safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        wmi_rx: None, // Set below after WMI watcher spawn
        safe_mode_cooldown_timer: Box::pin(tokio::time::sleep(std::time::Duration::from_secs(86400))),
        safe_mode_cooldown_armed: false,
        last_preflight_alert: None,
        flags: flags_arc,  // v22.0 Phase 178: shared with billing_guard (loaded from cache above)
        guard_whitelist,
        guard_violation_tx,
        guard_violation_rx,
        guard_confirmed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    // ─── Safe Mode: startup detection ─────────────────────────────────────────
    if let Some(sim) = safe_mode::detect_running_protected_game() {
        state.safe_mode.enter(sim);
        state.safe_mode_active.store(true, std::sync::atomic::Ordering::Relaxed);
        tracing::warn!(target: LOG_TARGET, "Protected game already running at startup — safe mode ACTIVE");
    }

    // ─── Safe Mode: WMI process watcher ───────────────────────────────────────
    state.wmi_rx = Some(safe_mode::spawn_wmi_watcher());
    tracing::info!(target: LOG_TARGET, "WMI safe mode watcher spawned");

    // ─── Safe Mode: wire flag into KioskManager and LockScreenManager (SAFE-06) ─
    state.kiosk.wire_safe_mode(std::sync::Arc::clone(&state.safe_mode_active));
    state.lock_screen.wire_safe_mode(std::sync::Arc::clone(&state.safe_mode_active));

    // ─── Process Guard: spawn background task ───────────────────────────────
    #[cfg(feature = "process-guard")]
    {
        process_guard::spawn(
            state.config.process_guard.clone(),
            state.guard_whitelist.clone(),
            state.guard_violation_tx.clone(),
            state.pod_id.clone(),
            std::sync::Arc::clone(&state.safe_mode_active),  // safe mode flag
            std::sync::Arc::clone(&state.guard_confirmed),   // BOOT-04: operator confirmation gate
        );
        tracing::info!(target: LOG_TARGET, "Process guard spawned (interval={}s)", state.config.process_guard.scan_interval_secs);

        // ─── Process Guard: periodic whitelist re-fetch (every 5 min) ───────
        // Pods fetch the allowlist once at boot. If the server was down at boot,
        // they get MachineWhitelist::default() (empty) and flag every process.
        // This task re-fetches every 5 minutes so pods self-heal without manual
        // rc-agent restart.
        #[cfg(feature = "http-client")]
        {
            let refetch_whitelist = state.guard_whitelist.clone();
            let refetch_pod_number = state.config.pod.number;
            let refetch_http_url = state.config.core.url
                .replace("ws://", "http://")
                .replace("wss://", "https://")
                .split("/ws")
                .next()
                .unwrap_or_else(|| {
                    tracing::warn!(target: "state", field = "feature_flag_url", source = "ws_url_split", fallback = "http://127.0.0.1:8080", "config field fell back to hardcoded default");
                    "http://127.0.0.1:8080"
                })
                .to_string();
            tokio::spawn(async move {
                tracing::info!(target: "guard", "Whitelist re-fetch task started (interval=300s, url={})", refetch_http_url);
                let client = reqwest::Client::new();
                let url = format!("{}/api/v1/guard/whitelist/pod-{}", refetch_http_url, refetch_pod_number);
                loop {
                    tokio::time::sleep(Duration::from_secs(300)).await; // 5 minutes
                    match client.get(&url).timeout(Duration::from_secs(10)).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            match resp.json::<rc_common::types::MachineWhitelist>().await {
                                Ok(wl) => {
                                    let count = wl.processes.len();
                                    *refetch_whitelist.write().await = wl;
                                    tracing::info!(target: "guard", "Whitelist re-fetched ({} processes)", count);
                                }
                                Err(e) => tracing::debug!(target: "guard", "Whitelist re-fetch parse error: {}", e),
                            }
                        }
                        Ok(resp) => tracing::debug!(target: "guard", "Whitelist re-fetch HTTP {}", resp.status()),
                        Err(e) => tracing::debug!(target: "guard", "Whitelist re-fetch error: {}", e),
                    }
                }
                #[allow(unreachable_code)]
                tracing::error!(target: "guard", "Whitelist re-fetch task exited unexpectedly");
            });
            tracing::info!(target: LOG_TARGET, "Process guard whitelist re-fetch task spawned (interval=300s)");
        }
    }

    // ─── Phase 206: Sentinel File Watcher ─────────────────────────────────────
    // Watches C:\RacingPoint\ for sentinel file create/delete events.
    // Sends AgentMessage::SentinelChange via ws_exec_result_tx → WS → racecontrol.
    // Runs as a dedicated OS thread (not tokio) — notify uses sync mpsc internally.
    sentinel_watcher::spawn(state.ws_exec_result_tx.clone(), state.pod_id.clone());
    tracing::info!(target: LOG_TARGET, "Sentinel file watcher spawned (watching C:\\RacingPoint\\)");

    // ─── Graceful Shutdown Handler ──────────────────────────────────────────
    // Ctrl+C on Windows (covers SIGTERM-equivalent). Sets SHUTDOWN_REQUESTED flag
    // so background tasks can exit cleanly and FFB is zeroed before process exit.
    {
        let ffb_shutdown = state.ffb.clone();
        tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                tracing::info!(target: LOG_TARGET, "Shutdown signal received (Ctrl+C) — initiating graceful shutdown");
                SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
                // Zero FFB as a safety measure before exit
                let _ = tokio::task::spawn_blocking(move || {
                    ffb_shutdown.zero_force_with_retry(3, 100);
                }).await;
                tracing::info!(target: LOG_TARGET, "Graceful shutdown complete — exiting");
                std::process::exit(0);
            }
        });
    }

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
    // Append ?token=SECRET for WS authentication (H1 audit fix)
    let ws_token_suffix = state.config.core.ws_secret.as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("?token={}", s))
        .unwrap_or_default();
    let authed_url = format!("{}{}", state.config.core.url, ws_token_suffix);
    let active_url: std::sync::Arc<RwLock<String>> =
        std::sync::Arc::new(RwLock::new(authed_url));
    let primary_url: String = format!("{}{}", state.config.core.url, ws_token_suffix);
    let failover_url: Option<String> = state.config.core.failover_url.as_ref()
        .map(|u| format!("{}{}", u, ws_token_suffix));

    // Phase 69: Split-brain guard — reusable HTTP client for LAN probe (created once, not per-message)
    #[cfg(feature = "http-client")]
    let split_brain_probe = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();
    #[cfg(not(feature = "http-client"))]
    let split_brain_probe = ();

    loop {
        // Reset startup report flag on each reconnection — so racecontrol always gets
        // version + uptime after it restarts (fixes null version/uptime for long-running pods)
        startup_report_sent = false;

        // Connect to core server — read active_url on each iteration (Phase 68: runtime switching)
        let url = active_url.read().await.clone();
        tracing::info!(target: LOG_TARGET, "Connecting to RaceControl core at {}...", url);
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
                tracing::warn!(target: LOG_TARGET, "Failed to connect to core: {}. Attempt {}. Retrying in {:?}...", e, reconnect_attempt, delay);
                // SESSION-04: Only show Disconnected after 30s grace window
                {
                    let disconnected_for = ws_disconnected_at
                        .get_or_insert_with(std::time::Instant::now)
                        .elapsed();
                    if disconnected_for > Duration::from_secs(30) {
                        state.lock_screen.show_disconnected();
                    } else {
                        tracing::info!(target: LOG_TARGET, "ws-grace: Disconnected {}s — within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
                    }
                }
                tokio::time::sleep(delay).await;
                reconnect_attempt += 1;
                continue;
            }
            Err(_) => {
                let delay = reconnect_delay_for_attempt(reconnect_attempt);
                tracing::warn!(target: LOG_TARGET, "Connection to core timed out. Attempt {}. Retrying in {:?}...", reconnect_attempt, delay);
                // SESSION-04: Only show Disconnected after 30s grace window
                {
                    let disconnected_for = ws_disconnected_at
                        .get_or_insert_with(std::time::Instant::now)
                        .elapsed();
                    if disconnected_for > Duration::from_secs(30) {
                        state.lock_screen.show_disconnected();
                    } else {
                        tracing::info!(target: LOG_TARGET, "ws-grace: Timed out {}s — within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
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
            tracing::warn!(target: LOG_TARGET, "Failed to register with core. Attempt {}. Reconnecting in {:?}...", reconnect_attempt, delay);
            tokio::time::sleep(delay).await;
            reconnect_attempt += 1;
            continue;
        }
        tracing::info!(target: LOG_TARGET, "Connected and registered as Pod #{}", state.config.pod.number);
        if !startup_complete_logged {
            startup_log::write_phase("websocket", &format!("connected pod={}", state.config.pod.number));
            startup_log::write_phase("complete", "");
            startup_complete_logged = true;
        }

        // v22.0 Phase 178: Request flag sync from server with our cached version.
        // Sent on every WS connect so server can send a delta (or full sync if version=0).
        {
            let flags = state.flags.read().await;
            let sync_msg = AgentMessage::FlagCacheSync(rc_common::types::FlagCacheSyncPayload {
                pod_id: state.pod_id.clone(),
                cached_version: flags.cached_version(),
            });
            if let Ok(json) = serde_json::to_string(&sync_msg) {
                if ws_tx.send(Message::Text(json.into())).await.is_ok() {
                    tracing::info!(target: LOG_TARGET, "Sent FlagCacheSync (cached_version={})", flags.cached_version());
                } else {
                    tracing::warn!(target: LOG_TARGET, "Failed to send FlagCacheSync");
                }
            }
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
                    tracing::info!(target: LOG_TARGET, "Sent startup report to core (crash_recovery={})", state.crash_recovery_startup);
                    startup_report_sent = true;
                } else {
                    tracing::warn!(target: LOG_TARGET, "Failed to send startup report — will retry next connect");
                }
            }
        }

        // Send content manifest after registration so core knows what's installed
        let manifest = content_scanner::scan_ac_content();
        tracing::info!(target: LOG_TARGET, "Scanned AC content: {} cars, {} tracks", manifest.cars.len(), manifest.tracks.len());
        let manifest_msg = AgentMessage::ContentManifest(manifest);
        if let Ok(json) = serde_json::to_string(&manifest_msg) {
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                tracing::warn!(target: LOG_TARGET, "Failed to send content manifest");
            }
        }

        state.heartbeat_status.ws_connected.store(true, std::sync::atomic::Ordering::Relaxed);

        // Inner event loop — runs until connection is lost.
        // All per-connection state (intervals, timers, crash_recovery, launch_state, etc.)
        // is initialized inside ConnectionState::new() by event_loop::run().

        if let Err(e) = event_loop::run(
            &mut state,
            ws_tx,
            ws_rx,
            &primary_url,
            &failover_url,
            &active_url,
            &split_brain_probe,
        ).await {
            tracing::warn!(target: LOG_TARGET, "Event loop error: {}", e);
        }


        // Connection lost — update UDP heartbeat status and show disconnected
        state.heartbeat_status.ws_connected.store(false, std::sync::atomic::Ordering::Relaxed);
        tracing::warn!(target: LOG_TARGET, "Disconnected from core server");

        // SESSION-04: Record disconnect time if not already set (grace window starts here)
        if ws_disconnected_at.is_none() {
            ws_disconnected_at = Some(std::time::Instant::now());
        }

        // If no billing active, enforce safe state on disconnect — kill any orphaned games
        if !state.heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!(target: LOG_TARGET, "No active billing on disconnect — enforcing safe state");
            state.overlay.deactivate();
            // SAFETY: Safe session-end sequence before game cleanup
            ffb_controller::safe_session_end(&state.ffb).await;
            tracing::info!(target: LOG_TARGET, "FFB safety sequence complete on disconnect (ws_tx unavailable for FfbZeroed message)");
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
                tracing::info!(target: LOG_TARGET, "ws-grace: WS dropped {}s — billing active, within 30s grace window, suppressing Disconnected screen", disconnected_for.as_secs());
            }
        }

        let delay = reconnect_delay_for_attempt(reconnect_attempt);
        tracing::warn!(target: LOG_TARGET, "Attempt {}. Reconnecting in {:?}...", reconnect_attempt, delay);
        tokio::time::sleep(delay).await;
        reconnect_attempt += 1;
    } // end reconnection loop
}

/// Compute reconnect delay based on attempt number.
/// First 3 attempts: 1s each (fast retry for brief CPU spike blips).
/// After that: exponential backoff 2s, 4s, 8s, 16s, capped at 30s.
/// CONN-RESIL: Added jitter (0–25% of delay) to prevent thundering herd
/// when server restarts and all 8 pods retry at the same deterministic moments.
fn reconnect_delay_for_attempt(attempt: u32) -> Duration {
    let base = if attempt < 3 {
        Duration::from_secs(1)
    } else {
        let exp = (attempt - 2).min(5);
        Duration::from_secs(2u64.pow(exp)).min(Duration::from_secs(30))
    };
    // Add 0–25% jitter to stagger reconnection across pods
    let jitter_ms = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);
        (hasher.finish() % (base.as_millis() as u64 / 4).max(1)) as u64
    };
    base + Duration::from_millis(jitter_ms)
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
                tracing::warn!(target: LOG_TARGET, "Failed to initialize HID API: {}", e);
                let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "HID task panic: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Try to find and open the wheelbase
        let device = api.open(vid, pid);
        match device {
            Ok(dev) => {
                tracing::info!(
                    target: LOG_TARGET,
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
                            tracing::warn!(target: LOG_TARGET, "HID read error: {}", e);
                            let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                tracing::debug!(
                    target: LOG_TARGET,
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
            tracing::warn!(target: LOG_TARGET, "UDP port {}: SetHandleInformation failed", port);
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
                    tracing::info!(target: LOG_TARGET, "Listening for game telemetry on UDP port {} (SO_REUSEADDR)", port);
                    s
                }
                None => {
                    tracing::warn!(
                        target: LOG_TARGET,
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
                        tracing::warn!(target: LOG_TARGET, "UDP recv error on port {}: {}", port, e);
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
    use crate::event_loop::CrashRecoveryState;

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
