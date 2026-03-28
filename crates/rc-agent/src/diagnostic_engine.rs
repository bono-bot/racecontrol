//! Diagnostic Engine — autonomous anomaly detection for rc-agent.
//!
//! Runs two detection paths:
//!   1. Periodic scan: every 5 minutes, checks all 9 anomaly classes
//!   2. Event-triggered: called by failure_monitor or ws_handler when specific conditions arise
//!
//! Emits DiagnosticEvent via mpsc channel to the tier decision engine (Plan 229-02).
//! This module is detection-only — it does NOT apply fixes. Fixes are Plan 02's job.
//!
//! Detection triggers (DIAG-01):
//!   - health_check_fail: rc-agent HTTP health not responding (self-check)
//!   - process_crash: WerFault or abnormal exit detected via sysinfo
//!   - game_launch_fail: launch_started_at > 90s elapsed + no game_pid
//!   - display_mismatch: edge_process_count == 0 when lock_screen_state == blanked
//!   - error_spike: >5 error-level log lines in the last 60s (read from rc-bot-events.log)
//!   - ws_disconnect: ws_connected == false for >30s
//!   - sentinel_unexpected: unexpected sentinel file present (not RCAGENT_SELF_RESTART or OTA_DEPLOYING)
//!   - violation_spike: process guard violation_count delta >50 in 5 min
//!   - periodic: scheduled 5-minute scan (always runs)
//!
//! MMA-trained detection methods (v26.1):
//!   MiMo SRE: CLOSE_WAIT socket accumulation on :8090 (port exhaustion)
//!   MiMo SRE: Orphan PowerShell processes (memory leak from self-restart)
//!   R1 Reasoner: Self-heartbeat logging (meta-monitoring — is the diagnostic engine itself alive?)
//!   Gemini Security: Sentinel file age check (MAINTENANCE_MODE stuck with no TTL)

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use crate::config::NodeType;
use crate::failure_monitor::FailureMonitorState;
use crate::udp_heartbeat::HeartbeatStatus;

const LOG_TARGET: &str = "diagnostic-engine";
const SCAN_INTERVAL_SECS: u64 = 300; // 5 minutes per DIAG-07
const STARTUP_GRACE_SECS: u64 = 60; // don't scan immediately — let rc-agent fully init
const WS_DISCONNECT_TRIGGER_SECS: u64 = 30; // DIAG-01: ws disconnect threshold
const ERROR_SPIKE_THRESHOLD: u64 = 5; // DIAG-01: errors/min before spike is declared
const VIOLATION_SPIKE_DELTA: u64 = 50; // DIAG-01: violation count increase before spike

/// All possible reasons a diagnostic cycle can be triggered.
/// Sent as part of DiagnosticEvent to the tier engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticTrigger {
    /// Scheduled 5-minute periodic scan (DIAG-07)
    Periodic,
    /// rc-agent health endpoint not responding
    HealthCheckFail,
    /// WerFault or abnormal process exit detected
    ProcessCrash { process_name: String },
    /// Game launch timed out (>90s, no game_pid)
    GameLaunchFail,
    /// Edge process count is 0 when blanking screen should be active
    DisplayMismatch { expected_edge_count: u32, actual_edge_count: u32 },
    /// Error log rate exceeded threshold (>5 errors/min)
    ErrorSpike { errors_per_min: u64 },
    /// WebSocket disconnected for more than 30s
    WsDisconnect { disconnected_secs: u64 },
    /// Unexpected sentinel file found in C:\RacingPoint\
    SentinelUnexpected { file_name: String },
    /// Process guard violation count spiked (delta >50 in 5 min)
    ViolationSpike { delta: u64 },
    /// Pre-flight check failed — emitted by ws_handler on BillingStarted failure.
    /// The check_name identifies which of the 11 checks failed (e.g. "hid", "conspit_link").
    /// Multiple PreFlightFailed events may be emitted per pre-flight run (one per failed check).
    PreFlightFailed {
        check_name: String,
        detail: String,
    },

    // ─── POS-Specific Triggers (v26.0 Meshed Intelligence — POS node) ──────────
    /// POS: Kiosk Edge browser not running or unresponsive
    PosKioskDown { detail: String },
    /// POS: Network connectivity to racecontrol server lost (HTTP probe failed)
    PosNetworkDown { server_ip: String, detail: String },
    /// POS: Billing API unresponsive or returning errors
    PosBillingApiError { endpoint: String, status_code: u16 },

    // ─── UI State Triggers (DIAG-01n: taskbar enforcement) ──────────────────
    /// Taskbar was found visible when it should be hidden (kiosk mode active).
    /// This indicates explorer.exe restarted and ShowWindow(SW_HIDE) was lost.
    TaskbarVisible,
}

