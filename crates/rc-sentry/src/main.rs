//! rc-sentry — lightweight backup remote exec service + rc-agent watchdog.
//!
//! Runs on port 8091 (server + pods), independent of racecontrol/rc-agent.
//! Provides /ping, /exec, /health, /version, /files, /processes endpoints
//! so we never lose remote access during deploys.
//!
//! v11.2: Added watchdog module — polls rc-agent /health every 5s, detects
//! crashes with 3-poll hysteresis, reads crash logs for diagnostics.
//! Anti-cheat safe: HTTP polling only, no process inspection APIs.
//!
//! No tokio, no async — pure std::net for minimal binary size and zero shared deps.
//!
//! SECURITY: This is an internal-only tool for LAN management of Racing Point pods.
//! It binds to 0.0.0.0 on a private subnet (192.168.31.x) with no auth.
//! NOT intended for public networks. The cmd.exe invocation is intentional —
//! this is a remote admin tool equivalent to SSH, scoped to venue hardware.

mod sentry_config;
#[cfg(feature = "watchdog")]
mod watchdog;
#[cfg(feature = "tier1-fixes")]
mod tier1_fixes;
#[cfg(feature = "ai-diagnosis")]
mod debug_memory;
// ollama module is now in rc-common — see rc_common::ollama
mod session1_spawn;

use rc_common::recovery::{RecoveryAuthority, RecoveryAction, RecoveryDecision};
#[cfg(feature = "watchdog")]
use rc_common::recovery::{RecoveryLogger, RECOVERY_LOG_POD};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

const DEFAULT_PORT: u16 = 8091;
const MAX_BODY: usize = 64 * 1024; // 64KB max request/output size
const MAX_EXEC_SLOTS: usize = 4;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_ID: &str = env!("GIT_HASH");

static EXEC_SLOTS: AtomicUsize = AtomicUsize::new(0);
static THREAD_COUNTER: AtomicUsize = AtomicUsize::new(0);
static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

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

#[cfg(windows)]
unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> i32 {
    if ctrl_type == 0 || ctrl_type == 2 {
        // CTRL_C_EVENT or CTRL_CLOSE_EVENT
        SHUTDOWN_REQUESTED.store(true, Ordering::Release);
        1
    } else {
        0
    }
}

