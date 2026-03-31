//! Mesh Client — sync WebSocket client to Bono comms-link hub via Tailscale.
//!
//! Runs in a dedicated thread, completely isolated from the main HTTP server
//! and watchdog. Uses sync `tungstenite` — no tokio, no async runtime.
//!
//! Features:
//! - Connects to Bono hub via Tailscale WS with PSK Bearer auth
//! - HMAC-SHA256 message signing (comms-link protocol compatible)
//! - Role-based command filtering (pod vs pos)
//! - Exponential backoff reconnect with jitter
//! - Heartbeat every N seconds
//! - Bridges exec_request to local rc_common::exec::run_cmd_sync()

use crate::sentry_config::MeshConfig;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::http::Request;
use tungstenite::{connect, Message};

type HmacSha256 = Hmac<Sha256>;

/// Commands blocked for POS role (safety — POS must never control games).
const POS_BLOCKED: &[&str] = &[
    "game_launch", "stop_game", "ffb_control", "lock_screen",
    "blank_screen", "process_guard", "kill_switch",
    "game_state", "fleet_learning",
];

/// Start the mesh client in a background thread.
/// Returns the JoinHandle. The thread runs until `shutdown` is set to true.
pub fn spawn(config: &MeshConfig, shutdown: &'static AtomicBool) -> std::thread::JoinHandle<()> {
    let config = config.clone();
    std::thread::Builder::new()
        .name("mesh-client".to_string())
        .spawn(move || run_loop(&config, shutdown))
        .expect("spawn mesh-client thread")
}

fn run_loop(config: &MeshConfig, shutdown: &AtomicBool) {
    let mut attempt: u32 = 0;

    loop {
        if shutdown.load(Ordering::Acquire) {
            tracing::info!(target: "mesh", "shutdown requested — exiting mesh client");
            return;
        }

        match try_connect(config) {
            Ok(mut ws) => {
                attempt = 0;
                tracing::info!(target: "mesh", node_id = %config.node_id, role = %config.role, "connected to hub");

                // Send registration heartbeat
                let _ = send_heartbeat(&mut ws, config);

                // Message loop
                if let Err(e) = message_loop(&mut ws, config, shutdown) {
                    tracing::warn!(target: "mesh", "connection lost: {e}");
                }
            }
            Err(e) => {
                tracing::warn!(target: "mesh", attempt, "connect failed: {e}");
            }
        }

        if shutdown.load(Ordering::Acquire) {
            return;
        }

        // Exponential backoff with jitter
        attempt += 1;
        let base_ms = 1000u64 * 2u64.pow(attempt.min(5));
        let max_ms = 30_000u64;
        let delay_ms = base_ms.min(max_ms);
        // Simple jitter: 0-25%
        let jitter = (delay_ms as f64 * 0.25 * rand_f64()) as u64;
        let total = delay_ms + jitter;
        tracing::info!(target: "mesh", attempt, delay_ms = total, "reconnecting");
        std::thread::sleep(Duration::from_millis(total));
    }
}

fn try_connect(config: &MeshConfig) -> Result<tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>, Box<dyn std::error::Error>> {
    let request = Request::builder()
        .uri(&config.hub_url)
        .header("Authorization", format!("Bearer {}", config.psk))
        .header("Host", extract_host(&config.hub_url))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", tungstenite::handshake::client::generate_key())
        .body(())?;

    let (ws, _response) = connect(request)?;
    Ok(ws)
}

