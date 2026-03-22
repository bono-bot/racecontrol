//! Lock screen UI for customer authentication on gaming PCs.
//!
//! Serves a fullscreen HTML page via a local HTTP server and launches
//! Edge in kiosk mode to display PIN entry or QR code screens.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const LOG_TARGET: &str = "lock-screen";

/// Create a Command with CREATE_NO_WINDOW on Windows (prevents console flash).
/// Used for background utilities (taskkill, reg, powershell). NOT for browser launches.
fn hidden_cmd(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Query the virtual screen bounds (covers all monitors).
/// Returns (x, y, width, height) of the full virtual desktop.
/// On single-monitor setups this is typically (0, 0, 1920, 1080) or similar.
/// On multi-monitor / surround setups this covers the entire span
/// (e.g. triple 2560x1440 → (0, 0, 7680, 1440)).
#[cfg(windows)]
fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
    // SM_XVIRTUALSCREEN=76, SM_YVIRTUALSCREEN=77, SM_CXVIRTUALSCREEN=78, SM_CYVIRTUALSCREEN=79
    unsafe extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }
    let x = unsafe { GetSystemMetrics(76) };
    let y = unsafe { GetSystemMetrics(77) };
    let w = unsafe { GetSystemMetrics(78) };
    let h = unsafe { GetSystemMetrics(79) };
    if w == 0 || h == 0 {
        // Fallback to primary monitor
        let pw = unsafe { GetSystemMetrics(0) }; // SM_CXSCREEN
        let ph = unsafe { GetSystemMetrics(1) }; // SM_CYSCREEN
        (0, 0, pw, ph)
    } else {
        (x, y, w, h)
    }
}

#[cfg(not(windows))]
fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
    (0, 0, 1920, 1080)
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// Current lock screen state.
#[derive(Debug, Clone)]
pub enum LockScreenState {
    /// No lock screen displayed.
    Hidden,
    /// PIN entry screen.
    PinEntry {
        token_id: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
        pin_error: Option<String>,
    },
    /// QR code display screen.
    QrDisplay {
        token_id: String,
        qr_payload: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
    },
    /// Active session — shows time remaining.
    ActiveSession {
        driver_name: String,
        remaining_seconds: u32,
        allocated_seconds: u32,
    },
    /// Session ended — shows summary with optional performance stats.
    /// Results stay on screen indefinitely until next session starts (SESS-03).
    SessionSummary {
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        /// Top speed recorded during the session (SESS-01). None if not available.
        top_speed_kmh: Option<f32>,
        /// Race finishing position (SESS-02). None if not a race or position unavailable.
        race_position: Option<u32>,
    },
    /// Between sessions — sub-session ended, customer can pick next race.
    BetweenSessions {
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        wallet_balance_paise: i64,
        current_split_number: u32,
        total_splits: u32,
    },
    /// Awaiting staff assistance (F1 25 or manual-launch games).
    AwaitingAssistance {
        driver_name: String,
        message: String,
    },
    /// Launch splash — shown while game loads (~10s). Covers the desktop gap.
    LaunchSplash {
        driver_name: String,
        message: String,
    },
    /// Screen blanked — pure black screen between sessions.
    ScreenBlanked,
    /// Disconnected from core server — shown during reconnection attempts.
    Disconnected,
    /// Startup connecting — shown immediately at boot while rc-agent waits to connect.
    /// Eliminates ERR_CONNECTION_REFUSED race (LOCK-01) and gives customers a branded
    /// waiting page from first boot (LOCK-02).
    StartupConnecting,
    /// Configuration error — shown when rc-agent.toml is invalid or missing.
    /// The technical error details are logged to stderr only; this screen shows
    /// a generic message so customers do not see internal configuration details.
    ConfigError {
        message: String,
    },
    /// Kiosk lockdown — unauthorized software detected.
    /// Shows "please contact staff" message. Only cleared by employee PIN or server approval.
    Lockdown {
        message: String,
    },
    /// Pre-flight checks failed — pod blocked until staff clears or auto-retry succeeds.
    MaintenanceRequired {
        failures: Vec<String>,
    },
}

/// Events emitted by the lock screen to the agent main loop.
pub enum LockScreenEvent {
    /// Customer submitted a PIN.
    PinEntered { pin: String },
}

// ─── Manager ─────────────────────────────────────────────────────────────────

/// Manages the lock screen lifecycle: state, HTTP server, and browser window.
pub struct LockScreenManager {
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
    port: u16,
    #[cfg(windows)]
    browser_process: Option<std::process::Child>,
    /// Optional wallpaper URL for lock screen background (BRAND-02).
    /// When set, renders as CSS background-image on all states except ScreenBlanked.
    wallpaper_url: Arc<Mutex<Option<String>>>,
    /// SAFE-06: gates Focus Assist registry writes during protected game sessions.
    /// Wired after AppState construction via wire_safe_mode().
    safe_mode_active: Arc<AtomicBool>,
}

