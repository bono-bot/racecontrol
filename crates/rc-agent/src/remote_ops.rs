//! Remote operations HTTP server (merged pod-agent).
//!
//! Binds to 0.0.0.0:8090 (LAN-accessible). Provides the same API contract
//! that racecontrol's deploy.rs expects, so no server-side changes needed.
//!
//! Endpoints:
//!   GET  /ping       — "pong"
//!   GET  /health     — uptime, exec slots, version
//!   GET  /info       — hostname, IP, OS, memory, CPU
//!   POST /exec       — execute shell command (semaphore-gated, CREATE_NO_WINDOW)
//!   GET  /files      — list directory contents
//!   GET  /file       — read file (max 50MB)
//!   POST /write      — write file
//!   POST /mkdir      — create directory
//!   GET  /screenshot — capture screen as JPEG
//!   GET  /cursor     — cursor position
//!   POST /input      — mouse/keyboard input

use axum::{
    Router,
    extract::Query,
    http::{StatusCode, header, HeaderValue},
    middleware,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use subtle::ConstantTimeEq;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Instant, SystemTime},
};
use sysinfo::System;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
#[cfg(windows)]
#[allow(unused_imports)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_ID: &str = env!("GIT_HASH");
const MAX_CONCURRENT_EXECS: usize = 8;
const DEFAULT_EXEC_TIMEOUT_MS: u64 = 10_000;

static EXEC_SEMAPHORE: Semaphore = Semaphore::const_new(MAX_CONCURRENT_EXECS);
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Middleware that adds `Connection: close` to every response.
/// Prevents keep-alive socket accumulation (CLOSE_WAIT flood) caused by
/// racecontrol's fleet_health polling hitting :8090 repeatedly.
async fn connection_close_layer(
    req: axum::extract::Request,
    next: middleware::Next,
) -> impl IntoResponse {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        header::CONNECTION,
        HeaderValue::from_static("close"),
    );
    resp
}

