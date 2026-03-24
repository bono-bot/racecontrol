//! Debug HTTP server for remote pod diagnostics.
//!
//! Binds to 0.0.0.0:18924 (LAN-accessible).
//! Endpoints:
//!   GET /status     — JSON with lock screen state, uptime, pod info
//!   GET /screenshot — captures pod screen, returns PNG
//!   GET /page       — returns the lock screen HTML (what the browser sees)

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use socket2::{Domain, Protocol, Socket, Type};

use crate::lock_screen::LockScreenState;

const LOG_TARGET: &str = "debug-server";

/// Shared state for game launch errors (visible in debug console).
/// Set by the LaunchGame handler when CM reports errors.
pub type LastLaunchError = Arc<Mutex<Option<String>>>;

/// Start the debug server (call once at startup).
pub fn spawn(
    state: Arc<Mutex<LockScreenState>>,
    pod_name: String,
    pod_number: u32,
    last_launch_error: LastLaunchError,
) {
    tokio::spawn(async move {
        // Use SO_REUSEADDR to allow binding even if zombie connections from old process hold the port
        let listener = match (|| -> std::io::Result<TcpListener> {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
            socket.set_reuse_address(true)?;
            socket.set_nonblocking(true)?;
            let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 18924);
            socket.bind(&addr.into())?;
            socket.listen(128)?;
            TcpListener::from_std(std::net::TcpListener::from(socket))
        })() {
            Ok(l) => {
                tracing::info!(target: LOG_TARGET, "Debug server listening on http://0.0.0.0:18924 (SO_REUSEADDR)");
                l
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Debug server failed to bind port 18924: {}", e);
                return;
            }
        };

        loop {
            let (mut stream, addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => continue,
            };

            let state = state.clone();
            let pod_name = pod_name.clone();
            let last_launch_error = last_launch_error.clone();

            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                // Read with 5s timeout — prevents hung connections from leaking tasks
                let n = match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
                    Ok(Ok(n)) if n > 0 => n,
                    _ => {
                        let _ = stream.shutdown().await;
                        return;
                    }
                };

                let request = String::from_utf8_lossy(&buf[..n]);
                let first_line = request.lines().next().unwrap_or("");
                tracing::debug!(target: LOG_TARGET, "Debug server request from {}: {}", addr, first_line);

                if first_line.contains("/screenshot") {
                    serve_screenshot(&mut stream).await;
                } else if first_line.contains("/page") {
                    serve_page(&mut stream, &state).await;
                } else {
                    // Default: /status
                    serve_status(&mut stream, &state, &pod_name, pod_number, &last_launch_error).await;
                }
                // Ensure connection is closed (prevents stale ESTABLISHED connections)
                let _ = stream.shutdown().await;
            });
        }
    });
}

async fn serve_status(
    stream: &mut (impl AsyncWriteExt + Unpin),
    state: &Arc<Mutex<LockScreenState>>,
    pod_name: &str,
    pod_number: u32,
    last_launch_error: &LastLaunchError,
) {
    // Use try_lock to avoid blocking the tokio worker thread.
    // std::sync::Mutex::lock() blocks the thread, which in an async context can deadlock
    // if the lock holder is on the same tokio worker thread.
    // Clone immediately to drop the guard before any await point.
    let lock_result = state.try_lock().ok().map(|g| g.clone());
    let current = match lock_result {
        Some(s) => s,
        None => {
            let body = r#"{"debug_server":"busy","error":"lock_screen_state mutex contested"}"#;
            let resp = format!(
                "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            return;
        }
    };
    let state_name = match &current {
        LockScreenState::Hidden => "hidden",
        LockScreenState::PinEntry { .. } => "pin_entry",
        LockScreenState::QrDisplay { .. } => "qr_display",
        LockScreenState::ActiveSession { .. } => "active_session",
        LockScreenState::SessionSummary { .. } => "session_summary",
        LockScreenState::BetweenSessions { .. } => "between_sessions",
        LockScreenState::AwaitingAssistance { .. } => "awaiting_assistance",
        LockScreenState::LaunchSplash { .. } => "launch_splash",
        LockScreenState::ScreenBlanked => "screen_blanked",
        LockScreenState::Disconnected => "disconnected",
        LockScreenState::StartupConnecting => "startup_connecting",
        LockScreenState::ConfigError { .. } => "config_error",
        LockScreenState::Lockdown { .. } => "lockdown",
        LockScreenState::MaintenanceRequired { .. } => "maintenance_required",
    };

    let launch_err = last_launch_error.try_lock().ok().and_then(|guard| guard.clone());
    let launch_err_json = match &launch_err {
        Some(err) => {
            // Escape JSON special chars in error message
            let escaped = err.replace('\\', "\\\\").replace('"', "\\\"");
            format!(r#","last_launch_error":"{}""#, escaped)
        }
        None => String::new(),
    };

    let body = format!(
        r#"{{"pod":"{}","pod_number":{},"lock_screen_state":"{}","debug_server":"ok"{}}}"#,
        pod_name, pod_number, state_name, launch_err_json
    );

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

async fn serve_page(
    stream: &mut (impl AsyncWriteExt + Unpin),
    state: &Arc<Mutex<LockScreenState>>,
) {
    let current = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let body = crate::lock_screen::render_page_public(&current);
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

async fn serve_screenshot(stream: &mut (impl AsyncWriteExt + Unpin)) {
    match take_screenshot().await {
        Ok(png_data) => {
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                png_data.len()
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.write_all(&png_data).await;
        }
        Err(e) => {
            let body = format!(r#"{{"error":"screenshot failed: {}"}}"#, e);
            let resp = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
        }
    }
}

#[cfg(windows)]
async fn take_screenshot() -> Result<Vec<u8>, String> {
    // Use PowerShell to capture screen to a temp file, then read it
    let tmp = std::env::temp_dir().join("rc_debug_screenshot.png");
    let tmp_path = tmp.to_string_lossy().to_string();

    let ps_script = format!(
        r#"Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $b = New-Object System.Drawing.Bitmap($s.Width, $s.Height); $g = [System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($s.Location, [System.Drawing.Point]::Empty, $s.Size); $b.Save('{}'); $g.Dispose(); $b.Dispose()"#,
        tmp_path.replace('\\', "\\\\")
    );

    let output = {
        let mut cmd = tokio::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &ps_script]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        cmd.output().await.map_err(|e| format!("powershell exec failed: {}", e))?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("powershell failed: {}", stderr));
    }

    tokio::fs::read(&tmp)
        .await
        .map_err(|e| format!("read screenshot file failed: {}", e))
}

#[cfg(not(windows))]
async fn take_screenshot() -> Result<Vec<u8>, String> {
    // Try scrot, then import (ImageMagick), then gnome-screenshot
    let tmp = "/tmp/rc_debug_screenshot.png";

    let tools = [
        ("scrot", vec![tmp.to_string()]),
        ("import", vec!["-window".to_string(), "root".to_string(), tmp.to_string()]),
        ("gnome-screenshot", vec!["-f".to_string(), tmp.to_string()]),
    ];

    for (tool, args) in &tools {
        if let Ok(output) = tokio::process::Command::new(tool)
            .args(args)
            .output()
            .await
        {
            if output.status.success() {
                return tokio::fs::read(tmp)
                    .await
                    .map_err(|e| format!("read screenshot failed: {}", e));
            }
        }
    }

    Err("no screenshot tool available (install scrot or imagemagick)".to_string())
}
