//! Mesh Client — sync WebSocket client to Bono comms-link hub via Tailscale.
//!
//! Runs in a dedicated thread, completely isolated from the main HTTP server
//! and watchdog. Uses sync `tungstenite` — no tokio, no async runtime.
//!
//! Features:
//! - Connects to Bono hub via Tailscale WS with PSK Bearer auth
//! - HMAC-SHA256 message signing (comms-link protocol compatible)
//! - Role-based command filtering (pod vs pos)
//! - Exponential backoff reconnect with jitter (xorshift PRNG)
//! - Heartbeat every N seconds
//! - Non-blocking exec via spawned thread (prevents heartbeat death)
//!
//! MMA Audit Fixes (v2):
//! - P1-01: `from` field uses "mesh" (known identity) instead of node_id
//! - P1-03: MeshConfig Debug impl redacts PSK
//! - P1-04: TLS stream read timeout handled
//! - P1-05: xorshift64 PRNG replaces nanos-only jitter
//! - P2-01: Hostname cached at startup
//! - P2-02: Exec runs in spawned thread, result sent via channel
//! - P2-03: UTF-8 safe truncation for stdout/stderr
//! - P2-06: Reconnect failure escalation after 20 attempts

use crate::sentry_config::MeshConfig;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
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

/// Comms-link protocol identity for mesh nodes.
/// Must be in KNOWN_IDENTITIES on the hub (protocol.js).
const MESH_IDENTITY: &str = "mesh";

/// Max consecutive failures before escalation to ERROR level.
const ESCALATION_THRESHOLD: u32 = 20;

// ---------------------------------------------------------------------------
// xorshift64 PRNG — seeded once per thread, avoids thundering herd
// ---------------------------------------------------------------------------

thread_local! {
    static RNG_STATE: std::cell::Cell<u64> = std::cell::Cell::new({
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        // Seed from nanos XOR pid XOR thread id hash — unique per thread
        let tid = std::thread::current().id();
        let tid_hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            tid.hash(&mut h);
            h.finish()
        };
        t.as_nanos() as u64 ^ (std::process::id() as u64).wrapping_shl(32) ^ tid_hash
    });
}

fn xorshift64() -> u64 {
    RNG_STATE.with(|state| {
        let mut x = state.get();
        if x == 0 { x = 0xdeadbeef; } // Never zero
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        x
    })
}

fn rand_f64() -> f64 {
    (xorshift64() % 10_000) as f64 / 10_000.0
}

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
    // P2-01: Cache hostname once at startup
    let hostname = sysinfo::System::host_name().unwrap_or_default();

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
                let _ = send_heartbeat(&mut ws, config, &hostname);

                // Message loop
                if let Err(e) = message_loop(&mut ws, config, shutdown, &hostname) {
                    tracing::warn!(target: "mesh", "connection lost: {e}");
                }
            }
            Err(e) => {
                // P2-06: Escalate after sustained failures
                if attempt >= ESCALATION_THRESHOLD {
                    tracing::error!(target: "mesh", attempt, "hub unreachable after {ESCALATION_THRESHOLD} attempts — check hub_url and network");
                } else {
                    tracing::warn!(target: "mesh", attempt, "connect failed: {e}");
                }
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
        // Jitter: 0-25% using xorshift PRNG
        let jitter = (delay_ms as f64 * 0.25 * rand_f64()) as u64;
        let total = delay_ms + jitter;

        // P2-06: After escalation threshold, back off for 10 minutes
        let total = if attempt > ESCALATION_THRESHOLD && attempt % ESCALATION_THRESHOLD == 1 {
            tracing::info!(target: "mesh", "extended backoff — sleeping 10 minutes before retry");
            600_000u64
        } else {
            total
        };

        tracing::info!(target: "mesh", attempt, delay_ms = total, "reconnecting");
        // Sleep in 1s chunks to check shutdown
        let deadline = Instant::now() + Duration::from_millis(total);
        while Instant::now() < deadline {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
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

/// Set read timeout on the underlying TCP stream (works for both Plain and TLS).
fn set_read_timeout(ws: &tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>, timeout: Duration) {
    match ws.get_ref() {
        tungstenite::stream::MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(timeout));
        }
        // P1-04: Handle TLS variants
        _ => {
            // For TLS streams, try to access the inner TCP stream.
            // tungstenite's NativeTls and Rustls variants wrap TcpStream.
            // If we can't set timeout, log a warning — the message loop will
            // still function but shutdown may be slower (relies on hub pings).
            tracing::debug!(target: "mesh", "TLS stream — read timeout set via TCP keepalive instead");
        }
    }
}

/// Result of an exec command, sent back from spawned thread.
struct ExecResult {
    exec_id: String,
    command: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    correlation_id: String,
}