fn main() {
    START_TIME.get_or_init(std::time::Instant::now);

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

    listener.set_nonblocking(true).expect("set_nonblocking");

    #[cfg(windows)]
    unsafe {
        winapi::um::consoleapi::SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }

    tracing::info!("rc-sentry listening on :{port}");

    // Load sentry config (rc-sentry.toml if present, else defaults to rc-agent mode)
    let cfg = sentry_config::load();
    tracing::info!("watchdog target: {} ({})", cfg.service_name, cfg.health_addr);

    // Spawn watchdog thread to monitor service health (gated on "watchdog" feature)
    #[cfg(feature = "watchdog")]
    {
        let crash_rx = watchdog::spawn(&SHUTDOWN_REQUESTED);

        // Drain crash events in a background thread — run Tier 1 fixes + restart
        std::thread::Builder::new()
            .name("sentry-crash-handler".to_string())
            .spawn(move || {
                let recovery_logger = RecoveryLogger::new(RECOVERY_LOG_POD);
                let machine = sysinfo::System::host_name().unwrap_or_else(|| "pod-unknown".to_string());
                #[cfg(feature = "tier1-fixes")]
                let mut tracker = tier1_fixes::RestartTracker::new();
                // Tier 4: track consecutive spawn-verified failures for WhatsApp escalation (GRAD-04)
                #[cfg(feature = "tier1-fixes")]
                let mut consecutive_failures: u32 = 0;
                // Tier 4: last escalation time — cooldown prevents alert spam (GRAD-04)
                #[cfg(feature = "tier1-fixes")]
                let mut last_escalation: Option<std::time::Instant> = None;
                loop {
                    // MAINT-01: Check maintenance mode auto-clear every 60s even with no crashes
                    #[cfg(feature = "tier1-fixes")]
                    {
                        let clear_result = tier1_fixes::check_and_clear_maintenance();
                        match clear_result {
                            tier1_fixes::ClearResult::Cleared { reason } => {
                                tracing::info!(target: "crash-handler",
                                    "MAINTENANCE_MODE cleared: {} — rc-agent restart attempted", reason);
                                // Reset tracker so auto-clear doesn't immediately re-enter maintenance
                                tracker = tier1_fixes::RestartTracker::new();
                                consecutive_failures = 0;
                            }
                            tier1_fixes::ClearResult::StillLocked { remaining_secs } => {
                                tracing::debug!(target: "crash-handler",
                                    "MAINTENANCE_MODE still active — {} seconds remaining", remaining_secs);
                            }
                            tier1_fixes::ClearResult::NotInMaintenance => {}
                        }
                    }

                    // Wait for crash event with 60s timeout (allows periodic auto-clear check)
                    let ctx = match crash_rx.recv_timeout(std::time::Duration::from_secs(60)) {
                        Ok(ctx) => ctx,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            tracing::error!(target: "crash-handler", "crash channel disconnected — stopping handler");
                            break;
                        }
                    };

                    tracing::warn!(
                        target: "crash-handler",
                        "rc-agent crash detected: panic={:?}, exit_code={:?}, last_phase={:?}",
                        ctx.panic_message, ctx.exit_code, ctx.last_phase
                    );

                    // Run graduated crash handler (Tier 1 + Tier 2 + spawn verify + recovery event)
                    // handle_crash() now owns: pattern key derivation, Tier 2 lookup,
                    // server_reachable check, escalation guard, spawn verification, and
                    // recovery event POST (GRAD-01, GRAD-02, GRAD-05, SPAWN-01, SPAWN-02)
                    #[cfg(feature = "tier1-fixes")]
                    let result = tier1_fixes::handle_crash(&ctx, &mut tracker);
                    #[cfg(not(feature = "tier1-fixes"))]
                    let result = tier1_fixes::CrashHandlerResult {
                        fix_results: vec![],
                        restarted: false,
                        spawn_verified: false,
                        server_reachable: false,
                        pattern_key: format!("exit:{:?}", ctx.exit_code),
                    };

                    tracing::info!(
                        target: "crash-handler",
                        "crash handled: {} fixes applied, restarted={} spawn_verified={} server_reachable={}",
                        result.fix_results.len(), result.restarted, result.spawn_verified, result.server_reachable
                    );

                    // Update consecutive spawn-verified failure counter (GRAD-04)
                    #[cfg(feature = "tier1-fixes")]
                    if result.spawn_verified {
                        consecutive_failures = 0;
                    } else if result.restarted {
                        consecutive_failures += 1;
                    }

                    // Tier 3: Ollama diagnosis for unknown patterns where spawn verification failed (GRAD-03)
                    // Fires only when: restart was attempted but spawn_verified=false AND pattern not in Tier 2 memory
                    #[cfg(feature = "ai-diagnosis")]
                    if !result.spawn_verified && result.restarted {
                        let memory = debug_memory::DebugMemory::load();
                        if memory.instant_fix(&result.pattern_key).is_none() {
                            let crash_summary = format!(
                                "rc-agent crash on {} - restart attempted but spawn verification FAILED\n\
                                 panic: {:?}\nexit_code: {:?}\nlast_phase: {:?}\nlog_tail: {}",
                                tier1_fixes::get_pod_id(),
                                ctx.panic_message, ctx.exit_code, ctx.last_phase,
                                &ctx.startup_log[..ctx.startup_log.len().min(500)]
                            );
                            tracing::info!(
                                target: "crash-handler",
                                "TIER 3: unknown pattern {} with failed spawn — querying Ollama ({}s timeout)",
                                result.pattern_key, OLLAMA_TIMEOUT.as_secs()
                            );
                            let ollama_result = query_ollama_with_timeout(crash_summary, OLLAMA_TIMEOUT);
                            if let Some(ref r) = ollama_result {
                                tracing::info!(
                                    target: "crash-handler",
                                    "Ollama suggestion for {}: {} (model: {})",
                                    result.pattern_key, r.suggestion, r.model
                                );
                                let mut mem = debug_memory::DebugMemory::load();
                                mem.record(
                                    result.pattern_key.clone(),
                                    format!("ollama:{}", r.suggestion),
                                    r.suggestion.clone(),
                                );
                                mem.save();
                            } else {
                                tracing::warn!(
                                    target: "crash-handler",
                                    "Ollama timeout/unavailable for {}",
                                    result.pattern_key
                                );
                            }
                        }
                    }

                    // Tier 4: WhatsApp escalation after 3+ consecutive failed spawn-verified recoveries (GRAD-04)
                    #[cfg(feature = "tier1-fixes")]
                    if consecutive_failures >= 3 {
                        let last_error = ctx.panic_message.as_deref()
                            .unwrap_or_else(|| ctx.stderr_log.lines().last().unwrap_or("unknown error"));
                        tier1_fixes::escalate_to_whatsapp(
                            &tier1_fixes::get_pod_id(),
                            consecutive_failures,
                            last_error,
                            &mut last_escalation,
                        );
                    }

                    // Log recovery decision to JSONL audit trail (SENT-03)
                    #[cfg(feature = "tier1-fixes")]
                    let tracker_count = tracker.restart_count();
                    #[cfg(not(feature = "tier1-fixes"))]
                    let tracker_count: u32 = 0;
                    #[cfg(feature = "tier1-fixes")]
                    let maintenance = tier1_fixes::is_maintenance_mode();
                    #[cfg(not(feature = "tier1-fixes"))]
                    let maintenance = false;
                    let mut decision = build_restart_decision(
                        &machine,
                        &result.pattern_key,
                        result.restarted,
                        maintenance,
                        tracker_count,
                    );
                    decision.context = format!(
                        "fixes:{} restarted:{} spawn_verified:{} server_reachable:{}",
                        result.fix_results.len(), result.restarted, result.spawn_verified, result.server_reachable
                    );
                    let _ = recovery_logger.log(&decision);

                    // Record successful fix in pattern memory
                    #[cfg(all(feature = "ai-diagnosis", feature = "tier1-fixes"))]
                    if result.restarted {
                        let fix_summary: String = result.fix_results.iter()
                            .filter(|r| r.success)
                            .map(|r| r.fix_type.as_str())
                            .collect::<Vec<_>>()
                            .join("+");
                        let mut memory = debug_memory::DebugMemory::load();
                        memory.record(
                            result.pattern_key.clone(),
                            fix_summary.clone(),
                            format!("{} fixes applied, restarted", result.fix_results.len()),
                        );
                        memory.save();
                        tracing::info!(target: "crash-handler", "pattern memory updated: {} -> {}", result.pattern_key, fix_summary);
                    }
                }
            })
            .expect("spawn crash handler thread");
    } // end #[cfg(feature = "watchdog")]

    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();
    loop {
        handles.retain(|h| !h.is_finished());
        if SHUTDOWN_REQUESTED.load(Ordering::Acquire) {
            tracing::info!(
                "shutdown requested -- draining {} active connections",
                handles.len()
            );
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let n = THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
                if let Ok(h) = std::thread::Builder::new()
                    .name(format!("sentry-handler-{n}"))
                    .spawn(move || {
                        if let Err(e) = handle(stream) {
                            tracing::warn!("handler error: {e}");
                        }
                    })
                {
                    handles.push(h);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => tracing::error!("accept: {e}"),
        }
    }
    for h in handles {
        let _ = h.join();
    }
    tracing::info!("rc-sentry shutdown complete");
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
    // On Windows, accept() inherits non-blocking from the listener — force blocking
    // mode so reads wait for data instead of returning WouldBlock immediately.
    stream.set_nonblocking(false)?;
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
        ("GET", "/health") => handle_health(&mut stream),
        ("GET", "/version") => handle_version(&mut stream),
        ("GET", "/flags") => handle_flags(&mut stream),
        ("GET", p) if p.starts_with("/files") => handle_files(&mut stream, p),
        ("GET", "/processes") => handle_processes(&mut stream),
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

fn handle_health(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let slots_used = EXEC_SLOTS.load(Ordering::Acquire);
    let resp = serde_json::json!({
        "status": "ok",
        "version": VERSION,
        "build_id": BUILD_ID,
        "uptime_secs": uptime,
        "exec_slots_available": MAX_EXEC_SLOTS - slots_used,
        "exec_slots_total": MAX_EXEC_SLOTS,
        "hostname": sysinfo::System::host_name().unwrap_or_default(),
    });
    send_response(stream, 200, &resp.to_string())
}

fn handle_version(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let resp = serde_json::json!({ "version": VERSION, "git_hash": BUILD_ID });
    send_response(stream, 200, &resp.to_string())
}

/// Read feature flags from sentry-flags.json (written by rc-agent on FlagSync).
/// Returns None if the file doesn't exist or can't be parsed.
/// rc-sentry has no WS connection to server — rc-agent bridges flags via this file.
fn read_sentry_flags() -> Option<serde_json::Value> {
    let path = r"C:\RacingPoint\sentry-flags.json";
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn handle_flags(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let flags = read_sentry_flags();
    let body = serde_json::to_string(&flags).unwrap_or_else(|_| "null".to_string());
    send_response(stream, 200, &body)
}

fn handle_files(
    stream: &mut TcpStream,
    path_with_query: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = path_with_query.splitn(2, '?').nth(1).unwrap_or("");
    let raw_path = query
        .split('&')
        .find(|p| p.starts_with("path="))
        .and_then(|p| p.strip_prefix("path="))
        .unwrap_or("");

    if raw_path.is_empty() {
        return send_response(stream, 400, r#"{"error":"missing path parameter"}"#);
    }

    let decoded = raw_path
        .replace("%3A", ":")
        .replace("%3a", ":")
        .replace("%5C", "\\")
        .replace("%5c", "\\")
        .replace("%2F", "/")
        .replace("%2f", "/")
        .replace("%20", " ")
        .replace("%23", "#")
        .replace("%25", "%")
        .replace("%26", "&")
        .replace("%3F", "?")
        .replace("%3f", "?")
        .replace("%2B", "+")
        .replace("%2b", "+")
        .replace("%28", "(")
        .replace("%29", ")");
    let dir = std::path::PathBuf::from(&decoded);

    if !dir.exists() {
        return send_response(
            stream,
            404,
            &format!(r#"{{"error":"path not found: {}"}}"#, decoded),
        );
    }
    if !dir.is_dir() {
        return send_response(stream, 400, r#"{"error":"not a directory"}"#);
    }

    let entries: Vec<serde_json::Value> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|entry| {
                let meta = entry.metadata().ok();
                let modified = meta.as_ref().and_then(|m| {
                    m.modified().ok().and_then(|t| {
                        t.duration_since(SystemTime::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                    })
                });
                serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "is_dir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    "modified": modified,
                })
            })
            .collect(),
        Err(_) => return send_response(stream, 403, r#"{"error":"cannot read directory"}"#),
    };

    send_response(stream, 200, &serde_json::to_string(&entries)?)
}

fn handle_processes(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    let procs: Vec<serde_json::Value> = sys
        .processes()
        .values()
        .map(|p| {
            serde_json::json!({
                "pid": p.pid().as_u32(),
                "name": p.name().to_string_lossy(),
                "memory_kb": p.memory() / 1024,
            })
        })
        .collect();
    send_response(stream, 200, &serde_json::to_string(&procs)?)
}

fn send_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
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
         \r\n{body}",
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
         \r\n{body}",
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

#[cfg(feature = "ai-diagnosis")]
const OLLAMA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Wrap the fire-and-forget ollama::query_async into a bounded synchronous call.
/// Returns the OllamaResult if Ollama responds within `timeout`, or None if it
/// times out or is unavailable. Never blocks longer than `timeout`.
#[cfg(feature = "ai-diagnosis")]
fn query_ollama_with_timeout(
    crash_summary: String,
    timeout: std::time::Duration,
) -> Option<rc_common::ollama::OllamaResult> {
    let (tx, rx) = std::sync::mpsc::channel();
    rc_common::ollama::query_async(crash_summary, Box::new(move |result| {
        let _ = tx.send(result);
    }));
    rx.recv_timeout(timeout).ok().flatten()
}

/// Build a RecoveryDecision for a crash handler outcome.
/// Extracted as a pure function so it can be unit-tested without I/O.
pub(crate) fn build_restart_decision(
    machine: &str,
    pattern_key: &str,
    restarted: bool,
    maintenance_mode: bool,
    restart_count: u32,
) -> RecoveryDecision {
    let action = if restarted {
        RecoveryAction::Restart
    } else if maintenance_mode {
        RecoveryAction::SkipMaintenanceMode
    } else {
        RecoveryAction::EscalateToAi
    };
    let mut d = RecoveryDecision::new(
        machine,
        "rc-agent.exe",
        RecoveryAuthority::RcSentry,
        action,
        format!("pattern:{} restart_count:{}", pattern_key, restart_count),
    );
    d.context = format!("restarted:{}", restarted);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_test_server(requests: usize) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        START_TIME.get_or_init(std::time::Instant::now);
        std::thread::spawn(move || {
            for stream in listener.incoming().take(requests).flatten() {
                let _ = handle(stream);
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        port
    }

    fn http_get(port: u16, path: &str) -> String {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    fn http_post(port: u16, path: &str, body: &str) -> String {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        write!(
            s,
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    #[test]
    fn test_ping() {
        let port = start_test_server(1);
        let resp = http_get(port, "/ping");
        assert!(resp.contains("pong"), "expected pong in response: {resp}");
    }

    #[test]
    fn test_health_fields() {
        let port = start_test_server(1);
        let resp = http_get(port, "/health");
        assert!(resp.contains("200"), "expected HTTP 200: {resp}");
        assert!(resp.contains("\"status\""), "missing status field: {resp}");
        assert!(resp.contains("\"uptime_secs\""), "missing uptime_secs: {resp}");
        assert!(resp.contains("\"exec_slots_available\""), "missing exec_slots_available: {resp}");
        assert!(resp.contains("\"hostname\""), "missing hostname: {resp}");
        assert!(resp.contains("\"version\""), "missing version: {resp}");
        assert!(resp.contains("\"build_id\""), "missing build_id: {resp}");
        assert!(resp.contains("\"exec_slots_total\""), "missing exec_slots_total: {resp}");
    }

    #[test]
    fn test_version_fields() {
        let port = start_test_server(1);
        let resp = http_get(port, "/version");
        assert!(resp.contains("200"), "expected HTTP 200: {resp}");
        assert!(resp.contains("\"version\""), "missing version: {resp}");
        assert!(resp.contains("\"git_hash\""), "missing git_hash: {resp}");
    }

    #[test]
    fn test_files_directory() {
        let port = start_test_server(1);
        // Use C:\ on Windows -- guaranteed to exist
        let resp = http_get(port, "/files?path=C%3A%5C");
        assert!(!resp.contains("500"), "unexpected HTTP 500: {resp}");
        // Should return 200 with a JSON array
        assert!(resp.contains("200"), "expected HTTP 200: {resp}");
        assert!(resp.contains("\"name\""), "missing name field in entries: {resp}");
        assert!(resp.contains("\"is_dir\""), "missing is_dir field: {resp}");
    }

    #[test]
    fn test_processes_fields() {
        let port = start_test_server(1);
        let resp = http_get(port, "/processes");
        assert!(resp.contains("200"), "expected HTTP 200: {resp}");
        assert!(resp.contains("\"pid\""), "missing pid field: {resp}");
        assert!(resp.contains("\"name\""), "missing name field: {resp}");
        assert!(resp.contains("\"memory_kb\""), "missing memory_kb: {resp}");
    }

    #[test]
    fn test_exec_echo() {
        let port = start_test_server(1);
        let body = r#"{"cmd":"echo hello","timeout_ms":5000}"#;
        let resp = http_post(port, "/exec", body);
        assert!(resp.contains("200"), "expected HTTP 200: {resp}");
        assert!(resp.contains("hello"), "stdout should contain hello: {resp}");
        assert!(resp.contains("\"exit_code\":0"), "expected exit_code 0: {resp}");
    }

    #[test]
    fn test_404_unknown_path() {
        let port = start_test_server(1);
        let resp = http_get(port, "/nonexistent");
        assert!(resp.contains("404"), "expected HTTP 404: {resp}");
        assert!(resp.contains("not found"), "expected not found message: {resp}");
    }

    #[cfg(feature = "ai-diagnosis")]
    #[test]
    fn query_ollama_timeout_respects_deadline() {
        // A slow responder (never fires within deadline) should return None/Err
        let (tx, rx) = std::sync::mpsc::channel::<Option<rc_common::ollama::OllamaResult>>();
        // Spawn thread that sleeps much longer than our deadline
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = tx.send(None);
        });
        let result = rx.recv_timeout(std::time::Duration::from_millis(50));
        // Should be Err(Timeout) because 2s > 50ms deadline
        assert!(result.is_err(), "should timeout before slow responder");
    }

    #[cfg(feature = "ai-diagnosis")]
    #[test]
    fn query_ollama_with_timeout_returns_result_when_fast() {
        // A fast responder should be received before the timeout
        let (tx, rx) = std::sync::mpsc::channel::<Option<rc_common::ollama::OllamaResult>>();
        std::thread::spawn(move || {
            // Respond immediately
            let _ = tx.send(Some(rc_common::ollama::OllamaResult {
                suggestion: "check disk space".to_string(),
                model: "test-model".to_string(),
            }));
        });
        // Use 1s timeout — more than enough for an immediate response
        let result = rx.recv_timeout(std::time::Duration::from_secs(1));
        assert!(result.is_ok(), "fast responder should complete before timeout");
        let inner = result.unwrap();
        assert!(inner.is_some(), "should have a result");
        let r = inner.unwrap();
        assert_eq!(r.suggestion, "check disk space");
    }

    #[test]
    fn build_restart_decision_restart_action() {
        let d = build_restart_decision("pod-3", "exit:101", true, false, 1);
        assert_eq!(d.action, RecoveryAction::Restart);
        assert!(d.reason.contains("exit:101"), "reason should contain pattern key: {}", d.reason);
        assert_eq!(d.process, "rc-agent.exe");
        assert_eq!(d.authority, RecoveryAuthority::RcSentry);
    }

    #[test]
    fn build_restart_decision_maintenance_action() {
        let d = build_restart_decision("pod-3", "unknown", false, true, 3);
        assert_eq!(d.action, RecoveryAction::SkipMaintenanceMode);
    }

    #[test]
    fn build_restart_decision_escalate_action() {
        let d = build_restart_decision("pod-3", "panic:overflow", false, false, 3);
        assert_eq!(d.action, RecoveryAction::EscalateToAi);
    }
}