/// A single diagnostic event emitted by this module and consumed by the tier engine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiagnosticEvent {
    pub trigger: DiagnosticTrigger,
    /// Snapshot of pod state at trigger time (from failure_monitor watch channel)
    pub pod_state: FailureMonitorState,
    /// IST timestamp string of when this event was created
    pub timestamp: String,
    /// Current rc-agent build ID (from compile-time GIT_HASH)
    pub build_id: &'static str,
}

/// Emit a diagnostic event from an external source (e.g. pre-flight checks).
///
/// Non-blocking: drops the event if the channel is full (tier engine overwhelmed).
/// Returns true if the event was sent, false if dropped.
pub fn emit_external_event(
    event_tx: &mpsc::Sender<DiagnosticEvent>,
    trigger: DiagnosticTrigger,
    pod_state: &FailureMonitorState,
) -> bool {
    let event = make_event(trigger, pod_state);
    match event_tx.try_send(event) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!(target: LOG_TARGET, "External DiagnosticEvent dropped — tier engine channel full");
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            tracing::error!(target: LOG_TARGET, "External DiagnosticEvent dropped — tier engine channel closed");
            false
        }
    }
}

/// Spawn the diagnostic engine background task.
///
/// Parameters:
///   heartbeat_status  — Arc<HeartbeatStatus> for ws_connected polling
///   failure_monitor_rx — watch::Receiver<FailureMonitorState> for pod state snapshots
///   event_tx — mpsc::Sender<DiagnosticEvent> — Plan 02's tier engine reads from the other end
///
/// Lifecycle logs per standing rule: started, first-item-processed, exit reason.
pub fn spawn(
    heartbeat_status: Arc<HeartbeatStatus>,
    failure_monitor_rx: watch::Receiver<FailureMonitorState>,
    event_tx: mpsc::Sender<DiagnosticEvent>,
    node_type: NodeType,
) {
    let is_pos = node_type == NodeType::Pos;
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "diagnostic_engine", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Diagnostic engine started (scan interval: {}s)", SCAN_INTERVAL_SECS);

        // Startup grace — let rc-agent fully initialize before first scan
        tokio::time::sleep(Duration::from_secs(STARTUP_GRACE_SECS)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(SCAN_INTERVAL_SECS));
        // Sonnet audit fix: Skip missed ticks instead of burst-firing after a delay
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut ws_disconnect_since: Option<Instant> = None;
        let mut last_violation_count: u64 = 0;
        let mut first_scan_done = false;

        loop {
            interval.tick().await;

            let pod_state = failure_monitor_rx.borrow().clone();
            let mut events: Vec<DiagnosticEvent> = Vec::new();

            // Always emit Periodic trigger (DIAG-07)
            events.push(make_event(DiagnosticTrigger::Periodic, &pod_state));

            // WS disconnect check (DIAG-01: ws_disconnect)
            if heartbeat_status.ws_connected.load(Ordering::Relaxed) {
                ws_disconnect_since = None;
            } else {
                let since = ws_disconnect_since.get_or_insert_with(Instant::now);
                let secs = since.elapsed().as_secs();
                if secs >= WS_DISCONNECT_TRIGGER_SECS {
                    events.push(make_event(
                        DiagnosticTrigger::WsDisconnect { disconnected_secs: secs },
                        &pod_state,
                    ));
                }
            }

            // Game launch fail check (DIAG-01: game_launch_fail) — pods only
            if !is_pos {
                if let Some(launch_at) = pod_state.launch_started_at {
                    if pod_state.game_pid.is_none() && launch_at.elapsed().as_secs() > 90 {
                        events.push(make_event(DiagnosticTrigger::GameLaunchFail, &pod_state));
                    }
                }
            }

            // POS-specific checks — kiosk, billing API, network
            if is_pos {
                // Check kiosk Edge browser health
                if let Some(trigger) = check_pos_kiosk_health() {
                    events.push(make_event(trigger, &pod_state));
                }
                // Check network to racecontrol
                if let Some(trigger) = check_pos_network_health() {
                    events.push(make_event(trigger, &pod_state));
                }
            }

            // Sentinel file check (DIAG-01: sentinel_unexpected)
            // Known-safe sentinels: RCAGENT_SELF_RESTART, OTA_DEPLOYING, MAINTENANCE_MODE (handled by Tier 1)
            // We still DETECT MAINTENANCE_MODE here — Tier 1 will clear it
            for sentinel in detect_unexpected_sentinels() {
                events.push(make_event(
                    DiagnosticTrigger::SentinelUnexpected { file_name: sentinel },
                    &pod_state,
                ));
            }

            // Process crash check via WerFault detection (DIAG-01: process_crash)
            for crash in detect_werfault_processes() {
                events.push(make_event(
                    DiagnosticTrigger::ProcessCrash { process_name: crash },
                    &pod_state,
                ));
            }

            // Violation spike check (DIAG-01: violation_spike)
            // Note: violation_count_24h is on the server-side fleet health, not local.
            // We track the local self-reported count from rc-bot-events.log line count as proxy.
            let current_violations = count_recent_violation_lines();
            let delta = current_violations.saturating_sub(last_violation_count);
            if delta >= VIOLATION_SPIKE_DELTA {
                events.push(make_event(
                    DiagnosticTrigger::ViolationSpike { delta },
                    &pod_state,
                ));
            }
            last_violation_count = current_violations;

            // Error spike check (DIAG-01: error_spike)
            let errors_per_min = count_recent_error_lines();
            if errors_per_min >= ERROR_SPIKE_THRESHOLD {
                events.push(make_event(
                    DiagnosticTrigger::ErrorSpike { errors_per_min },
                    &pod_state,
                ));
            }

            // Taskbar visibility check (DIAG-01n: taskbar_visible) — pods only
            // The enforcement loop in event_loop.rs auto-fixes this, but the
            // diagnostic engine tracks it so the tier engine can log, count,
            // and escalate if it happens repeatedly (e.g. explorer crash loop).
            if !is_pos {
                if check_taskbar_visible() {
                    events.push(make_event(DiagnosticTrigger::TaskbarVisible, &pod_state));
                }
            }

            if !first_scan_done {
                tracing::info!(target: "state", task = "diagnostic_engine", event = "lifecycle", "lifecycle: first_scan_complete");
                tracing::info!(target: LOG_TARGET, "Diagnostic engine: first scan complete, {} events emitted", events.len());
                first_scan_done = true;
            }

            // Send all events to tier engine — non-blocking, drop if channel full
            for event in events {
                tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, "Emitting diagnostic event");
                match event_tx.try_send(event) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        tracing::warn!(target: LOG_TARGET, "DiagnosticEvent channel FULL — tier engine overwhelmed, event DROPPED");
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        tracing::error!(target: LOG_TARGET, "DiagnosticEvent channel CLOSED — tier engine may have crashed");
                    }
                }
            }
        }

        // If loop exits (only on channel close), log it
        #[allow(unreachable_code)]
        {
            tracing::warn!(target: "state", task = "diagnostic_engine", event = "lifecycle", "lifecycle: exited (channel closed)");
        }
    });
}

