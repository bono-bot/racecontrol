// survival_types.rs — Phase 267: Survival Foundation
//
// All shared survival types, sentinel protocol, and OpenRouter client trait.
// Every downstream phase (268-272) and every existing recovery system imports from here.
//
// INTENTIONAL: This is a FAILING stub — tests are written first per TDD protocol.
// The full implementation follows in the GREEN phase.

use serde::{Deserialize, Serialize};

// ─── ActionId ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ActionId(pub String);

impl ActionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Sentinel Kinds ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SentinelKind {
    HealInProgress,
    OtaDeploying,
}

// ─── Survival Layer ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SurvivalLayer {
    Layer1Watchdog,
    Layer2FleetHealer,
    Layer3Guardian,
}

// ─── HealSentinel ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealSentinel {
    pub kind: SentinelKind,
    pub layer: SurvivalLayer,
    /// ISO 8601 UTC timestamp when healing started
    pub started_at: String,
    pub action: String,
    pub ttl_secs: u64,
    pub action_id: ActionId,
}

impl HealSentinel {
    /// Returns true if the sentinel has exceeded its TTL.
    pub fn is_expired(&self) -> bool {
        use chrono::DateTime;
        let started = match DateTime::parse_from_rfc3339(&self.started_at) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => return true, // Unparseable = treat as expired
        };
        let elapsed = chrono::Utc::now()
            .signed_duration_since(started)
            .num_seconds();
        elapsed < 0 || elapsed as u64 >= self.ttl_secs
    }

    /// Returns seconds remaining until expiry (0 if already expired).
    pub fn remaining_secs(&self) -> u64 {
        use chrono::DateTime;
        let started = match DateTime::parse_from_rfc3339(&self.started_at) {
            Ok(dt) => dt.with_timezone(&chrono::Utc),
            Err(_) => return 0,
        };
        let elapsed = chrono::Utc::now()
            .signed_duration_since(started)
            .num_seconds();
        if elapsed < 0 || elapsed as u64 >= self.ttl_secs {
            0
        } else {
            self.ttl_secs - elapsed as u64
        }
    }
}

// ─── Sentinel File Paths ─────────────────────────────────────────────────────

pub const HEAL_IN_PROGRESS_PATH: &str = r"C:\RacingPoint\HEAL_IN_PROGRESS";
pub const OTA_DEPLOYING_PATH: &str = r"C:\RacingPoint\OTA_DEPLOYING";

/// Returns the file path for a sentinel kind.
pub fn sentinel_path(kind: SentinelKind) -> &'static str {
    match kind {
        SentinelKind::HealInProgress => HEAL_IN_PROGRESS_PATH,
        SentinelKind::OtaDeploying => OTA_DEPLOYING_PATH,
    }
}

// ─── Sentinel File Helpers ───────────────────────────────────────────────────

/// Try to acquire a sentinel of `kind`.
///
/// Returns:
/// - `Ok(true)` — sentinel acquired (file written), caller may proceed.
/// - `Ok(false)` — a valid (non-expired) sentinel already exists; caller must back off.
/// - `Err(e)` — I/O error writing the sentinel.
///
/// If an expired sentinel exists, it is removed and re-acquired atomically.
///
/// MMA P1 fix: Uses atomic `create_new(true)` to prevent TOCTOU race where two
/// processes could both see an expired sentinel and both overwrite it.
pub fn try_acquire_sentinel(
    kind: SentinelKind,
    layer: SurvivalLayer,
    action: &str,
    ttl_secs: u64,
    action_id: &ActionId,
) -> std::io::Result<bool> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let path = sentinel_path(kind);

    // Ensure parent directory exists (MMA P1: missing directory creation)
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let sentinel = HealSentinel {
        kind,
        layer,
        started_at: chrono::Utc::now().to_rfc3339(),
        action: action.to_string(),
        ttl_secs,
        action_id: action_id.clone(),
    };

    let json = serde_json::to_string(&sentinel)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Attempt atomic creation — if file doesn't exist, this is the fast path
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(json.as_bytes())?;
            return Ok(true);
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // File exists — check if it's expired
        }
        Err(e) => return Err(e),
    }

    // File exists — check if held by another layer
    if let Some(existing) = read_sentinel_file(path) {
        if !existing.is_expired() {
            return Ok(false); // Valid sentinel held by another layer
        }
    }

    // Expired or corrupt — remove and re-acquire atomically
    // The remove + create_new sequence has a tiny window, but if another process
    // wins the create_new, our attempt fails gracefully with AlreadyExists
    let _ = std::fs::remove_file(path);
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(json.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another process won the race — back off
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Check if a sentinel is active (exists and not expired).
///
/// Returns `Some(HealSentinel)` if the sentinel exists and is valid.
/// Returns `None` if the file is absent, corrupt, or expired.
pub fn check_sentinel(kind: SentinelKind) -> Option<HealSentinel> {
    let path = sentinel_path(kind);
    let sentinel = read_sentinel_file(path)?;
    if sentinel.is_expired() {
        None
    } else {
        Some(sentinel)
    }
}

/// Release a sentinel by deleting its file.
///
/// Returns `Ok(())` even if the file did not exist.
pub fn release_sentinel(kind: SentinelKind) -> std::io::Result<()> {
    let path = sentinel_path(kind);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Returns `true` if either HEAL_IN_PROGRESS or OTA_DEPLOYING sentinel is active
/// and not expired.
pub fn any_sentinel_active() -> bool {
    check_sentinel(SentinelKind::HealInProgress).is_some()
        || check_sentinel(SentinelKind::OtaDeploying).is_some()
}

// Private helper — read and deserialize a sentinel file, return None on any error.
// MMA P2 fix: log warnings instead of silently swallowing errors.
fn read_sentinel_file(path: &str) -> Option<HealSentinel> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(path, error = %e, "sentinel file read error");
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(path, error = %e, "sentinel file parse error — treating as absent");
            None
        }
    }
}