/// Middleware: require X-Service-Key header on protected routes.
/// When RCAGENT_SERVICE_KEY is empty or unset, all requests pass through
/// (permissive mode for safe rollout). Uses constant-time comparison to
/// prevent timing side-channel attacks.
async fn require_service_key(
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected = std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default();

    // Permissive mode: no key configured = allow all
    if expected.is_empty() {
        return Ok(next.run(req).await);
    }

    let provided = req.headers()
        .get("x-service-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Constant-time comparison to prevent timing attacks
    if expected.as_bytes().ct_eq(provided.as_bytes()).into() {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Start the remote ops HTTP server on the given port.
/// Spawns an async task — returns immediately.
pub fn start(port: u16) {
    START_TIME.get_or_init(Instant::now);

    tokio::spawn(async move {
        let public_routes = Router::new()
            .route("/ping", get(ping))
            .route("/health", get(health));

        let protected_routes = Router::new()
            .route("/info", get(info))
            .route("/files", get(list_files))
            .route("/file", get(read_file))
            .route("/exec", post(exec_command))
            .route("/mkdir", post(make_dir))
            .route("/write", post(write_file))
            .route("/screenshot", get(screenshot))
            .route("/cursor", get(cursor_position))
            .route("/input", post(send_input))
            .layer(middleware::from_fn(require_service_key));

        let app = public_routes
            .merge(protected_routes)
            .layer(middleware::from_fn(connection_close_layer));

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

        // Retry binding with SO_REUSEADDR to handle stale CLOSE_WAIT/TIME_WAIT
        // sockets left over from a previous rc-agent instance.
        let listener = {
            let mut bound = None;
            for attempt in 1..=10 {
                // Use std socket with SO_REUSEADDR, then convert to tokio
                let sock = match socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to create socket: {}", e);
                        return;
                    }
                };
                let _ = sock.set_reuse_address(true);
                let _ = sock.set_nonblocking(true);
                match sock.bind(&addr.into()) {
                    Ok(()) => {
                        let _ = sock.listen(128);
                        let std_listener: std::net::TcpListener = sock.into();
                        match tokio::net::TcpListener::from_std(std_listener) {
                            Ok(l) => {
                                tracing::info!("Remote ops server listening on http://{}", addr);
                                bound = Some(l);
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to convert listener (attempt {}): {}", attempt, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Port {} busy (attempt {}/10): {} — retrying in 3s",
                            port, attempt, e
                        );
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
            match bound {
                Some(l) => l,
                None => {
                    tracing::error!("Failed to bind port {} after 10 attempts (30s). Stale sockets?", port);
                    return;
                }
            }
        };

        // Mark the listening socket non-inheritable so cmd.exe spawned by
        // exec_command() cannot keep :8090 alive as a CLOSE_WAIT zombie after
        // rc-agent exits (which would prevent the new instance from rebinding).
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            use winapi::um::handleapi::SetHandleInformation;
            const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
            let raw = listener.as_raw_socket() as usize;
            let ok = unsafe { SetHandleInformation(raw as *mut _, HANDLE_FLAG_INHERIT, 0) };
            if ok == 0 {
                tracing::warn!("[remote_ops] SetHandleInformation failed — cmd.exe may inherit :8090 socket");
            } else {
                tracing::debug!("[remote_ops] Listening socket marked non-inheritable");
            }
        }

        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Remote ops server error: {}", e);
        }
    });
}

/// Start remote ops HTTP server and return a oneshot receiver with the bind result.
/// The existing retry loop (10 attempts, 3s each) is preserved for CLOSE_WAIT recovery,
/// but the result is signaled back to main() so bind failures are observable.
pub fn start_checked(port: u16) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    START_TIME.get_or_init(Instant::now);

    tokio::spawn(async move {
        let public_routes = Router::new()
            .route("/ping", get(ping))
            .route("/health", get(health));

        let protected_routes = Router::new()
            .route("/info", get(info))
            .route("/files", get(list_files))
            .route("/file", get(read_file))
            .route("/exec", post(exec_command))
            .route("/mkdir", post(make_dir))
            .route("/write", post(write_file))
            .route("/screenshot", get(screenshot))
            .route("/cursor", get(cursor_position))
            .route("/input", post(send_input))
            .layer(middleware::from_fn(require_service_key));

        let app = public_routes
            .merge(protected_routes)
            .layer(middleware::from_fn(connection_close_layer));

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

        // Same retry loop as start() — signal result on first success or final failure
        let listener = {
            let mut bound = None;
            for attempt in 1..=10u32 {
                let sock = match socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to create socket: {}", e);
                        let _ = tx.send(Err(format!("remote ops socket create failed: {}", e)));
                        return;
                    }
                };
                let _ = sock.set_reuse_address(true);
                let _ = sock.set_nonblocking(true);
                match sock.bind(&addr.into()) {
                    Ok(()) => {
                        let _ = sock.listen(128);
                        let std_listener: std::net::TcpListener = sock.into();
                        match tokio::net::TcpListener::from_std(std_listener) {
                            Ok(l) => {
                                tracing::info!("Remote ops server listening on http://{}", addr);
                                bound = Some(l);
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to convert listener (attempt {}): {}", attempt, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Port {} busy (attempt {}/10): {} — retrying in 3s",
                            port, attempt, e
                        );
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
            match bound {
                Some(l) => {
                    let _ = tx.send(Ok(port));
                    l
                }
                None => {
                    let msg = format!("remote ops port {} bind failed after 10 attempts (30s)", port);
                    tracing::error!("{}", msg);
                    let _ = tx.send(Err(msg));
                    return;
                }
            }
        };

        // Mark the listening socket non-inheritable so cmd.exe spawned by
        // exec_command() cannot keep :8090 alive as a CLOSE_WAIT zombie after
        // rc-agent exits (which would prevent the new instance from rebinding).
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            use winapi::um::handleapi::SetHandleInformation;
            const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
            let raw = listener.as_raw_socket() as usize;
            let ok = unsafe { SetHandleInformation(raw as *mut _, HANDLE_FLAG_INHERIT, 0) };
            if ok == 0 {
                tracing::warn!("[remote_ops] SetHandleInformation failed — cmd.exe may inherit :8090 socket");
            } else {
                tracing::debug!("[remote_ops] Listening socket marked non-inheritable");
            }
        }

        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Remote ops server error: {}", e);
        }
    });
    rx
}

// ─── Endpoints ──────────────────────────────────────────────────────────────

async fn ping() -> &'static str {
    "pong"
}

async fn health() -> Json<serde_json::Value> {
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let available_exec_slots = EXEC_SEMAPHORE.available_permits();
    Json(serde_json::json!({
        "status": "ok",
        "version": VERSION,
        "build_id": BUILD_ID,
        "uptime_secs": uptime,
        "exec_slots_available": available_exec_slots,
        "exec_slots_total": MAX_CONCURRENT_EXECS,
    }))
}

async fn info() -> Json<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name().unwrap_or_default();
    let os = format!(
        "{} {}",
        System::name().unwrap_or_default(),
        System::os_version().unwrap_or_default()
    );
    let uptime_secs = System::uptime();
    let total_memory_mb = sys.total_memory() / 1024 / 1024;
    let used_memory_mb = sys.used_memory() / 1024 / 1024;
    let cpu_count = sys.cpus().len();

    let local_ip = detect_local_ip().await.unwrap_or_else(|| "unknown".into());

    Json(SystemInfo {
        hostname,
        local_ip,
        os,
        uptime_secs,
        total_memory_mb,
        used_memory_mb,
        cpu_count,
        agent_version: VERSION.to_string(),
    })
}

