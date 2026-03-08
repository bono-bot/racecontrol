//! Racing HUD overlay displayed at the top of the screen during active billing sessions.
//!
//! Serves a thin HTML bar via a local HTTP server (port 18925) and launches
//! Edge in --app mode to display session timer, lap times, and sector splits.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use rc_common::types::{LapData, TelemetryFrame};

// ─── Types ───────────────────────────────────────────────────────────────────

/// A completed lap record for overlay display.
#[derive(Debug, Clone, serde::Serialize)]
struct LapRecord {
    lap_time_ms: u32,
    sector1_ms: Option<u32>,
    sector2_ms: Option<u32>,
    sector3_ms: Option<u32>,
    valid: bool,
}

/// Shared state between the HTTP server and the overlay manager.
#[derive(Debug, Clone, serde::Serialize)]
struct OverlayData {
    active: bool,
    driver_name: String,
    remaining_seconds: u32,
    allocated_seconds: u32,
    // Current lap info from telemetry
    current_lap_number: u32,
    current_lap_time_ms: u32,
    current_sector: u8,
    current_lap_invalid: bool,
    car: String,
    track: String,
    // Completed lap records
    previous_lap: Option<LapRecord>,
    best_lap: Option<LapRecord>,
}

impl Default for OverlayData {
    fn default() -> Self {
        Self {
            active: false,
            driver_name: String::new(),
            remaining_seconds: 0,
            allocated_seconds: 0,
            current_lap_number: 0,
            current_lap_time_ms: 0,
            current_sector: 0,
            current_lap_invalid: false,
            car: String::new(),
            track: String::new(),
            previous_lap: None,
            best_lap: None,
        }
    }
}

// ─── Manager ─────────────────────────────────────────────────────────────────