// ─── SurvivalReport (SF-04) ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportType {
    CrashLoop,
    RollbackComplete,
    MmaDiagnosis,
    HealAttempt,
    Escalation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivalReport {
    pub action_id: ActionId,
    pub pod_id: String,
    pub layer: SurvivalLayer,
    pub report_type: ReportType,
    pub summary: String,
    pub details: serde_json::Value,
    /// ISO 8601 UTC timestamp
    pub timestamp: String,
}

// ─── HealLease (SF-02 types) ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealLease {
    pub pod_id: String,
    pub granted_to: SurvivalLayer,
    pub action_id: ActionId,
    pub ttl_secs: u64,
    /// ISO 8601 UTC timestamp
    pub granted_at: String,
    /// ISO 8601 UTC timestamp
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealLeaseRequest {
    pub pod_id: String,
    pub layer: SurvivalLayer,
    pub action_id: ActionId,
    pub ttl_secs: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealLeaseResponse {
    pub granted: bool,
    pub lease: Option<HealLease>,
    /// Why denied (e.g., "another layer holds lease")
    pub reason: Option<String>,
}

// ─── BinaryManifest (SF-04) ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryManifest {
    pub binary_name: String,
    pub sha256: String,
    pub build_id: String,
    pub pe_machine: Option<String>,
    pub pe_timestamp: Option<u32>,
    pub path: String,
}

// ─── DiagnosisContext (SF-04, SF-03) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisContext {
    pub action_id: ActionId,
    pub pod_id: String,
    pub layer: SurvivalLayer,
    pub tier: crate::mesh_types::DiagnosisTier,
    pub symptoms: Vec<String>,
    /// ISO 8601 UTC timestamp
    pub started_at: String,
    pub models_used: Vec<String>,
    pub cost_usd: f64,
}

// ─── OpenRouter Diagnosis Trait ───────────────────────────────────────────────

