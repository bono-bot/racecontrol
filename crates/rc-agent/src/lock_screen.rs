//! Lock screen UI for customer authentication on gaming PCs.
//!
//! Manages a native Win32 fullscreen window via the `native_lock` module.
//! The local HTTP server serves JSON-only endpoints for health, state,
//! and countdown-warning. All visual rendering is done via GDI painting
//! in `native_lock::painter`.

use std::sync::atomic::AtomicBool;
use rc_common::spawn_safe::spawn_safe;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const LOG_TARGET: &str = "lock-screen";

/// Query the virtual screen bounds (covers all monitors).
/// Returns (x, y, width, height) of the full virtual desktop.
/// On single-monitor setups this is typically (0, 0, 1920, 1080) or similar.
/// On multi-monitor / surround setups this covers the entire span
/// (e.g. triple 2560x1440 → (0, 0, 7680, 1440)).
#[cfg(windows)]
pub(crate) fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
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
        tracing::warn!("Virtual screen returned 0 — falling back to primary monitor {}x{}", pw, ph);
        (0, 0, pw, ph)
    } else {
        // NVIDIA Surround failure detection: if virtual screen is suspiciously small
        // (e.g. 1024x768) when we expect triple-wide (7680x1440), log a warning.
        // This doesn't fix Surround — only a reboot can — but it makes the issue visible.
        if w < 1920 || h < 1080 {
            tracing::warn!(
                "Virtual screen {}x{} is below minimum expected (1920x1080) — \
                 NVIDIA Surround may have failed. Blanking will only cover partial display. \
                 Fix: reboot the pod to restore Surround.",
                w, h
            );
        }
        (x, y, w, h)
    }
}