/// Manages the racing HUD overlay lifecycle: state, HTTP server, and browser window.
pub struct OverlayManager {
    state: Arc<Mutex<OverlayData>>,
    port: u16,
    #[cfg(windows)]
    browser_process: Option<std::process::Child>,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OverlayData::default())),
            port: 18925,
            #[cfg(windows)]
            browser_process: None,
        }
    }

    /// Start the local HTTP server for overlay pages (call once at startup).
    pub fn start_server(&self) {
        let state = self.state.clone();
        let port = self.port;
        tokio::spawn(async move {
            serve_overlay(port, state).await;
        });
    }

    /// Activate overlay for a new billing session.
    pub fn activate(&mut self, driver_name: String, allocated_seconds: u32) {
        {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *data = OverlayData {
                active: true,
                driver_name,
                remaining_seconds: allocated_seconds,
                allocated_seconds,
                ..OverlayData::default()
            };
            // Keep active=true since default() sets it false
            data.active = true;
        }
        self.launch_browser();
        // Schedule topmost enforcement after browser has time to open
        let state = self.state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if state.lock().unwrap_or_else(|e| e.into_inner()).active {
                #[cfg(windows)]
                set_topmost();
            }
        });
    }

    /// Update billing timer from BillingTick.
    pub fn update_billing(&self, remaining_seconds: u32) {
        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if data.active {
            data.remaining_seconds = remaining_seconds;
        }
    }

    /// Update telemetry data from current frame.
    pub fn update_telemetry(&self, frame: &TelemetryFrame) {
        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !data.active {
            return;
        }
        data.current_lap_number = frame.lap_number;
        data.current_lap_time_ms = frame.lap_time_ms;
        data.current_sector = frame.sector;
        data.current_lap_invalid = frame.current_lap_invalid.unwrap_or(false);
        data.car = frame.car.clone();
        data.track = frame.track.clone();
    }

    /// Record a completed lap — update previous_lap and possibly best_lap.
    pub fn on_lap_completed(&self, lap: &LapData) {
        let record = LapRecord {
            lap_time_ms: lap.lap_time_ms,
            sector1_ms: lap.sector1_ms,
            sector2_ms: lap.sector2_ms,
            sector3_ms: lap.sector3_ms,
            valid: lap.valid,
        };

        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !data.active {
            return;
        }

        // Always update previous lap
        data.previous_lap = Some(record.clone());

        // Update best lap if this lap is valid and faster (or first valid lap)
        if lap.valid {
            let dominated = match &data.best_lap {
                Some(best) => lap.lap_time_ms < best.lap_time_ms,
                None => true,
            };
            if dominated {
                data.best_lap = Some(record);
            }
        }
    }

    /// Deactivate overlay — close browser, clear state.
    pub fn deactivate(&mut self) {
        {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            data.active = false;
        }
        self.close_browser();
    }

    /// Re-enforce HWND_TOPMOST (call periodically from main loop).
    pub fn enforce_topmost(&self) {
        let data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if data.active {
            #[cfg(windows)]
            set_topmost();
        }
    }

    #[cfg(windows)]
    fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);
        let edge_paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            "msedge.exe",
        ];
        for edge_path in &edge_paths {
            match std::process::Command::new(edge_path)
                .args([
                    &format!("--app={}", url),
                    "--window-size=1920,72",
                    "--window-position=0,0",
                    "--no-first-run",
                    "--no-default-browser-check",
                    "--disable-notifications",
                    "--disable-infobars",
                    "--disable-session-crashed-bubble",
                    "--disable-component-update",
                    "--user-data-dir=C:\\RacingPoint\\overlay-profile",
                ])
                .spawn()
            {
                Ok(child) => {
                    self.browser_process = Some(child);
                    tracing::info!("Overlay browser launched at {} using {}", url, edge_path);
                    return;
                }
                Err(e) => {
                    tracing::warn!("Failed to launch Edge for overlay from {}: {}", edge_path, e);
                }
            }
        }
        tracing::error!("Overlay: could not launch Edge from any known path");
    }

    #[cfg(not(windows))]
    fn launch_browser(&mut self) {
        tracing::warn!("Overlay browser not supported on non-Windows platforms");
    }

    #[cfg(windows)]
    fn close_browser(&mut self) {
        if let Some(ref mut child) = self.browser_process {
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!("Overlay browser closed");
        }
        self.browser_process = None;
    }

    #[cfg(not(windows))]
    fn close_browser(&mut self) {}
}

// ─── HWND Topmost ────────────────────────────────────────────────────────────

/// Find the overlay Edge window by title and set it to TOPMOST + borderless.
#[cfg(windows)]
fn set_topmost() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let title: Vec<u16> = OsStr::new("Racing HUD")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hwnd = winapi::um::winuser::FindWindowW(std::ptr::null(), title.as_ptr());
        if hwnd.is_null() {
            return;
        }

        // Strip caption and thick frame for borderless appearance
        let style = winapi::um::winuser::GetWindowLongW(hwnd, winapi::um::winuser::GWL_STYLE);
        let new_style = style
            & !(winapi::um::winuser::WS_CAPTION as i32)
            & !(winapi::um::winuser::WS_THICKFRAME as i32);
        winapi::um::winuser::SetWindowLongW(hwnd, winapi::um::winuser::GWL_STYLE, new_style);

        // Set topmost and reposition
        winapi::um::winuser::SetWindowPos(
            hwnd,
            winapi::um::winuser::HWND_TOPMOST,
            0,
            0,
            1920,
            72,
            winapi::um::winuser::SWP_SHOWWINDOW,
        );
    }
}

// ─── HTTP Server ─────────────────────────────────────────────────────────────