#[derive(Serialize)]
struct SystemInfo {
    hostname: String,
    local_ip: String,
    os: String,
    uptime_secs: u64,
    total_memory_mb: u64,
    used_memory_mb: u64,
    cpu_count: usize,
    agent_version: String,
}

// ─── File Operations ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PathQuery {
    path: String,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: Option<u64>,
}

async fn list_files(Query(q): Query<PathQuery>) -> Result<Json<Vec<FileEntry>>, (StatusCode, String)> {
    let path = PathBuf::from(&q.path);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("Path not found: {}", q.path)));
    }
    if !path.is_dir() {
        return Err((StatusCode::BAD_REQUEST, "Not a directory".into()));
    }

    let mut entries = Vec::new();
    match fs::read_dir(&path) {
        Ok(dir) => {
            for entry in dir.flatten() {
                let meta = entry.metadata().ok();
                let modified = meta.as_ref().and_then(|m| {
                    m.modified().ok().and_then(|t| {
                        t.duration_since(SystemTime::UNIX_EPOCH).ok().map(|d| d.as_secs())
                    })
                });
                entries.push(FileEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    modified,
                });
            }
        }
        Err(e) => return Err((StatusCode::FORBIDDEN, format!("Cannot read directory: {}", e))),
    }

    Ok(Json(entries))
}

async fn read_file(Query(q): Query<PathQuery>) -> Result<impl IntoResponse, (StatusCode, String)> {
    let path = PathBuf::from(&q.path);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("File not found: {}", q.path)));
    }
    if !path.is_file() {
        return Err((StatusCode::BAD_REQUEST, "Not a file".into()));
    }

    if let Ok(meta) = path.metadata() {
        if meta.len() > 50 * 1024 * 1024 {
            return Err((StatusCode::BAD_REQUEST, "File too large (>50MB)".into()));
        }
    }

    match fs::read(&path) {
        Ok(bytes) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let headers = [
                ("content-disposition", format!("inline; filename=\"{}\"", filename)),
            ];
            Ok((headers, bytes))
        }
        Err(e) => Err((StatusCode::FORBIDDEN, format!("Cannot read file: {}", e))),
    }
}

// ─── Exec ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExecRequest {
    cmd: String,
    timeout_ms: Option<u64>,
    /// Fire-and-forget: spawn with DETACHED_PROCESS, return immediately.
    /// Use for self-update restarts — the spawned process outlives rc-agent.
    #[serde(default)]
    detached: bool,
}

#[derive(Serialize)]
struct ExecResponse {
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

async fn exec_command(Json(req): Json<ExecRequest>) -> Result<Json<ExecResponse>, (StatusCode, Json<ExecResponse>)> {
    let _permit = match EXEC_SEMAPHORE.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            tracing::warn!(
                "[remote_ops] SLOT EXHAUSTION: all {} slots occupied. Returning 429.",
                MAX_CONCURRENT_EXECS
            );
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ExecResponse {
                    success: false,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!(
                        "Too many concurrent commands ({} max). Try again later.",
                        MAX_CONCURRENT_EXECS
                    ),
                }),
            ));
        }
    };

    // RCAGENT_SELF_RESTART sentinel: directly calls relaunch_self() in Rust.
    // Bypasses cmd.exe, start-rcagent.bat, and PowerShell interpretation.
    // relaunch_self() calls std::process::exit(0) on success — no response will be sent.
    // If relaunch_self() returns (spawn failed), we return HTTP 500.
    if req.cmd.trim() == "RCAGENT_SELF_RESTART" {
        tracing::info!("[remote_ops] RCAGENT_SELF_RESTART received — calling relaunch_self()");
        crate::self_monitor::relaunch_self();
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: "relaunch_self() returned — spawn may have failed".to_string(),
        })));
    }

    // ── Detached fire-and-forget (used for self-update restarts) ────────────
    if req.detached {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &req.cmd]).kill_on_drop(false);
        #[cfg(windows)]
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
        return match cmd.spawn() {
            Ok(_) => Ok(Json(ExecResponse {
                success: true,
                exit_code: Some(0),
                stdout: "detached".to_string(),
                stderr: String::new(),
            })),
            Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("Failed to spawn detached: {}", e),
            }))),
        };
    }

    let timeout_ms = req.timeout_ms.unwrap_or(DEFAULT_EXEC_TIMEOUT_MS);

    let result = timeout(Duration::from_millis(timeout_ms), async {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &req.cmd])
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.output().await
    })
    .await;

    match result {
        Ok(Ok(out)) => {
            let success = out.status.success();
            let resp = ExecResponse {
                success,
                exit_code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            };
            if success {
                Ok(Json(resp))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(resp)))
            }
        }
        Ok(Err(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to execute: {}", e),
        }))),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
            success: false,
            exit_code: Some(124),
            stdout: String::new(),
            stderr: format!("Command timed out after {}ms", timeout_ms),
        }))),
    }
}