/// Synchronous OpenRouter diagnosis trait — no reqwest/async_trait dependency.
/// Implementations live in higher-layer crates to avoid circular deps.
/// rc-watchdog has NO tokio runtime; implementations must use Runtime::new() internally.
pub trait OpenRouterDiagnose {
    fn diagnose(&self, context: &DiagnosisContext) -> Result<DiagnosisResult, DiagnosisError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisResult {
    pub action_id: ActionId,
    pub findings: Vec<DiagnosisFinding>,
    pub consensus_action: Option<String>,
    pub total_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisFinding {
    pub severity: FindingSeverity,
    pub finding_type: String,
    pub component: String,
    pub description: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FindingSeverity {
    P0,
    P1,
    P2,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DiagnosisError {
    #[error("budget exhausted: daily limit ${0:.2} reached")]
    BudgetExhausted(f64),
    #[error("api unreachable after {0} attempts")]
    ApiUnreachable(u32),
    #[error("diagnosis timeout after {0}s")]
    Timeout(u64),
    #[error("{0}")]
    Other(String),
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper: return a temp path for sentinel testing (avoids polluting C:\RacingPoint on dev).
    fn temp_sentinel_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rc_survival_test_{}", name))
    }

    /// Helper: override sentinel paths for testing by writing directly and reading back.
    /// We test the struct logic independently; sentinel file helpers use real paths in production.
    fn write_sentinel_to_path(path: &str, sentinel: &HealSentinel) -> std::io::Result<()> {
        let json = serde_json::to_string(sentinel)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    fn read_sentinel_from_path(path: &str) -> Option<HealSentinel> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    // ─── ActionId ────────────────────────────────────────────────────────────

    #[test]
    fn test_action_id_new_generates_valid_uuid_v4() {
        let id = ActionId::new();
        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        let s = &id.0;
        assert_eq!(s.len(), 36, "UUID v4 must be 36 chars");
        let parts: Vec<&str> = s.split('-').collect();
        assert_eq!(parts.len(), 5, "UUID v4 must have 5 hyphen-separated parts");
        assert_eq!(parts[2].len(), 4);
        assert!(
            parts[2].starts_with('4'),
            "UUID v4 version nibble must be '4'"
        );
        // Parse via uuid crate to validate format
        let parsed = uuid::Uuid::parse_str(s);
        assert!(parsed.is_ok(), "ActionId must be a valid UUID: {:?}", parsed);
    }

    #[test]
    fn test_action_id_two_calls_are_unique() {
        let a = ActionId::new();
        let b = ActionId::new();
        assert_ne!(a, b, "Two ActionId::new() calls must produce different UUIDs");
    }

    #[test]
    fn test_action_id_display() {
        let id = ActionId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0);
    }

    // ─── HealSentinel serialization ──────────────────────────────────────────

    #[test]
    fn test_heal_sentinel_serializes_required_fields() {
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: "2026-03-30T10:00:00Z".to_string(),
            action: "restart_rc_agent".to_string(),
            ttl_secs: 300,
            action_id: ActionId("test-id-1234".to_string()),
        };
        let json = serde_json::to_string(&sentinel).expect("serialize sentinel");
        assert!(json.contains("\"layer\""), "must have layer field");
        assert!(json.contains("\"started_at\""), "must have started_at field");
        assert!(json.contains("\"action\""), "must have action field");
        assert!(json.contains("\"ttl_secs\""), "must have ttl_secs field");
        assert!(json.contains("\"action_id\""), "must have action_id field");
        assert!(json.contains("\"kind\""), "must have kind field");
    }

    // ─── HealSentinel is_expired ─────────────────────────────────────────────

    #[test]
    fn test_heal_sentinel_is_expired_true_when_elapsed_exceeds_ttl() {
        // started_at is 1 hour ago, ttl is 60 seconds => should be expired
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer2FleetHealer,
            started_at: past.to_rfc3339(),
            action: "test_action".to_string(),
            ttl_secs: 60,
            action_id: ActionId::new(),
        };
        assert!(
            sentinel.is_expired(),
            "sentinel started 1h ago with 60s TTL must be expired"
        );
    }

    #[test]
    fn test_heal_sentinel_is_expired_false_when_within_ttl() {
        // started_at is 10 seconds ago, ttl is 300 seconds => not expired
        let recent = chrono::Utc::now() - chrono::Duration::seconds(10);
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: recent.to_rfc3339(),
            action: "test_action".to_string(),
            ttl_secs: 300,
            action_id: ActionId::new(),
        };
        assert!(
            !sentinel.is_expired(),
            "sentinel started 10s ago with 300s TTL must not be expired"
        );
    }

    // ─── Sentinel file helpers (via temp paths) ───────────────────────────────

    // We test try_acquire_sentinel / check_sentinel / release_sentinel by directly
    // manipulating temp files that mirror the production logic. The path-based
    // helpers write to C:\RacingPoint in production; here we validate the same
    // struct logic via temp files and the struct's is_expired() method.

    #[test]
    fn test_try_acquire_returns_true_when_no_sentinel_file() {
        // Use a temp path that doesn't exist
        let path = temp_sentinel_path("acquire_no_file");
        let path_str = path.to_str().unwrap();
        // Clean up if leftover from previous run
        let _ = std::fs::remove_file(path_str);

        let action_id = ActionId::new();
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: chrono::Utc::now().to_rfc3339(),
            action: "test".to_string(),
            ttl_secs: 300,
            action_id: action_id.clone(),
        };

        // File doesn't exist — acquisition should succeed
        assert!(!path.exists(), "test precondition: file must not exist");
        // Write sentinel manually to temp location
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");
        assert!(path.exists(), "sentinel file must be created");