/// Build a DiagnosticEvent with current timestamp in IST.
fn make_event(trigger: DiagnosticTrigger, pod_state: &FailureMonitorState) -> DiagnosticEvent {
    // IST = UTC + 5:30 — chrono FixedOffset
    use chrono::{FixedOffset, Utc};
    let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset 0 is always valid"));
    let now_ist = Utc::now().with_timezone(&ist_offset);
    DiagnosticEvent {
        trigger,
        pod_state: pod_state.clone(),
        timestamp: now_ist.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        build_id: crate::BUILD_ID,
    }
}

/// Check C:\RacingPoint\ for unexpected sentinel files.
/// Returns file names (without path) of sentinels that are NOT in the known-safe list.
/// Known-safe: RCAGENT_SELF_RESTART, OTA_DEPLOYING, GRACEFUL_RELAUNCH, rcagent-restart-sentinel.txt
/// MAINTENANCE_MODE is NOT safe — we detect it so Tier 1 can clear it.
fn detect_unexpected_sentinels() -> Vec<String> {
    let sentinel_dir = std::path::Path::new(r"C:\RacingPoint");
    // Files that indicate a diagnostic situation but are NOT "unexpected" in the sense of being unknown
    // We include MAINTENANCE_MODE because Tier 1 needs to know about it to clear it
    let known_operational = &[
        "OTA_DEPLOYING",
        "RCAGENT_SELF_RESTART",
        "GRACEFUL_RELAUNCH",
        "rcagent-restart-sentinel.txt",
    ];
    let sentinel_indicators = &["MAINTENANCE_MODE", "FORCE_CLEAN", "SAFE_MODE"];

    let mut found = Vec::new();
    for name in sentinel_indicators {
        let path = sentinel_dir.join(name);
        if path.exists() {
            found.push(name.to_string());
        }
    }
    // Also scan for completely unknown sentinel files (not in either list)
    if let Ok(entries) = std::fs::read_dir(sentinel_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy().to_string();
            // Only flag files with no extension that look like sentinel names (all caps)
            if fname_str.chars().all(|c| c.is_uppercase() || c == '_')
                && !known_operational.contains(&fname_str.as_str())
                && !sentinel_indicators.contains(&fname_str.as_str())
            {
                found.push(fname_str);
            }
        }
    }
    found
}

