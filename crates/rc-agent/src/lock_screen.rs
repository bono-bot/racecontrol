//! Lock screen UI for customer authentication on gaming PCs.
//!
//! Serves a fullscreen HTML page via a local HTTP server and launches
//! Edge in kiosk mode to display PIN entry or QR code screens.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

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
    /// Session ended — shows summary, auto-returns to idle.
    SessionSummary {
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
    },
    /// Between sessions — sub-session ended, customer can pick next race.
    BetweenSessions {
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
        wallet_balance_paise: i64,
    },
    /// Awaiting staff assistance (F1 25 or manual-launch games).
    AwaitingAssistance {
        driver_name: String,
        message: String,
    },
    /// Screen blanked — pure black screen between sessions.
    ScreenBlanked,
    /// Disconnected from core server — shown during reconnection attempts.
    Disconnected,
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
}

impl LockScreenManager {
    pub fn new(event_tx: mpsc::Sender<LockScreenEvent>) -> Self {
        Self {
            state: Arc::new(Mutex::new(LockScreenState::Hidden)),
            event_tx,
            port: 18923,
            #[cfg(windows)]
            browser_process: None,
        }
    }

    /// Start the local HTTP server for lock screen pages (call once at startup).
    pub fn start_server(&self) {
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let port = self.port;
        tokio::spawn(async move {
            serve_lock_screen(port, state, event_tx).await;
        });
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
            };
        }
        self.launch_browser();
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
        // Always relaunch browser to ensure blank/pin screen transitions immediately
        self.launch_browser();
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

    /// Show the session summary screen (auto-returns to idle after 15 seconds).
    pub fn show_session_summary(
        &mut self,
        driver_name: String,
        total_laps: u32,
        best_lap_ms: Option<u32>,
        driving_seconds: u32,
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::SessionSummary {
                driver_name,
                total_laps,
                best_lap_ms,
                driving_seconds,
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
    ) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::BetweenSessions {
                driver_name,
                total_laps,
                best_lap_ms,
                driving_seconds,
                wallet_balance_paise,
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
        matches!(*state, LockScreenState::Hidden | LockScreenState::ScreenBlanked)
    }

    /// Returns true if the screen is currently blanked.
    pub fn is_blanked(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*state, LockScreenState::ScreenBlanked)
    }

    /// Show a blank (black) screen — used between sessions when screen blanking is enabled.
    pub fn show_blank_screen(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::ScreenBlanked;
        }
        #[cfg(windows)]
        suppress_notifications(true);
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
    fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);
        // Try common Edge install paths, then fall back to PATH lookup
        let edge_paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            "msedge.exe",
        ];
        for edge_path in &edge_paths {
            match std::process::Command::new(edge_path)
                .args([
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
                ])
                .spawn()
            {
                Ok(child) => {
                    self.browser_process = Some(child);
                    tracing::info!("Lock screen browser launched at {} using {}", url, edge_path);
                    return;
                }
                Err(e) => {
                    tracing::warn!("Failed to launch Edge from {}: {}", edge_path, e);
                }
            }
        }
        tracing::error!("Could not launch Edge from any known path");
    }

    #[cfg(not(windows))]
    fn launch_browser(&mut self) {
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
                    tracing::info!("Lock screen browser launched ({}) at {}", browser, url);
                    return;
                }
                Err(_) => continue,
            }
        }
        tracing::error!("Lock screen: no browser found. Install chromium or microsoft-edge.");
    }

    #[cfg(windows)]
    fn close_browser(&mut self) {
        if let Some(ref mut child) = self.browser_process {
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!("Lock screen browser closed");
        }
        self.browser_process = None;
    }

    #[cfg(not(windows))]
    fn close_browser(&mut self) {
        // Kill any kiosk browser we may have spawned
        let _ = std::process::Command::new("pkill").args(["-f", &format!("127.0.0.1:{}", self.port)]).spawn();
    }
}