impl LockScreenManager {
    pub fn new(event_tx: mpsc::Sender<LockScreenEvent>) -> Self {
        Self {
            state: Arc::new(Mutex::new(LockScreenState::Hidden)),
            event_tx,
            port: 18923,
            #[cfg(windows)]
            browser_process: None,
            wallpaper_url: Arc::new(Mutex::new(None)),
            safe_mode_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wire the shared safe mode flag from AppState into this LockScreenManager.
    /// Call once after AppState is constructed (main.rs, before the reconnect loop).
    pub fn wire_safe_mode(&mut self, flag: Arc<AtomicBool>) {
        self.safe_mode_active = flag;
    }

    /// Set the wallpaper URL for lock screen background (BRAND-02).
    /// Pass None to clear the wallpaper and revert to default gradient.
    pub fn set_wallpaper_url(&self, url: Option<String>) {
        let mut w = self.wallpaper_url.lock().unwrap_or_else(|e| e.into_inner());
        *w = url;
    }

    /// Start the local HTTP server for lock screen pages (call once at startup).
    pub fn start_server(&self) {
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let port = self.port;
        let wallpaper_url = self.wallpaper_url.clone();
        tokio::spawn(async move {
            serve_lock_screen(port, state, event_tx, wallpaper_url).await;
        });
    }

    /// Start the lock screen HTTP server and return a oneshot receiver that
    /// resolves with Ok(port) on successful bind or Err(message) on failure.
    /// This replaces the fire-and-forget `start_server()` for the main lock screen
    /// (the early lock screen still uses `start_server()` since bind failure there
    /// is non-fatal — we just need a branded error page).
    pub fn start_server_checked(&self) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let port = self.port;
        let wallpaper_url = self.wallpaper_url.clone();
        tokio::spawn(async move {
            let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            let socket = match tokio::net::TcpSocket::new_v4() {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(format!("lock screen socket create failed: {}", e)));
                    return;
                }
            };
            let _ = socket.set_reuseaddr(true);
            if let Err(e) = socket.bind(addr) {
                let _ = tx.send(Err(format!("lock screen port {} bind failed: {}", port, e)));
                return;
            }
            let listener = match socket.listen(128) {
                Ok(l) => {
                    tracing::info!(target: LOG_TARGET, "Lock screen server listening on http://127.0.0.1:{}", port);
                    let _ = tx.send(Ok(port));
                    l
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("lock screen port {} listen failed: {}", port, e)));
                    return;
                }
            };
            // Run the accept loop with pre-bound listener
            serve_with_listener(listener, state, event_tx, wallpaper_url).await;
        });
        rx
    }

    /// Wait until the local HTTP server is ready to accept connections (port 18923 bound).
    ///
    /// Polls `127.0.0.1:{port}` with a 100ms connect timeout, retrying every 50ms.
    /// Gives up after 5 seconds and logs a warning — does NOT panic.
    /// This eliminates the ERR_CONNECTION_REFUSED race condition (LOCK-01).
    pub async fn wait_for_self_ready(&mut self) {
        let addr = format!("127.0.0.1:{}", self.port)
            .parse::<std::net::SocketAddr>()
            .expect("hardcoded addr");
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);

        loop {
            let timeout_result = tokio::time::timeout(
                tokio::time::Duration::from_millis(100),
                tokio::net::TcpStream::connect(addr),
            ).await;

            match timeout_result {
                Ok(Ok(_stream)) => {
                    tracing::info!(target: LOG_TARGET, "Lock screen HTTP server ready on port {}", self.port);
                    return;
                }
                _ => {
                    if tokio::time::Instant::now() >= deadline {
                        tracing::warn!(
                            target: LOG_TARGET,
                            "Lock screen HTTP server not ready after 5s on port {} — continuing anyway",
                            self.port
                        );
                        return;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            }
        }
    }

    /// Show the branded startup page immediately at boot.
    ///
    /// Opens Edge kiosk pointing at the local server which now shows the StartupConnecting
    /// page. As rc-agent transitions to other states (Disconnected, PinEntry, etc.) the
    /// browser auto-reloads every 3s and picks up the new state.
    pub fn show_startup_connecting(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::StartupConnecting;
        }
        self.launch_browser();
    }

    /// Show the PIN entry lock screen.
    pub fn show_pin_screen(
        &mut self,
        token_id: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::PinEntry {
                token_id,
                driver_name,
                pricing_tier_name,
                allocated_seconds,
                pin_error: None,
            };
        }
        self.launch_browser();
    }

    /// Show the idle PinEntry screen — pod is ready for next customer.
    /// Used after session end (SESSION-02) and orphan auto-end (SESSION-01).
    /// Renders a clean "Ready" screen without requiring a booking token.
    pub fn show_idle_pin_entry(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::PinEntry {
                token_id: String::new(),
                driver_name: String::new(),
                pricing_tier_name: String::new(),
                allocated_seconds: 0,
                pin_error: None,
            };
        }
        self.launch_browser();
    }

    /// Show PIN validation error on lock screen (wrong PIN feedback).
    pub fn show_pin_error(&self, reason: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let LockScreenState::PinEntry { ref mut pin_error, .. } = *state {
            *pin_error = Some(reason.to_string());
        }
        // The next page poll from the browser will pick up the error
    }

    /// Show the QR code lock screen.
    pub fn show_qr_screen(
        &mut self,
        token_id: String,
        qr_payload: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::QrDisplay {
                token_id,
                qr_payload,
                driver_name,
                pricing_tier_name,
                allocated_seconds,
            };
        }
        self.launch_browser();
    }

    /// Show the active session screen with countdown timer.
    /// Closes the lock screen browser so the game is visible during gameplay.
    pub fn show_active_session(
        &mut self,
        driver_name: String,
        remaining_seconds: u32,
        allocated_seconds: u32,
    ) {
        let was_blanked = self.is_blanked();
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ActiveSession {
                driver_name,
                remaining_seconds,
                allocated_seconds,
            };
        }
        // Close the browser so the game is visible — blanking/lock screen
        // should not cover the screen during active sessions
        self.close_browser();
        if was_blanked {
            #[cfg(windows)]
            suppress_notifications(false);
        }
    }

    /// Update remaining seconds on the active session screen.
    pub fn update_remaining(&self, remaining_seconds: u32) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let LockScreenState::ActiveSession { remaining_seconds: ref mut r, .. } = *state {
            *r = remaining_seconds;
        }
    }

    /// Show the session summary screen with optional performance stats (SESS-01, SESS-02).
    /// Results stay on screen indefinitely until next session starts (SESS-03).
    pub fn show_session_summary(
        &mut self,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        top_speed_kmh: Option<f32>,
        race_position: Option<u32>,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::SessionSummary {
                driver_name,
                total_laps,
                best_lap_ms,
                driving_seconds,
                top_speed_kmh,
                race_position,
            };
        }
        self.launch_browser();
    }

    /// Show between-sessions screen (sub-session ended, customer can pick next race).
    pub fn show_between_sessions(
        &mut self,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        wallet_balance_paise: i64,
        current_split_number: u32,
        total_splits: u32,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::BetweenSessions {
                driver_name,
                total_laps,
                best_lap_ms,
                driving_seconds,
                wallet_balance_paise,
                current_split_number,
                total_splits,
            };
        }
        self.launch_browser();
    }

    /// Show assistance screen (waiting for staff to launch game).
    pub fn show_assistance(
        &mut self,
        driver_name: String,
        message: String,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::AwaitingAssistance {
                driver_name,
                message,
            };
        }
        self.launch_browser();
    }

    /// Returns true if the screen is idle (Hidden) or already blanked.
    /// Get a clone of the state handle for external use (e.g., debug server).
    pub fn state_handle(&self) -> Arc<Mutex<LockScreenState>> {
        self.state.clone()
    }

    pub fn is_idle_or_blanked(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*state, LockScreenState::Hidden | LockScreenState::ScreenBlanked | LockScreenState::Disconnected | LockScreenState::StartupConnecting | LockScreenState::MaintenanceRequired { .. })
    }

    /// Returns true if the lock screen is showing something to a customer (not hidden/blanked).
    pub fn is_active(&self) -> bool {
        !self.is_idle_or_blanked()
    }

    /// Returns true if the screen is currently blanked.
    pub fn is_blanked(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*state, LockScreenState::ScreenBlanked)
    }

    /// Show a branded splash screen while the game loads (~10s gap after launch).
    /// Covers the desktop so customers never see Windows during game startup.
    pub fn show_launch_splash(&mut self, driver_name: String) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::LaunchSplash {
                driver_name,
                message: "Preparing your session...".to_string(),
            };
        }
        self.launch_browser();
    }

    /// Show a blank (black) screen — used between sessions when screen blanking is enabled.
    pub fn show_blank_screen(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ScreenBlanked;
        }
        #[cfg(windows)]
        // ─── SAFE-06: skip Focus Assist registry write during safe mode ───
        if !self.safe_mode_active.load(std::sync::atomic::Ordering::Relaxed) {
            suppress_notifications(true);
        } else {
            tracing::info!(target: LOG_TARGET, "safe mode active — Focus Assist registry write deferred");
        }
        self.launch_browser();
    }

    /// Show a branded configuration error screen.
    /// The `message` parameter is NOT shown to the customer — a generic message is used instead.
    /// Technical details should be logged separately via tracing::error!.
    pub fn show_config_error(&mut self, _message: &str) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ConfigError {
                message: "Configuration Error - contact staff".to_string(),
            };
        }
        self.launch_browser();
    }

    /// Clear/dismiss the lock screen.
    pub fn show_disconnected(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Don't override active sessions — customer might still be driving
        if matches!(*state, LockScreenState::ActiveSession { .. }) {
            return;
        }
        *state = LockScreenState::Disconnected;
    }

    /// Show lockdown screen — "please contact staff" message.
    pub fn show_lockdown(&mut self, message: &str) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::Lockdown {
                message: message.to_string(),
            };
        }
        self.launch_browser();
    }

    /// Show maintenance required screen — pre-flight checks failed.
    /// Displays a branded error page with failure details. Pod remains blocked
    /// until staff sends ClearMaintenance or auto-retry passes pre-flight.
    pub fn show_maintenance_required(&mut self, failures: Vec<String>) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::MaintenanceRequired { failures };
        }
        self.launch_browser();
    }

    /// Returns true if the lock screen is currently showing the MaintenanceRequired page.
    pub fn is_maintenance_required(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*state, LockScreenState::MaintenanceRequired { .. })
    }

    pub fn clear(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::Hidden;
        }
        self.close_browser();
        #[cfg(windows)]
        suppress_notifications(false);
    }

    #[cfg(windows)]
    pub fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);
        // Try common Edge install paths, then fall back to PATH lookup
        let edge_paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            "msedge.exe",
        ];

        // Query virtual screen bounds to cover multi-monitor / surround setups
        // (e.g. triple 2560x1440 = 7680x1440 virtual desktop)
        let (vx, vy, vw, vh) = get_virtual_screen_bounds();
        let window_pos = format!("--window-position={},{}", vx, vy);
        let window_size = format!("--window-size={},{}", vw, vh);
        let use_window_sizing = vw > 2560; // Only override for multi-monitor setups

        for edge_path in &edge_paths {
            let mut args: Vec<&str> = vec![
                "--kiosk",
                &url,
                "--edge-kiosk-type=fullscreen",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-notifications",
                "--disable-popup-blocking",
                "--disable-infobars",
                "--disable-session-crashed-bubble",
                "--disable-component-update",
                "--autoplay-policy=no-user-gesture-required",
                "--suppress-message-center-popups",
                "--disable-extensions",
                "--disable-dev-tools",
                "--disable-dev-tools-extension",
                "--disable-translate",
                "--disable-features=FileSystemAPI",
                "--disable-file-system",
                "--incognito",
                "--disable-pinch",
                "--disable-print-preview",
                "--no-experiments",
                "--disable-background-networking",
                "--block-new-web-contents",
            ];
            if use_window_sizing {
                args.push(&window_pos);
                args.push(&window_size);
            }
            match std::process::Command::new(edge_path)
                .args(&args)
                .spawn()
            {
                Ok(child) => {
                    self.browser_process = Some(child);
                    tracing::info!(
                        target: LOG_TARGET,
                        "Lock screen browser launched at {} using {} (virtual screen: {}x{} at {},{})",
                        url, edge_path, vw, vh, vx, vy
                    );
                    // Edge --kiosk ignores --window-size on some multi-monitor setups.
                    // Force the window to cover the full virtual screen after launch.
                    if use_window_sizing {
                        let (fx, fy, fw, fh) = (vx, vy, vw, vh);
                        std::thread::spawn(move || {
                            // Wait for Edge to create its window
                            std::thread::sleep(std::time::Duration::from_secs(3));
                            unsafe extern "system" {
                                fn FindWindowA(class: *const u8, title: *const u8) -> isize;
                                fn MoveWindow(hwnd: isize, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
                            }
                            let hwnd = unsafe { FindWindowA(b"Chrome_WidgetWin_1\0".as_ptr(), std::ptr::null()) };
                            if hwnd != 0 {
                                let ok = unsafe { MoveWindow(hwnd, fx, fy, fw, fh, 1) };
                                tracing::info!(target: LOG_TARGET, "MoveWindow(Edge, {},{},{},{}) = {}", fx, fy, fw, fh, ok);
                            } else {
                                tracing::warn!(target: LOG_TARGET, "Edge window not found for MoveWindow resize");
                            }
                        });
                    }
                    return;
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "Failed to launch Edge from {}: {}", edge_path, e);
                }
            }
        }
        tracing::error!(target: LOG_TARGET, "Could not launch Edge from any known path");
    }

    #[cfg(not(windows))]
    pub fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);
        // Try browsers in order: Edge, Chromium, Chrome, Firefox
        let browsers = [
            ("microsoft-edge", vec!["--kiosk", &url, "--no-first-run"]),
            ("chromium-browser", vec!["--kiosk", &url, "--no-first-run", "--noerrdialogs"]),
            ("chromium", vec!["--kiosk", &url, "--no-first-run", "--noerrdialogs"]),
            ("google-chrome", vec!["--kiosk", &url, "--no-first-run", "--noerrdialogs"]),
            ("firefox", vec!["--kiosk", &url]),
        ];
        for (browser, args) in &browsers {
            match std::process::Command::new(browser).args(args).spawn() {
                Ok(_child) => {
                    tracing::info!(target: LOG_TARGET, "Lock screen browser launched ({}) at {}", browser, url);
                    return;
                }
                Err(_) => continue,
            }
        }
        tracing::error!(target: LOG_TARGET, "Lock screen: no browser found. Install chromium or microsoft-edge.");
    }

    #[cfg(windows)]
    pub fn close_browser(&mut self) {
        if let Some(ref mut child) = self.browser_process {
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!(target: LOG_TARGET, "Lock screen browser closed (child handle)");
        }
        self.browser_process = None;

        // BWDOG-04: Skip taskkill during protected game sessions (anti-cheat safe mode).
        // The child handle kill above always runs (it only kills our own process).
        if self.safe_mode_active.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!(target: LOG_TARGET, "Safe mode active — skipping taskkill for Edge/WebView2 processes");
            return;
        }

        // Kill ALL Edge and Edge WebView2 processes aggressively via taskkill.
        // On gaming pods, there should be no user Edge sessions — only our kiosk windows.
        // This prevents the stacking bug where repeated show_blank_screen() calls
        // spawn new Edge windows without fully cleaning up the old ones.
        for exe in &["msedge.exe", "msedgewebview2.exe"] {
            match hidden_cmd("taskkill")
                .args(["/F", "/IM", exe])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                Ok(status) => {
                    if status.success() {
                        tracing::info!(target: LOG_TARGET, "Killed all {} processes", exe);
                    }
                }
                Err(e) => tracing::warn!(target: LOG_TARGET, "Failed to run taskkill for {}: {}", exe, e),
            }
        }
        // Brief pause to let processes fully exit and release ports
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[cfg(not(windows))]
    pub fn close_browser(&mut self) {
        // Kill any kiosk browser we may have spawned
        let _ = std::process::Command::new("pkill").args(["-f", &format!("127.0.0.1:{}", self.port)]).spawn();
    }

    /// Check if the browser process we spawned is still running.
    /// Returns false if no browser was spawned or if the child process has exited.
    #[cfg(windows)]
    pub fn is_browser_alive(&mut self) -> bool {
        match self.browser_process.as_mut() {
            Some(child) => {
                // try_wait: Ok(Some(_)) = exited, Ok(None) = still running, Err = unknown
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        // Process has exited — clear the handle
                        tracing::warn!(target: LOG_TARGET, "Browser process has exited (watchdog detected)");
                        self.browser_process = None;
                        false
                    }
                    Ok(None) => true, // still running
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "Failed to check browser process status: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    #[cfg(not(windows))]
    pub fn is_browser_alive(&mut self) -> bool {
        false
    }

    /// Returns true when the lock screen is in a state where a browser is expected to be running.
    /// Used by browser watchdog to skip polling when no browser should be visible.
    /// Only Hidden means "no browser expected" — all other states (IdlePinEntry, ScreenBlanked,
    /// ConfigError, MaintenanceRequired, etc.) require a live browser.
    pub fn is_browser_expected(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        !matches!(*state, LockScreenState::Hidden)
    }

    /// Count running msedge.exe processes using tasklist.
    /// Returns 0 on non-Windows or if tasklist fails.
    /// Used by Plan 02 browser watchdog for stacking detection (BWDOG-02).
    #[cfg(windows)]
    pub fn count_edge_processes() -> usize {
        #[cfg(test)]
        { return 0; }

        #[cfg(not(test))]
        {
            match hidden_cmd("tasklist")
                .args(["/FI", "IMAGENAME eq msedge.exe", "/FO", "CSV", "/NH"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Each line with "msedge.exe" is one process
                    stdout.lines().filter(|l| l.contains("msedge.exe")).count()
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "Failed to count Edge processes: {}", e);
                    0
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn count_edge_processes() -> usize {
        0
    }
}

/// Suppress or restore Windows toast notifications and popups.
/// When `suppress=true`: enables Focus Assist (Do Not Disturb), kills notification center.
/// When `suppress=false`: restores normal notification behavior.
#[cfg(windows)]
fn suppress_notifications(suppress: bool) {
    if suppress {
        // Enable Focus Assist (priority only) via registry — suppresses all toast notifications
        let _ = hidden_cmd("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
                "/v", "NOC_GLOBAL_SETTING_TOASTS_ENABLED",
                "/t", "REG_DWORD",
                "/d", "0",
                "/f",
            ])
            .output();
        // Disable balloon notifications
        let _ = hidden_cmd("reg")
            .args([
                "add",
                r"HKCU\Software\Policies\Microsoft\Windows\Explorer",
                "/v", "DisableNotificationCenter",
                "/t", "REG_DWORD",
                "/d", "1",
                "/f",
            ])
            .output();
        // Kill any active notification toasts / action center
        let _ = hidden_cmd("powershell")
            .args(["-NoProfile", "-Command",
                "Get-Process -Name 'ShellExperienceHost' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue"])
            .output();
        tracing::info!(target: LOG_TARGET, "Notifications suppressed for blanking screen");
    } else {
        // Re-enable toast notifications
        let _ = hidden_cmd("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
                "/v", "NOC_GLOBAL_SETTING_TOASTS_ENABLED",
                "/t", "REG_DWORD",
                "/d", "1",
                "/f",
            ])
            .output();
        // Re-enable notification center
        let _ = hidden_cmd("reg")
            .args([
                "delete",
                r"HKCU\Software\Policies\Microsoft\Windows\Explorer",
                "/v", "DisableNotificationCenter",
                "/f",
            ])
            .output();
        tracing::info!(target: LOG_TARGET, "Notifications restored after blanking screen cleared");
    }
}

/// Force the kiosk Edge window to the foreground (above Conspit Link, Steam, etc.).
/// Standalone function so it can be called from spawned tasks without owning a LockScreenManager.
#[cfg(windows)]
pub fn enforce_kiosk_foreground() {
    let ps_script = r#"
        Add-Type -Name WinFG -Namespace NativeFG -MemberDefinition '
            [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
            [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
        '
        # Find Edge processes with "Racing Point" in the title (kiosk lock screen)
        Get-Process -Name msedge -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowTitle -like '*Racing Point*' -and $_.MainWindowHandle -ne [IntPtr]::Zero } |
            ForEach-Object {
                [NativeFG.WinFG]::ShowWindow($_.MainWindowHandle, 3) | Out-Null  # SW_MAXIMIZE
                [NativeFG.WinFG]::SetForegroundWindow($_.MainWindowHandle) | Out-Null
                Write-Output "Kiosk foreground: $($_.ProcessName) (PID $($_.Id))"
            }
    "#;
    let _ = hidden_cmd("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output();
}

#[cfg(not(windows))]
pub fn enforce_kiosk_foreground() {}

// ─── HTTP Server ─────────────────────────────────────────────────────────────

/// Minimal HTTP server bound to localhost only.
async fn serve_lock_screen(
    port: u16,
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
    wallpaper_url: Arc<Mutex<Option<String>>>,
) {
    // Use SO_REUSEADDR to bind even if port is in TIME_WAIT from previous Edge connections
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    let socket = match tokio::net::TcpSocket::new_v4() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Lock screen: failed to create socket: {}", e);
            return;
        }
    };
    let _ = socket.set_reuseaddr(true);
    if let Err(e) = socket.bind(addr) {
        tracing::error!(target: LOG_TARGET, "Lock screen server failed to bind port {}: {}", port, e);
        return;
    }
    let listener = match socket.listen(128) {
        Ok(l) => {
            tracing::info!(target: LOG_TARGET, "Lock screen server listening on http://127.0.0.1:{}", port);
            l
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "Lock screen server failed to listen on port {}: {}", port, e);
            return;
        }
    };
    serve_with_listener(listener, state, event_tx, wallpaper_url).await;
}

/// Accept loop shared between `serve_lock_screen` and `start_server_checked`.
async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
    wallpaper_url: Arc<Mutex<Option<String>>>,
) {
    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let state = state.clone();
        let event_tx = event_tx.clone();
        let wallpaper_url = wallpaper_url.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let request = String::from_utf8_lossy(&buf[..n]);
            let first_line = request.lines().next().unwrap_or("");

            // Handle favicon requests
            if first_line.contains("/favicon") {
                let resp = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            // GET /health — lock screen liveness endpoint for post-restart verification
            if first_line.contains("GET /health") {
                let current = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let body = health_response_body(&current);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            if first_line.starts_with("POST /pin") {
                // Parse PIN from URL-encoded form body
                let pin = request
                    .rfind("\r\n\r\n")
                    .map(|i| &request[i + 4..])
                    .and_then(|body| {
                        body.split('&')
                            .find(|p| p.starts_with("pin="))
                            .map(|p| p[4..].trim().to_string())
                    })
                    .unwrap_or_default();

                if pin.len() == 4 && pin.chars().all(|c| c.is_ascii_digit()) {
                    let _ = event_tx.send(LockScreenEvent::PinEntered { pin }).await;
                }

                let body = render_verifying_page();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            } else {
                // GET — serve current lock screen page
                let current = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let wallpaper = wallpaper_url.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let body = render_page(&current, wallpaper.as_deref());
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        });
    }
}

