//! rc-sentry — lightweight backup remote exec service.
//!
//! Runs on port 8091 (server + pods), independent of racecontrol/rc-agent.
//! Provides /ping and /exec endpoints so we never lose remote access during deploys.
//! No tokio, no async — pure std::net for minimal binary size and zero shared deps.
//!
//! SECURITY: This is an internal-only tool for LAN management of Racing Point pods.
//! It binds to 0.0.0.0 on a private subnet (192.168.31.x) with no auth.
//! NOT intended for public networks. The cmd.exe invocation is intentional —
//! this is a remote admin tool equivalent to SSH, scoped to venue hardware.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const DEFAULT_PORT: u16 = 8091;
const MAX_BODY: usize = 64 * 1024; // 64KB max request/output size
const MAX_EXEC_SLOTS: usize = 4;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

static EXEC_SLOTS: AtomicUsize = AtomicUsize::new(0);
static THREAD_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct SlotGuard;

impl SlotGuard {
    fn acquire() -> Option<Self> {
        loop {
            let current = EXEC_SLOTS.load(Ordering::Acquire);
            if current >= MAX_EXEC_SLOTS {
                return None;
            }
            match EXEC_SLOTS.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(SlotGuard),
                Err(_) => continue,
            }
        }
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        EXEC_SLOTS.fetch_sub(1, Ordering::Release);
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("bind :{port} failed: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("rc-sentry listening on :{port}");

    for stream in listener.incoming().flatten() {
        let n = THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
        if let Err(e) = std::thread::Builder::new()
            .name(format!("sentry-handler-{}", n))
            .spawn(move || {
                if let Err(e) = handle(stream) {
                    tracing::warn!("handler error: {e}");
                }
            })
        {
            tracing::error!("thread spawn failed: {e}");
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];

    // Read until we have the full header (ends with \r\n\r\n)
    let header_end = loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err("connection closed".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > MAX_BODY {
            return Err("request too large".into());
        }
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };

    let header_str = std::str::from_utf8(&buf[..header_end])?;
    let content_length = header_str
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let body_start = header_end + 4; // skip \r\n\r\n
    let body_received = buf.len().saturating_sub(body_start);
    let mut remaining = content_length.saturating_sub(body_received).min(MAX_BODY);

    // Read remaining body bytes
    while remaining > 0 {
        let to_read = remaining.min(4096);
        let n = stream.read(&mut tmp[..to_read])?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        remaining = remaining.saturating_sub(n);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn handle(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let request = read_request(&mut stream)?;

    // Parse HTTP request line
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let (method, path) = match parts.as_slice() {
        [m, p, ..] => (*m, *p),
        _ => return send_response(&mut stream, 400, "Bad Request"),
    };

    match (method, path) {
        ("GET", "/ping") => send_plain(&mut stream, 200, "pong"),
        ("POST", "/exec") => handle_exec(&mut stream, &request),
        ("OPTIONS", _) => send_cors_preflight(&mut stream),
        _ => send_response(&mut stream, 404, r#"{"error":"not found"}"#),
    }
}

fn handle_exec(stream: &mut TcpStream, request: &str) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = match SlotGuard::acquire() {
        Some(g) => g,
        None => {
            tracing::warn!("exec rejected: all {MAX_EXEC_SLOTS} slots in use");
            return send_response(stream, 429, r#"{"error":"too many concurrent requests"}"#);
        }
    };

    // Find JSON body after the empty line
    let body = request
        .find("\r\n\r\n")
        .map(|i| &request[i + 4..])
        .or_else(|| request.find("\n\n").map(|i| &request[i + 2..]))
        .unwrap_or("");

    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let cmd = parsed["cmd"].as_str().unwrap_or("");
    let timeout_ms = parsed["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);

    if cmd.is_empty() {
        return send_response(stream, 400, r#"{"error":"missing cmd"}"#);
    }

    tracing::info!(cmd = cmd, timeout_ms = timeout_ms, "exec request");

    let result = rc_common::exec::run_cmd_sync(
        cmd,
        Duration::from_millis(timeout_ms),
        MAX_BODY,
    );

    let resp = serde_json::json!({
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.exit_code,
        "timed_out": result.timed_out,
        "truncated": result.truncated,
    });

    send_response(stream, 200, &resp.to_string())
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn send_plain(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = format!(
        "HTTP/1.1 {status} OK\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn send_cors_preflight(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let response = "HTTP/1.1 204 No Content\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Connection: close\r\n\
         \r\n";
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}