/// Suppress or restore Windows toast notifications and popups.
/// When `suppress=true`: enables Focus Assist (Do Not Disturb), kills notification center.
/// When `suppress=false`: restores normal notification behavior.
#[cfg(windows)]
fn suppress_notifications(suppress: bool) {
    if suppress {
        // Enable Focus Assist (priority only) via registry — suppresses all toast notifications
        let _ = std::process::Command::new("reg")
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
        let _ = std::process::Command::new("reg")
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
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-Process -Name 'ShellExperienceHost' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue"])
            .output();
        tracing::info!("Notifications suppressed for blanking screen");
    } else {
        // Re-enable toast notifications
        let _ = std::process::Command::new("reg")
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
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Policies\Microsoft\Windows\Explorer",
                "/v", "DisableNotificationCenter",
                "/f",
            ])
            .output();
        tracing::info!("Notifications restored after blanking screen cleared");
    }
}

// ─── HTTP Server ─────────────────────────────────────────────────────────────

/// Minimal HTTP server bound to localhost only.
async fn serve_lock_screen(
    port: u16,
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
) {
    // Use SO_REUSEADDR to bind even if port is in TIME_WAIT from previous Edge connections
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    let socket = match tokio::net::TcpSocket::new_v4() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Lock screen: failed to create socket: {}", e);
            return;
        }
    };
    let _ = socket.set_reuseaddr(true);
    if let Err(e) = socket.bind(addr) {
        tracing::error!("Lock screen server failed to bind port {}: {}", port, e);
        return;
    }
    let listener = match socket.listen(128) {
        Ok(l) => {
            tracing::info!("Lock screen server listening on http://127.0.0.1:{}", port);
            l
        }
        Err(e) => {
            tracing::error!("Lock screen server failed to listen on port {}: {}", port, e);
            return;
        }
    };

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let state = state.clone();
        let event_tx = event_tx.clone();

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
                let body = render_page(&current);
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
pub fn render_page_public(state: &LockScreenState) -> String {
    render_page(state)
}

fn render_page(state: &LockScreenState) -> String {
    match state {
        LockScreenState::Hidden => render_idle_page(),
        LockScreenState::PinEntry {
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            ..
        } => render_pin_page(driver_name, pricing_tier_name, *allocated_seconds),
        LockScreenState::QrDisplay {
            qr_payload,
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            ..
        } => render_qr_page(qr_payload, driver_name, pricing_tier_name, *allocated_seconds),
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
        } => render_session_summary_page(driver_name, *total_laps, *best_lap_ms, *driving_seconds),
        LockScreenState::BetweenSessions {
            driver_name,
            total_laps,
            best_lap_ms,
            driving_seconds,
            wallet_balance_paise,
        } => render_between_sessions_page(driver_name, *total_laps, *best_lap_ms, *driving_seconds, *wallet_balance_paise),
        LockScreenState::AwaitingAssistance {
            driver_name,
            message,
        } => render_assistance_page(driver_name, message),
        LockScreenState::ScreenBlanked => render_blank_page(),
        LockScreenState::Disconnected => render_disconnected_page(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page_shell(title: &str, content: &str) -> String {
    PAGE_SHELL
        .replace("{{TITLE}}", title)
        .replace("{{CONTENT}}", content)
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

fn render_idle_page() -> String {
    page_shell(
        "Racing Point",
        r#"<div class="msg">Session not active — please see the front desk.</div>
<script>setTimeout(function(){location.reload()},5000)</script>"#,
    )
}

fn render_pin_page(driver_name: &str, pricing_tier_name: &str, allocated_seconds: u32) -> String {
    let minutes = allocated_seconds / 60;
    let content = PIN_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TIER_NAME}}", &html_escape(pricing_tier_name))
        .replace("{{MINUTES}}", &minutes.to_string());
    page_shell("Enter PIN - Racing Point", &content)
}

fn render_qr_page(
    qr_payload: &str,
    driver_name: &str,
    pricing_tier_name: &str,
    allocated_seconds: u32,
) -> String {
    let minutes = allocated_seconds / 60;
    let qr_svg = generate_qr_svg(qr_payload);
    let content = QR_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TIER_NAME}}", &html_escape(pricing_tier_name))
        .replace("{{MINUTES}}", &minutes.to_string())
        .replace("{{QR_SVG}}", &qr_svg);
    page_shell("Scan QR - Racing Point", &content)
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
        .replace("{{REMAINING}}", &remaining_seconds.to_string());
    page_shell("Session Active - Racing Point", &content)
}