// ─── HTML Rendering ──────────────────────────────────────────────────────────

/// Public wrapper for debug server to render lock screen HTML.
/// Uses no wallpaper (None) since debug server doesn't have wallpaper state.
pub fn render_page_public(state: &LockScreenState) -> String {
    render_page(state, None)
}

fn render_page(state: &LockScreenState, wallpaper_url: Option<&str>) -> String {
    match state {
        LockScreenState::Hidden => render_idle_page(wallpaper_url),
        LockScreenState::PinEntry {
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            pin_error,
            ..
        } => render_pin_page(driver_name, pricing_tier_name, *allocated_seconds, pin_error.as_deref(), wallpaper_url),
        LockScreenState::QrDisplay {
            qr_payload,
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            ..
        } => render_qr_page(qr_payload, driver_name, pricing_tier_name, *allocated_seconds, wallpaper_url),
        LockScreenState::ActiveSession {
            driver_name,
            remaining_seconds,
            allocated_seconds,
        } => render_active_session_page(driver_name, *remaining_seconds, *allocated_seconds),
        LockScreenState::SessionSummary {
            driver_name,
            total_laps,
            best_lap_ms,
            driving_seconds,
            top_speed_kmh,
            race_position,
        } => render_session_summary_page(driver_name, *total_laps, *best_lap_ms, *driving_seconds, *top_speed_kmh, *race_position),
        LockScreenState::BetweenSessions {
            driver_name,
            total_laps,
            best_lap_ms,
            driving_seconds,
            wallet_balance_paise,
            current_split_number,
            total_splits,
        } => render_between_sessions_page(driver_name, *total_laps, *best_lap_ms, *driving_seconds, *wallet_balance_paise, *current_split_number, *total_splits),
        LockScreenState::AwaitingAssistance {
            driver_name,
            message,
        } => render_assistance_page(driver_name, message),
        LockScreenState::LaunchSplash {
            driver_name,
            message,
        } => render_launch_splash_page(driver_name, message),
        // ScreenBlanked: pure black, never gets wallpaper (BRAND-02)
        LockScreenState::ScreenBlanked => render_blank_page(),
        LockScreenState::Disconnected => render_disconnected_page(),
        LockScreenState::StartupConnecting => render_startup_connecting_page(),
        LockScreenState::ConfigError { .. } => render_config_error_page(),
        LockScreenState::Lockdown { message } => render_lockdown_page(message),
        LockScreenState::MaintenanceRequired { failures } => render_maintenance_required_page(failures),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page_shell(title: &str, content: &str) -> String {
    page_shell_with_bg(title, content, None)
}

fn render_blank_page() -> String {
    page_shell("Racing Point", BLANK_PIN_PAGE)
}

fn render_disconnected_page() -> String {
    page_shell(
        "Racing Point — Reconnecting",
        r#"<div style="text-align:center;padding-top:30vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2em;color:#E10600;margin-bottom:20px">CONNECTION LOST</div>
<div class="msg">Reconnecting to Race Control...</div>
<div style="margin-top:20px;font-size:0.9em;color:#5A5A5A">Your session will continue. Please wait.</div>
</div>
<script>setTimeout(function(){location.reload()},3000)</script>"#,
    )
}

fn render_startup_connecting_page() -> String {
    page_shell(
        "Racing Point -- Starting",
        r#"<div style="text-align:center;padding-top:30vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.2em;color:#E10600;letter-spacing:0.08em;margin-bottom:16px">RACING POINT</div>
<div style="font-size:1em;color:#888;margin-bottom:40px">Starting up...</div>
<div style="display:inline-block;width:48px;height:48px;border:4px solid #333;border-top-color:#E10600;border-radius:50%;animation:spin 0.9s linear infinite"></div>
</div>
<style>
@keyframes spin {
    0%   { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}
</style>
<script>setTimeout(function(){location.reload()},3000)</script>"#,
    )
}

fn render_launch_splash_page(driver_name: &str, _message: &str) -> String {
    let driver_display = html_escape(driver_name);
    let content = format!(r#"<div style="text-align:center;padding-top:15vh">
<div style="margin-bottom:24px">{logo}</div>
<div style="font-family:Enthocentric,sans-serif;font-size:2.8em;color:#E10600;letter-spacing:0.08em;margin-bottom:24px">PREPARING YOUR SESSION</div>
<div style="font-size:1.1em;color:#ccc;margin-bottom:8px">Welcome, <span style="color:#fff;font-weight:600">{driver}</span></div>
<div style="font-size:0.95em;color:#5A5A5A;margin-bottom:48px">Loading your race...</div>
<div style="display:inline-block;width:60px;height:60px;border:4px solid #333;border-top-color:#E10600;border-radius:50%;animation:spin 0.9s linear infinite"></div>
</div>
<style>
@keyframes spin {{
    0%   {{ transform: rotate(0deg); }}
    100% {{ transform: rotate(360deg); }}
}}
</style>"#, logo = RP_LOGO_SVG, driver = driver_display);
    page_shell("Racing Point -- Loading", &content)
}

fn render_config_error_page() -> String {
    page_shell(
        "Racing Point — Configuration Error",
        r#"<div style="text-align:center;padding-top:30vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.5em;color:#E10600;margin-bottom:20px">CONFIGURATION ERROR</div>
<div class="msg" style="font-size:1.2em;margin-bottom:30px">Configuration Error - contact staff</div>
<div style="margin-top:20px;font-size:0.9em;color:#5A5A5A">Please contact a member of staff to resolve this issue.</div>
</div>"#,
    )
}

fn render_lockdown_page(message: &str) -> String {
    let escaped = html_escape(message);
    page_shell(
        "Racing Point — Security",
        &format!(
            r#"<div style="text-align:center;padding-top:25vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.5em;color:#E10600;margin-bottom:20px;animation:pulse 2s infinite">SECURITY ALERT</div>
<div style="font-size:1.3em;color:#fff;margin-bottom:30px;max-width:600px;margin-left:auto;margin-right:auto">{}</div>
<div style="margin-top:40px;font-size:1.1em;color:#5A5A5A">Please contact a member of staff to continue.</div>
<div style="margin-top:10px;font-size:0.8em;color:#333">Enter employee PIN to unlock.</div>
</div>
<style>@keyframes pulse {{ 0%,100% {{ opacity:1 }} 50% {{ opacity:0.6 }} }}</style>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#,
            escaped
        ),
    )
}

fn render_maintenance_required_page(failures: &[String]) -> String {
    let failure_items: String = failures
        .iter()
        .map(|f| format!("<li style=\"margin-bottom:8px\">{}</li>", html_escape(f)))
        .collect::<Vec<_>>()
        .join("\n");
    page_shell(
        "Racing Point — Maintenance",
        &format!(
            r#"<div style="text-align:center;padding-top:20vh">
<div style="font-family:Enthocentric,sans-serif;font-size:2.5em;color:#E10600;margin-bottom:20px">MAINTENANCE REQUIRED</div>
<div style="font-size:1.2em;color:#fff;margin-bottom:24px;max-width:600px;margin-left:auto;margin-right:auto">Staff have been notified. This pod is temporarily unavailable.</div>
<ul style="text-align:left;display:inline-block;background:#222;border:1px solid #333;border-radius:8px;padding:20px 30px;margin-bottom:24px;color:#fff;font-size:1em">
{failure_items}
</ul>
<div style="margin-top:20px;font-size:0.9em;color:#5A5A5A">This pod will automatically recover once the issue is resolved.</div>
</div>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#,
            failure_items = failure_items,
        ),
    )
}

fn render_idle_page(wallpaper_url: Option<&str>) -> String {
    page_shell_with_bg(
        "Racing Point",
        r#"<div class="msg">Session not active — please see the front desk.</div>
<script>setTimeout(function(){location.reload()},5000)</script>"#,
        wallpaper_url,
    )
}

fn render_pin_page(driver_name: &str, pricing_tier_name: &str, allocated_seconds: u32, pin_error: Option<&str>, wallpaper_url: Option<&str>) -> String {
    // SESSION-02: If driver_name is empty AND allocated_seconds is 0, show the idle "Ready" screen.
    // This is the post-session idle state — pod is ready for next customer.
    if driver_name.is_empty() && allocated_seconds == 0 {
        let idle_content = r#"<div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;text-align:center;padding:40px;">
  <div style="font-family:'Montserrat',sans-serif;font-size:24px;font-weight:700;color:#FFFFFF;margin-bottom:16px;">Ready</div>
  <div style="font-family:'Montserrat',sans-serif;font-size:16px;font-weight:400;color:#CCCCCC;margin-bottom:12px;">Scan the QR code on this rig to begin your session</div>
  <div style="font-family:'Montserrat',sans-serif;font-size:14px;font-weight:400;color:#5A5A5A;">Or ask staff to assign a session</div>
</div>"#;
        return page_shell_with_bg("Ready - Racing Point", idle_content, wallpaper_url);
    }

    let minutes = allocated_seconds / 60;
    let error_html = if let Some(_err) = pin_error {
        format!(r#"<div style="color:#ff4444;font-size:18px;margin-bottom:16px;font-weight:bold">Invalid PIN — try again</div>"#)
    } else {
        String::new()
    };
    let content = PIN_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TIER_NAME}}", &html_escape(pricing_tier_name))
        .replace("{{MINUTES}}", &minutes.to_string())
        .replace("{{PIN_ERROR}}", &error_html);
    page_shell_with_bg("Enter PIN - Racing Point", &content, wallpaper_url)
}

fn render_qr_page(
    qr_payload: &str,
    driver_name: &str,
    pricing_tier_name: &str,
    allocated_seconds: u32,
    wallpaper_url: Option<&str>,
) -> String {
    let minutes = allocated_seconds / 60;
    let qr_svg = generate_qr_svg(qr_payload);
    let content = QR_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TIER_NAME}}", &html_escape(pricing_tier_name))
        .replace("{{MINUTES}}", &minutes.to_string())
        .replace("{{QR_SVG}}", &qr_svg);
    page_shell_with_bg("Scan QR - Racing Point", &content, wallpaper_url)
}

fn render_active_session_page(driver_name: &str, remaining_seconds: u32, allocated_seconds: u32) -> String {
    let mins = remaining_seconds / 60;
    let secs = remaining_seconds % 60;
    let progress = if allocated_seconds > 0 {
        ((allocated_seconds - remaining_seconds) as f64 / allocated_seconds as f64 * 100.0) as u32
    } else {
        0
    };
    let warning_class = if remaining_seconds <= 60 { "time-warning" } else { "" };
    let warning_level = if remaining_seconds <= 10 {
        "critical"
    } else if remaining_seconds <= 60 {
        "urgent"
    } else if remaining_seconds <= 300 {
        "caution"
    } else {
        "none"
    };
    let content = ACTIVE_SESSION_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{MINUTES}}", &format!("{:02}", mins))
        .replace("{{SECONDS}}", &format!("{:02}", secs))
        .replace("{{PROGRESS}}", &progress.to_string())
        .replace("{{WARNING_CLASS}}", warning_class)
        .replace("{{WARNING_LEVEL}}", warning_level)
        .replace("{{REMAINING}}", &remaining_seconds.to_string())
        .replace("{{ALLOCATED}}", &allocated_seconds.to_string());
    page_shell("Session Active - Racing Point", &content)
}

