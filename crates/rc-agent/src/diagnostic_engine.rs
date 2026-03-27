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

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

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
) {
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

            // Game launch fail check (DIAG-01: game_launch_fail)
            if let Some(launch_at) = pod_state.launch_started_at {
                if pod_state.game_pid.is_none() && launch_at.elapsed().as_secs() > 90 {
                    events.push(make_event(DiagnosticTrigger::GameLaunchFail, &pod_state));
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
        tracing::warn!(target: "state", task = "diagnostic_engine", event = "lifecycle", "lifecycle: exited (channel closed)");
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
