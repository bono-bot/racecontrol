//! Server operations HTTP endpoint (port 8090).
//!
//! Provides the same API contract as rc-agent's remote_ops, enabling
//! remote command execution, file operations, and system info on the
//! server machine. This is the "permanent connection" — since racecontrol
//! auto-starts via HKLM Run key, this endpoint is always available.
//!
//! Endpoints:
//!   GET  /ping       — "pong"
//!   GET  /health     — uptime, exec slots, version
//!   GET  /info       — hostname, IP, OS, memory, CPU
//!   POST /exec       — execute shell command (semaphore-gated, CREATE_NO_WINDOW)
//!   GET  /files      — list directory contents
//!   GET  /file       — read file (max 50MB)
//!   POST /write      — write file (binary via base64 or text)
//!   POST /mkdir      — create directory

use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
};
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
const MAX_CONCURRENT_EXECS: usize = 4;
const DEFAULT_EXEC_TIMEOUT_MS: u64 = 30_000;
const SERVER_OPS_PORT: u16 = 8090;

static EXEC_SEMAPHORE: Semaphore = Semaphore::const_new(MAX_CONCURRENT_EXECS);
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Start the server ops HTTP endpoint on port 8090.
/// Spawns an async task — returns immediately.
pub fn start() {
    START_TIME.get_or_init(Instant::now);

    tokio::spawn(async move {
        let app = Router::new()
            .route("/ping", get(ping))
            .route("/health", get(health))
            .route("/info", get(info))
            .route("/files", get(list_files))
            .route("/file", get(read_file))
            .route("/exec", post(exec_command))
            .route("/mkdir", post(make_dir))
            .route("/write", post(write_file));

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], SERVER_OPS_PORT));

        // Retry binding with SO_REUSEADDR to handle stale sockets
        let listener = {
            let mut bound = None;
            for attempt in 1..=10 {
                let sock = match socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("[server_ops] Failed to create socket: {}", e);
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
                                tracing::info!("[server_ops] Listening on http://{}", addr);
                                bound = Some(l);
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("[server_ops] Failed to convert listener (attempt {}): {}", attempt, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[server_ops] Port {} busy (attempt {}/10): {} — retrying in 3s",
                            SERVER_OPS_PORT, attempt, e
                        );
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
            match bound {
                Some(l) => l,
                None => {
                    tracing::error!("[server_ops] Failed to bind port {} after 10 attempts", SERVER_OPS_PORT);
                    return;
                }
            }
        };

        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("[server_ops] Server error: {}", e);
        }
    });
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
        "service": "racecontrol",
        "version": VERSION,
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
        service_version: VERSION.to_string(),
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
    service_version: String,
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
                "[server_ops] All {} exec slots occupied. Returning 429.",
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

    tracing::info!("[server_ops] exec: {}", &req.cmd);

    // Detached fire-and-forget
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
    /// If true, content is base64-encoded binary data
    #[serde(default)]
    base64: bool,
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

    let bytes = if req.base64 {
        use std::io::Read;
        let mut decoder = base64_decode_reader(req.content.as_bytes());
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("Invalid base64: {}", e))
        })?;
        buf
    } else {
        req.content.into_bytes()
    };

    let len = bytes.len();
    match fs::write(&path, &bytes) {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "written",
            "path": req.path.clone(),
            "bytes": len
        }))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {}", e))),
    }
}

/// Simple base64 decoder (avoids adding a dependency)
fn base64_decode_reader(input: &[u8]) -> std::io::Cursor<Vec<u8>> {
    let mut output = Vec::new();
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in input {
        if byte == b'=' || byte == b'\n' || byte == b'\r' || byte == b' ' {
            continue;
        }
        let val = match table.iter().position(|&b| b == byte) {
            Some(v) => v as u32,
            None => continue,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    std::io::Cursor::new(output)
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
    use tower::ServiceExt;

    fn test_router() -> Router {
        Router::new()
            .route("/exec", post(exec_command))
            .route("/health", get(health))
            .route("/ping", get(ping))
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
    async fn test_exec_echo() {
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "echo hello",
            "timeout_ms": 10000
        })).await;
        assert_eq!(status, 200);
        assert_eq!(json["success"], true);
        assert!(json["stdout"].as_str().unwrap_or("").contains("hello"));
    }

    #[tokio::test]
    async fn test_exec_timeout() {
        let app = test_router();
        let (status, json) = exec_post(app, serde_json::json!({
            "cmd": "echo timeout_test",
            "timeout_ms": 1
        })).await;
        assert_eq!(status, 500);
        assert_eq!(json["success"], false);
    }

    #[tokio::test]
    async fn test_health() {
        START_TIME.get_or_init(Instant::now);
        let app = test_router();
        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["service"], "racecontrol");
    }

    #[tokio::test]
    async fn test_base64_decoder() {
        use std::io::Read;
        let mut reader = base64_decode_reader(b"SGVsbG8gV29ybGQ=");
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "Hello World");
    }
}