#[cfg(not(windows))]
pub(crate) fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
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
        #[allow(dead_code)]
        token_id: String,
        driver_name: String,
        pricing_tier_name: String,
        allocated_seconds: u32,
        pin_error: Option<String>,
    },
    /// QR code display screen.
    QrDisplay {
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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

/// BILL-02: Countdown warning level for the persistent overlay.
#[derive(Debug, Clone, PartialEq)]
pub enum CountdownWarningLevel {
    /// Yellow warning — 5 minutes remaining.
    Yellow,
    /// Red warning — 1 minute remaining.
    Red,
}

/// BILL-02: Active countdown warning state served via /countdown-warning endpoint.
#[derive(Debug, Clone)]
pub struct CountdownWarningState {
    pub remaining_secs: u32,
    pub level: CountdownWarningLevel,
}

/// Manages the lock screen lifecycle: state, HTTP server, and native Win32 window.
pub struct LockScreenManager {
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
    port: u16,
    #[cfg(windows)]
    native_window: Option<crate::native_lock::NativeLockScreen>,
    /// SAFE-06: gates Focus Assist registry writes during protected game sessions.
    /// Wired after AppState construction via wire_safe_mode().
    safe_mode_active: Arc<AtomicBool>,
    /// POS-01: When true, lock screen state is tracked but native window is never launched.
    /// Used on POS/auxiliary devices where Edge kiosk shows the billing page —
    /// launching the lock screen window would overlay and hide the billing UI.
    browser_disabled: bool,
    /// When false (default), the pod idles on the animated blank screen.
    /// When true, the pod idles on an empty PIN pad for customer self-service entry.
    /// Racing Point venue uses staff-initiated billing — leave false.
    customer_self_service_mode: bool,
    /// BILL-02: Current countdown warning state. Served via /countdown-warning endpoint.
    /// None = no warning displayed. The HTTP server reads this on each request.
    pub(crate) countdown_warning: Arc<Mutex<Option<CountdownWarningState>>>,
}

impl LockScreenManager {
    pub fn new(event_tx: mpsc::Sender<LockScreenEvent>) -> Self {
        Self {
            state: Arc::new(Mutex::new(LockScreenState::Hidden)),
            event_tx,
            port: 18923,
            #[cfg(windows)]
            native_window: None,
            countdown_warning: Arc::new(Mutex::new(None)),
            safe_mode_active: Arc::new(AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
        }
    }

    /// POS-01: Disable native window launch for auxiliary devices (POS, staff terminals).
    /// State transitions still work for health reporting, but no window is created.
    pub fn set_browser_disabled(&mut self, disabled: bool) {
        self.browser_disabled = disabled;
        if disabled {
            tracing::info!(target: LOG_TARGET, "Lock screen native window DISABLED (POS/auxiliary mode)");
        }
    }

    /// Set whether this pod uses customer self-service PIN entry for idle state.
    /// When false (default), `show_idle_state()` calls `show_blank_screen()`.
    /// When true, `show_idle_state()` calls `show_idle_pin_entry()`.
    /// Wire from AgentConfig.lock_screen.customer_self_service_mode in main.rs.
    pub fn set_customer_self_service_mode(&mut self, enabled: bool) {
        self.customer_self_service_mode = enabled;
        tracing::info!(
            target: LOG_TARGET,
            enabled,
            "customer_self_service_mode set — idle state will be {}",
            if enabled { "PIN pad" } else { "blank screen" }
        );
    }

    /// Show the correct idle state for this venue configuration.
    /// - `customer_self_service_mode = false` (default): animated blank screen.
    /// - `customer_self_service_mode = true`: empty PIN pad for customer self-service.
    /// All idle transitions should call this instead of `show_idle_pin_entry()` directly.
    pub fn show_idle_state(&mut self) {
        if self.customer_self_service_mode {
            self.show_idle_pin_entry();
        } else {
            self.show_blank_screen();
        }
    }

    /// Wire the shared safe mode flag from AppState into this LockScreenManager.
    /// Call once after AppState is constructed (main.rs, before the reconnect loop).
    pub fn wire_safe_mode(&mut self, flag: Arc<AtomicBool>) {
        self.safe_mode_active = flag;
    }

    /// BILL-02: Show a persistent countdown warning overlay on the customer's screen.
    pub fn show_countdown_warning(&self, remaining_secs: u32, level: &str) {
        let warning_level = match level {
            "red" => CountdownWarningLevel::Red,
            _ => CountdownWarningLevel::Yellow,
        };
        tracing::info!(
            target: LOG_TARGET,
            remaining_secs,
            level,
            "BILL-02: countdown_warning"
        );
        let mut w = self.countdown_warning.lock().unwrap_or_else(|e| e.into_inner());
        *w = Some(CountdownWarningState {
            remaining_secs,
            level: warning_level,
        });
    }

    /// BILL-02: Dismiss the countdown warning overlay (session ended or time extended).
    pub fn dismiss_countdown_warning(&self) {
        let mut w = self.countdown_warning.lock().unwrap_or_else(|e| e.into_inner());
        *w = None;
    }

    /// Start the local HTTP server for lock screen JSON endpoints (call once at startup).
    pub fn start_server(&self) {
        let state = self.state.clone();
        let port = self.port;
        let countdown_warning = self.countdown_warning.clone();
        tokio::spawn(async move {
            serve_lock_screen(port, state, countdown_warning).await;
        });
    }

    /// Start the lock screen HTTP server and return a oneshot receiver that
    /// resolves with Ok(port) on successful bind or Err(message) on failure.
    pub fn start_server_checked(&self) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let state = self.state.clone();
        let port = self.port;
        let countdown_warning = self.countdown_warning.clone();
        tokio::spawn(async move {
            let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port);
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
            serve_with_listener(listener, state, countdown_warning).await;
        });
        rx
    }

    /// Wait until the local HTTP server is ready to accept connections (port 18923 bound).
    #[allow(dead_code)]
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
    pub fn show_startup_connecting(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::StartupConnecting;
        }
        self.show_native_window();
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
        self.show_native_window();
    }

    /// Show the idle PinEntry screen — pod is ready for next customer.
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
        self.show_native_window();
    }

    /// Show PIN validation error on lock screen (wrong PIN feedback).
    pub fn show_pin_error(&self, reason: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let LockScreenState::PinEntry { ref mut pin_error, .. } = *state {
            *pin_error = Some(reason.to_string());
        }
        // The native window will pick up the error on next repaint
        #[cfg(windows)]
        if let Some(ref nw) = self.native_window {
            nw.request_repaint();
        }
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
        self.show_native_window();
    }

    /// Show the active session screen with countdown timer.
    /// Hides the lock screen window so the game is visible during gameplay.
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
        // Hide the native window so the game is visible
        self.hide_native_window();
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
        self.show_native_window();
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
        self.show_native_window();
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
        self.show_native_window();
    }

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
    pub fn show_launch_splash(&mut self, driver_name: String) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::LaunchSplash {
                driver_name,
                message: "Preparing your session...".to_string(),
            };
        }
        self.show_native_window();
    }

    /// Show a blank (black) screen — used between sessions when screen blanking is enabled.
    /// State is set to ScreenBlanked only AFTER the native window is confirmed alive.
    pub fn show_blank_screen(&mut self) {
        #[cfg(windows)]
        // ─── SAFE-06: skip Focus Assist registry write during safe mode ───
        if !self.safe_mode_active.load(std::sync::atomic::Ordering::Relaxed) {
            suppress_notifications(true);
        } else {
            tracing::info!(target: LOG_TARGET, "safe mode active — Focus Assist registry write deferred");
        }
        self.show_native_window();
        // Gate state change on native window actually being alive — prevents
        // "state=blanked but no window" when window creation fails.
        #[cfg(windows)]
        let window_alive = self.native_window.as_ref()
            .map(|nw| nw.is_alive())
            .unwrap_or(false);
        #[cfg(not(windows))]
        let window_alive = false;
        if window_alive || self.browser_disabled {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ScreenBlanked;
        } else {
            tracing::error!(target: LOG_TARGET,
                "show_blank_screen: native window failed to launch — state NOT set to ScreenBlanked");
        }
    }

    /// Show a branded configuration error screen.
    pub fn show_config_error(&mut self, _message: &str) {
        self.show_native_window();
        #[cfg(windows)]
        let window_alive = self.native_window.as_ref()
            .map(|nw| nw.is_alive())
            .unwrap_or(false);
        #[cfg(not(windows))]
        let window_alive = false;
        if window_alive || self.browser_disabled {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ConfigError {
                message: "Configuration Error - contact staff".to_string(),
            };
        } else {
            tracing::error!(target: LOG_TARGET,
                "show_config_error: native window failed to launch — state NOT set to ConfigError");
        }
    }

    /// Show disconnected state.
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
        self.show_native_window();
    }

    /// Show maintenance required screen — pre-flight checks failed.
    pub fn show_maintenance_required(&mut self, failures: Vec<String>) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::MaintenanceRequired { failures };
        }
        self.show_native_window();
    }

    /// Returns true if the lock screen is currently showing the MaintenanceRequired page.
    #[allow(dead_code)]
    pub fn is_maintenance_required(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*state, LockScreenState::MaintenanceRequired { .. })
    }

    pub fn clear(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::Hidden;
        }
        self.hide_native_window();
        #[cfg(windows)]
        suppress_notifications(false);
    }

    /// Returns true when the lock screen is in a state where the native window is expected.
    /// Used by the window watchdog to skip polling when no window should be visible.
    pub fn is_window_expected(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        !matches!(*state, LockScreenState::Hidden)
    }

    /// Check if the native lock screen window is still alive.
    #[cfg(windows)]
    pub fn is_window_alive(&self) -> bool {
        self.native_window.as_ref().map_or(false, |nw| nw.is_alive())
    }

    #[cfg(not(windows))]
    pub fn is_window_alive(&self) -> bool {
        false
    }

    // ─── Native Window Management ───────────────────────────────────────────

    /// Show the native Win32 lock screen window. If no window exists, spawns
    /// the window thread. If a window already exists, triggers a repaint.
    #[cfg(windows)]
    fn show_native_window(&mut self) {
        // Never launch native window during tests
        #[cfg(test)]
        { return; }

        // POS-01: auxiliary devices never launch the lock screen
        #[allow(unreachable_code)]
        if self.browser_disabled {
            return;
        }

        // If native window is already alive, bring it to foreground and repaint.
        // LAUNCH-FIX-4: Must call request_show() (WM_APP+3 = SW_SHOW + SetForegroundWindow)
        // in addition to repaint. After close_browser() → SW_HIDE, the HWND slot remains Some
        // (is_alive()=true) but the window is invisible. A repaint-only call paints a hidden
        // window — no-op. request_show() re-displays it before the repaint renders content.
        if self.native_window.as_ref().map_or(false, |nw| nw.is_alive()) {
            if let Some(ref nw) = self.native_window {
                nw.request_show();
                nw.request_repaint();
            }
            return;
        }

        // Create a new native Win32 lock screen window
        let mut nw = crate::native_lock::NativeLockScreen::new();
        nw.show(self.state.clone(), self.event_tx.clone());

        // Brief pause to let the window thread create the HWND
        std::thread::sleep(std::time::Duration::from_millis(100));

        let alive = nw.is_alive();
        tracing::info!(
            target: LOG_TARGET,
            "Native lock screen launched (alive={})",
            alive
        );
        self.native_window = Some(nw);
    }

    #[cfg(not(windows))]
    fn show_native_window(&mut self) {
        // No native window on non-Windows platforms
    }

    /// Hide the native lock screen window (keeps it alive for fast re-show).
    #[cfg(windows)]
    fn hide_native_window(&mut self) {
        if let Some(ref nw) = self.native_window {
            nw.hide();
            tracing::info!(target: LOG_TARGET, "Native lock screen hidden");
        }
    }

    #[cfg(not(windows))]
    fn hide_native_window(&mut self) {}

    // ─── Legacy API compatibility ───────────────────────────────────────────
    // These methods maintain the old public API so callers in event_loop.rs,
    // ai_debugger.rs etc. continue to work without changes.

    /// Show the native window (legacy name from Edge era).
    pub fn launch_browser(&mut self) {
        self.show_native_window();
    }

    /// Hide the native window (legacy name from Edge era).
    pub fn close_browser(&mut self) {
        self.hide_native_window();
    }

    /// Check if the native window is alive (legacy name from Edge era).
    pub fn is_browser_alive(&self) -> bool {
        self.is_window_alive()
    }

    /// Check if a window is expected (legacy name from Edge era).
    pub fn is_browser_expected(&self) -> bool {
        self.is_window_expected()
    }
}