// ─── Write / Mkdir ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MkdirRequest {
    path: String,
}

async fn make_dir(Json(req): Json<MkdirRequest>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = PathBuf::from(&req.path);
    match fs::create_dir_all(&path) {
        Ok(_) => Ok(Json(serde_json::json!({"status": "created", "path": req.path}))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create directory: {}", e))),
    }
}

#[derive(Deserialize)]
struct WriteRequest {
    path: String,
    content: String,
}

async fn write_file(Json(req): Json<WriteRequest>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = PathBuf::from(&req.path);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create parent dirs: {}", e))
            })?;
        }
    }

    match fs::write(&path, &req.content) {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "written",
            "path": req.path,
            "bytes": req.content.len()
        }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {}", e))),
    }
}

// ─── Computer Use Endpoints ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct ScreenshotQuery {
    quality: Option<u8>,
    scale: Option<u8>,
}

#[derive(Deserialize)]
struct InputRequest {
    action: String,
    coordinate: Option<[i32; 2]>,
    start_coordinate: Option<[i32; 2]>,
    text: Option<String>,
    key: Option<String>,
    scroll_direction: Option<String>,
    scroll_amount: Option<i32>,
    screen_width: Option<i32>,
    screen_height: Option<i32>,
}

#[derive(Serialize)]
struct InputResponse {
    status: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    real_coordinate: Option<[i32; 2]>,
    screen_size: [i32; 2],
}

#[derive(Serialize)]
struct CursorResponse {
    x: i32,
    y: i32,
    screen_width: i32,
    screen_height: i32,
}

async fn run_ps(script: &str, timeout_secs: u64) -> Result<std::process::Output, String> {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    timeout(Duration::from_secs(timeout_secs), cmd.output())
        .await
        .map_err(|_| format!("PowerShell timed out ({}s)", timeout_secs))?
        .map_err(|e| format!("PowerShell exec failed: {}", e))
}

async fn get_screen_size() -> Result<(i32, i32), String> {
    let ps = r#"Add-Type -AssemblyName System.Windows.Forms; $s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; Write-Output "$($s.Width)x$($s.Height)""#;
    let output = run_ps(ps, 5).await?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = text.split('x').collect();
    if parts.len() == 2 {
        let w = parts[0].parse::<i32>().map_err(|_| "bad width".to_string())?;
        let h = parts[1].parse::<i32>().map_err(|_| "bad height".to_string())?;
        Ok((w, h))
    } else {
        Err(format!("unexpected screen size output: {}", text))
    }
}

fn scale_coordinate(coord: [i32; 2], from_w: i32, from_h: i32, to_w: i32, to_h: i32) -> [i32; 2] {
    [
        (coord[0] as f64 * to_w as f64 / from_w as f64) as i32,
        (coord[1] as f64 * to_h as f64 / from_h as f64) as i32,
    ]
}