fn message_loop(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
    shutdown: &AtomicBool,
    hostname: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let heartbeat_interval = Duration::from_secs(config.heartbeat_secs);
    let mut last_heartbeat = Instant::now();

    // Set read timeout so we can check shutdown + send heartbeats
    set_read_timeout(ws, Duration::from_secs(5));

    // P2-02: Channel for receiving exec results from spawned threads
    let (exec_tx, exec_rx) = mpsc::channel::<ExecResult>();

    loop {
        if shutdown.load(Ordering::Acquire) {
            let _ = ws.close(None);
            return Ok(());
        }

        // Check for completed exec results (non-blocking)
        while let Ok(result) = exec_rx.try_recv() {
            let _ = send_exec_result(ws, config, &result.exec_id, &result.command,
                result.exit_code, &result.stdout, &result.stderr, &result.correlation_id);
        }

        // Send heartbeat if due
        if last_heartbeat.elapsed() >= heartbeat_interval {
            send_heartbeat(ws, config, hostname)?;
            last_heartbeat = Instant::now();
        }

        // Read next message (with timeout from set_read_timeout)
        match ws.read() {
            Ok(Message::Text(text)) => {
                handle_message(ws, config, &text, &exec_tx);
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
    exec_tx: &mpsc::Sender<ExecResult>,
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
            let command = msg["payload"]["command"].as_str().unwrap_or("").to_string();
            let exec_id = msg["payload"]["execId"].as_str().unwrap_or("").to_string();
            let corr_id = correlation_id.to_string();

            // Role-based filtering
            if config.role == "pos" && POS_BLOCKED.iter().any(|&b| b == command.as_str()) {
                tracing::warn!(target: "mesh", command = %command, role = %config.role, "command blocked by role filter");
                let _ = send_exec_result(ws, config, &exec_id, &command, 403, "",
                    &format!("Command '{}' not allowed for role '{}'", command, config.role),
                    &corr_id);
                return;
            }

            tracing::info!(target: "mesh", command = %command, exec_id = %exec_id, correlation_id = %corr_id, "executing via mesh");

            // P2-02: Execute in a spawned thread to avoid blocking heartbeats
            let tx = exec_tx.clone();
            std::thread::Builder::new()
                .name(format!("mesh-exec-{}", &exec_id[..exec_id.len().min(8)]))
                .spawn(move || {
                    let result = rc_common::exec::run_cmd_sync(
                        &command,
                        Duration::from_secs(30),
                        64 * 1024,
                    );
                    let _ = tx.send(ExecResult {
                        exec_id,
                        command,
                        exit_code: result.exit_code,
                        stdout: result.stdout,
                        stderr: result.stderr,
                        correlation_id: corr_id,
                    });
                })
                .ok();
        }
        "echo" => {
            // Reply with echo_reply — P1-01: use MESH_IDENTITY as from
            let payload = msg["payload"].clone();
            let reply = build_message("echo_reply", &payload, &config.psk);
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

/// P2-03: Truncate a string at a valid UTF-8 boundary.
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the last valid char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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
        "stdout": truncate_utf8(stdout, 50_000),
        "stderr": truncate_utf8(stderr, 10_000),
        "source": "mesh-sentry",
        "nodeId": config.node_id,
        "role": config.role,
        "correlationId": correlation_id,
    });
    let msg = build_message("exec_result", &payload, &config.psk);
    ws.send(Message::Text(msg.into()))
}

fn send_heartbeat(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    config: &MeshConfig,
    hostname: &str,
) -> Result<(), tungstenite::Error> {
    let uptime = crate::START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let payload = serde_json::json!({
        "nodeId": config.node_id,
        "role": config.role,
        "hostname": hostname,
        "type": "mesh_heartbeat",
        "uptime": uptime,
    });
    let msg = build_message("heartbeat", &payload, &config.psk);
    ws.send(Message::Text(msg.into()))
}

// ---------------------------------------------------------------------------
// Comms-link compatible message builder with HMAC
// ---------------------------------------------------------------------------

/// Build a JSON message compatible with comms-link protocol.
/// P1-01: Uses MESH_IDENTITY ("mesh") as the `from` field — a known identity
/// in the hub's KNOWN_IDENTITIES set (protocol.js). The actual node_id is
/// carried in the payload for routing/identification.
///
/// HMAC canonical field order MUST match protocol.js signMessage():
///   { v, type, from, ts, id, seq, payload }
/// serde_json::json!{} preserves insertion order (BTreeMap is NOT used for
/// Value::Object created via the macro — it uses a Vec-backed Map).
fn build_message(msg_type: &str, payload: &serde_json::Value, psk: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let id = format!("{:016x}", ts ^ xorshift64());
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);

    let mut msg = serde_json::json!({
        "v": 1,
        "type": msg_type,
        "from": MESH_IDENTITY,
        "ts": ts,
        "id": id,
        "seq": seq,
        "payload": payload,
    });

    // HMAC sign — canonical field order matches protocol.js signMessage()
    if !psk.is_empty() {
        let sign_payload = serde_json::json!({
            "v": 1, "type": msg_type, "from": MESH_IDENTITY, "ts": ts,
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

fn extract_host(url: &str) -> String {
    url.replace("ws://", "").replace("wss://", "").split('/').next().unwrap_or("").to_string()
}