fn message_loop(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
    shutdown: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error>> {
    let heartbeat_interval = Duration::from_secs(config.heartbeat_secs);
    let mut last_heartbeat = Instant::now();

    // Set read timeout so we can check shutdown + send heartbeats
    match ws.get_ref() {
        tungstenite::stream::MaybeTlsStream::Plain(stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        }
        _ => {}
    }

    loop {
        if shutdown.load(Ordering::Acquire) {
            let _ = ws.close(None);
            return Ok(());
        }

        // Send heartbeat if due
        if last_heartbeat.elapsed() >= heartbeat_interval {
            send_heartbeat(ws, config)?;
            last_heartbeat = Instant::now();
        }

        // Read next message (with timeout from set_read_timeout)
        match ws.read() {
            Ok(Message::Text(text)) => {
                handle_message(ws, config, &text);
            }
            Ok(Message::Ping(data)) => {
                let _ = ws.send(Message::Pong(data));
            }
            Ok(Message::Close(_)) => {
                return Err("server closed connection".into());
            }
            Ok(_) => {} // Binary, Pong — ignore
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                // Read timeout — expected, loop back to check heartbeat/shutdown
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

fn handle_message(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
    text: &str,
) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "mesh", "invalid message JSON: {e}");
            return;
        }
    };

    let msg_type = msg["type"].as_str().unwrap_or("");
    let correlation_id = msg["payload"]["correlationId"].as_str().unwrap_or("");

    match msg_type {
        "exec_request" => {
            let command = msg["payload"]["command"].as_str().unwrap_or("");
            let exec_id = msg["payload"]["execId"].as_str().unwrap_or("");

            // Role-based filtering
            if config.role == "pos" && POS_BLOCKED.iter().any(|&b| b == command) {
                tracing::warn!(target: "mesh", command, role = %config.role, "command blocked by role filter");
                let _ = send_exec_result(ws, config, exec_id, command, 403, "",
                    &format!("Command '{}' not allowed for role '{}'", command, config.role),
                    correlation_id);
                return;
            }

            tracing::info!(target: "mesh", command, exec_id, correlation_id, "executing via mesh");

            // Execute using rc_common::exec::run_cmd_sync (same as HTTP /exec)
            let result = rc_common::exec::run_cmd_sync(
                command,
                Duration::from_secs(30),
                64 * 1024,
            );

            let _ = send_exec_result(ws, config, exec_id, command,
                result.exit_code, &result.stdout, &result.stderr, correlation_id);
        }
        "echo" => {
            // Reply with echo_reply
            let payload = msg["payload"].clone();
            let reply = build_message("echo_reply", &config.node_id, &payload, &config.psk);
            let _ = ws.send(Message::Text(reply.into()));
        }
        "replay_request" => {
            // Not yet supported — ignore gracefully
            tracing::info!(target: "mesh", "replay_request received (not yet supported)");
        }
        _ => {
            tracing::debug!(target: "mesh", msg_type, "unhandled message type");
        }
    }
}

fn send_exec_result(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
    exec_id: &str, command: &str, exit_code: i32, stdout: &str, stderr: &str,
    correlation_id: &str,
) -> Result<(), tungstenite::Error> {
    let payload = serde_json::json!({
        "execId": exec_id,
        "command": command,
        "exitCode": exit_code,
        "stdout": &stdout[..stdout.len().min(50_000)],
        "stderr": &stderr[..stderr.len().min(10_000)],
        "source": "mesh-sentry",
        "nodeId": config.node_id,
        "role": config.role,
        "correlationId": correlation_id,
    });
    let msg = build_message("exec_result", &config.node_id, &payload, &config.psk);
    ws.send(Message::Text(msg.into()))
}

fn send_heartbeat(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
) -> Result<(), tungstenite::Error> {
    let uptime = crate::START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let hostname = sysinfo::System::host_name().unwrap_or_default();
    let payload = serde_json::json!({
        "nodeId": config.node_id,
        "role": config.role,
        "hostname": hostname,
        "type": "mesh_heartbeat",
        "uptime": uptime,
    });
    let msg = build_message("heartbeat", &config.node_id, &payload, &config.psk);
    ws.send(Message::Text(msg.into()))
}

// ---------------------------------------------------------------------------
// Comms-link compatible message builder with HMAC
// ---------------------------------------------------------------------------

/// Build a JSON message compatible with comms-link protocol.
/// Signs with HMAC-SHA256 if PSK is provided.
fn build_message(msg_type: &str, from: &str, payload: &serde_json::Value, psk: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let id = format!("{:016x}", ts ^ rand_u64());
    // Use a simple incrementing sequence per session
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);

    let mut msg = serde_json::json!({
        "v": 1,
        "type": msg_type,
        "from": from,
        "ts": ts,
        "id": id,
        "seq": seq,
        "payload": payload,
    });

    // HMAC sign if PSK is provided
    if !psk.is_empty() {
        let sign_payload = serde_json::json!({
            "v": 1, "type": msg_type, "from": from, "ts": ts,
            "id": id, "seq": seq, "payload": payload,
        });
        let mac = hmac_sign(psk, &sign_payload.to_string());
        msg["mac"] = serde_json::Value::String(mac);
    }

    msg.to_string()
}

fn hmac_sign(key: &str, data: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

// Simple deterministic "random" without pulling in rand crate
fn rand_u64() -> u64 {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    t.as_nanos() as u64 ^ (std::process::id() as u64) << 32
}

fn rand_f64() -> f64 {
    (rand_u64() % 1000) as f64 / 1000.0
}

fn extract_host(url: &str) -> String {
    url.replace("ws://", "").replace("wss://", "").split('/').next().unwrap_or("").to_string()
}