async fn serve_overlay(port: u16, state: Arc<Mutex<OverlayData>>) {
    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    // Retry binding up to 10 times (covers TIME_WAIT from crashed previous instance)
    let listener = loop {
        let socket = match tokio::net::TcpSocket::new_v4() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Overlay: failed to create socket: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };
        let _ = socket.set_reuseaddr(true);
        match socket.bind(addr) {
            Ok(()) => match socket.listen(128) {
                Ok(l) => {
                    tracing::info!("Overlay server listening on http://127.0.0.1:{}", port);
                    break l;
                }
                Err(e) => {
                    tracing::warn!("Overlay: listen failed on port {}: {} — retrying in 3s", port, e);
                }
            },
            Err(e) => {
                tracing::warn!("Overlay: bind failed on port {}: {} — retrying in 3s", port, e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    };

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };

        let state = state.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let request = String::from_utf8_lossy(&buf[..n]);
            let first_line = request.lines().next().unwrap_or("");

            if first_line.contains("/favicon") {
                let resp = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            if first_line.contains("/data") {
                // JSON endpoint for polling
                let data = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let json = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    json.len(), json
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            } else {
                // Serve the HTML overlay page
                let body = render_overlay_page();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        });
    }
}

// ─── HTML/CSS/JS ─────────────────────────────────────────────────────────────

fn render_overlay_page() -> String {
    OVERLAY_HTML.to_string()
}

const OVERLAY_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Racing HUD</title>
<link href="https://fonts.googleapis.com/css2?family=Montserrat:wght@400;600;700;800&display=swap" rel="stylesheet">
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body {
    height: 72px;
    overflow: hidden;
    background: rgba(26, 26, 26, 0.88);
    color: #fff;
    font-family: 'Montserrat', 'Segoe UI', system-ui, sans-serif;
    user-select: none;
    -webkit-user-select: none;
}
body {
    display: flex;
    align-items: center;
    padding: 0 24px;
    gap: 0;
    border-bottom: 2px solid #333;
}

.section {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 20px;
    height: 100%;
}
.section + .section {
    border-left: 1px solid #444;
}

/* Timer section */
.timer-section {
    min-width: 180px;
}
.timer-icon {
    font-size: 20px;
    color: #888;
}
.timer-label {
    font-size: 11px;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 1.5px;
}
.timer-value {
    font-size: 28px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    letter-spacing: 1px;
    color: #fff;
}
.timer-value.warning {
    color: #F59E0B;
}
.timer-value.critical {
    color: #E10600;
    animation: pulse 0.5s ease-in-out infinite;
}

/* Lap sections */
.lap-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1.5px;
    color: #888;
    white-space: nowrap;
}
.lap-time {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
}
.sectors {
    display: flex;
    gap: 8px;
}
.sector {
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #999;
    white-space: nowrap;
}
.sector-tag {
    font-size: 9px;
    font-weight: 700;
    color: #666;
    margin-right: 2px;
}

.prev-section .lap-time {
    color: #E5E7EB;
}
.best-section .lap-time {
    color: #A855F7;
}
.best-section .lap-label {
    color: #A855F7;
}

/* Current lap section */
.lap-info {
    display: flex;
    align-items: center;
    gap: 14px;
}
.lap-number {
    font-size: 20px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    color: #fff;
}
.invalid-badge {
    font-size: 10px;
    font-weight: 700;
    background: #E10600;
    color: #fff;
    padding: 2px 8px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 1px;
    display: none;
}
.invalid-badge.show {
    display: inline-block;
}

