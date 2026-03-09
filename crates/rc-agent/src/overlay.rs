//! Racing HUD overlay displayed at the top of the screen during active billing sessions.
//!
//! Serves a floating HTML bar via a local HTTP server (port 18925) and launches
//! Edge in --app mode to display session timer, lap times, and sector splits.
//! The window is made borderless and topmost via Windows API after launch.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use rc_common::types::{LapData, TelemetryFrame};

/// Height of the visible HUD bar content (px).
const BAR_HEIGHT: i32 = 72;
/// Extra height added for Edge's app-mode title bar (stripped after launch).
const TITLE_BAR_ALLOWANCE: i32 = 40;
/// Total initial window height before title bar is stripped.
const INITIAL_WINDOW_HEIGHT: i32 = BAR_HEIGHT + TITLE_BAR_ALLOWANCE;

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
    speed_kmh: f32,
    gear: i8,
    rpm: u32,
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
            speed_kmh: 0.0,
            gear: 0,
            rpm: 0,
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
            // Try multiple times with short delays — Edge needs a moment to create the window
            for delay_ms in [1500, 1000, 1000, 2000] {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                if !state.lock().unwrap_or_else(|e| e.into_inner()).active {
                    return;
                }
                #[cfg(windows)]
                if set_topmost_centered() {
                    tracing::info!("Overlay: title bar stripped and window centered");
                    return;
                }
            }
            tracing::warn!("Overlay: could not find window to strip title bar after 5.5s");
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
        data.speed_kmh = frame.speed_kmh;
        data.gear = frame.gear;
        data.rpm = frame.rpm;
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
            set_topmost_centered();
        }
    }

    #[cfg(windows)]
    fn launch_browser(&mut self) {
        self.close_browser();
        let url = format!("http://127.0.0.1:{}", self.port);

        // Wait for overlay HTTP server to be ready before launching Edge.
        // The server may still be retrying its bind after a restart (TIME_WAIT).
        let addr = format!("127.0.0.1:{}", self.port);
        for attempt in 0..25 {
            if std::net::TcpStream::connect(&addr).is_ok() {
                if attempt > 0 {
                    tracing::info!("Overlay: server ready after {}ms", attempt * 200);
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        // Center horizontally, pin to TOP of screen
        let (screen_w, _screen_h) = get_screen_size();
        let x = (screen_w - 1920).max(0) / 2;
        let y = 0;

        let edge_paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            "msedge.exe",
        ];
        for edge_path in &edge_paths {
            match std::process::Command::new(edge_path)
                .args([
                    &format!("--app={}", url),
                    &format!("--window-size=1920,{}", INITIAL_WINDOW_HEIGHT),
                    &format!("--window-position={},{}", x, y),
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

// ─── Windows Helpers ─────────────────────────────────────────────────────────

/// Get primary monitor resolution.
#[cfg(windows)]
fn get_screen_size() -> (i32, i32) {
    unsafe {
        let w = winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CXSCREEN);
        let h = winapi::um::winuser::GetSystemMetrics(winapi::um::winuser::SM_CYSCREEN);
        if w > 0 && h > 0 { (w, h) } else { (1920, 1080) }
    }
}

/// Find the overlay Edge window by title, strip title bar, make borderless
/// topmost, and center it on screen. Returns true if window was found.
#[cfg(windows)]
fn set_topmost_centered() -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Try the page <title> — Edge --app mode uses it as window title
    let titles_to_try = ["Racing HUD", "Racing HUD - Racing HUD"];
    let mut hwnd = std::ptr::null_mut();

    for title_str in &titles_to_try {
        let title: Vec<u16> = OsStr::new(title_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let h = winapi::um::winuser::FindWindowW(std::ptr::null(), title.as_ptr());
            if !h.is_null() {
                hwnd = h;
                break;
            }
        }
    }

    if hwnd.is_null() {
        return false;
    }

    let (screen_w, _screen_h) = get_screen_size();
    let bar_w = screen_w.min(1920); // Use full screen width up to 1920
    let x = (screen_w - bar_w).max(0) / 2;
    let y = 0; // Pin to top of screen

    unsafe {
        // Strip caption (title bar) and thick frame (resize border) for clean borderless look
        let style = winapi::um::winuser::GetWindowLongW(hwnd, winapi::um::winuser::GWL_STYLE);
        let new_style = style
            & !(winapi::um::winuser::WS_CAPTION as i32)
            & !(winapi::um::winuser::WS_THICKFRAME as i32)
            & !(winapi::um::winuser::WS_SYSMENU as i32)
            & !(winapi::um::winuser::WS_MINIMIZEBOX as i32)
            & !(winapi::um::winuser::WS_MAXIMIZEBOX as i32);
        winapi::um::winuser::SetWindowLongW(hwnd, winapi::um::winuser::GWL_STYLE, new_style);

        // Strip extended borders + add TOOLWINDOW (hides from taskbar) + NOACTIVATE (don't steal focus from game)
        let ex_style = winapi::um::winuser::GetWindowLongW(hwnd, winapi::um::winuser::GWL_EXSTYLE);
        let new_ex_style = (ex_style
            & !(winapi::um::winuser::WS_EX_CLIENTEDGE as i32)
            & !(winapi::um::winuser::WS_EX_WINDOWEDGE as i32)
            & !(winapi::um::winuser::WS_EX_DLGMODALFRAME as i32)
            & !(winapi::um::winuser::WS_EX_APPWINDOW as i32))
            | (winapi::um::winuser::WS_EX_TOOLWINDOW as i32)
            | (winapi::um::winuser::WS_EX_NOACTIVATE as i32);
        winapi::um::winuser::SetWindowLongW(hwnd, winapi::um::winuser::GWL_EXSTYLE, new_ex_style);

        // Set topmost, centered, exact bar dimensions
        winapi::um::winuser::SetWindowPos(
            hwnd,
            winapi::um::winuser::HWND_TOPMOST,
            x,
            y,
            bar_w,
            BAR_HEIGHT,
            winapi::um::winuser::SWP_SHOWWINDOW | winapi::um::winuser::SWP_FRAMECHANGED,
        );
    }

    true
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
<style>
@font-face {
    font-family: 'HUD';
    src: local('Montserrat'), local('Segoe UI'), local('system-ui');
}
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body {
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: transparent;
    color: #fff;
    font-family: 'Montserrat', 'Segoe UI', system-ui, sans-serif;
    user-select: none;
    -webkit-user-select: none;
    -webkit-app-region: no-drag;
}

/* The bar itself — positioned at top of window. Once title bar is
   stripped by Windows API, this fills the entire 72px window. */
#hud {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 72px;
    background: rgba(18, 18, 18, 0.94);
    display: flex;
    align-items: center;
    justify-content: center;
    border-top: 2px solid #E10600;
    border-bottom: 2px solid #E10600;
}

.sec {
    display: flex;
    align-items: center;
    padding: 0 20px;
    height: 100%;
    flex-shrink: 0;
}
.sec + .sec { border-left: 1px solid rgba(255,255,255,0.08); }
.sec-inner { display: flex; flex-direction: column; justify-content: center; }

.lbl {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1.5px;
    color: #555;
    line-height: 1;
    margin-bottom: 3px;
}

/* Session Timer */
.timer-sec { min-width: 120px; }
.timer-val {
    font-size: 24px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    letter-spacing: 1px;
    color: #fff;
    line-height: 1.1;
}
.timer-val.warning { color: #F59E0B; }
.timer-val.critical { color: #E10600; animation: pulse 0.5s ease-in-out infinite; }

/* Current Lap */
.clap-sec { min-width: 150px; }
.clap-val {
    font-size: 24px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    color: #fff;
    line-height: 1.1;
    padding-left: 6px;
    border-left: 3px solid transparent;
}
.clap-val.invalid { border-left-color: #E10600; color: #ff8a84; }

/* Speed / Gear */
.sg-sec { min-width: 90px; }
.sg-wrap {
    display: flex;
    align-items: baseline;
    gap: 8px;
}
.gear-val {
    font-size: 34px;
    font-weight: 900;
    font-variant-numeric: tabular-nums;
    color: #fff;
    line-height: 1;
    min-width: 28px;
    text-align: center;
}
.speed-col {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
}
.speed-val {
    font-size: 17px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: #bbb;
    line-height: 1.1;
}
.speed-unit {
    font-size: 8px;
    font-weight: 600;
    color: #444;
    text-transform: uppercase;
    letter-spacing: 1px;
}

/* Prev / Best Lap */
.lap-val {
    font-size: 19px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    line-height: 1.1;
}
.prev-sec .lap-val { color: #E5E7EB; }
.best-sec .lap-val { color: #A855F7; }
.best-sec .lbl { color: #A855F7; }

.sectors { display: flex; gap: 6px; margin-top: 2px; }
.sk {
    font-size: 10px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #666;
    white-space: nowrap;
}
.sk-tag {
    font-size: 8px;
    font-weight: 700;
    color: #444;
    margin-right: 2px;
}
/* Sector delta colors */
.sk.green .sk-val { color: #22C55E; }
.sk.purple .sk-val { color: #A855F7; }
.sk.yellow .sk-val { color: #F59E0B; }

/* Lap Counter */
.lc-sec { min-width: 65px; }
.lc-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
}
.lap-num {
    font-size: 20px;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    color: #fff;
    line-height: 1;
}
.inv-badge {
    font-size: 8px;
    font-weight: 700;
    background: #E10600;
    color: #fff;
    padding: 2px 6px;
    border-radius: 3px;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    display: none;
}
.inv-badge.show { display: inline-block; }

.nd { color: #333; }

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}
.hidden { display: none !important; }
</style>
</head>
<body>

<div id="hud">

<!-- Session Timer -->
<div class="sec timer-sec">
    <div class="sec-inner">
        <div class="lbl">Session</div>
        <div class="timer-val" id="timer">--:--</div>
    </div>
</div>

<!-- Current Lap Time -->
<div class="sec clap-sec">
    <div class="sec-inner">
        <div class="lbl">Current Lap</div>
        <div class="clap-val" id="curLap">--:--.---</div>
    </div>
</div>

<!-- Speed / Gear -->
<div class="sec sg-sec">
    <div class="sg-wrap">
        <div class="gear-val" id="gear">N</div>
        <div class="speed-col">
            <div class="speed-val" id="speed">---</div>
            <div class="speed-unit">km/h</div>
        </div>
    </div>
</div>

<!-- Previous Lap -->
<div class="sec prev-sec">
    <div class="sec-inner">
        <div class="lbl">Prev</div>
        <div class="lap-val" id="prevLap">--:--.---</div>
        <div class="sectors">
            <span class="sk" id="ps1"><span class="sk-tag">S1</span><span class="sk-val" id="ps1v">--.--</span></span>
            <span class="sk" id="ps2"><span class="sk-tag">S2</span><span class="sk-val" id="ps2v">--.--</span></span>
            <span class="sk" id="ps3"><span class="sk-tag">S3</span><span class="sk-val" id="ps3v">--.--</span></span>
        </div>
    </div>
</div>

<!-- Best Lap -->
<div class="sec best-sec">
    <div class="sec-inner">
        <div class="lbl">Best</div>
        <div class="lap-val" id="bestLap">--:--.---</div>
        <div class="sectors">
            <span class="sk purple" id="bs1"><span class="sk-tag">S1</span><span class="sk-val" id="bs1v">--.--</span></span>
            <span class="sk purple" id="bs2"><span class="sk-tag">S2</span><span class="sk-val" id="bs2v">--.--</span></span>
            <span class="sk purple" id="bs3"><span class="sk-tag">S3</span><span class="sk-val" id="bs3v">--.--</span></span>
        </div>
    </div>
</div>

<!-- Lap Counter -->
<div class="sec lc-sec">
    <div class="sec-inner">
        <div class="lbl">Lap</div>
        <div class="lc-wrap">
            <div class="lap-num" id="lapNum">-</div>
            <div class="inv-badge" id="invBadge">INV</div>
        </div>
    </div>
</div>

</div><!-- #hud -->

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
        return (ms / 1000).toFixed(1);
    }
    function fmtTimer(sec) {
        if (sec == null || sec < 0) return '--:--';
        var m = Math.floor(sec / 60);
        var s = sec % 60;
        return String(m).padStart(2, '0') + ':' + String(s).padStart(2, '0');
    }
    function gearStr(g) {
        if (g === 0) return 'N';
        if (g < 0) return 'R';
        return String(g);
    }
    function sectorClass(prevMs, bestMs) {
        if (!prevMs || prevMs <= 0 || !bestMs || bestMs <= 0) return '';
        if (prevMs <= bestMs) return 'purple';
        if (prevMs - bestMs <= 300) return 'green';
        return 'yellow';
    }

    var timer = document.getElementById('timer');
    var curLap = document.getElementById('curLap');
    var gearEl = document.getElementById('gear');
    var speedEl = document.getElementById('speed');
    var prevLap = document.getElementById('prevLap');
    var ps1 = document.getElementById('ps1');
    var ps1v = document.getElementById('ps1v');
    var ps2 = document.getElementById('ps2');
    var ps2v = document.getElementById('ps2v');
    var ps3 = document.getElementById('ps3');
    var ps3v = document.getElementById('ps3v');
    var bestLapEl = document.getElementById('bestLap');
    var bs1v = document.getElementById('bs1v');
    var bs2v = document.getElementById('bs2v');
    var bs3v = document.getElementById('bs3v');
    var lapNum = document.getElementById('lapNum');
    var invBadge = document.getElementById('invBadge');

    function setSector(wrap, valEl, ms, cls) {
        valEl.textContent = fmtSec(ms);
        wrap.className = 'sk' + (cls ? ' ' + cls : '');
    }

    function update(d) {
        if (!d || !d.active) {
            timer.textContent = '--:--';
            curLap.textContent = '--:--.---';
            curLap.className = 'clap-val';
            gearEl.textContent = 'N';
            speedEl.textContent = '---';
            return;
        }

        // Timer
        timer.textContent = fmtTimer(d.remaining_seconds);
        timer.className = 'timer-val';
        if (d.remaining_seconds <= 10) timer.className = 'timer-val critical';
        else if (d.remaining_seconds <= 60) timer.className = 'timer-val warning';

        // Current lap time
        curLap.textContent = fmt(d.current_lap_time_ms);
        curLap.className = d.current_lap_invalid ? 'clap-val invalid' : 'clap-val';

        // Speed + Gear
        gearEl.textContent = gearStr(d.gear);
        speedEl.textContent = d.speed_kmh > 0 ? Math.round(d.speed_kmh) : '---';

        // Previous lap with sector delta colors
        if (d.previous_lap) {
            prevLap.textContent = fmt(d.previous_lap.lap_time_ms);
            var b = d.best_lap;
            setSector(ps1, ps1v, d.previous_lap.sector1_ms, b ? sectorClass(d.previous_lap.sector1_ms, b.sector1_ms) : '');
            setSector(ps2, ps2v, d.previous_lap.sector2_ms, b ? sectorClass(d.previous_lap.sector2_ms, b.sector2_ms) : '');
            setSector(ps3, ps3v, d.previous_lap.sector3_ms, b ? sectorClass(d.previous_lap.sector3_ms, b.sector3_ms) : '');
        }

        // Best lap (sectors always purple)
        if (d.best_lap) {
            bestLapEl.textContent = fmt(d.best_lap.lap_time_ms);
            bs1v.textContent = fmtSec(d.best_lap.sector1_ms);
            bs2v.textContent = fmtSec(d.best_lap.sector2_ms);
            bs3v.textContent = fmtSec(d.best_lap.sector3_ms);
        }

        // Lap counter
        lapNum.textContent = d.current_lap_number > 0 ? d.current_lap_number : '-';
        invBadge.className = d.current_lap_invalid ? 'inv-badge show' : 'inv-badge';
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
