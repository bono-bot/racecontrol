//! Type definitions used by AppState and its consumers.
//!
//! Extracted from state.rs for ARCH-03 (<500 line modules).

use chrono::{DateTime, Utc};
use std::time::Instant;

/// Watchdog recovery state for a single pod.
///
/// Tracks where the watchdog is in the restart/verify cycle so
/// pod_monitor and pod_healer can coordinate without racing.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogState {
    /// Pod heartbeat is current — no action needed.
    Healthy,
    /// Watchdog sent a restart command; waiting for rc-agent to come back.
    Restarting { attempt: u32, started_at: DateTime<Utc> },
    /// Restart command sent; now running post-restart verification checks.
    Verifying { attempt: u32, started_at: DateTime<Utc> },
    /// All restart attempts exhausted; manual intervention required.
    RecoveryFailed { attempt: u32, failed_at: DateTime<Utc> },
}

/// Cached assist state for a pod (abs, tc, auto_shifter, ffb_percent).
/// Populated by WebSocket handlers when agent reports assist changes or state queries.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CachedAssistState {
    pub abs: u8,
    pub tc: u8,
    pub auto_shifter: bool,
    pub ffb_percent: u8,
}

impl Default for CachedAssistState {
    fn default() -> Self {
        Self {
            abs: 0,
            tc: 0,
            auto_shifter: true,
            ffb_percent: 70,
        }
    }
}

/// Tracks OTP request count and window start per phone number
pub struct OtpRateLimit {
    pub count: u32,
    pub window_start: Instant,
}

/// Tracks failed OTP verification attempts per phone number
pub struct OtpFailedAttempts {
    pub count: u32,
    pub locked_until: Option<Instant>,
}

/// Result of a command ACK from an agent (LaunchGame/StopGame).
/// Phase 312: WS ACK Protocol.
#[derive(Debug)]
pub struct CommandAckResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Result of a WebSocket command sent to a pod agent.
/// Stored in a oneshot channel and resolved when ExecResult arrives.
#[derive(Debug)]
pub struct WsExecResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Venue config snapshot received from James via comms-link sync_push.
/// Stores the latest sanitized config from the on-premise server.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct VenueConfigSnapshot {
    pub venue_name: String,
    pub venue_location: String,
    pub venue_timezone: String,
    pub pod_count: u64,
    pub pod_discovery: bool,
    pub pod_healer_enabled: bool,
    pub pod_healer_interval_secs: u64,
    pub branding_primary_color: String,
    pub branding_theme: String,
    pub source: String,
    pub pushed_at: u64,
    pub config_hash: String,
    pub received_at: chrono::DateTime<chrono::Utc>,
}

/// Status of the backup pipeline — updated each tick, readable by downstream API consumers.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BackupStatus {
    /// ISO timestamp of the last successful backup (IST)
    pub last_backup_at: Option<String>,
    /// Size in bytes of the last backup file
    pub last_backup_size_bytes: Option<u64>,
    /// Filename of the last backup created
    pub last_backup_file: Option<String>,
    /// Whether the remote backup host was reachable on last attempt
    pub remote_reachable: bool,
    /// ISO timestamp of last successful remote transfer (IST)
    pub last_remote_transfer_at: Option<String>,
    /// Whether the last checksum verification passed
    pub last_checksum_match: Option<bool>,
    /// Total number of local backup files across both databases
    pub backup_count_local: usize,
    /// Hours since the most recent backup file was created (None if no backups exist)
    pub staleness_hours: Option<f64>,
    /// ISO timestamp of the last successful admin.db backup (IST). None if admin_db_path unconfigured.
    pub last_admin_backup_at: Option<String>,
    /// Size in bytes of the last admin.db backup file. None if not yet backed up.
    pub last_admin_backup_size: Option<u64>,
}

/// Phase 317 (LAUNCH-04): Tracks consecutive game launch failures per pod+SimType.
/// Resets when launch succeeds (GameState::Running) or 10-minute window expires.
#[derive(Debug, Clone, Default)]
pub struct ChainFailureState {
    pub consecutive_failures: u32,
    pub window_start: Option<std::time::Instant>,
    /// true once EscalationRequest sent for this chain — prevents re-alert within window
    pub alerted: bool,
}

impl ChainFailureState {
    /// Returns true if the 10-minute failure window has expired.
    pub fn is_window_expired(&self) -> bool {
        self.window_start
            .map(|t| t.elapsed().as_secs() >= 600)
            .unwrap_or(true)
    }

    /// Reset to clean state (launch succeeded or window expired).
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.window_start = None;
        self.alerted = false;
    }
}