/// Detect WerFault processes that indicate a crash occurred.
/// Uses sysinfo — already a dependency in Cargo.toml.
fn detect_werfault_processes() -> Vec<String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, false);
    sys.processes()
        .values()
        .filter_map(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            if name.contains("werfault") || name.contains("werreport") {
                Some(p.name().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Count recent error lines in C:\RacingPoint\rc-bot-events.log.
/// Returns approximate errors per minute (reads last 120 lines, looks for ERROR keyword).
/// Returns 0 if file unreadable.
fn count_recent_error_lines() -> u64 {
    let log_path = std::path::Path::new(r"C:\RacingPoint\rc-bot-events.log");
    match std::fs::read_to_string(log_path) {
        Ok(content) => {
            // Count lines from last minute (look for "ERROR" in the last ~120 lines as proxy)
            let lines: Vec<&str> = content.lines().rev().take(120).collect();
            lines.iter().filter(|l| l.contains("ERROR") || l.contains("error")).count() as u64
        }
        Err(_) => 0,
    }
}

/// Count recent violation log lines as a proxy for process guard violation delta.
/// Returns the total count found (caller computes delta vs last run).
fn count_recent_violation_lines() -> u64 {
    let log_path = std::path::Path::new(r"C:\RacingPoint\rc-bot-events.log");
    match std::fs::read_to_string(log_path) {
        Ok(content) => {
            content
                .lines()
                .filter(|l| l.contains("violation") || l.contains("ProcessViolation"))
                .count() as u64
        }
        Err(_) => 0,
    }
}

// ─── MMA-Trained Detection Methods ────────────────────────────────────────────
// These detection functions were learned from Multi-Model Audit analysis.
// Each method comes from a specific model's diagnostic methodology:

/// MiMo SRE method: Count CLOSE_WAIT sockets on port 8090.
/// TCP port exhaustion from accumulated stale connections is a silent killer.
/// MMA finding: "CLOSE_WAIT accumulation on :8090 causes rc-agent to stop
/// accepting connections — health checks pass but new clients can't connect."
/// Returns the count of CLOSE_WAIT sockets (0 = healthy).
pub fn count_close_wait_sockets() -> u64 {
    // Use absolute path to prevent PATH hijacking, with 10s timeout
    use std::time::Duration;
    let child = std::process::Command::new(r"C:\Windows\System32\NETSTAT.EXE")
        .args(["-n", "-p", "tcp"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();
    let output = match child {
        Ok(c) => c.wait_with_output(),
        Err(_) => return 0,
    };
    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            // Match only LOCAL address :8090 in CLOSE_WAIT state.
            // Windows netstat -n format: "  TCP    <local_addr>:<port>    <remote_addr>:<port>    CLOSE_WAIT"
            // We split by whitespace and check that the LOCAL address column (index 1) ends with :8090.
            text.lines()
                .filter(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    parts.len() >= 4
                        && parts[0] == "TCP"
                        && parts[1].ends_with(":8090")
                        && parts[3] == "CLOSE_WAIT"
                })
                .count() as u64
        }
        _ => 0,
    }
}

/// MiMo SRE method: Count orphan PowerShell processes.
/// Self-restart via PowerShell+DETACHED_PROCESS leaks ~90MB per restart.
/// MMA finding: "Orphan PowerShell processes from relaunch_self() accumulate
/// unbounded — 15 orphans = 1.35GB wasted RAM, degrades game performance."
/// Returns count of powershell.exe processes (0-1 is normal, >3 is a leak).
pub fn count_orphan_powershell() -> u32 {
    use sysinfo::{System, ProcessesToUpdate, Pid};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    // Count PowerShell processes whose parent is no longer running (orphans).
    // Normal PowerShell (admin session, user terminal) has a live parent (explorer.exe, cmd.exe).
    // Orphans from self-restart leaks have dead parents (the rc-agent that spawned them is gone).
    sys.processes()
        .values()
        .filter(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            if name != "powershell.exe" {
                return false;
            }
            // Check if parent process is dead — orphan indicator
            match p.parent() {
                Some(parent_pid) => sys.process(parent_pid).is_none(), // parent dead = orphan
                None => true, // no parent = orphan
            }
        })
        .count() as u32
}

/// R1 Reasoner method: Check MAINTENANCE_MODE sentinel age.
/// Absence-based bug: MAINTENANCE_MODE has no TTL — pod stuck forever.
/// MMA finding: "MAINTENANCE_MODE sentinel written after 3 restarts in 10 min,
/// but no timeout, no auto-clear, no alert. Pod permanently dead."
/// Returns Some(age_secs) if MAINTENANCE_MODE exists, None if absent.
pub fn check_maintenance_mode_age() -> Option<u64> {
    let path = std::path::Path::new(r"C:\RacingPoint\MAINTENANCE_MODE");
    if !path.exists() {
        return None;
    }
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs())
}

