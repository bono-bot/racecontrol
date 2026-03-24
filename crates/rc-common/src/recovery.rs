use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ─── Log Path Constants ──────────────────────────────────────────────────────

pub const RECOVERY_LOG_SERVER: &str = r"C:\RacingPoint\recovery-log.jsonl";
pub const RECOVERY_LOG_POD: &str    = r"C:\RacingPoint\recovery-log.jsonl";
pub const RECOVERY_LOG_JAMES: &str  = r"C:\Users\bono\racingpoint\recovery-log.jsonl";

// ─── Authority ───────────────────────────────────────────────────────────────

/// Which recovery system owns a given process.
/// Each process may have exactly one owner — attempting to register two owners
/// returns OwnershipConflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAuthority {
    /// rc-sentry (runs on each pod, monitors rc-agent)
    RcSentry,
    /// pod_healer (runs on racecontrol server, server-side healing of pods)
    PodHealer,
    /// james_monitor (runs on James .27, monitors Ollama/Claude/comms-link/webterm)
    JamesMonitor,
}

impl fmt::Display for RecoveryAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryAuthority::RcSentry => write!(f, "rc_sentry"),
            RecoveryAuthority::PodHealer => write!(f, "pod_healer"),
            RecoveryAuthority::JamesMonitor => write!(f, "james_monitor"),
        }
    }
}

// ─── Ownership Registry ──────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ProcessOwnership {
    map: HashMap<String, RecoveryAuthority>,
}

#[derive(Debug)]
pub enum OwnershipConflict {
    AlreadyOwned {
        process: String,
        current_owner: RecoveryAuthority,
        attempted_owner: RecoveryAuthority,
    },
}

impl fmt::Display for OwnershipConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OwnershipConflict::AlreadyOwned {
                process,
                current_owner,
                attempted_owner,
            } => write!(
                f,
                "process {:?} already owned by {:?}, cannot register {:?}",
                process, current_owner, attempted_owner
            ),
        }
    }
}

impl std::error::Error for OwnershipConflict {}

impl ProcessOwnership {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register process_name -> authority. Returns Err if already registered with a different authority.
    /// Registering the same authority again (idempotent) returns Ok(()).
    pub fn register(
        &mut self,
        process: &str,
        authority: RecoveryAuthority,
    ) -> Result<(), OwnershipConflict> {
        if let Some(&current) = self.map.get(process) {
            if current != authority {
                return Err(OwnershipConflict::AlreadyOwned {
                    process: process.to_string(),
                    current_owner: current,
                    attempted_owner: authority,
                });
            }
            // Same authority — idempotent, return Ok
            return Ok(());
        }
        self.map.insert(process.to_string(), authority);
        Ok(())
    }

    /// Look up who owns a process. Returns None if unregistered.
    pub fn owner_of(&self, process: &str) -> Option<RecoveryAuthority> {
        self.map.get(process).copied()
    }

    /// All registered entries (process -> authority).
    pub fn all(&self) -> &HashMap<String, RecoveryAuthority> {
        &self.map
    }
}

// ─── Decision Log ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Restart,
    Kill,
    WakeOnLan,
    SkipCascadeGuardActive,
    SkipMaintenanceMode,
    EscalateToAi,
    AlertStaff,
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryAction::Restart => write!(f, "restart"),
            RecoveryAction::Kill => write!(f, "kill"),
            RecoveryAction::WakeOnLan => write!(f, "wake_on_lan"),
            RecoveryAction::SkipCascadeGuardActive => write!(f, "skip_cascade_guard_active"),
            RecoveryAction::SkipMaintenanceMode => write!(f, "skip_maintenance_mode"),
            RecoveryAction::EscalateToAi => write!(f, "escalate_to_ai"),
            RecoveryAction::AlertStaff => write!(f, "alert_staff"),
        }
    }
}