fn render_session_summary_page(
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    top_speed_kmh: Option<f32>,
    race_position: Option<u32>,
) -> String {
    let best_lap_display = match best_lap_ms {
        Some(ms) => {
            let mins = ms / 60000;
            let secs = (ms % 60000) / 1000;
            let millis = ms % 1000;
            format!("{}:{:02}.{:03}", mins, secs, millis)
        }
        None => "--:--.---".to_string(),
    };
    let session_mins = driving_seconds / 60;
    let session_secs = driving_seconds % 60;

    // Build optional stat cards
    let top_speed_card = match top_speed_kmh {
        Some(spd) if spd > 0.0 => format!(
            r#"<div class="stat-item">
            <div class="stat-value">{}</div>
            <div class="stat-label">Top Speed km/h</div>
        </div>"#,
            spd as u32  // truncate to integer (245.5 → 245)
        ),
        _ => String::new(),
    };

    let position_suffix = |pos: u32| match pos {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    let race_position_card = match race_position {
        Some(pos) => format!(
            r#"<div class="stat-item">
            <div class="stat-value">{}{}</div>
            <div class="stat-label">Race Position</div>
        </div>"#,
            pos,
            position_suffix(pos)
        ),
        None => String::new(),
    };

    let content = SESSION_SUMMARY_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TOTAL_LAPS}}", &total_laps.to_string())
        .replace("{{BEST_LAP}}", &best_lap_display)
        .replace("{{SESSION_MINS}}", &session_mins.to_string())
        .replace("{{SESSION_SECS}}", &format!("{:02}", session_secs))
        .replace("{{TOP_SPEED_CARD}}", &top_speed_card)
        .replace("{{RACE_POSITION_CARD}}", &race_position_card);
    page_shell("Session Complete - Racing Point", &content)
}