.no-data {
    color: #555;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}

/* Inactive / hidden state */
.hidden { display: none !important; }
</style>
</head>
<body>

<!-- Timer -->
<div class="section timer-section">
    <div>
        <div class="timer-label">Session</div>
        <div class="timer-value" id="timer">--:--</div>
    </div>
</div>

<!-- Previous Lap -->
<div class="section prev-section">
    <div>
        <div class="lap-label">Prev Lap</div>
        <div class="lap-time" id="prevLap">--:--.---</div>
        <div class="sectors" id="prevSectors">
            <span class="sector"><span class="sector-tag">S1</span><span id="prevS1">--.--</span></span>
            <span class="sector"><span class="sector-tag">S2</span><span id="prevS2">--.--</span></span>
            <span class="sector"><span class="sector-tag">S3</span><span id="prevS3">--.--</span></span>
        </div>
    </div>
</div>

<!-- Best Lap -->
<div class="section best-section">
    <div>
        <div class="lap-label">Best Lap</div>
        <div class="lap-time" id="bestLap">--:--.---</div>
        <div class="sectors" id="bestSectors">
            <span class="sector"><span class="sector-tag">S1</span><span id="bestS1">--.--</span></span>
            <span class="sector"><span class="sector-tag">S2</span><span id="bestS2">--.--</span></span>
            <span class="sector"><span class="sector-tag">S3</span><span id="bestS3">--.--</span></span>
        </div>
    </div>
</div>

<!-- Current Lap Info -->
<div class="section">
    <div class="lap-info">
        <div>
            <div class="lap-label">Lap</div>
            <div class="lap-number" id="lapNum">-</div>
        </div>
        <div class="invalid-badge" id="invalidBadge">INVALID</div>
    </div>
</div>

<script>
(function() {
    function fmt(ms) {
        if (!ms || ms <= 0) return '--:--.---';
        var m = Math.floor(ms / 60000);
        var s = Math.floor((ms % 60000) / 1000);
        var ml = ms % 1000;
        return m + ':' + String(s).padStart(2, '0') + '.' + String(ml).padStart(3, '0');
    }
    function fmtSec(ms) {
        if (!ms || ms <= 0) return '--.--';
        var s = (ms / 1000).toFixed(1);
        return s;
    }
    function fmtTimer(sec) {
        if (sec == null || sec < 0) return '--:--';
        var m = Math.floor(sec / 60);
        var s = sec % 60;
        return String(m).padStart(2, '0') + ':' + String(s).padStart(2, '0');
    }

    var timer = document.getElementById('timer');
    var prevLap = document.getElementById('prevLap');
    var prevS1 = document.getElementById('prevS1');
    var prevS2 = document.getElementById('prevS2');
    var prevS3 = document.getElementById('prevS3');
    var bestLap = document.getElementById('bestLap');
    var bestS1 = document.getElementById('bestS1');
    var bestS2 = document.getElementById('bestS2');
    var bestS3 = document.getElementById('bestS3');
    var lapNum = document.getElementById('lapNum');
    var invalidBadge = document.getElementById('invalidBadge');

    function update(d) {
        if (!d || !d.active) {
            timer.textContent = '--:--';
            return;
        }

        // Timer
        timer.textContent = fmtTimer(d.remaining_seconds);
        timer.className = 'timer-value';
        if (d.remaining_seconds <= 10) {
            timer.className = 'timer-value critical';
        } else if (d.remaining_seconds <= 60) {
            timer.className = 'timer-value warning';
        }

        // Previous lap
        if (d.previous_lap) {
            prevLap.textContent = fmt(d.previous_lap.lap_time_ms);
            prevS1.textContent = fmtSec(d.previous_lap.sector1_ms);
            prevS2.textContent = fmtSec(d.previous_lap.sector2_ms);
            prevS3.textContent = fmtSec(d.previous_lap.sector3_ms);
        }

        // Best lap
        if (d.best_lap) {
            bestLap.textContent = fmt(d.best_lap.lap_time_ms);
            bestS1.textContent = fmtSec(d.best_lap.sector1_ms);
            bestS2.textContent = fmtSec(d.best_lap.sector2_ms);
            bestS3.textContent = fmtSec(d.best_lap.sector3_ms);
        }

        // Current lap
        lapNum.textContent = d.current_lap_number > 0 ? d.current_lap_number : '-';
        if (d.current_lap_invalid) {
            invalidBadge.className = 'invalid-badge show';
        } else {
            invalidBadge.className = 'invalid-badge';
        }
    }

    function poll() {
        var xhr = new XMLHttpRequest();
        xhr.open('GET', '/data', true);
        xhr.timeout = 500;
        xhr.onload = function() {
            if (xhr.status === 200) {
                try { update(JSON.parse(xhr.responseText)); } catch(e) {}
            }
        };
        xhr.send();
    }

    setInterval(poll, 200);
    poll();
})();
</script>
</body>
</html>"##;