/// Suppress or restore Windows toast notifications and popups.
#[cfg(windows)]
fn suppress_notifications(suppress: bool) {
    if suppress {
        let _ = spawn_safe("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
                "/v", "NOC_GLOBAL_SETTING_TOASTS_ENABLED",
                "/t", "REG_DWORD",
                "/d", "0",
                "/f",
            ])
            .output();
        let _ = spawn_safe("reg")
            .args([
                "add",
                r"HKCU\Software\Policies\Microsoft\Windows\Explorer",
                "/v", "DisableNotificationCenter",
                "/t", "REG_DWORD",
                "/d", "1",
                "/f",
            ])
            .output();
        let _ = spawn_safe("powershell")
            .args(["-NoProfile", "-Command",
                "Get-Process -Name 'ShellExperienceHost' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue"])
            .output();
        tracing::info!(target: LOG_TARGET, "Notifications suppressed for blanking screen");
    } else {
        let _ = spawn_safe("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings",
                "/v", "NOC_GLOBAL_SETTING_TOASTS_ENABLED",
                "/t", "REG_DWORD",
                "/d", "1",
                "/f",
            ])
            .output();
        let _ = spawn_safe("reg")
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

// ─── HTTP Server (JSON only) ────────────────────────────────────────────────

/// Minimal HTTP server bound to localhost only.
/// Serves JSON endpoints: /health, /state, /countdown-warning.
async fn serve_lock_screen(
    port: u16,
    state: Arc<Mutex<LockScreenState>>,
    countdown_warning: Arc<Mutex<Option<CountdownWarningState>>>,
) {
    let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port);
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
    serve_with_listener(listener, state, countdown_warning).await;
}