fn render_between_sessions_page(
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    wallet_balance_paise: i64,
    current_split_number: u32,
    total_splits: u32,
) -> String {
    let best_lap_display = match best_lap_ms {
        Some(ms) => {
            let mins = ms / 60000;
            let secs = (ms % 60000) / 1000;
            let millis = ms % 1000;
            format!("{}:{:02}.{:03}", mins, secs, millis)
        }
        None => "--:--.---".to_string(),
    };
    let session_mins = driving_seconds / 60;
    let session_secs = driving_seconds % 60;
    let balance_rupees = wallet_balance_paise as f64 / 100.0;
    let remaining_splits = total_splits.saturating_sub(current_split_number);

    let content = format!(
        r#"<div style="text-align:center;padding:40px 20px">
<div style="font-size:48px;margin-bottom:10px">&#127937;</div>
<h1 style="font-size:32px;margin:0 0 10px">Race {current} of {total} complete!</h1>
<p style="font-size:18px;color:#ccc;margin:0 0 20px">Great driving, {driver}!</p>
<div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:20px;max-width:600px;margin:20px auto">
<div style="background:#222;border-radius:12px;padding:20px">
<div style="font-size:36px;font-weight:700">{laps}</div>
<div style="font-size:14px;color:#999">Laps</div>
</div>
<div style="background:#222;border-radius:12px;padding:20px">
<div style="font-size:36px;font-weight:700">{best}</div>
<div style="font-size:14px;color:#999">Best Lap</div>
</div>
<div style="background:#222;border-radius:12px;padding:20px">
<div style="font-size:36px;font-weight:700">{mins}:{secs:02}</div>
<div style="font-size:14px;color:#999">Session Time</div>
</div>
</div>
<div style="background:#1a1a3a;border:2px solid #4444aa;border-radius:12px;padding:20px;max-width:400px;margin:20px auto">
<div style="font-size:14px;color:#93c5fd">Remaining Races</div>
<div style="font-size:42px;font-weight:700;color:#60a5fa">{remaining}</div>
</div>
<p style="font-size:20px;color:#ccc;margin-top:20px">Staff will set up your next race — sit tight!</p>
<p style="font-size:14px;color:#666;margin-top:30px">This pod will return to idle in 5 minutes if no new session is started.</p>
</div>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#,
        driver = html_escape(driver_name),
        current = current_split_number,
        total = total_splits,
        laps = total_laps,
        best = best_lap_display,
        mins = session_mins,
        secs = session_secs,
        remaining = remaining_splits,
    );
    page_shell("Pick Next Race - Racing Point", &content)
}

fn render_assistance_page(driver_name: &str, message: &str) -> String {
    let content = format!(
        r#"<div style="text-align:center;padding:60px 20px">
<div style="font-size:64px;margin-bottom:20px">&#128075;</div>
<h1 style="font-size:36px;margin:0 0 15px">Welcome, {driver}!</h1>
<div style="background:#3a2a00;border:2px solid #E10600;border-radius:12px;padding:30px;max-width:500px;margin:20px auto">
<div style="font-size:22px;color:#fbbf24;margin-bottom:10px">Staff Assistance Needed</div>
<p style="font-size:18px;color:#fff;margin:0">{msg}</p>
</div>
<p style="font-size:16px;color:#999;margin-top:30px">Please wait — a team member will be with you shortly.</p>
</div>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#,
        driver = html_escape(driver_name),
        msg = html_escape(message),
    );
    page_shell("Staff Assistance - Racing Point", &content)
}

fn render_verifying_page() -> String {
    page_shell(
        "Verifying - Racing Point",
        r#"<div class="msg">Verifying your PIN&hellip;</div>
<script>setTimeout(function(){location='/'},3000)</script>"#,
    )
}

fn generate_qr_svg(data: &str) -> String {
    use qrcode::render::svg;
    use qrcode::QrCode;

    match QrCode::new(data) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(250, 250)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(e) => {
            let msg: String = e.to_string();
            tracing::error!(target: LOG_TARGET, "QR code generation failed: {}", msg);
            format!(
                "<p style=\"color:#000;font-size:14px\">QR Error: {}</p>",
                html_escape(&msg)
            )
        }
    }
}

// ─── Health Check Helper ─────────────────────────────────────────────────────

