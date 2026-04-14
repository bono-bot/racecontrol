//! Fleet Deploy Orchestration — Phase 304
//!
//! Provides `FleetDeploySession`, `run_fleet_deploy()`, and supporting types for
//! orchestrating rolling binary deployments across all pods.
//!
//! Wave layout (from ota_pipeline constants):
//!   Wave 1 (canary): Pod 8
//!   Wave 2:          Pods 1-4
//!   Wave 3:          Pods 5-7
//!
//! Canary failure halts the entire deploy. Non-canary pod failure triggers per-pod
//! rollback but the deploy continues to the next pod.

use crate::ota_pipeline::{WAVE_1, WAVE_2, WAVE_3};

#[path = "fleet_deploy_orchestration.rs"]
mod orchestration;

pub use orchestration::run_fleet_deploy;

// ---------------------------------------------------------------------------
// Request / Scope types
// ---------------------------------------------------------------------------

/// Request body for `POST /api/v1/fleet/deploy`.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct FleetDeployRequest {
    /// Expected SHA-256 of the binary.
    pub binary_hash: String,
    /// HTTP URL to download the binary from (staging server).
    pub binary_url: String,
    /// Deployment scope: all pods, canary only, or a specific set.
    pub scope: DeployScope,
    /// Seconds to wait between waves (default 5).
    #[serde(default = "default_wave_delay")]
    pub wave_delay_secs: u64,
    /// Override weekend peak-hour deploy lock.
    #[serde(default)]
    pub force: bool,
}

fn default_wave_delay() -> u64 {
    5
}

/// Which pods to target in a fleet deploy.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeployScope {
    /// All 8 pods in canonical wave order (Wave 1 → 2 → 3).
    All,
    /// Canary only (Pod 8 — Wave 1).
    Canary,
    /// Specific pod numbers (treated as a single wave).
    Pods(Vec<u32>),
}

// ---------------------------------------------------------------------------
// Session state types
// ---------------------------------------------------------------------------

/// Overall status of a fleet deploy session.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeployOverallStatus {
    Pending,
    Running,
    Completed,
    Failed,
    RollingBack,
}

/// Status of a single wave in a fleet deploy.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WaveDeployStatus {
    Pending,
    Running,
    Passed,
    Failed,
}

/// Per-wave tracking: which pods, their results, and timestamps.
#[derive(serde::Serialize, Clone, Debug)]
pub struct WaveStatus {
    pub wave_number: u8,
    /// Pod IDs in this wave, e.g. `["pod_8"]` or `["pod_1","pod_2","pod_3","pod_4"]`.
    pub pods: Vec<String>,
    pub status: WaveDeployStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub pod_results: Vec<PodDeployResult>,
}

/// Outcome for one pod in a wave.
#[derive(serde::Serialize, Clone, Debug)]
pub struct PodDeployResult {
    pub pod_id: String,
    /// `"complete"` | `"failed"` | `"waiting_session"` | `"rolled_back"` | `"skipped"`
    pub status: String,
    pub detail: Option<String>,
}

/// A rollback that was triggered for a specific pod in a wave.
#[derive(serde::Serialize, Clone, Debug)]
pub struct RollbackEvent {
    pub wave: u8,
    pub pod_id: String,
    pub reason: String,
    pub rolled_back_at: String,
    /// `"success"` | `"failed"`
    pub outcome: String,
}