/// One recovery decision entry — written to JSONL log on each machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryDecision {
    /// ISO 8601 UTC timestamp
    pub timestamp: DateTime<Utc>,
    /// Machine identifier (e.g. "pod-3", "server", "james")
    pub machine: String,
    /// Process name (e.g. "rc-agent.exe", "ollama.exe")
    pub process: String,
    /// Who is making this decision
    pub authority: RecoveryAuthority,
    /// What action was taken or skipped
    pub action: RecoveryAction,
    /// Human-readable reason (e.g. "heartbeat_timeout_60s", "pattern_seen_3x")
    pub reason: String,
    /// Optional context (crash pattern, backoff attempt, etc.) — empty string if none
    pub context: String,
}

impl RecoveryDecision {
    pub fn new(
        machine: impl Into<String>,
        process: impl Into<String>,
        authority: RecoveryAuthority,
        action: RecoveryAction,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            machine: machine.into(),
            process: process.into(),
            authority,
            action,
            reason: reason.into(),
            context: String::new(),
        }
    }

    /// Serialize to a single JSONL line (no trailing newline).
    /// Returns Err only if serde_json serialization fails (never in practice).
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

// ─── Logger ───────────────────────────────────────────────────────────────────

/// Append-only JSONL writer for recovery decisions.
/// One line per decision. Never truncates. Never panics — warns on I/O error.
pub struct RecoveryLogger {
    path: std::path::PathBuf,
}

impl RecoveryLogger {
    /// Create logger pointing at `path`. Does NOT create the file — created lazily on first write.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append one JSONL line for `decision`. Creates file + parent dirs if absent.
    /// On I/O error: logs warn!(target: "recovery_logger", ...) and returns Ok(()).
    /// Never returns Err — callers must not be burdened with log write failures.
    pub fn log(&self, decision: &RecoveryDecision) -> std::io::Result<()> {
        let line = match decision.to_json_line() {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(
                    target: "recovery_logger",
                    path = %self.path.display(),
                    error = %e,
                    "failed to serialize recovery decision"
                );
                return Ok(());
            }
        };

        let result = self.write_line(&line);
        if let Err(ref e) = result {
            tracing::warn!(
                target: "recovery_logger",
                path = %self.path.display(),
                error = %e,
                "failed to write recovery decision to log"
            );
        }
        Ok(())
    }

    fn write_line(&self, line: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        use std::io::Write;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

// ─── Recovery Intent (COORD-02) ──────────────────────────────────────────────

/// A recovery intent registered by an authority before acting on a pod+process.
/// Any other authority that finds an active (non-expired) intent for the same
/// pod+process must back off for the TTL window (2 minutes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryIntent {
    /// Pod being recovered (e.g. "pod-3")
    pub pod_id: String,
    /// Process being recovered (e.g. "rc-agent.exe")
    pub process: String,
    /// Which recovery authority registered this intent
    pub authority: RecoveryAuthority,
    /// Human-readable reason for the recovery action
    pub reason: String,
    /// UTC timestamp when this intent was created
    pub created_at: DateTime<Utc>,
}

impl RecoveryIntent {
    /// Create a new RecoveryIntent stamped with the current UTC time.
    pub fn new(
        pod_id: impl Into<String>,
        process: impl Into<String>,
        authority: RecoveryAuthority,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            pod_id: pod_id.into(),
            process: process.into(),
            authority,
            reason: reason.into(),
            created_at: Utc::now(),
        }
    }

    /// Returns true if this intent is older than 2 minutes (120 seconds).
    pub fn is_expired(&self) -> bool {
        (Utc::now() - self.created_at).num_seconds() > 120
    }
}

// ─── Recovery Event (COORD-04) ────────────────────────────────────────────────