/// Returns the JSON body for GET /health based on the current lock screen state.
///
/// Returns `{"status":"ok"}` when lock screen is actively showing something
/// (PinEntry, QrDisplay, ActiveSession, SessionSummary, BetweenSessions, AssistanceScreen).
/// Returns `{"status":"degraded"}` when Hidden, Disconnected, or ConfigError.
///
/// The HTTP 200 status itself signals "server is alive". This JSON body provides
/// extra state context for monitoring and future use.
pub fn health_response_body(state: &LockScreenState) -> String {
    let is_active = !matches!(
        state,
        LockScreenState::Hidden
            | LockScreenState::Disconnected
            | LockScreenState::StartupConnecting
            | LockScreenState::ConfigError { .. }
            | LockScreenState::MaintenanceRequired { .. }
    );
    let status_str = if is_active { "ok" } else { "degraded" };
    format!(r#"{{"status":"{}"}}"#, status_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_ok_for_pin_entry() {
        let state = LockScreenState::PinEntry {
            token_id: "tok-1".to_string(),
            driver_name: "Alonso".to_string(),
            pricing_tier_name: "30min".to_string(),
            allocated_seconds: 1800,
            pin_error: None,
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#);
    }

    #[test]
    fn health_ok_for_active_session() {
        let state = LockScreenState::ActiveSession {
            driver_name: "Alonso".to_string(),
            remaining_seconds: 900,
            allocated_seconds: 1800,
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#);
    }

    #[test]
    fn health_degraded_for_hidden() {
        let state = LockScreenState::Hidden;
        assert_eq!(health_response_body(&state), r#"{"status":"degraded"}"#);
    }

    #[test]
    fn health_degraded_for_disconnected() {
        let state = LockScreenState::Disconnected;
        assert_eq!(health_response_body(&state), r#"{"status":"degraded"}"#);
    }

    #[test]
    fn health_degraded_for_config_error() {
        let state = LockScreenState::ConfigError {
            message: "missing pod number".to_string(),
        };
        assert_eq!(health_response_body(&state), r#"{"status":"degraded"}"#);
    }

    #[test]
    fn health_ok_for_qr_display() {
        let state = LockScreenState::QrDisplay {
            token_id: "tok-2".to_string(),
            qr_payload: "https://racingpoint.in/auth/qr/tok-2".to_string(),
            driver_name: "Hamilton".to_string(),
            pricing_tier_name: "60min".to_string(),
            allocated_seconds: 3600,
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#);
    }

    #[test]
    fn launch_splash_renders_branded_html() {
        let state = LockScreenState::LaunchSplash {
            driver_name: "Verstappen".to_string(),
            message: "Preparing your session...".to_string(),
        };
        let html = render_page_public(&state);
        assert!(html.contains("PREPARING YOUR SESSION"), "LaunchSplash must contain 'PREPARING YOUR SESSION'");
        assert!(html.contains("#E10600"), "LaunchSplash must contain Racing Point red #E10600");
        assert!(!html.contains("C:\\\\"), "LaunchSplash must not contain Windows file paths (C:\\\\)");
        assert!(!html.contains(".exe"), "LaunchSplash must not contain .exe references");
        assert!(!html.contains("\\\\Users\\\\"), "LaunchSplash must not contain \\\\Users\\\\ paths");
    }

    #[test]
    fn launch_splash_health_ok() {
        let state = LockScreenState::LaunchSplash {
            driver_name: "Leclerc".to_string(),
            message: "Preparing your session...".to_string(),
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#);
    }

    #[test]
    fn launch_splash_not_blanked() {
        let state = LockScreenState::LaunchSplash {
            driver_name: "Norris".to_string(),
            message: "Preparing your session...".to_string(),
        };
        // LaunchSplash is an active customer-facing state — not blanked, not idle
        // We verify this by checking that render_page_public does not return the blank page content
        let html = render_page_public(&state);
        // Blank page renders BLANK_PIN_PAGE which does not contain "PREPARING YOUR SESSION"
        assert!(html.contains("PREPARING YOUR SESSION"), "LaunchSplash is not blanked — must render splash content");
    }

    #[test]
    fn launch_splash_not_idle_or_blanked() {
        // is_idle_or_blanked only matches Hidden | ScreenBlanked | Disconnected
        // LaunchSplash is customer-facing — must NOT be treated as idle
        // Verify via health check: idle states return "degraded", active states return "ok"
        let state = LockScreenState::LaunchSplash {
            driver_name: "Sainz".to_string(),
            message: "Preparing your session...".to_string(),
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#,
            "LaunchSplash is not idle — health must be ok");
    }

    // ─── StartupConnecting tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn wait_for_self_ready_succeeds_when_port_open() {
        use tokio::net::TcpListener;

        // Bind a loopback listener on an ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();

        // Create manager on that port and confirm it returns quickly
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let mut manager = LockScreenManager { state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)), event_tx: tx, port, #[cfg(windows)] browser_process: None, wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)), safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) };

        let start = std::time::Instant::now();
        manager.wait_for_self_ready().await;
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 1, "wait_for_self_ready should succeed well under 1s when port is open, took {:?}", elapsed);
    }

    #[tokio::test]
    async fn wait_for_self_ready_timeout() {
        // Do NOT bind port 18922 — let it fail
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let mut manager = LockScreenManager { state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)), event_tx: tx, port: 18922, #[cfg(windows)] browser_process: None, wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)), safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) };

        let start = std::time::Instant::now();
        // Should return (not panic) within ~6 seconds
        manager.wait_for_self_ready().await;
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() <= 6, "wait_for_self_ready must return within 6s on timeout, took {:?}", elapsed);
    }

    #[test]
    fn startup_connecting_renders_branded_html() {
        let state = LockScreenState::StartupConnecting;
        let html = render_page_public(&state);
        assert!(html.contains("RACING POINT"), "StartupConnecting must contain 'RACING POINT'");
        assert!(html.contains("#E10600"), "StartupConnecting must use Racing Point red #E10600");
    }

    #[test]
    fn startup_connecting_has_reload_script() {
        let state = LockScreenState::StartupConnecting;
        let html = render_page_public(&state);
        assert!(html.contains("location.reload"), "StartupConnecting must include JS reload");
        assert!(html.contains("3000"), "StartupConnecting must reload every 3 seconds");
    }

    #[test]
    fn startup_connecting_is_idle_or_blanked() {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::StartupConnecting)),
            event_tx: tx,
            port: 18923,
            #[cfg(windows)]
            browser_process: None,
            wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        assert!(manager.is_idle_or_blanked(), "StartupConnecting must be treated as idle (pod not ready for customers)");
    }

    #[test]
    fn health_degraded_for_startup_connecting() {
        let state = LockScreenState::StartupConnecting;
        assert_eq!(
            health_response_body(&state),
            r#"{"status":"degraded"}"#,
            "StartupConnecting is a startup/waiting state — health must be degraded"
        );
    }

    // ─── BRAND-01: Logo in page shell ────────────────────────────────────────

    // RED: will pass after Task 2 implementation (PAGE_SHELL must contain inline SVG logo)
    #[test]
    fn logo_in_page_shell() {
        let state = LockScreenState::Hidden;
        let html = render_page_public(&state);
        assert!(
            html.contains("<svg") && (html.contains("RP_LOGO") || html.contains("Racing Point") || html.contains("racingpoint")),
            "PAGE_SHELL must contain an inline SVG Racing Point logo (found <svg> with brand content)"
        );
        // More specifically: the SVG element must be present
        assert!(
            html.contains("<svg"),
            "PAGE_SHELL must contain an inline SVG logo element"
        );
    }

    // ─── BRAND-03: Logo in launch splash ─────────────────────────────────────

    // RED: will pass after Task 2 implementation (LaunchSplash page must embed SVG logo)
    #[test]
    fn logo_in_launch_splash() {
        let state = LockScreenState::LaunchSplash {
            driver_name: "Hamilton".to_string(),
            message: "Preparing your session...".to_string(),
        };
        let html = render_page_public(&state);
        assert!(
            html.contains("<svg"),
            "LaunchSplash must contain an inline SVG Racing Point logo element"
        );
    }

    // ─── BRAND-02: Wallpaper URL renders as CSS background-image ─────────────

    // RED: will pass after Task 2 implementation (page_shell_with_bg injects background-image)
    #[test]
    fn wallpaper_url_renders_in_css() {
        let html = page_shell_with_bg(
            "Test",
            "<p>content</p>",
            Some("http://example.com/bg.jpg"),
        );
        assert!(
            html.contains("background-image"),
            "page_shell_with_bg with Some(url) must inject background-image CSS"
        );
        assert!(
            html.contains("http://example.com/bg.jpg"),
            "page_shell_with_bg must embed the wallpaper URL in the CSS"
        );
    }

    // RED: will pass after Task 2 implementation (None wallpaper uses default gradient)
    #[test]
    fn wallpaper_empty_uses_default_bg() {
        let html = page_shell_with_bg("Test", "<p>content</p>", None);
        assert!(
            !html.contains("background-image"),
            "page_shell_with_bg with None must NOT inject background-image CSS"
        );
        assert!(
            html.contains("linear-gradient"),
            "page_shell_with_bg with None must use the default gradient background"
        );
    }

    // ─── BRAND-02: Wallpaper NOT applied to blank screen ─────────────────────

    // The blank page uses page_shell() directly (not page_shell_with_bg), so no wallpaper injection.
    // This test verifies the EXISTING blank page does NOT get wallpaper even if we call page_shell_with_bg
    // with Some(url) and then render_blank_page separately — they are independent.
    // Note: render_blank_page() always calls page_shell() without wallpaper — this is already correct.
    #[test]
    fn wallpaper_not_on_blank_page() {
        // render_blank_page uses page_shell() directly — must never have background-image from wallpaper
        let state = LockScreenState::ScreenBlanked;
        let html = render_page_public(&state);
        assert!(
            !html.contains("background-image"),
            "ScreenBlanked page must NOT contain background-image CSS (wallpaper not applied)"
        );
    }

    // ─── SESS-01: Session summary shows top speed ─────────────────────────────

    // RED: will pass after Task 2 implementation (render_session_summary_page_full renders top speed card)
    #[test]
    fn session_summary_shows_top_speed() {
        let html = render_session_summary_page_full(
            "Verstappen", 15, Some(89500), 3600,
            Some(245.5),  // top_speed_kmh
            None,          // race_position
        );
        assert!(
            html.contains("245") || html.contains("245."),
            "Session summary must show top speed value (245.5 → '245')"
        );
        assert!(
            html.contains("Top Speed") || html.contains("km/h"),
            "Session summary must show 'Top Speed' label or 'km/h' unit"
        );
    }

    // Stub should pass for zero-speed: stub doesn't show the card at all (no "Top Speed" text).
    // Task 2: must not show a top speed card when speed is 0.
    #[test]
    fn session_summary_hides_top_speed_when_zero() {
        let html = render_session_summary_page_full(
            "Norris", 5, None, 900,
            Some(0.0),  // top_speed_kmh = 0 → should be hidden
            None,
        );
        // When top_speed is 0.0, no top speed card should appear
        // (stub returns same as render_session_summary_page which has no Top Speed)
        assert!(
            !html.contains("Top Speed"),
            "Session summary must NOT show Top Speed card when speed is 0.0"
        );
    }

    // ─── SESS-02: Session summary shows race position ─────────────────────────

    // RED: will pass after Task 2 implementation (render_session_summary_page_full renders position card)
    #[test]
    fn session_summary_shows_race_position() {
        let html = render_session_summary_page_full(
            "Leclerc", 10, Some(95000), 2400,
            None,
            Some(3),  // race_position = 3rd
        );
        assert!(
            html.contains("3rd") || html.contains("Position"),
            "Session summary must show race position ('3rd') or 'Position' label"
        );
    }

    // RED: will pass after Task 2 (None race_position → no Position card)
    #[test]
    fn session_summary_hides_position_when_none() {
        let html = render_session_summary_page_full(
            "Sainz", 8, None, 1800,
            None,
            None,  // race_position = None → no card
        );
        assert!(
            !html.contains("Race Position") && !html.contains("Position"),
            "Session summary must NOT show Position stat card when race_position is None"
        );
    }

    // ─── SESS-03: Session summary must NOT auto-reload ────────────────────────

    // RED: will pass after Task 2 removes `setTimeout(function(){location.reload()},15000)` from SESSION_SUMMARY_PAGE
    #[test]
    fn session_summary_no_auto_reload() {
        let state = LockScreenState::SessionSummary {
            driver_name: "Alonso".to_string(),
            total_laps: 20,
            best_lap_ms: Some(92000),
            driving_seconds: 3600,
            top_speed_kmh: None,
            race_position: None,
        };
        let html = render_page_public(&state);
        assert!(
            !html.contains("location.reload"),
            "Session summary page must NOT contain location.reload — results stay on screen indefinitely (SESS-03)"
        );
    }

    // ─── Phase 49 Plan 01: Idle PinEntry (SESSION-02) ─────────────────────────

    #[test]
    fn idle_pin_entry_state_has_empty_fields() {
        // show_idle_pin_entry() sets PinEntry with empty token_id, driver_name, pricing_tier_name, 0 allocated_seconds
        let state = LockScreenState::PinEntry {
            token_id: String::new(),
            driver_name: String::new(),
            pricing_tier_name: String::new(),
            allocated_seconds: 0,
            pin_error: None,
        };
        if let LockScreenState::PinEntry { token_id, driver_name, allocated_seconds, .. } = &state {
            assert!(token_id.is_empty(), "idle PinEntry token_id must be empty");
            assert!(driver_name.is_empty(), "idle PinEntry driver_name must be empty");
            assert_eq!(*allocated_seconds, 0, "idle PinEntry allocated_seconds must be 0");
        } else {
            panic!("Expected PinEntry state");
        }
    }

    #[test]
    fn idle_pin_entry_health_ok() {
        // PinEntry state (even idle) returns health "ok" — pod is functional
        let state = LockScreenState::PinEntry {
            token_id: String::new(),
            driver_name: String::new(),
            pricing_tier_name: String::new(),
            allocated_seconds: 0,
            pin_error: None,
        };
        assert_eq!(health_response_body(&state), r#"{"status":"ok"}"#,
            "Idle PinEntry state must return health 'ok'");
    }

    #[test]
    fn idle_pin_entry_renders_ready_heading() {
        // When driver_name is empty and allocated_seconds=0, render_pin_page shows "Ready" heading
        let state = LockScreenState::PinEntry {
            token_id: String::new(),
            driver_name: String::new(),
            pricing_tier_name: String::new(),
            allocated_seconds: 0,
            pin_error: None,
        };
        let html = render_page_public(&state);
        assert!(html.contains("Ready"),
            "Idle PinEntry must contain 'Ready' heading, got: {}", &html[..html.len().min(500)]);
        assert!(html.contains("Scan the QR code") || html.contains("QR"),
            "Idle PinEntry must mention QR code scanning");
    }

    // ─── Centering: all page states must have proper centering CSS ────────

    /// Verify PAGE_SHELL body has all required centering properties.
    #[test]
    fn page_shell_has_centering_css() {
        let html = render_page_public(&LockScreenState::Hidden);
        assert!(html.contains("justify-content: center") || html.contains("justify-content:center"),
            "body must have justify-content: center for vertical centering");
        assert!(html.contains("align-items: center") || html.contains("align-items:center"),
            "body must have align-items: center for horizontal centering");
        assert!(html.contains("text-align: center") || html.contains("text-align:center"),
            "body must have text-align: center for text centering");
    }

    /// Verify all page states produce HTML with centering CSS (no state bypasses the shell).
    #[test]
    fn all_states_have_centering_css() {
        let states: Vec<(&str, LockScreenState)> = vec![
            ("Hidden", LockScreenState::Hidden),
            ("ScreenBlanked", LockScreenState::ScreenBlanked),
            ("Disconnected", LockScreenState::Disconnected),
            ("StartupConnecting", LockScreenState::StartupConnecting),
            ("ConfigError", LockScreenState::ConfigError { message: "test".to_string() }),
            ("Lockdown", LockScreenState::Lockdown { message: "test".to_string() }),
            ("PinEntry", LockScreenState::PinEntry {
                token_id: String::new(),
                driver_name: "Test".to_string(),
                pricing_tier_name: "30min".to_string(),
                allocated_seconds: 1800,
                pin_error: None,
            }),
            ("LaunchSplash", LockScreenState::LaunchSplash {
                driver_name: "Test".to_string(),
                message: "Loading...".to_string(),
            }),
        ];
        for (name, state) in states {
            let html = render_page_public(&state);
            assert!(html.contains("justify-content: center") || html.contains("justify-content:center"),
                "{} page missing justify-content: center", name);
            assert!(html.contains("align-items: center") || html.contains("align-items:center"),
                "{} page missing align-items: center", name);
            assert!(html.contains("text-align: center") || html.contains("text-align:center"),
                "{} page missing text-align: center", name);
        }
    }

    /// Verify .pin-row has justify-content: center for pin box centering.
    #[test]
    fn pin_row_has_centering() {
        let html = render_page_public(&LockScreenState::ScreenBlanked);
        assert!(html.contains("justify-content: center") || html.contains("justify-content:center"),
            "pin-row must have justify-content: center");
    }

    // ─── PF-04 / PF-05: MaintenanceRequired lock screen tests ────────────────

    #[test]
    fn maintenance_required_renders_html() {
        let html = render_maintenance_required_page(&["HID missing".to_string()]);
        assert!(html.contains("MAINTENANCE REQUIRED"), "MaintenanceRequired page must contain 'MAINTENANCE REQUIRED'");
        assert!(html.contains("HID missing"), "MaintenanceRequired page must contain the failure string");
    }

    #[test]
    fn health_degraded_for_maintenance_required() {
        let state = LockScreenState::MaintenanceRequired { failures: vec!["HID device not found".to_string()] };
        assert_eq!(health_response_body(&state), r#"{"status":"degraded"}"#,
            "MaintenanceRequired is a blocked state — health must be degraded");
    }

    #[test]
    fn maintenance_required_is_idle_or_blanked() {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::MaintenanceRequired {
                failures: vec!["HID device not found".to_string()],
            })),
            event_tx: tx,
            port: 18924,
            #[cfg(windows)]
            browser_process: None,
            wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        assert!(manager.is_idle_or_blanked(), "MaintenanceRequired must be treated as idle (pod not serving customer)");
    }

    // ─── BWDOG-03/04: close_browser safe mode gating and count_edge_processes ─

    #[test]
    fn test_count_edge_processes_returns_zero_in_test() {
        assert_eq!(LockScreenManager::count_edge_processes(), 0);
    }

    #[test]
    fn test_close_browser_safe_mode_skips_taskkill() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let mut manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)),
            event_tx: tx,
            port: 18923,
            #[cfg(windows)]
            browser_process: None,
            wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        // Should not panic or hang — safe mode gate skips taskkill
        manager.close_browser();
    }

    #[test]
    fn test_close_browser_normal_mode() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let mut manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)),
            event_tx: tx,
            port: 18923,
            #[cfg(windows)]
            browser_process: None,
            wallpaper_url: std::sync::Arc::new(std::sync::Mutex::new(None)),
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        // Should not panic — with no browser_process and no real Edge running, this is a no-op
        manager.close_browser();
    }
}