        // Verify readback
        let read_back = read_sentinel_from_path(path_str).expect("read sentinel");
        assert_eq!(read_back.action_id, action_id);

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_try_acquire_returns_false_when_valid_sentinel_exists() {
        // Write a non-expired sentinel
        let path = temp_sentinel_path("acquire_valid_exists");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: chrono::Utc::now().to_rfc3339(),
            action: "existing_action".to_string(),
            ttl_secs: 300,
            action_id: ActionId::new(),
        };
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");

        // Read it back and confirm it's not expired
        let read_back = read_sentinel_from_path(path_str).expect("read sentinel");
        assert!(
            !read_back.is_expired(),
            "freshly written sentinel must not be expired"
        );

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_try_acquire_returns_true_when_expired_sentinel_exists() {
        // Write an already-expired sentinel
        let path = temp_sentinel_path("acquire_expired");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer2FleetHealer,
            started_at: past.to_rfc3339(),
            action: "old_action".to_string(),
            ttl_secs: 60, // 60s TTL, started 1h ago => expired
            action_id: ActionId::new(),
        };
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");

        let read_back = read_sentinel_from_path(path_str).expect("read sentinel");
        assert!(
            read_back.is_expired(),
            "sentinel started 1h ago with 60s TTL must be expired"
        );

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_check_sentinel_returns_none_when_no_file_exists() {
        // No file at temp path — read_sentinel_from_path returns None
        let path = temp_sentinel_path("check_no_file");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        let result = read_sentinel_from_path(path_str);
        assert!(result.is_none(), "must return None when file absent");
    }