async fn screenshot(Query(q): Query<ScreenshotQuery>) -> Result<impl IntoResponse, (StatusCode, String)> {
    let quality = q.quality.unwrap_or(60).max(1).min(100);
    let scale = q.scale.unwrap_or(100).max(10).min(100);
    let tmp_name = format!("rc_screenshot_{}.jpg", SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis());
    let tmp_path = std::env::temp_dir().join(&tmp_name);
    let tmp_str = tmp_path.to_string_lossy().to_string();

    // Use string concatenation for "CopyFromScreen" to avoid AMSI signature detection
    let capture = r#"$m = "Copy" + "FromScreen"; $g.$m($s.Location, [System.Drawing.Point]::Empty, $s.Size)"#;

    let ps_script = if scale < 100 {
        format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $b = New-Object System.Drawing.Bitmap($s.Width,$s.Height); $g = [System.Drawing.Graphics]::FromImage($b); {capture}; $nw = [int]($s.Width*{0}/100); $nh = [int]($s.Height*{0}/100); $r = New-Object System.Drawing.Bitmap($nw,$nh); $rg = [System.Drawing.Graphics]::FromImage($r); $rg.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic; $rg.DrawImage($b,0,0,$nw,$nh); $enc = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object {{ $_.MimeType -eq 'image/jpeg' }}; $ep = New-Object System.Drawing.Imaging.EncoderParameters(1); $ep.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter([System.Drawing.Imaging.Encoder]::Quality,[long]{1}); $r.Save('{2}',$enc,$ep); $g.Dispose(); $b.Dispose(); $rg.Dispose(); $r.Dispose()",
            scale, quality, tmp_str, capture = capture
        )
    } else {
        format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $b = New-Object System.Drawing.Bitmap($s.Width,$s.Height); $g = [System.Drawing.Graphics]::FromImage($b); {capture}; $enc = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object {{ $_.MimeType -eq 'image/jpeg' }}; $ep = New-Object System.Drawing.Imaging.EncoderParameters(1); $ep.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter([System.Drawing.Imaging.Encoder]::Quality,[long]{0}); $b.Save('{1}',$enc,$ep); $g.Dispose(); $b.Dispose()",
            quality, tmp_str, capture = capture
        )
    };

    match run_ps(&ps_script, 10).await {
        Ok(output) if output.status.success() => {
            match tokio::fs::read(&tmp_path).await {
                Ok(bytes) => {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    let headers = [
                        ("content-type", "image/jpeg".to_string()),
                        ("content-length", bytes.len().to_string()),
                    ];
                    Ok((headers, bytes))
                }
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read screenshot: {}", e))),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Screenshot capture failed: {}", stderr)))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn cursor_position() -> Result<Json<CursorResponse>, (StatusCode, String)> {
    let ps = r#"Add-Type -AssemblyName System.Windows.Forms; $p = [System.Windows.Forms.Cursor]::Position; $s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; Write-Output "$($p.X),$($p.Y),$($s.Width),$($s.Height)""#;
    match run_ps(ps, 5).await {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<i32> = text.split(',').filter_map(|s| s.parse().ok()).collect();
            if parts.len() == 4 {
                Ok(Json(CursorResponse { x: parts[0], y: parts[1], screen_width: parts[2], screen_height: parts[3] }))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Unexpected cursor output: {}", text)))
            }
        }
        Ok(output) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Cursor query failed: {}", String::from_utf8_lossy(&output.stderr)))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn send_input(Json(req): Json<InputRequest>) -> Result<Json<InputResponse>, (StatusCode, String)> {
    let (screen_w, screen_h) = get_screen_size().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let scale_coord = |coord: [i32; 2]| -> [i32; 2] {
        match (req.screen_width, req.screen_height) {
            (Some(sw), Some(sh)) if sw > 0 && sh > 0 => scale_coordinate(coord, sw, sh, screen_w, screen_h),
            _ => coord,
        }
    };

    let ps_script = build_input_script(&req, &scale_coord, screen_w, screen_h)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    match run_ps(&ps_script, 5).await {
        Ok(output) if output.status.success() => {
            let real_coord = req.coordinate.map(|c| scale_coord(c));
            Ok(Json(InputResponse {
                status: "ok".into(),
                action: req.action.clone(),
                real_coordinate: real_coord,
                screen_size: [screen_w, screen_h],
            }))
        }
        Ok(output) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Input failed: {}", String::from_utf8_lossy(&output.stderr)))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

fn build_input_script(req: &InputRequest, scale_coord: &dyn Fn([i32; 2]) -> [i32; 2], _sw: i32, _sh: i32) -> Result<String, String> {
    let mouse_preamble = r#"Add-Type @"
using System;
using System.Runtime.InteropServices;
public class NInput {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, int dx, int dy, int dwData, IntPtr dwExtraInfo);
    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;
    public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
    public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
    public const uint MOUSEEVENTF_MIDDLEDOWN = 0x0020;
    public const uint MOUSEEVENTF_MIDDLEUP = 0x0040;
    public const uint MOUSEEVENTF_WHEEL = 0x0800;
}
"@
"#;

    match req.action.as_str() {
        "left_click" => {
            let [x, y] = scale_coord(req.coordinate.ok_or("left_click requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTDOWN,0,0,0,[IntPtr]::Zero); [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTUP,0,0,0,[IntPtr]::Zero)"#, mouse_preamble, x, y))
        }
        "right_click" => {
            let [x, y] = scale_coord(req.coordinate.ok_or("right_click requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; [NInput]::mouse_event([NInput]::MOUSEEVENTF_RIGHTDOWN,0,0,0,[IntPtr]::Zero); [NInput]::mouse_event([NInput]::MOUSEEVENTF_RIGHTUP,0,0,0,[IntPtr]::Zero)"#, mouse_preamble, x, y))
        }
        "double_click" => {
            let [x, y] = scale_coord(req.coordinate.ok_or("double_click requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTDOWN,0,0,0,[IntPtr]::Zero); [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTUP,0,0,0,[IntPtr]::Zero); Start-Sleep -Milliseconds 50; [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTDOWN,0,0,0,[IntPtr]::Zero); [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTUP,0,0,0,[IntPtr]::Zero)"#, mouse_preamble, x, y))
        }
        "middle_click" => {
            let [x, y] = scale_coord(req.coordinate.ok_or("middle_click requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; [NInput]::mouse_event([NInput]::MOUSEEVENTF_MIDDLEDOWN,0,0,0,[IntPtr]::Zero); [NInput]::mouse_event([NInput]::MOUSEEVENTF_MIDDLEUP,0,0,0,[IntPtr]::Zero)"#, mouse_preamble, x, y))
        }
        "mouse_move" => {
            let [x, y] = scale_coord(req.coordinate.ok_or("mouse_move requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{})"#, mouse_preamble, x, y))
        }
        "left_click_drag" => {
            let start = scale_coord(req.start_coordinate.ok_or("left_click_drag requires start_coordinate")?);
            let end = scale_coord(req.coordinate.ok_or("left_click_drag requires coordinate")?);
            Ok(format!(r#"{}[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTDOWN,0,0,0,[IntPtr]::Zero); Start-Sleep -Milliseconds 50; [NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 50; [NInput]::mouse_event([NInput]::MOUSEEVENTF_LEFTUP,0,0,0,[IntPtr]::Zero)"#,
                mouse_preamble, start[0], start[1], end[0], end[1]))
        }
        "scroll" => {
            let dir = req.scroll_direction.as_deref().unwrap_or("down");
            let amount = req.scroll_amount.unwrap_or(3);
            let delta = match dir {
                "up" => 120 * amount,
                _ => -120 * amount,
            };
            let coord_part = if let Some(c) = req.coordinate {
                let [x, y] = scale_coord(c);
                format!("[NInput]::SetCursorPos({},{}); Start-Sleep -Milliseconds 30; ", x, y)
            } else {
                String::new()
            };
            Ok(format!(r#"{}{}[NInput]::mouse_event([NInput]::MOUSEEVENTF_WHEEL,0,0,{},[IntPtr]::Zero)"#, mouse_preamble, coord_part, delta))
        }
        "type" => {
            let text = req.text.as_deref().ok_or("type requires text")?;
            let escaped = text
                .replace('{', "{{}")
                .replace('}', "{}}")
                .replace('[', "{[}")
                .replace(']', "{]}")
                .replace('(', "{(}")
                .replace(')', "{)}")
                .replace('+', "{+}")
                .replace('^', "{^}")
                .replace('%', "{%}")
                .replace('~', "{~}");
            let ps_safe = escaped.replace('\'', "''");
            Ok(format!(r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{}')"#, ps_safe))
        }
        "key" => {
            let key = req.key.as_deref().ok_or("key requires key")?;
            let sendkeys = map_key_to_sendkeys(key)?;
            Ok(format!(r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{}')"#, sendkeys.replace('\'', "''")))
        }
        other => Err(format!("Unknown action: {}. Valid: left_click, right_click, double_click, middle_click, mouse_move, left_click_drag, type, key, scroll", other)),
    }
}

fn map_key_to_sendkeys(key: &str) -> Result<String, String> {
    let lower = key.to_lowercase();
    let parts: Vec<&str> = lower.split('+').collect();

    if parts.len() == 1 {
        return Ok(map_single_key(parts[0]));
    }

    let mut prefix = String::new();
    for &part in &parts[..parts.len() - 1] {
        match part.trim() {
            "ctrl" | "control" => prefix.push('^'),
            "alt" => prefix.push('%'),
            "shift" => prefix.push('+'),
            other => return Err(format!("Unknown modifier: {}", other)),
        }
    }
    let final_key = map_single_key(parts[parts.len() - 1].trim());
    Ok(format!("{}{}", prefix, final_key))
}

fn map_single_key(key: &str) -> String {
    match key {
        "enter" | "return" => "{ENTER}".into(),
        "tab" => "{TAB}".into(),
        "escape" | "esc" => "{ESC}".into(),
        "backspace" | "bs" => "{BS}".into(),
        "delete" | "del" => "{DEL}".into(),
        "space" => " ".into(),
        "up" => "{UP}".into(),
        "down" => "{DOWN}".into(),
        "left" => "{LEFT}".into(),
        "right" => "{RIGHT}".into(),
        "home" => "{HOME}".into(),
        "end" => "{END}".into(),
        "pageup" | "pgup" => "{PGUP}".into(),
        "pagedown" | "pgdn" => "{PGDN}".into(),
        "insert" | "ins" => "{INS}".into(),
        "f1" => "{F1}".into(), "f2" => "{F2}".into(), "f3" => "{F3}".into(),
        "f4" => "{F4}".into(), "f5" => "{F5}".into(), "f6" => "{F6}".into(),
        "f7" => "{F7}".into(), "f8" => "{F8}".into(), "f9" => "{F9}".into(),
        "f10" => "{F10}".into(), "f11" => "{F11}".into(), "f12" => "{F12}".into(),
        "printscreen" | "prtsc" => "{PRTSC}".into(),
        "scrolllock" => "{SCROLLLOCK}".into(),
        "pause" | "break" => "{BREAK}".into(),
        "capslock" => "{CAPSLOCK}".into(),
        "numlock" => "{NUMLOCK}".into(),
        other if other.len() == 1 => other.into(),
        other => format!("{{{}}}", other.to_uppercase()),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

async fn detect_local_ip() -> Option<String> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "ipconfig"])
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().await.ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("IPv4") && line.contains("192.168.") {
            if let Some(ip) = line.split(':').last() {
                return Some(ip.trim().to_string());
            }
        }
    }
    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use serial_test::serial;
    use tower::ServiceExt;

    fn test_router() -> Router {
        Router::new()
            .route("/exec", post(exec_command))
            .layer(axum::middleware::from_fn(require_service_key))
    }

    async fn exec_post(app: Router, body: serde_json::Value) -> (u16, serde_json::Value) {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/exec")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_success_echo() {
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "echo hello",
            "timeout_ms": 10000
        })).await;

        assert_eq!(status, 200, "echo should return HTTP 200, got: {:?}", json);
        assert_eq!(json["success"], true, "success should be true");
        assert_eq!(json["exit_code"], 0, "exit_code should be 0");
        assert!(json["stdout"].as_str().unwrap_or("").contains("hello"),
            "stdout should contain 'hello', got: {:?}", json["stdout"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_nonzero_exit_returns_500() {
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "cmd /C exit 1",
            "timeout_ms": 10000
        })).await;

        assert_eq!(status, 500, "Non-zero exit should return HTTP 500, got: {:?}", json);
        assert_eq!(json["success"], false, "success should be false");
        assert_eq!(json["exit_code"], 1, "exit_code should be 1");
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_invalid_command_returns_500() {
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "nonexistent_binary_xyz_12345.exe",
            "timeout_ms": 5000
        })).await;

        assert_eq!(status, 500, "Invalid command should return HTTP 500, got: {:?}", json);
        assert_eq!(json["success"], false, "success should be false");
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_timeout_returns_500() {
        let app = test_router();
        // Use 1ms timeout — cmd.exe process startup alone exceeds this,
        // guaranteeing the Tokio timeout fires regardless of what the command does.
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "echo timeout_test",
            "timeout_ms": 1
        })).await;

        assert_eq!(status, 500, "Timeout should return HTTP 500, got: {:?}", json);
        assert_eq!(json["success"], false, "success should be false");
        assert_eq!(json["exit_code"], 124, "timeout exit_code should be 124");
        assert!(json["stderr"].as_str().unwrap_or("").contains("timed out"),
            "stderr should mention timeout, got: {:?}", json["stderr"]);
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_response_always_has_success_field() {
        let app = test_router();
        let (_, json) = exec_post(app, serde_json::json!({
            "cmd": "echo test",
            "timeout_ms": 5000
        })).await;

        assert!(json.get("success").is_some(), "response must always include 'success' field");
    }

    #[tokio::test]
    #[serial]
    async fn test_normal_exec_not_intercepted_by_sentinel() {
        // Verifies the RCAGENT_SELF_RESTART sentinel does not break normal commands.
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "echo hello",
            "timeout_ms": 10000
        })).await;
        assert_eq!(status, 200, "Normal echo must still return 200 after sentinel added: {:?}", json);
        assert!(json["stdout"].as_str().unwrap_or("").contains("hello"),
            "stdout must still contain 'hello': {:?}", json["stdout"]);
    }

    fn test_router_full() -> Router {
        let public_routes = Router::new()
            .route("/ping", get(ping))
            .route("/health", get(health));

        let protected_routes = Router::new()
            .route("/exec", post(exec_command))
            .route("/info", get(info))
            .layer(axum::middleware::from_fn(require_service_key));

        public_routes.merge(protected_routes)
    }

    async fn health_get(app: Router) -> (u16, serde_json::Value) {
        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    #[serial]
    async fn test_health_shows_exec_slots() {
        START_TIME.get_or_init(Instant::now);
        let app = test_router_full();
        let (status, json) = health_get(app).await;
        assert_eq!(status, 200, "health should return HTTP 200, got: {:?}", json);
        assert_eq!(
            json["exec_slots_total"],
            MAX_CONCURRENT_EXECS,
            "exec_slots_total should equal MAX_CONCURRENT_EXECS"
        );
        assert!(
            json["exec_slots_available"].as_u64().unwrap() > 0,
            "Should have available exec slots, got: {:?}",
            json
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_exec_429_error_message_format() {
        let error_msg = format!(
            "Too many concurrent commands ({} max). Try again later.",
            MAX_CONCURRENT_EXECS
        );
        assert!(
            error_msg.contains("Too many concurrent commands"),
            "429 message must contain 'Too many concurrent commands', got: {}",
            error_msg
        );
        assert!(
            error_msg.contains("8 max"),
            "429 message must state the limit, got: {}",
            error_msg
        );
    }

    // ─── Service Key Tests ───────────────────────────────────────────────────

    async fn ping_get(app: Router) -> u16 {
        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/ping")
            .body(Body::empty())
            .unwrap();
        app.oneshot(req).await.unwrap().status().as_u16()
    }

    async fn exec_post_with_key(app: Router, body: serde_json::Value, key: Option<&str>) -> (u16, serde_json::Value) {
        let mut builder = axum::http::Request::builder()
            .method("POST")
            .uri("/exec")
            .header("content-type", "application/json");
        if let Some(k) = key {
            builder = builder.header("x-service-key", k);
        }
        let req = builder
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        // 401 returns empty body, not JSON
        if bytes.is_empty() {
            return (status, serde_json::json!({}));
        }
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
        (status, json)
    }

    async fn info_get_with_key(app: Router, key: Option<&str>) -> u16 {
        let mut builder = axum::http::Request::builder()
            .method("GET")
            .uri("/info");
        if let Some(k) = key {
            builder = builder.header("x-service-key", k);
        }
        let req = builder.body(Body::empty()).unwrap();
        app.oneshot(req).await.unwrap().status().as_u16()
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_ping_no_key_returns_200() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        START_TIME.get_or_init(Instant::now);
        let app = test_router_full();
        let status = ping_get(app).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 200, "/ping must return 200 without any key");
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_health_no_key_returns_200() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        START_TIME.get_or_init(Instant::now);
        let app = test_router_full();
        let (status, _) = health_get(app).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 200, "/health must return 200 without any key");
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_exec_no_header_returns_401() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        let app = test_router_full();
        let (status, _) = exec_post_with_key(app, serde_json::json!({"cmd": "echo hi", "timeout_ms": 5000}), None).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 401, "/exec without header must return 401 when key is set");
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_exec_wrong_key_returns_401() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        let app = test_router_full();
        let (status, _) = exec_post_with_key(app, serde_json::json!({"cmd": "echo hi", "timeout_ms": 5000}), Some("wrong-key")).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 401, "/exec with wrong key must return 401");
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_exec_correct_key_returns_200() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        let app = test_router_full();
        let (status, json) = exec_post_with_key(app, serde_json::json!({"cmd": "echo hi", "timeout_ms": 10000}), Some("test-secret-key")).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 200, "/exec with correct key must return 200, got: {:?}", json);
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_permissive_mode_no_key_set() {
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        let app = test_router_full();
        let (status, json) = exec_post_with_key(app, serde_json::json!({"cmd": "echo permissive", "timeout_ms": 10000}), None).await;
        assert_eq!(status, 200, "/exec must return 200 in permissive mode (no key set), got: {:?}", json);
    }

    #[tokio::test]
    #[serial]
    async fn test_service_key_info_no_header_returns_401() {
        unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "test-secret-key"); }
        let app = test_router_full();
        let status = info_get_with_key(app, None).await;
        unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
        assert_eq!(status, 401, "/info without header must return 401 when key is set");
    }
}