/// Render session summary page with top speed and race position params.
/// Conditionally adds stat cards for top speed (when > 0) and race position (when Some).
pub fn render_session_summary_page_full(
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    top_speed_kmh: Option<f32>,
    race_position: Option<u32>,
) -> String {
    render_session_summary_page(driver_name, total_laps, best_lap_ms, driving_seconds, top_speed_kmh, race_position)
}

/// Page shell with optional wallpaper URL background (BRAND-02).
/// When wallpaper_url is Some(url) and not empty: injects CSS background-image over the default gradient.
/// When None or empty: uses the default gradient background.
/// NOTE: render_blank_page() uses page_shell() which passes None — ScreenBlanked never gets wallpaper.
pub fn page_shell_with_bg(title: &str, content: &str, wallpaper_url: Option<&str>) -> String {
    let wallpaper_style = match wallpaper_url {
        Some(url) if !url.is_empty() => {
            // Escape single quotes in URL to prevent CSS injection
            let safe_url = url.replace('\'', "%27");
            format!(
                "\n<style>\nbody {{ background-image: url('{}'); background-size: cover; background-position: center; }}\n</style>",
                safe_url
            )
        }
        _ => String::new(),
    };
    PAGE_SHELL
        .replace("{{RP_LOGO_SVG}}", RP_LOGO_SVG)
        .replace("{{TITLE}}", title)
        .replace("{{CONTENT}}", &format!("{}{}", content, wallpaper_style))
}

// ─── HTML Templates ──────────────────────────────────────────────────────────

/// Racing Point inline SVG logo — red wordmark with checkered flag accent (BRAND-01, BRAND-03).
/// Used in PAGE_SHELL and LaunchSplash to ensure consistent branding across all lock screen states.
const RP_LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 220 64" width="220" height="64" role="img" aria-label="Racing Point">
  <!-- Checkered flag accent -->
  <rect x="0" y="4" width="8" height="8" fill="#E10600"/>
  <rect x="8" y="4" width="8" height="8" fill="#ffffff" opacity="0.15"/>
  <rect x="0" y="12" width="8" height="8" fill="#ffffff" opacity="0.15"/>
  <rect x="8" y="12" width="8" height="8" fill="#E10600"/>
  <!-- Wordmark: RACING POINT -->
  <text x="24" y="36" font-family="Montserrat,Segoe UI,system-ui,sans-serif" font-weight="800" font-size="26" letter-spacing="4" fill="#E10600">RACING</text>
  <text x="24" y="58" font-family="Montserrat,Segoe UI,system-ui,sans-serif" font-weight="300" font-size="18" letter-spacing="6" fill="#ffffff" opacity="0.85">POINT</text>
</svg>"##;