fn render_session_summary_page(
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
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
    let content = SESSION_SUMMARY_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{TOTAL_LAPS}}", &total_laps.to_string())
        .replace("{{BEST_LAP}}", &best_lap_display)
        .replace("{{SESSION_MINS}}", &session_mins.to_string())
        .replace("{{SESSION_SECS}}", &format!("{:02}", session_secs));
    page_shell("Session Complete - Racing Point", &content)
}

fn render_between_sessions_page(
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    wallet_balance_paise: i64,
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

    let content = format!(
        r#"<div style="text-align:center;padding:40px 20px">
<div style="font-size:48px;margin-bottom:10px">&#127937;</div>
<h1 style="font-size:32px;margin:0 0 10px">Great session, {driver}!</h1>
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
<div style="background:#1a3a1a;border:2px solid #2d6a2d;border-radius:12px;padding:20px;max-width:400px;margin:20px auto">
<div style="font-size:14px;color:#4ade80">Wallet Balance</div>
<div style="font-size:42px;font-weight:700;color:#4ade80">&#x20B9;{balance:.0}</div>
</div>
<p style="font-size:20px;color:#ccc;margin-top:20px">Open the app on your phone to pick your next race!</p>
<p style="font-size:14px;color:#666;margin-top:30px">This pod will return to idle in 5 minutes if no new session is started.</p>
</div>
<script>setTimeout(function(){{location.reload()}},5000)</script>"#,
        driver = html_escape(driver_name),
        laps = total_laps,
        best = best_lap_display,
        mins = session_mins,
        secs = session_secs,
        balance = balance_rupees,
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
            tracing::error!("QR code generation failed: {}", msg);
            format!(
                "<p style=\"color:#000;font-size:14px\">QR Error: {}</p>",
                html_escape(&msg)
            )
        }
    }
}

// ─── HTML Templates ──────────────────────────────────────────────────────────

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
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
}
.logo {
    font-size: 2.8em;
    font-weight: 800;
    letter-spacing: 6px;
    color: #E10600;
    margin-bottom: 2px;
}
.tagline {
    font-size: 0.95em;
    color: #5A5A5A;
    letter-spacing: 3px;
    margin-bottom: 50px;
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
<div class="logo">RACING POINT</div>
<div class="tagline">May the Fastest Win.</div>
{{CONTENT}}
</body>
</html>"#;

const PIN_PAGE: &str = r#"<div class="welcome">Welcome, {{DRIVER_NAME}}!</div>
<div class="session-info">{{TIER_NAME}} &mdash; {{MINUTES}} minutes</div>
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
<div class="warning-wrapper warning-{{WARNING_LEVEL}}">
<div class="warning-overlay" id="warningBanner"></div>
<div class="welcome">{{DRIVER_NAME}}</div>
<div class="session-label">Time Remaining</div>
<div class="timer-display {{WARNING_CLASS}}">{{MINUTES}}<span class="colon">:</span>{{SECONDS}}</div>
<div class="progress-container">
    <div class="progress-bar" style="width: {{PROGRESS}}%"></div>
</div>
<div class="hint">Enjoy your session! Need help? Ask at reception.</div>
</div>
<script>
(function(){
    var remaining = {{REMAINING}};
    var banner = document.getElementById('warningBanner');
    if (remaining <= 10) {
        banner.textContent = remaining + ' SECONDS';
    } else if (remaining <= 60) {
        banner.textContent = 'LESS THAN 1 MINUTE REMAINING';
    } else if (remaining <= 300) {
        banner.textContent = Math.ceil(remaining / 60) + ' MINUTES REMAINING';
    }
    setTimeout(function(){location.reload()},3000);
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
    </div>
    <div class="farewell">See you next time at Racing Point!</div>
</div>
<script>setTimeout(function(){location.reload()},15000)</script>"#;