/// A recovery event reported by a pod-side recovery authority to the server.
/// Used for cross-machine recovery visibility (COORD-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryEvent {
    /// Pod identifier (e.g. "pod-1", "pod-8")
    pub pod_id: String,
    /// Process that was recovered (e.g. "rc-agent.exe")
    pub process: String,
    /// Which recovery system reported this
    pub authority: RecoveryAuthority,
    /// What action was taken
    pub action: RecoveryAction,
    /// Whether the spawned process was verified alive after restart
    pub spawn_verified: Option<bool>,
    /// Whether the racecontrol server was reachable at time of recovery
    pub server_reachable: Option<bool>,
    /// Human-readable reason
    pub reason: String,
    /// Optional context (crash pattern, error message, etc.)
    #[serde(default)]
    pub context: String,
    /// UTC timestamp -- set by server on receipt if not provided
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owner_of_returns_registered_authority() {
        let mut ownership = ProcessOwnership::new();
        ownership
            .register("rc-agent.exe", RecoveryAuthority::RcSentry)
            .expect("register should succeed");
        assert_eq!(
            ownership.owner_of("rc-agent.exe"),
            Some(RecoveryAuthority::RcSentry)
        );
    }

    #[test]
    fn test_owner_of_nonexistent_returns_none() {
        let ownership = ProcessOwnership::new();
        assert_eq!(ownership.owner_of("nonexistent.exe"), None);
    }

    #[test]
    fn test_register_conflict_returns_err() {
        let mut ownership = ProcessOwnership::new();
        ownership
            .register("rc-agent.exe", RecoveryAuthority::RcSentry)
            .expect("first register should succeed");
        let result = ownership.register("rc-agent.exe", RecoveryAuthority::PodHealer);
        assert!(
            result.is_err(),
            "re-registering with different authority should return Err"
        );
    }

    #[test]
    fn test_register_same_authority_idempotent() {
        let mut ownership = ProcessOwnership::new();
        ownership
            .register("rc-agent.exe", RecoveryAuthority::RcSentry)
            .expect("first register should succeed");
        let result = ownership.register("rc-agent.exe", RecoveryAuthority::RcSentry);
        assert!(result.is_ok(), "re-registering same authority should be Ok");
    }

    #[test]
    fn test_recovery_decision_serializes_all_fields() {
        let decision = RecoveryDecision::new(
            "pod-3",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            RecoveryAction::Restart,
            "heartbeat_timeout_60s",
        );
        let line = decision.to_json_line().expect("serialization should succeed");
        // Check all fields are present in the JSON output
        assert!(line.contains("\"timestamp\""), "timestamp field missing");
        assert!(line.contains("\"machine\""), "machine field missing");
        assert!(line.contains("\"process\""), "process field missing");
        assert!(line.contains("\"authority\""), "authority field missing");
        assert!(line.contains("\"action\""), "action field missing");
        assert!(line.contains("\"reason\""), "reason field missing");
        assert!(line.contains("\"context\""), "context field missing");
    }

    #[test]
    fn test_recovery_decision_roundtrip() {
        let decision = RecoveryDecision::new(
            "server",
            "ollama.exe",
            RecoveryAuthority::JamesMonitor,
            RecoveryAction::WakeOnLan,
            "pattern_seen_3x",
        );
        let line = decision.to_json_line().expect("serialization should succeed");
        let restored: RecoveryDecision =
            serde_json::from_str(&line).expect("deserialization should succeed");
        assert_eq!(restored.machine, decision.machine);
        assert_eq!(restored.process, decision.process);
        assert_eq!(restored.authority, decision.authority);
        assert_eq!(restored.action, decision.action);
        assert_eq!(restored.reason, decision.reason);
        assert_eq!(restored.context, decision.context);
    }

    #[test]
    fn test_recovery_logger_bad_path_returns_ok() {
        // Use a path in a nonexistent deeply nested directory that won't be created
        // by normal filesystem ops but IS writable if dirs exist. We use a drive letter
        // that doesn't exist to guarantee failure.
        //
        // On Windows, Z:\ typically doesn't exist. On CI / Linux, /proc/nonexistent works.
        // Use a temp-based approach: pass a file under a path we can't write to.
        #[cfg(windows)]
        let bad_path = r"Z:\nonexistent_drive\recovery-log.jsonl";
        #[cfg(not(windows))]
        let bad_path = "/proc/nonexistent/deep/recovery-log.jsonl";

        let logger = RecoveryLogger::new(bad_path);
        let decision = RecoveryDecision::new(
            "james",
            "ollama.exe",
            RecoveryAuthority::JamesMonitor,
            RecoveryAction::Restart,
            "test",
        );
        // Must not panic; must return Ok(())
        let result = logger.log(&decision);
        assert!(result.is_ok(), "log() on bad path must return Ok(())");
    }

    #[test]
    fn test_recovery_logger_creates_file() {
        let dir = std::env::temp_dir().join("recovery_logger_test");
        let path = dir.join("recovery-log.jsonl");
        // Clean up if exists
        let _ = std::fs::remove_dir_all(&dir);

        let logger = RecoveryLogger::new(&path);
        let decision = RecoveryDecision::new(
            "pod-1",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            RecoveryAction::Restart,
            "test_write",
        );
        logger.log(&decision).expect("log should succeed");
        assert!(path.exists(), "log file should be created");
        let content = std::fs::read_to_string(&path).expect("should be able to read log file");
        assert!(content.contains("rc-agent.exe"), "log should contain process name");

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recovery_event_serde_roundtrip() {
        use chrono::Utc;
        let event = RecoveryEvent {
            pod_id: "pod-8".to_string(),
            process: "rc-agent.exe".to_string(),
            authority: RecoveryAuthority::RcSentry,
            action: RecoveryAction::Restart,
            spawn_verified: Some(true),
            server_reachable: Some(false),
            reason: "heartbeat_timeout_60s".to_string(),
            context: "crash_pattern_3x".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        // Verify all fields present
        assert!(json.contains("\"pod_id\""), "pod_id missing");
        assert!(json.contains("\"process\""), "process missing");
        assert!(json.contains("\"authority\""), "authority missing");
        assert!(json.contains("\"action\""), "action missing");
        assert!(json.contains("\"spawn_verified\""), "spawn_verified missing");
        assert!(json.contains("\"server_reachable\""), "server_reachable missing");
        assert!(json.contains("\"reason\""), "reason missing");
        assert!(json.contains("\"context\""), "context missing");
        assert!(json.contains("\"timestamp\""), "timestamp missing");
        // Roundtrip
        let restored: RecoveryEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.pod_id, "pod-8");
        assert_eq!(restored.process, "rc-agent.exe");
        assert_eq!(restored.spawn_verified, Some(true));
        assert_eq!(restored.server_reachable, Some(false));
        assert_eq!(restored.reason, "heartbeat_timeout_60s");
        assert_eq!(restored.context, "crash_pattern_3x");
    }

    #[test]
    fn test_recovery_intent_not_expired_when_fresh() {
        let intent = RecoveryIntent::new(
            "pod-1",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            "test_reason",
        );
        assert!(!intent.is_expired(), "freshly created intent must not be expired");
    }

    #[test]
    fn test_recovery_intent_fields() {
        let intent = RecoveryIntent::new(
            "pod-5",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            "heartbeat_timeout",
        );
        assert_eq!(intent.pod_id, "pod-5");
        assert_eq!(intent.process, "rc-agent.exe");
        assert_eq!(intent.authority, RecoveryAuthority::RcSentry);
        assert_eq!(intent.reason, "heartbeat_timeout");
        assert!(!intent.created_at.timestamp().is_negative(), "created_at must be a valid UTC timestamp");
    }

    #[test]
    fn test_log_path_constants_defined() {
        // Ensure constants are non-empty strings
        assert!(!RECOVERY_LOG_SERVER.is_empty());
        assert!(!RECOVERY_LOG_POD.is_empty());
        assert!(!RECOVERY_LOG_JAMES.is_empty());
    }
}