const PAGE_SHELL: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{{TITLE}}</title>
<link href="https://fonts.googleapis.com/css2?family=Montserrat:wght@300;400;600;700;800&display=swap" rel="stylesheet">
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    background: linear-gradient(135deg, #1A1A1A 0%, #222222 50%, #1A1A1A 100%);
    color: #fff;
    font-family: 'Montserrat', 'Segoe UI', system-ui, sans-serif;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
}
.logo {
    margin-bottom: 2px;
    line-height: 0;
}
.logo svg {
    height: 64px;
    width: auto;
}
.tagline {
    font-size: 0.95em;
    color: #5A5A5A;
    letter-spacing: 3px;
    margin-bottom: 32px;
    text-transform: uppercase;
}
.welcome {
    font-size: 1.7em;
    font-weight: 300;
    margin-bottom: 6px;
}
.session-info {
    font-size: 1.15em;
    color: #888;
    margin-bottom: 40px;
}
.pin-row {
    display: flex;
    justify-content: center;
    gap: 16px;
    margin-bottom: 32px;
}
.pin-box {
    width: 72px;
    height: 92px;
    text-align: center;
    font-size: 2.6em;
    font-weight: 700;
    border: 2px solid #333333;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    outline: none;
    caret-color: #E10600;
    transition: border-color 0.2s, box-shadow 0.2s;
}
.pin-box:focus {
    border-color: #E10600;
    box-shadow: 0 0 0 3px rgba(230, 57, 70, 0.2);
}
.btn {
    padding: 16px 64px;
    font-size: 1.25em;
    font-weight: 600;
    background: #E10600;
    color: #fff;
    border: none;
    border-radius: 14px;
    cursor: pointer;
    letter-spacing: 1px;
    transition: background 0.2s, transform 0.1s;
}
.btn:hover { background: #C60500; }
.btn:active { transform: scale(0.97); }
.btn:disabled { background: #333333; color: #5A5A5A; cursor: default; transform: none; }
.hint {
    color: #5A5A5A;
    margin-top: 28px;
    font-size: 0.9em;
    letter-spacing: 0.5px;
}
.qr-box {
    background: #fff;
    padding: 28px;
    border-radius: 20px;
    margin-bottom: 32px;
    display: inline-block;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
.qr-box svg { display: block; }
.msg {
    font-size: 1.4em;
    color: #5A5A5A;
    font-weight: 300;
}
</style>
</head>
<body>
<div class="logo">{{RP_LOGO_SVG}}</div>
<div class="tagline">May the Fastest Win.</div>
{{CONTENT}}
</body>
</html>"#;

const PIN_PAGE: &str = r#"<div class="welcome">Welcome, {{DRIVER_NAME}}!</div>
<div class="session-info">{{TIER_NAME}} &mdash; {{MINUTES}} minutes</div>
{{PIN_ERROR}}
<form method="POST" action="/pin" id="pinForm">
  <div class="pin-row">
    <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric" autofocus>
    <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric">
    <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric">
    <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric">
  </div>
  <input type="hidden" name="pin" id="pinValue">
  <button class="btn" type="submit" id="submitBtn" disabled>START SESSION</button>
</form>
<div class="hint">Enter the 4-digit PIN from your receipt. Need help? Ask at reception.</div>
<script>
(function() {
    var boxes = document.querySelectorAll('.pin-box');
    var hidden = document.getElementById('pinValue');
    var btn = document.getElementById('submitBtn');

    function update() {
        var p = '';
        for (var i = 0; i < boxes.length; i++) p += boxes[i].value;
        hidden.value = p;
        btn.disabled = p.length !== 4;
    }

    for (var i = 0; i < boxes.length; i++) {
        (function(idx) {
            boxes[idx].addEventListener('input', function() {
                this.value = this.value.replace(/\D/g, '').slice(-1);
                update();
                if (this.value && idx < 3) boxes[idx + 1].focus();
            });
            boxes[idx].addEventListener('keydown', function(e) {
                if (e.key === 'Backspace' && !this.value && idx > 0) {
                    boxes[idx - 1].focus();
                    boxes[idx - 1].value = '';
                    update();
                }
            });
        })(i);
    }
})();
</script>"#;

const BLANK_PIN_PAGE: &str = r#"<style>
.blank-pin-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
}
.blank-pin-wrap .welcome {
    font-size: 1.4em;
    font-weight: 400;
    margin-bottom: 8px;
    color: #aaa;
}
.numpad {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    max-width: 340px;
    width: 100%;
    margin-top: 24px;
}
.numpad button {
    height: 72px;
    font-size: 2em;
    font-weight: 700;
    font-family: 'Montserrat', sans-serif;
    border: 2px solid #333;
    border-radius: 14px;
    background: rgba(255,255,255,0.05);
    color: #fff;
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
    -webkit-tap-highlight-color: transparent;
}
.numpad button:active {
    background: rgba(225,6,0,0.15);
    border-color: #E10600;
}
.numpad .fn-key {
    font-size: 0.9em;
    font-weight: 600;
    color: #888;
}
</style>
<div class="blank-pin-wrap">
  <div class="welcome">Have a PIN? Enter it below</div>
  <form method="POST" action="/pin" id="pinForm">
    <div class="pin-row">
      <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric" readonly>
      <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric" readonly>
      <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric" readonly>
      <input class="pin-box" type="tel" maxlength="1" pattern="[0-9]" inputmode="numeric" readonly>
    </div>
    <input type="hidden" name="pin" id="pinValue">
  </form>
  <div class="numpad" id="numpad">
    <button type="button" data-digit="1">1</button>
    <button type="button" data-digit="2">2</button>
    <button type="button" data-digit="3">3</button>
    <button type="button" data-digit="4">4</button>
    <button type="button" data-digit="5">5</button>
    <button type="button" data-digit="6">6</button>
    <button type="button" data-digit="7">7</button>
    <button type="button" data-digit="8">8</button>
    <button type="button" data-digit="9">9</button>
    <button type="button" class="fn-key" id="clearBtn">CLEAR</button>
    <button type="button" data-digit="0">0</button>
    <button type="button" class="fn-key" id="bkspBtn">&larr;</button>
  </div>
  <div class="hint">Book on the Racing Point app to get your PIN</div>
</div>
<script>
(function() {
    var boxes = document.querySelectorAll('.pin-box');
    var hidden = document.getElementById('pinValue');
    var pos = 0;

    function setDigit(d) {
        if (pos >= 4) return;
        boxes[pos].value = d;
        pos++;
        updateHidden();
        if (pos === 4) {
            setTimeout(function(){ document.getElementById('pinForm').submit(); }, 150);
        }
    }

    function backspace() {
        if (pos <= 0) return;
        pos--;
        boxes[pos].value = '';
        updateHidden();
    }

    function clearAll() {
        for (var i = 0; i < 4; i++) boxes[i].value = '';
        pos = 0;
        updateHidden();
    }

    function updateHidden() {
        var p = '';
        for (var i = 0; i < boxes.length; i++) p += boxes[i].value;
        hidden.value = p;
    }

    document.getElementById('numpad').addEventListener('click', function(e) {
        var btn = e.target.closest('button');
        if (!btn) return;
        var digit = btn.getAttribute('data-digit');
        if (digit !== null) setDigit(digit);
    });
    document.getElementById('clearBtn').addEventListener('click', clearAll);
    document.getElementById('bkspBtn').addEventListener('click', backspace);

    // Auto-reload every 3s to pick up state changes (e.g. session start)
    setTimeout(function(){ location.reload(); }, 3000);
})();
</script>"#;

const QR_PAGE: &str = r#"<div class="welcome">Welcome, {{DRIVER_NAME}}!</div>
<div class="session-info">{{TIER_NAME}} &mdash; {{MINUTES}} minutes</div>
<div class="qr-box">{{QR_SVG}}</div>
<div class="hint">Scan the QR code with your phone to start your session</div>
<script>setTimeout(function(){location.reload()},5000)</script>"#;

const ACTIVE_SESSION_PAGE: &str = r#"<style>
.timer-display {
    font-size: 8em;
    font-weight: 800;
    letter-spacing: 4px;
    margin: 20px 0;
    font-variant-numeric: tabular-nums;
}
.timer-display .colon {
    animation: blink 1s step-end infinite;
}
@keyframes blink {
    50% { opacity: 0.3; }
}
.progress-container {
    width: 80%;
    max-width: 600px;
    height: 12px;
    background: #333333;
    border-radius: 6px;
    overflow: hidden;
    margin: 30px auto;
}
.progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #E10600 0%, #FF3B30 100%);
    border-radius: 6px;
    transition: width 1s linear;
}
.session-label {
    font-size: 1.3em;
    color: #888;
    margin-bottom: 10px;
    text-transform: uppercase;
    letter-spacing: 3px;
}
.time-warning {
    color: #E10600;
}
/* Warning overlays */
.warning-overlay {
    display: none;
    position: fixed;
    top: 0; left: 0; right: 0;
    padding: 16px;
    text-align: center;
    font-weight: 700;
    font-size: 1.4em;
    letter-spacing: 2px;
    z-index: 100;
}
.warning-caution .warning-overlay {
    display: block;
    background: rgba(255, 165, 0, 0.15);
    color: #FFA500;
    border-bottom: 2px solid #FFA500;
}
.warning-urgent .warning-overlay {
    display: block;
    background: rgba(225, 6, 0, 0.2);
    color: #E10600;
    border-bottom: 2px solid #E10600;
    animation: pulse-bg 1s ease-in-out infinite;
}
.warning-critical .warning-overlay {
    display: block;
    background: rgba(225, 6, 0, 0.35);
    color: #fff;
    border-bottom: 3px solid #E10600;
    animation: pulse-bg 0.5s ease-in-out infinite;
    font-size: 2em;
}
.warning-urgent body, .warning-critical body {
    border: 3px solid #E10600;
}
@keyframes pulse-bg {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
}
</style>
<div id="warnWrap" class="warning-wrapper warning-{{WARNING_LEVEL}}">
<div class="warning-overlay" id="warningBanner"></div>
<div class="welcome">{{DRIVER_NAME}}</div>
<div class="session-label">Time Remaining</div>
<div id="timerDisp" class="timer-display {{WARNING_CLASS}}"><span id="mm">{{MINUTES}}</span><span class="colon">:</span><span id="ss">{{SECONDS}}</span></div>
<div class="progress-container">
    <div class="progress-bar" id="progBar" style="width: {{PROGRESS}}%"></div>
</div>
<div class="hint">Enjoy your session! Need help? Ask at reception.</div>
</div>
<script>
(function(){
    var rem = {{REMAINING}};
    var alloc = {{ALLOCATED}};
    var mm = document.getElementById('mm');
    var ss = document.getElementById('ss');
    var bar = document.getElementById('progBar');
    var banner = document.getElementById('warningBanner');
    var wrap = document.getElementById('warnWrap');
    var disp = document.getElementById('timerDisp');

    function pad(n){ return n < 10 ? '0' + n : '' + n; }

    function update(){
        mm.textContent = pad(Math.floor(rem / 60));
        ss.textContent = pad(rem % 60);
        if (alloc > 0) bar.style.width = Math.round((alloc - rem) / alloc * 100) + '%';

        // Warning classes
        var wl = rem <= 10 ? 'critical' : rem <= 60 ? 'urgent' : rem <= 300 ? 'caution' : 'none';
        wrap.className = 'warning-wrapper warning-' + wl;
        disp.className = rem <= 60 ? 'timer-display time-warning' : 'timer-display';

        // Banner text
        if (rem <= 10) banner.textContent = rem + ' SECONDS';
        else if (rem <= 60) banner.textContent = 'LESS THAN 1 MINUTE REMAINING';
        else if (rem <= 300) banner.textContent = Math.ceil(rem / 60) + ' MINUTES REMAINING';
        else banner.textContent = '';
    }

    update();
    setInterval(function(){ if (rem > 0) rem--; update(); }, 1000);
    // Reload periodically to sync with server state (session end, etc.)
    setTimeout(function(){ location.reload(); }, 30000);
})();
</script>"#;

const SESSION_SUMMARY_PAGE: &str = r#"<style>
.summary-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid #333333;
    border-radius: 20px;
    padding: 40px 60px;
    margin: 20px 0;
    text-align: center;
}
.checkmark {
    font-size: 4em;
    color: #4CAF50;
    margin-bottom: 10px;
}
.stats-grid {
    display: flex;
    gap: 60px;
    justify-content: center;
    margin: 30px 0;
    flex-wrap: wrap;
}
.stat-item {
    text-align: center;
}
.stat-value {
    font-size: 3.5em;
    font-weight: 800;
    color: #E10600;
    font-variant-numeric: tabular-nums;
}
.stat-label {
    font-size: 0.95em;
    color: #5A5A5A;
    text-transform: uppercase;
    letter-spacing: 2px;
    margin-top: 8px;
}
.farewell {
    font-size: 1.1em;
    color: #5A5A5A;
    margin-top: 20px;
}
</style>
<div class="summary-card">
    <div class="checkmark">&#10003;</div>
    <div class="welcome">Great drive, {{DRIVER_NAME}}!</div>
    <div class="stats-grid">
        <div class="stat-item">
            <div class="stat-value">{{TOTAL_LAPS}}</div>
            <div class="stat-label">Laps</div>
        </div>
        <div class="stat-item">
            <div class="stat-value">{{BEST_LAP}}</div>
            <div class="stat-label">Best Lap</div>
        </div>
        <div class="stat-item">
            <div class="stat-value">{{SESSION_MINS}}m {{SESSION_SECS}}s</div>
            <div class="stat-label">Session Time</div>
        </div>
        {{TOP_SPEED_CARD}}
        {{RACE_POSITION_CARD}}
    </div>
    <div class="farewell">See you next time at Racing Point!</div>
</div>"#;
