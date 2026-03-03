//! Lock screen UI for customer authentication on gaming PCs.
//!
//! Serves a fullscreen HTML page via a local HTTP server and launches
//! Edge in kiosk mode to display PIN entry or QR code screens.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        *state = LockScreenState::ActiveSession {
            driver_name,
            remaining_seconds,
            allocated_seconds,
        };
    }

    /// Update remaining seconds on the active session screen.
    pub fn update_remaining(&self, remaining_seconds: u32) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let LockScreenState::ActiveSession { remaining_seconds: ref mut r, .. } = *state {
            *r = remaining_seconds;
        }
    }

    /// Clear/dismiss the lock screen.
    pub fn clear(&mut self) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *state = LockScreenState::Hidden;
        }
        self.close_browser();
    }

    #[cfg(windows)]
    fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);
        match std::process::Command::new("msedge.exe")
            .args([
                "--kiosk",
                &url,
                "--edge-kiosk-type=fullscreen",
                "--no-first-run",
            ])
            .spawn()
        {
            Ok(child) => {
                self.browser_process = Some(child);
                tracing::info!("Lock screen browser launched at {}", url);
            }
            Err(e) => {
                tracing::error!("Failed to launch lock screen browser: {}", e);
            }
        }
    }

    #[cfg(not(windows))]
    fn launch_browser(&mut self) {
        tracing::info!("Lock screen: browser launch not supported on this platform");
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
    fn close_browser(&mut self) {}
}

// ─── HTTP Server ─────────────────────────────────────────────────────────────

/// Minimal HTTP server bound to localhost only.
async fn serve_lock_screen(
    port: u16,
    state: Arc<Mutex<LockScreenState>>,
    event_tx: mpsc::Sender<LockScreenEvent>,
) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(l) => {
            tracing::info!("Lock screen server listening on http://0.0.0.0:{}", port);
            l
        }
        Err(e) => {
            tracing::error!("Lock screen server failed to bind port {}: {}", port, e);
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
    let content = ACTIVE_SESSION_PAGE
        .replace("{{DRIVER_NAME}}", &html_escape(driver_name))
        .replace("{{MINUTES}}", &format!("{:02}", mins))
        .replace("{{SECONDS}}", &format!("{:02}", secs))
        .replace("{{PROGRESS}}", &progress.to_string())
        .replace("{{WARNING_CLASS}}", warning_class);
    page_shell("Session Active - Racing Point", &content)
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
            .render()
            .min_dimensions(250, 250)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(e) => {
            tracing::error!("QR code generation failed: {}", e);
            format!(
                "<p style=\"color:#000;font-size:14px\">QR Error: {}</p>",
                html_escape(&e.to_string())
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
</style>
<div class="welcome">{{DRIVER_NAME}}</div>
<div class="session-label">Time Remaining</div>
<div class="timer-display {{WARNING_CLASS}}">{{MINUTES}}<span class="colon">:</span>{{SECONDS}}</div>
<div class="progress-container">
    <div class="progress-bar" style="width: {{PROGRESS}}%"></div>
</div>
<div class="hint">Enjoy your session! Need help? Ask at reception.</div>
<script>setTimeout(function(){location.reload()},3000)</script>"#;
