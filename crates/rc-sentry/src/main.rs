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
use std::process::Command;
use std::time::Duration;

const DEFAULT_PORT: u16 = 8091;
const MAX_BODY: usize = 64 * 1024;

fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("rc-sentry: bind :{port} failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("rc-sentry listening on :{port}");

    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || {
            if let Err(e) = handle(stream) {
                eprintln!("rc-sentry: {e}");
            }
        });
    }
}

fn handle(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut buf = vec![0u8; MAX_BODY];
    let n = stream.read(&mut buf)?;
    let request = std::str::from_utf8(&buf[..n])?;

    // Parse HTTP request line
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let (method, path) = match parts.as_slice() {
        [m, p, ..] => (*m, *p),
        _ => return send_response(&mut stream, 400, "Bad Request"),
    };

    match (method, path) {
        ("GET", "/ping") => send_plain(&mut stream, 200, "pong"),
        ("POST", "/exec") => handle_exec(&mut stream, request),
        ("OPTIONS", _) => send_cors_preflight(&mut stream),
        _ => send_response(&mut stream, 404, r#"{"error":"not found"}"#),
    }
}

fn handle_exec(stream: &mut TcpStream, request: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Find JSON body after the empty line
    let body = request
        .find("\r\n\r\n")
        .map(|i| &request[i + 4..])
        .or_else(|| request.find("\n\n").map(|i| &request[i + 2..]))
        .unwrap_or("");

    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let cmd = parsed["cmd"].as_str().unwrap_or("");
    let _timeout_ms = parsed["timeout_ms"].as_u64().unwrap_or(30_000);

    if cmd.is_empty() {
        return send_response(stream, 400, r#"{"error":"missing cmd"}"#);
    }

    // Internal LAN admin tool — cmd.exe shell execution is intentional (equivalent to SSH).
    // Only accessible on private subnet 192.168.31.x, no public exposure.
    let result = Command::new("cmd.exe")
        .args(["/C", cmd])
        .output();

    let resp = match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            serde_json::json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": output.status.code().unwrap_or(-1),
            })
        }
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    };

    send_response(stream, 200, &resp.to_string())
}

fn send_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
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

fn send_plain(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), Box<dyn std::error::Error>> {
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