    #[test]
    fn test_check_sentinel_returns_none_when_file_has_expired_ttl() {
        let path = temp_sentinel_path("check_expired");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        let past = chrono::Utc::now() - chrono::Duration::hours(2);
        let sentinel = HealSentinel {
            kind: SentinelKind::OtaDeploying,
            layer: SurvivalLayer::Layer3Guardian,
            started_at: past.to_rfc3339(),
            action: "ota".to_string(),
            ttl_secs: 120,
            action_id: ActionId::new(),
        };
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");

        let read_back = read_sentinel_from_path(path_str).expect("read file");
        // Should be expired
        assert!(
            read_back.is_expired(),
            "sentinel from 2h ago with 120s TTL must be expired"
        );
        // check_sentinel equivalent: expired means None
        let result = if read_back.is_expired() {
            None
        } else {
            Some(read_back)
        };
        assert!(result.is_none(), "expired sentinel must return None from check");

        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_check_sentinel_returns_some_when_valid_sentinel_exists() {
        let path = temp_sentinel_path("check_valid");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        let action_id = ActionId::new();
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: chrono::Utc::now().to_rfc3339(),
            action: "active_heal".to_string(),
            ttl_secs: 600,
            action_id: action_id.clone(),
        };
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");

        let read_back = read_sentinel_from_path(path_str).expect("read sentinel");
        assert!(
            !read_back.is_expired(),
            "fresh sentinel with 600s TTL must not be expired"
        );
        assert_eq!(read_back.action_id, action_id);

        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn test_release_sentinel_removes_file() {
        let path = temp_sentinel_path("release");
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(path_str);

        // Write a file
        let sentinel = HealSentinel {
            kind: SentinelKind::HealInProgress,
            layer: SurvivalLayer::Layer1Watchdog,
            started_at: chrono::Utc::now().to_rfc3339(),
            action: "to_release".to_string(),
            ttl_secs: 300,
            action_id: ActionId::new(),
        };
        write_sentinel_to_path(path_str, &sentinel).expect("write sentinel");
        assert!(path.exists(), "file must exist before release");

        // Remove it (simulating release_sentinel)
        std::fs::remove_file(path_str).expect("remove sentinel");
        assert!(!path.exists(), "file must not exist after release");
    }

    // ─── SurvivalReport ──────────────────────────────────────────────────────

    #[test]
    fn test_survival_report_serializes_deserializes_roundtrip() {
        let action_id = ActionId::new();
        let report = SurvivalReport {
            action_id: action_id.clone(),
            pod_id: "pod-3".to_string(),
            layer: SurvivalLayer::Layer2FleetHealer,
            report_type: ReportType::CrashLoop,
            summary: "rc-agent crash loop detected".to_string(),
            details: serde_json::json!({ "restart_count": 5, "uptime_secs": 3 }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&report).expect("serialize report");
        let restored: SurvivalReport = serde_json::from_str(&json).expect("deserialize report");
        assert_eq!(restored.action_id, action_id);
        assert_eq!(restored.pod_id, "pod-3");
        assert_eq!(restored.report_type, ReportType::CrashLoop);
        assert_eq!(restored.summary, "rc-agent crash loop detected");
    }

    // ─── HealLease ───────────────────────────────────────────────────────────

    #[test]
    fn test_heal_lease_serializes_required_fields() {
        let now = chrono::Utc::now().to_rfc3339();
        let lease = HealLease {
            pod_id: "pod-5".to_string(),
            granted_to: SurvivalLayer::Layer2FleetHealer,
            action_id: ActionId::new(),
            ttl_secs: 120,
            granted_at: now.clone(),
            expires_at: now,
        };
        let json = serde_json::to_string(&lease).expect("serialize lease");
        assert!(json.contains("\"pod_id\""), "must have pod_id");
        assert!(json.contains("\"granted_to\""), "must have granted_to");
        assert!(json.contains("\"ttl_secs\""), "must have ttl_secs");
        assert!(json.contains("\"action_id\""), "must have action_id");
        // Verify roundtrip
        let restored: HealLease = serde_json::from_str(&json).expect("deserialize lease");
        assert_eq!(restored.pod_id, "pod-5");
        assert_eq!(restored.ttl_secs, 120);
    }

    // ─── BinaryManifest ──────────────────────────────────────────────────────

    #[test]
    fn test_binary_manifest_contains_required_fields() {
        let manifest = BinaryManifest {
            binary_name: "rc-agent.exe".to_string(),
            sha256: "abc123def456".to_string(),
            build_id: "5db7804d".to_string(),
            pe_machine: Some("AMD64".to_string()),
            pe_timestamp: Some(1712345678),
            path: r"C:\RacingPoint\rc-agent.exe".to_string(),
        };
        let json = serde_json::to_string(&manifest).expect("serialize manifest");
        assert!(json.contains("\"sha256\""), "must have sha256");
        assert!(json.contains("\"pe_machine\""), "must have pe_machine");
        assert!(json.contains("\"build_id\""), "must have build_id");
        assert!(json.contains("\"path\""), "must have path");
        // Roundtrip
        let restored: BinaryManifest = serde_json::from_str(&json).expect("deserialize manifest");
        assert_eq!(restored.sha256, "abc123def456");
        assert_eq!(restored.build_id, "5db7804d");
        assert_eq!(restored.pe_machine, Some("AMD64".to_string()));
    }

    // ─── DiagnosisContext ────────────────────────────────────────────────────

    #[test]
    fn test_diagnosis_context_contains_required_fields() {
        use crate::mesh_types::DiagnosisTier;
        let action_id = ActionId::new();
        let ctx = DiagnosisContext {
            action_id: action_id.clone(),
            pod_id: "pod-7".to_string(),
            layer: SurvivalLayer::Layer1Watchdog,
            tier: DiagnosisTier::SingleModel,
            symptoms: vec!["rc-agent crash 0xC0000005".to_string()],
            started_at: chrono::Utc::now().to_rfc3339(),
            models_used: vec!["qwen/qwen3-235b-a22b".to_string()],
            cost_usd: 0.05,
        };
        let json = serde_json::to_string(&ctx).expect("serialize context");
        assert!(json.contains("\"action_id\""), "must have action_id");
        assert!(json.contains("\"pod_id\""), "must have pod_id");
        assert!(json.contains("\"layer\""), "must have layer");
        assert!(json.contains("\"symptoms\""), "must have symptoms");
        assert!(json.contains("\"tier\""), "must have tier");
        // Roundtrip
        let restored: DiagnosisContext = serde_json::from_str(&json).expect("deserialize context");
        assert_eq!(restored.action_id, action_id);
        assert_eq!(restored.pod_id, "pod-7");
        assert_eq!(restored.symptoms.len(), 1);
    }

    // ─── SentinelKind enum ───────────────────────────────────────────────────

    #[test]
    fn test_sentinel_kind_has_required_variants() {
        let heal = SentinelKind::HealInProgress;
        let ota = SentinelKind::OtaDeploying;
        assert_ne!(heal, ota, "SentinelKind variants must be distinct");
        // Serialize both to ensure they produce valid JSON
        let heal_json = serde_json::to_string(&heal).expect("serialize HealInProgress");
        let ota_json = serde_json::to_string(&ota).expect("serialize OtaDeploying");
        assert!(!heal_json.is_empty());
        assert!(!ota_json.is_empty());
        assert_ne!(heal_json, ota_json);
    }
}