/// Accept loop shared between `serve_lock_screen` and `start_server_checked`.
async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    state: Arc<Mutex<LockScreenState>>,
    countdown_warning: Arc<Mutex<Option<CountdownWarningState>>>,
) {
    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let state = state.clone();
        let countdown_warning = countdown_warning.clone();

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

            // GET /health — lock screen liveness endpoint
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

            // GET /state — current state name
            if first_line.contains("GET /state") {
                let current = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let sn = state_name(&current);
                let body = format!(r#"{{"state":"{}"}}"#, sn);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            // GET /countdown-warning — BILL-02: countdown warning state as JSON
            if first_line.contains("GET /countdown-warning") {
                let warning = countdown_warning.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let body = match warning {
                    None => r#"{"active":false}"#.to_string(),
                    Some(w) => {
                        let level_str = match w.level {
                            CountdownWarningLevel::Yellow => "yellow",
                            CountdownWarningLevel::Red => "red",
                        };
                        format!(
                            r#"{{"active":true,"remaining_secs":{},"level":"{}"}}"#,
                            w.remaining_secs, level_str
                        )
                    }
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            // All other routes: redirect to /health
            let body = health_response_body(&state.lock().unwrap_or_else(|e| e.into_inner()).clone());
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        });
    }
}

// ─── Health Check Helper ─────────────────────────────────────────────────────

/// Short identifier for the current lock screen state.
pub fn state_name(state: &LockScreenState) -> &'static str {
    match state {
        LockScreenState::Hidden => "hidden",
        LockScreenState::ScreenBlanked => "blanked",
        LockScreenState::Disconnected => "disconnected",
        LockScreenState::StartupConnecting => "startup",
        LockScreenState::PinEntry { .. } => "pin",
        LockScreenState::QrDisplay { .. } => "qr",
        LockScreenState::ActiveSession { .. } => "active",
        LockScreenState::SessionSummary { .. } => "summary",
        LockScreenState::BetweenSessions { .. } => "between",
        LockScreenState::AwaitingAssistance { .. } => "assistance",
        LockScreenState::LaunchSplash { .. } => "splash",
        LockScreenState::ConfigError { .. } => "config_error",
        LockScreenState::Lockdown { .. } => "lockdown",
        LockScreenState::MaintenanceRequired { .. } => "maintenance",
    }
}