/// Gemini Security method: Check for multiple recovery systems running simultaneously.
/// MMA finding: "rc-sentry, RCWatchdog, and pod_monitor can all fire restart commands
/// at the same time — creates port conflicts, crash loop, MAINTENANCE_MODE."
/// Returns count of active recovery processes (0-1 normal, >1 = cascade risk).
pub fn count_recovery_processes() -> u32 {
    use sysinfo::{System, ProcessesToUpdate};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    let recovery_names = ["rc-sentry.exe", "rc-watchdog.exe"];
    sys.processes()
        .values()
        .filter(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            recovery_names.iter().any(|r| name == *r)
        })
        .count() as u32
}

// ─── Taskbar Visibility Check ───────────────────────────────────────────────────
// DIAG-01n: Detects when Windows taskbar is visible (should be hidden in kiosk mode).
// The enforcement loop auto-fixes this, but the diagnostic engine tracks occurrences
// so the tier engine can detect patterns (e.g. explorer crash loop causing repeated
// taskbar re-appearance).

/// Check if the Windows taskbar is currently visible.
/// Uses the same Win32 API as kiosk.rs but is read-only (does not hide it).
#[cfg(windows)]
fn check_taskbar_visible() -> bool {
    unsafe {
        let taskbar_class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
        let hwnd = winapi::um::winuser::FindWindowW(taskbar_class.as_ptr(), std::ptr::null());
        if hwnd.is_null() {
            return false;
        }
        winapi::um::winuser::IsWindowVisible(hwnd) != 0
    }
}

#[cfg(not(windows))]
fn check_taskbar_visible() -> bool {
    false
}

// ─── POS-Specific Diagnostic Checks ────────────────────────────────────────────
// These checks only run on POS nodes (node_type = "pos").
// They monitor billing kiosk health, network to server, and Edge browser status.

/// POS check: Is Microsoft Edge running? The POS kiosk uses Edge for billing UI.
/// Returns a diagnostic trigger if Edge is missing.
fn check_pos_kiosk_health() -> Option<DiagnosticTrigger> {
    use sysinfo::{System, ProcessesToUpdate};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);

    let edge_count = sys.processes()
        .values()
        .filter(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            name.contains("msedge")
        })
        .count();

    if edge_count == 0 {
        tracing::warn!(target: LOG_TARGET, "POS kiosk: Edge browser not running — billing UI may be down");
        Some(DiagnosticTrigger::PosKioskDown {
            detail: "Microsoft Edge not running — billing kiosk UI unavailable".to_string(),
        })
    } else {
        None
    }
}

/// POS check: Can we reach the racecontrol server?
/// Performs a simple TCP connect probe to port 8080.
/// MMA-POS: Uses env var or default for server IP (avoid hardcode drift).
///
/// MMA Round 1 fix (2/3 consensus):
/// - P1: Replace .expect() with graceful parse error handling (no panic)
fn check_pos_network_health() -> Option<DiagnosticTrigger> {
    use std::net::TcpStream;

    let server_ip = std::env::var("RACECONTROL_SERVER_IP").unwrap_or_else(|_| "192.168.31.23".to_string());
    let server_addr = format!("{}:8080", server_ip);

    // MMA Round 1 P1 fix: graceful parse instead of .expect() panic
    let sock_addr: std::net::SocketAddr = match server_addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, addr = %server_addr, error = %e,
                "POS network: invalid server address — check RACECONTROL_SERVER_IP env var");
            return Some(DiagnosticTrigger::PosNetworkDown {
                server_ip: server_ip.clone(),
                detail: format!("Invalid server address '{}': {}", server_addr, e),
            });
        }
    };

    match TcpStream::connect_timeout(&sock_addr, Duration::from_secs(5)) {
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "POS network: cannot reach racecontrol at {} — {}", server_addr, e);
            Some(DiagnosticTrigger::PosNetworkDown {
                server_ip: server_ip.clone(),
                detail: format!("TCP connect to {} failed: {}", server_addr, e),
            })
        }
    }
}