/// In-memory session tracking for the current (or last) fleet deploy.
/// Stored in `AppState::fleet_deploy_session`.
#[derive(serde::Serialize, Clone, Debug)]
pub struct FleetDeploySession {
    pub deploy_id: String,
    pub binary_hash: String,
    pub binary_url: String,
    pub scope: DeployScope,
    pub wave_delay_secs: u64,
    pub initiated_by: String,
    pub initiated_at: String,
    /// 0 = not started, 1 = canary wave, 2 = wave 2, 3 = wave 3.
    pub current_wave: u8,
    pub overall_status: DeployOverallStatus,
    pub waves: Vec<WaveStatus>,
    pub rollback_events: Vec<RollbackEvent>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the current time in IST formatted as RFC 3339.
fn now_ist_rfc3339() -> String {
    use chrono::{Duration, Utc};
    let utc = Utc::now();
    let ist = utc + Duration::hours(5) + Duration::minutes(30);
    // Build a FixedOffset-aware datetime for RFC 3339 with correct offset.
    // IST = UTC+5:30 = 19800 seconds. Matches pattern used in routes.rs:21121.
    // east_opt(0) returns Some unconditionally — this fallback is unreachable at runtime.
    #[allow(clippy::unwrap_used)]
    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
    let ist_with_offset = ist.with_timezone(&offset);
    ist_with_offset.to_rfc3339()
}

/// Build a `FleetDeploySession` from the incoming request.
/// Pre-populates `waves` based on scope; all start as `Pending`.
pub fn create_session(req: &FleetDeployRequest, initiated_by: &str) -> FleetDeploySession {
    let deploy_id = format!("{}-{}", &req.binary_hash[..8.min(req.binary_hash.len())], chrono::Utc::now().timestamp());

    let waves = match &req.scope {
        DeployScope::All => vec![
            make_wave(1, WAVE_1),
            make_wave(2, WAVE_2),
            make_wave(3, WAVE_3),
        ],
        DeployScope::Canary => vec![
            make_wave(1, WAVE_1),
        ],
        DeployScope::Pods(ids) => {
            let pod_ids: Vec<String> = ids.iter().map(|n| format!("pod_{n}")).collect();
            vec![WaveStatus {
                wave_number: 1,
                pods: pod_ids,
                status: WaveDeployStatus::Pending,
                started_at: None,
                completed_at: None,
                pod_results: vec![],
            }]
        }
    };

    FleetDeploySession {
        deploy_id,
        binary_hash: req.binary_hash.clone(),
        binary_url: req.binary_url.clone(),
        scope: req.scope.clone(),
        wave_delay_secs: req.wave_delay_secs,
        initiated_by: initiated_by.to_string(),
        initiated_at: now_ist_rfc3339(),
        current_wave: 0,
        overall_status: DeployOverallStatus::Pending,
        waves,
        rollback_events: vec![],
    }
}

fn make_wave(wave_number: u8, pod_numbers: &[u32]) -> WaveStatus {
    WaveStatus {
        wave_number,
        pods: pod_numbers.iter().map(|n| format!("pod_{n}")).collect(),
        status: WaveDeployStatus::Pending,
        started_at: None,
        completed_at: None,
        pod_results: vec![],
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(scope: DeployScope) -> FleetDeployRequest {
        FleetDeployRequest {
            binary_hash: "abcdef1234567890".to_string(),
            binary_url: "http://192.168.31.27:18889/rc-agent-abcdef12.exe".to_string(),
            scope,
            wave_delay_secs: 5,
            force: false,
        }
    }

    // --- create_session tests ---

    #[test]
    fn test_create_session_all_scope() {
        let req = make_request(DeployScope::All);
        let session = create_session(&req, "admin");
        assert_eq!(session.waves.len(), 3);
        assert_eq!(session.waves[0].wave_number, 1);
        assert_eq!(session.waves[0].pods, vec!["pod_8"]);
        assert_eq!(session.waves[1].wave_number, 2);
        assert_eq!(session.waves[1].pods, vec!["pod_1", "pod_2", "pod_3", "pod_4"]);
        assert_eq!(session.waves[2].wave_number, 3);
        assert_eq!(session.waves[2].pods, vec!["pod_5", "pod_6", "pod_7"]);
    }

    #[test]
    fn test_create_session_canary_scope() {
        let req = make_request(DeployScope::Canary);
        let session = create_session(&req, "admin");
        assert_eq!(session.waves.len(), 1);
        assert_eq!(session.waves[0].wave_number, 1);
        assert_eq!(session.waves[0].pods, vec!["pod_8"]);
    }

    #[test]
    fn test_create_session_specific_pods() {
        let req = make_request(DeployScope::Pods(vec![1, 3, 5]));
        let session = create_session(&req, "admin");
        assert_eq!(session.waves.len(), 1);
        assert_eq!(session.waves[0].wave_number, 1);
        assert_eq!(session.waves[0].pods, vec!["pod_1", "pod_3", "pod_5"]);
    }

    #[test]
    fn test_deploy_id_format() {
        let req = make_request(DeployScope::Canary);
        let session = create_session(&req, "admin");
        assert!(session.deploy_id.starts_with("abcdef12"), "deploy_id should start with first 8 chars of hash");
        assert!(session.deploy_id.contains('-'), "deploy_id should contain a hyphen separator");
    }

    #[test]
    fn test_deploy_scope_serde() {
        // All
        let all = DeployScope::All;
        let json = serde_json::to_string(&all).unwrap();
        let parsed: DeployScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployScope::All);

        // Canary
        let canary = DeployScope::Canary;
        let json = serde_json::to_string(&canary).unwrap();
        let parsed: DeployScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployScope::Canary);

        // Pods
        let pods = DeployScope::Pods(vec![1, 3, 5]);
        let json = serde_json::to_string(&pods).unwrap();
        let parsed: DeployScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeployScope::Pods(vec![1, 3, 5]));
    }

    #[test]
    fn test_default_wave_delay() {
        let json = r#"{"binary_hash":"abc123","binary_url":"http://host/file.exe","scope":"all"}"#;
        let req: FleetDeployRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.wave_delay_secs, 5);
    }

    #[test]
    fn test_rollback_event_serialization() {
        let event = RollbackEvent {
            wave: 1,
            pod_id: "pod_8".to_string(),
            reason: "deploy failed: size mismatch".to_string(),
            rolled_back_at: "2026-04-02T12:00:00+05:30".to_string(),
            outcome: "success".to_string(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["wave"], 1);
        assert_eq!(json["pod_id"], "pod_8");
        assert_eq!(json["outcome"], "success");
        assert!(json["rolled_back_at"].as_str().unwrap().contains("05:30"));
    }

    #[test]
    fn test_wave_status_lifecycle() {
        let mut wave = WaveStatus {
            wave_number: 1,
            pods: vec!["pod_8".to_string()],
            status: WaveDeployStatus::Pending,
            started_at: None,
            completed_at: None,
            pod_results: vec![],
        };
        assert_eq!(wave.status, WaveDeployStatus::Pending);
        wave.status = WaveDeployStatus::Running;
        wave.started_at = Some("2026-04-02T12:00:00+05:30".to_string());
        assert_eq!(wave.status, WaveDeployStatus::Running);
        wave.status = WaveDeployStatus::Passed;
        wave.completed_at = Some("2026-04-02T12:01:00+05:30".to_string());
        assert_eq!(wave.status, WaveDeployStatus::Passed);
        assert!(wave.started_at.is_some());
        assert!(wave.completed_at.is_some());
    }

    #[test]
    fn test_overall_status_variants() {
        let statuses = vec![
            (DeployOverallStatus::Pending, "pending"),
            (DeployOverallStatus::Running, "running"),
            (DeployOverallStatus::Completed, "completed"),
            (DeployOverallStatus::Failed, "failed"),
            (DeployOverallStatus::RollingBack, "rolling_back"),
        ];
        for (status, expected_str) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", expected_str), "Unexpected serialization for {:?}", status);
        }
    }

    #[test]
    fn test_fleet_deploy_request_deserialization() {
        let json = r#"{
            "binary_hash": "deadbeef12345678",
            "binary_url": "http://192.168.31.27:18889/rc-agent-deadbeef.exe",
            "scope": {"pods": [1, 2, 3]},
            "wave_delay_secs": 10,
            "force": true
        }"#;
        let req: FleetDeployRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.binary_hash, "deadbeef12345678");
        assert_eq!(req.wave_delay_secs, 10);
        assert!(req.force);
        assert_eq!(req.scope, DeployScope::Pods(vec![1, 2, 3]));
    }

    #[test]
    fn test_canary_is_wave_1() {
        let req = make_request(DeployScope::All);
        let session = create_session(&req, "admin");
        let canary_wave = session.waves.iter().find(|w| w.wave_number == 1).unwrap();
        assert_eq!(canary_wave.pods, vec!["pod_8"], "Wave 1 must contain only pod_8 (canary)");
    }
}