/// Returns the JSON body for GET /health based on the current lock screen state.
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
    let sn = state_name(state);
    format!(r#"{{"status":"{}","state":"{}"}}"#, status_str, sn)
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
        assert!(health_response_body(&state).contains(r#""status":"ok""#));
    }

    #[test]
    fn health_ok_for_active_session() {
        let state = LockScreenState::ActiveSession {
            driver_name: "Alonso".to_string(),
            remaining_seconds: 900,
            allocated_seconds: 1800,
        };
        assert!(health_response_body(&state).contains(r#""status":"ok""#));
    }

    #[test]
    fn health_degraded_for_hidden() {
        let state = LockScreenState::Hidden;
        assert!(health_response_body(&state).contains(r#""status":"degraded""#));
    }

    #[test]
    fn health_degraded_for_disconnected() {
        let state = LockScreenState::Disconnected;
        assert!(health_response_body(&state).contains(r#""status":"degraded""#));
    }

    #[test]
    fn health_degraded_for_config_error() {
        let state = LockScreenState::ConfigError {
            message: "missing pod number".to_string(),
        };
        assert!(health_response_body(&state).contains(r#""status":"degraded""#));
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
        assert!(health_response_body(&state).contains(r#""status":"ok""#));
    }

    #[test]
    fn launch_splash_health_ok() {
        let state = LockScreenState::LaunchSplash {
            driver_name: "Leclerc".to_string(),
            message: "Preparing your session...".to_string(),
        };
        assert!(health_response_body(&state).contains(r#""status":"ok""#));
    }

    // ─── StartupConnecting tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn wait_for_self_ready_succeeds_when_port_open() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();

        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let mut manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)),
            event_tx: tx,
            port,
            #[cfg(windows)]
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };

        let start = std::time::Instant::now();
        manager.wait_for_self_ready().await;
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 1, "wait_for_self_ready should succeed well under 1s when port is open, took {:?}", elapsed);
    }

    #[tokio::test]
    async fn wait_for_self_ready_timeout() {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let mut manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)),
            event_tx: tx,
            port: 18922,
            #[cfg(windows)]
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };

        let start = std::time::Instant::now();
        manager.wait_for_self_ready().await;
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() <= 6, "wait_for_self_ready must return within 6s on timeout, took {:?}", elapsed);
    }

    #[test]
    fn startup_connecting_is_idle_or_blanked() {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::StartupConnecting)),
            event_tx: tx,
            port: 18923,
            #[cfg(windows)]
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        assert!(manager.is_idle_or_blanked(), "StartupConnecting must be treated as idle");
    }

    #[test]
    fn health_degraded_for_startup_connecting() {
        let state = LockScreenState::StartupConnecting;
        assert!(
            health_response_body(&state).contains(r#""status":"degraded""#),
            "StartupConnecting is a startup/waiting state — health must be degraded"
        );
    }

    // ─── Phase 49 Plan 01: Idle PinEntry (SESSION-02) ─────────────────────────

    #[test]
    fn idle_pin_entry_state_has_empty_fields() {
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
        let state = LockScreenState::PinEntry {
            token_id: String::new(),
            driver_name: String::new(),
            pricing_tier_name: String::new(),
            allocated_seconds: 0,
            pin_error: None,
        };
        assert!(health_response_body(&state).contains(r#""status":"ok""#),
            "Idle PinEntry state must return health 'ok'");
    }

    // ─── PF-04 / PF-05: MaintenanceRequired lock screen tests ────────────────

    #[test]
    fn health_degraded_for_maintenance_required() {
        let state = LockScreenState::MaintenanceRequired { failures: vec!["HID device not found".to_string()] };
        assert!(health_response_body(&state).contains(r#""status":"degraded""#),
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
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        assert!(manager.is_idle_or_blanked(), "MaintenanceRequired must be treated as idle");
    }

    // ─── BILL-02: Countdown warning tests ────────────────────────────────────

    #[test]
    fn test_show_countdown_warning_stores_state() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let manager = LockScreenManager::new(tx);
        manager.show_countdown_warning(300, "yellow");
        let w = manager.countdown_warning.lock().unwrap();
        assert!(w.is_some());
        let state = w.as_ref().unwrap();
        assert_eq!(state.remaining_secs, 300);
        assert_eq!(state.level, CountdownWarningLevel::Yellow);
    }

    #[test]
    fn test_dismiss_countdown_warning_clears_state() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let manager = LockScreenManager::new(tx);
        manager.show_countdown_warning(60, "red");
        manager.dismiss_countdown_warning();
        let w = manager.countdown_warning.lock().unwrap();
        assert!(w.is_none(), "Dismissing warning must clear the state");
    }

    #[test]
    fn test_close_browser_safe_mode() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let mut manager = LockScreenManager {
            state: std::sync::Arc::new(std::sync::Mutex::new(LockScreenState::Hidden)),
            event_tx: tx,
            port: 18923,
            #[cfg(windows)]
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        // Should not panic — no native_window, this is a no-op
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
            native_window: None,
            safe_mode_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            browser_disabled: false,
            customer_self_service_mode: false,
            countdown_warning: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        manager.close_browser();
    }

    // ─── IDLE-01: show_idle_state() routing tests ────────────────────────────

    #[test]
    fn idle_state_blank_screen_when_self_service_disabled() {
        // Default config (customer_self_service_mode = false) → blank screen.
        // Use browser_disabled=true so show_blank_screen() sets ScreenBlanked state
        // even without a real native window (which doesn't exist in tests).
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let mut manager = LockScreenManager::new(tx);
        manager.set_browser_disabled(true); // no native window in tests
        // customer_self_service_mode defaults to false
        manager.show_idle_state();
        let state = manager.state.lock().unwrap();
        assert!(
            matches!(*state, LockScreenState::ScreenBlanked),
            "show_idle_state with customer_self_service_mode=false must yield ScreenBlanked, got {:?}",
            *state
        );
    }

    #[test]
    fn idle_state_pin_entry_when_self_service_enabled() {
        // Opt-in config (customer_self_service_mode = true) → PIN pad
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let mut manager = LockScreenManager::new(tx);
        manager.set_customer_self_service_mode(true);
        manager.show_idle_state();
        let state = manager.state.lock().unwrap();
        assert!(
            matches!(*state, LockScreenState::PinEntry { ref token_id, .. } if token_id.is_empty()),
            "show_idle_state with customer_self_service_mode=true must yield empty PinEntry, got {:?}",
            *state
        );
    }

    // ─── State name tests ────────────────────────────────────────────────────

    #[test]
    fn state_names_complete() {
        // Verify every variant has a unique state name (no wildcards)
        let states: Vec<(&str, LockScreenState)> = vec![
            ("hidden", LockScreenState::Hidden),
            ("blanked", LockScreenState::ScreenBlanked),
            ("disconnected", LockScreenState::Disconnected),
            ("startup", LockScreenState::StartupConnecting),
            ("pin", LockScreenState::PinEntry {
                token_id: String::new(),
                driver_name: String::new(),
                pricing_tier_name: String::new(),
                allocated_seconds: 0,
                pin_error: None,
            }),
            ("qr", LockScreenState::QrDisplay {
                token_id: String::new(),
                qr_payload: String::new(),
                driver_name: String::new(),
                pricing_tier_name: String::new(),
                allocated_seconds: 0,
            }),
            ("active", LockScreenState::ActiveSession {
                driver_name: String::new(),
                remaining_seconds: 0,
                allocated_seconds: 0,
            }),
            ("summary", LockScreenState::SessionSummary {
                driver_name: String::new(),
                total_laps: 0,
                best_lap_ms: None,
                driving_seconds: 0,
                top_speed_kmh: None,
                race_position: None,
            }),
            ("between", LockScreenState::BetweenSessions {
                driver_name: String::new(),
                total_laps: 0,
                best_lap_ms: None,
                driving_seconds: 0,
                wallet_balance_paise: 0,
                current_split_number: 0,
                total_splits: 0,
            }),
            ("assistance", LockScreenState::AwaitingAssistance {
                driver_name: String::new(),
                message: String::new(),
            }),
            ("splash", LockScreenState::LaunchSplash {
                driver_name: String::new(),
                message: String::new(),
            }),
            ("config_error", LockScreenState::ConfigError {
                message: String::new(),
            }),
            ("lockdown", LockScreenState::Lockdown {
                message: String::new(),
            }),
            ("maintenance", LockScreenState::MaintenanceRequired {
                failures: vec![],
            }),
        ];
        for (expected, state) in states {
            assert_eq!(state_name(&state), expected);
        }
    }
}
