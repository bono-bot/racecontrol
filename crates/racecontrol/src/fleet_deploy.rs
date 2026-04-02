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

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::AppState;
use crate::ota_pipeline::{WAVE_1, WAVE_2, WAVE_3};
use rc_common::types::DeployState;

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
// Core orchestration
// ---------------------------------------------------------------------------

/// Run a fleet deploy session end-to-end.
///
/// The caller creates a `FleetDeploySession` via `create_session()`, writes it to
/// `session_lock`, then calls this function in a `tokio::spawn` task.
///
/// # Wave semantics
/// - Wave 1 (canary): failure halts the entire deploy and triggers rollback.
/// - Wave 2/3: per-pod failure triggers per-pod rollback but the wave continues.
///
/// # Lock discipline
/// The `session_lock` guard is NEVER held across `.await` — state mutations use
/// the `{ let mut g = lock.write().await; ...; } // g dropped` pattern.
pub async fn run_fleet_deploy(
    state: Arc<AppState>,
    session_lock: Arc<RwLock<Option<FleetDeploySession>>>,
) {
    // Mark session as Running.
    {
        let mut guard = session_lock.write().await;
        if let Some(ref mut s) = *guard {
            s.overall_status = DeployOverallStatus::Running;
        }
    }

    // Collect wave data needed for orchestration without holding the lock.
    let (binary_url, binary_hash, wave_delay_secs, waves_snapshot) = {
        let guard = session_lock.read().await;
        if let Some(ref s) = *guard {
            (s.binary_url.clone(), s.binary_hash.clone(), s.wave_delay_secs, s.waves.clone())
        } else {
            return; // Session was cleared externally.
        }
    };

    // Collect all target pod IDs for OTA sentinel.
    let all_pod_ids: Vec<String> = waves_snapshot.iter().flat_map(|w| w.pods.clone()).collect();
    let all_pod_ips = resolve_pod_ips(&state, &all_pod_ids).await;

    // Set OTA sentinel + kill switch on all target pods.
    crate::ota_pipeline::set_ota_sentinel(&state.http_client, &all_pod_ips).await;
    crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, true).await;

    let wave_count = waves_snapshot.len();
    let mut deploy_halted = false;

    for wave_idx in 0..wave_count {
        let wave_num = waves_snapshot[wave_idx].wave_number;
        let is_canary = wave_num == 1;
        let pod_ids_in_wave = waves_snapshot[wave_idx].pods.clone();

        // Mark wave as Running.
        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                s.current_wave = wave_num;
                s.waves[wave_idx].status = WaveDeployStatus::Running;
                s.waves[wave_idx].started_at = Some(now_ist_rfc3339());
            }
        } // guard dropped

        let mut wave_failed = false;

        for pod_id in &pod_ids_in_wave {
            // Resolve pod IP — if pod not connected, mark as skipped.
            let pod_ip = {
                let pods = state.pods.read().await;
                pods.get(pod_id).map(|p| p.ip_address.clone())
            };

            let pod_ip = match pod_ip {
                Some(ip) => ip,
                None => {
                    // Pod not connected — skip.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "skipped".to_string(),
                        detail: Some("pod not connected".to_string()),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;
                    continue;
                }
            };

            // Billing drain check — must not hold lock across .await.
            let has_active_session = {
                let timers = state.billing.active_timers.read().await;
                timers.contains_key(pod_id)
            };

            if has_active_session {
                // Defer this pod — it will be triggered by check_and_trigger_pending_deploy
                // when the billing session ends.
                {
                    let mut deploy_states = state.pod_deploy_states.write().await;
                    deploy_states.insert(pod_id.clone(), DeployState::WaitingSession);
                }
                {
                    let mut pending = state.pending_deploys.write().await;
                    pending.insert(pod_id.clone(), binary_url.clone());
                }
                let result = PodDeployResult {
                    pod_id: pod_id.clone(),
                    status: "waiting_session".to_string(),
                    detail: Some("active billing session — deferred".to_string()),
                };
                append_pod_result(&session_lock, wave_idx, result).await;
                continue;
            }

            // Deploy this pod (infallible — returns ()).
            crate::deploy::deploy_pod(
                state.clone(),
                pod_id.clone(),
                pod_ip.clone(),
                binary_url.clone(),
            )
            .await;

            // Read result immediately after deploy_pod returns.
            let deploy_state = {
                let states = state.pod_deploy_states.read().await;
                states.get(pod_id).cloned()
            };

            let succeeded = matches!(deploy_state, Some(DeployState::Complete));
            let failure_reason = if let Some(DeployState::Failed { ref reason }) = deploy_state {
                Some(reason.clone())
            } else if !succeeded {
                Some("unknown deploy state".to_string())
            } else {
                None
            };

            if succeeded {
                let result = PodDeployResult {
                    pod_id: pod_id.clone(),
                    status: "complete".to_string(),
                    detail: None,
                };
                append_pod_result(&session_lock, wave_idx, result).await;
            } else {
                let reason = failure_reason.unwrap_or_else(|| "deploy failed".to_string());

                // Trigger rollback for this pod.
                let rollback_pod_ips = vec![(pod_id.clone(), pod_ip.clone())];
                let sentry_key = state.config.pods.sentry_service_key.as_deref();
                crate::ota_pipeline::rollback_wave(&state.http_client, &rollback_pod_ips, sentry_key).await;

                let rb_outcome = "success"; // rollback_wave is infallible from our perspective.

                let rollback_event = RollbackEvent {
                    wave: wave_num,
                    pod_id: pod_id.clone(),
                    reason: reason.clone(),
                    rolled_back_at: now_ist_rfc3339(),
                    outcome: rb_outcome.to_string(),
                };
                {
                    let mut guard = session_lock.write().await;
                    if let Some(ref mut s) = *guard {
                        s.rollback_events.push(rollback_event);
                    }
                } // guard dropped

                if is_canary {
                    // Canary failure — halt entire deploy.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "rolled_back".to_string(),
                        detail: Some(reason.clone()),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;

                    {
                        let mut guard = session_lock.write().await;
                        if let Some(ref mut s) = *guard {
                            s.waves[wave_idx].status = WaveDeployStatus::Failed;
                            s.waves[wave_idx].completed_at = Some(now_ist_rfc3339());
                            s.overall_status = DeployOverallStatus::Failed;
                        }
                    }

                    // Cleanup sentinels before returning.
                    crate::ota_pipeline::clear_ota_sentinel(&state.http_client, &all_pod_ips).await;
                    crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, false).await;
                    deploy_halted = true;
                    break; // stop processing pods in this wave
                } else {
                    // Non-canary: record rolled_back result, continue to next pod.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "rolled_back".to_string(),
                        detail: Some(reason),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;
                    wave_failed = true;
                }
            }
        }

        if deploy_halted {
            break;
        }

        // Mark wave complete.
        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                let status = if wave_failed { WaveDeployStatus::Failed } else { WaveDeployStatus::Passed };
                s.waves[wave_idx].status = status;
                s.waves[wave_idx].completed_at = Some(now_ist_rfc3339());
            }
        }

        // Inter-wave delay (skip after last wave).
        if wave_idx + 1 < wave_count {
            tokio::time::sleep(tokio::time::Duration::from_secs(wave_delay_secs)).await;
        }
    }

    if !deploy_halted {
        // All waves processed — cleanup and mark complete.
        crate::ota_pipeline::clear_ota_sentinel(&state.http_client, &all_pod_ips).await;
        crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, false).await;

        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                s.overall_status = DeployOverallStatus::Completed;
            }
        }
    }

    // Log final status.
    let final_status = {
        let guard = session_lock.read().await;
        guard.as_ref().map(|s| format!("{:?}", s.overall_status))
    };
    tracing::info!("Fleet deploy finished: {:?}", final_status);
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Resolve (pod_id, pod_ip) pairs for a list of pod IDs from AppState.
async fn resolve_pod_ips(state: &Arc<AppState>, pod_ids: &[String]) -> Vec<(String, String)> {
    let pods = state.pods.read().await;
    pod_ids
        .iter()
        .filter_map(|id| pods.get(id).map(|p| (id.clone(), p.ip_address.clone())))
        .collect()
}

/// Append a `PodDeployResult` to a wave in the session without holding the lock across await.
async fn append_pod_result(
    session_lock: &Arc<RwLock<Option<FleetDeploySession>>>,
    wave_idx: usize,
    result: PodDeployResult,
) {
    let mut guard = session_lock.write().await;
    if let Some(ref mut s) = *guard {
        if wave_idx < s.waves.len() {
            s.waves[wave_idx].pod_results.push(result);
        }
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
