//! Fleet Healer — Repair subsystem: confidence gating, dispatch, verification,
//! canary rollout, and pod isolation.
//!
//! Contains FH-04 (Confidence Gate), FH-05 (Repair Dispatch), FH-06 (Post-Fix
//! Verification), FH-07 (Canary Rollout), and FH-08 (Pod Isolation).

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use super::diagnosis::SshCommandResult;
use super::{FleetHealerError, LOG_TARGET, SshDiagnosticRunner};

// ─── FH-04 / FH-05: Repair Confidence Gate & Dispatch ──────────────────────

/// Confidence threshold for autonomous fix dispatch.
const CONFIDENCE_GATE: f64 = 0.8;

/// A repair action to be dispatched to a pod via SSH.
#[derive(Debug, Clone, Serialize)]
pub struct RepairAction {
    /// Unique ID for audit trail.
    pub action_id: String,
    /// Pod target.
    pub pod_id: String,
    /// SSH command to execute.
    pub ssh_command: String,
    /// Description for humans and audit.
    pub description: String,
    /// Fix type from fleet KB.
    pub fix_type: String,
    /// Confidence score from fleet KB.
    pub confidence: f64,
}

/// Result of a repair attempt.
#[derive(Debug, Clone, Serialize)]
pub struct RepairResult {
    pub action_id: String,
    pub pod_id: String,
    pub success: bool,
    pub ssh_result: Option<SshCommandResult>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Dispatches repairs to pods via SSH, gated by confidence and fix_type.
pub struct RepairDispatcher;

impl RepairDispatcher {
    /// Check whether a repair should be autonomously dispatched.
    /// FH-04: Only dispatch if confidence >= 0.8 AND fix_type is Deterministic or Config.
    pub fn should_dispatch(confidence: f64, fix_type: &str) -> bool {
        if confidence < CONFIDENCE_GATE {
            tracing::info!(
                target: LOG_TARGET,
                confidence = confidence,
                fix_type = fix_type,
                "Repair blocked: confidence {:.2} < gate {:.2}",
                confidence,
                CONFIDENCE_GATE
            );
            return false;
        }

        match fix_type {
            "Deterministic" | "Config" => true,
            _ => {
                tracing::info!(
                    target: LOG_TARGET,
                    fix_type = fix_type,
                    "Repair blocked: fix_type '{}' not eligible for autonomous dispatch",
                    fix_type
                );
                false
            }
        }
    }

    /// Dispatch a repair action to a pod via SSH.
    /// FH-05: Apply deterministic fixes from fleet KB remotely.
    pub async fn dispatch(
        pod_number: u32,
        action: &RepairAction,
    ) -> RepairResult {
        let action_id = action.action_id.clone();
        let pod_id = action.pod_id.clone();

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            description = %action.description,
            "Dispatching repair via SSH"
        );

        match SshDiagnosticRunner::run_command(pod_number, &action.ssh_command).await {
            Ok(result) => {
                let success = result.exit_code == Some(0);
                if success {
                    tracing::info!(
                        target: LOG_TARGET,
                        action_id = %action_id,
                        pod = pod_number,
                        "Repair executed successfully"
                    );
                } else {
                    tracing::warn!(
                        target: LOG_TARGET,
                        action_id = %action_id,
                        pod = pod_number,
                        exit_code = ?result.exit_code,
                        stderr = %result.stderr,
                        "Repair command returned non-zero exit code"
                    );
                }
                RepairResult {
                    action_id,
                    pod_id,
                    success,
                    ssh_result: Some(result),
                    error: None,
                    timestamp: Utc::now(),
                }
            }
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    action_id = %action_id,
                    pod = pod_number,
                    error = %e,
                    "Repair dispatch failed"
                );
                RepairResult {
                    action_id,
                    pod_id,
                    success: false,
                    ssh_result: None,
                    error: Some(e.to_string()),
                    timestamp: Utc::now(),
                }
            }
        }
    }
}

// ─── FH-06: Post-Fix Behavioral Verification ──────────────────────────────

/// Verifies that a fix actually worked by polling the pod's health and debug
/// endpoints for build_id match and edge_process_count > 0.
pub struct PostFixVerifier;

impl PostFixVerifier {
    /// Poll a pod's /health endpoint for the expected build_id, and its /debug
    /// endpoint for edge_process_count > 0.
    ///
    /// Retries up to `max_retries` times with `interval` between each attempt.
    pub async fn verify(
        http_client: &reqwest::Client,
        pod_ip: &str,
        expected_build_id: Option<&str>,
        max_retries: u32,
        interval: Duration,
    ) -> PostFixVerification {
        let health_url = format!("http://{}:8090/health", pod_ip);
        let debug_url = format!("http://{}:18924/debug", pod_ip);

        for attempt in 1..=max_retries {
            tracing::debug!(
                target: LOG_TARGET,
                pod_ip = pod_ip,
                attempt = attempt,
                "Post-fix verification attempt"
            );

            // Check /health for build_id
            let build_id_ok = match http_client
                .get(&health_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(body) = resp.json::<Value>().await {
                        match expected_build_id {
                            Some(expected) => {
                                body.get("build_id")
                                    .and_then(|v| v.as_str())
                                    .map(|id| id == expected)
                                    .unwrap_or(false)
                            }
                            None => true, // No expected build_id — skip check
                        }
                    } else {
                        false
                    }
                }
                Err(_) => false,
            };

            // Check /debug for edge_process_count > 0
            let edge_ok = match http_client
                .get(&debug_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(body) = resp.json::<Value>().await {
                        body.get("edge_process_count")
                            .and_then(|v| v.as_u64())
                            .map(|c| c > 0)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                Err(_) => false,
            };

            if build_id_ok && edge_ok {
                tracing::info!(
                    target: LOG_TARGET,
                    pod_ip = pod_ip,
                    attempt = attempt,
                    "Post-fix verification PASSED"
                );
                return PostFixVerification {
                    passed: true,
                    build_id_match: build_id_ok,
                    edge_process_ok: edge_ok,
                    attempts: attempt,
                    verified_at: Utc::now(),
                };
            }

            if attempt < max_retries {
                tokio::time::sleep(interval).await;
            }
        }

        tracing::warn!(
            target: LOG_TARGET,
            pod_ip = pod_ip,
            max_retries = max_retries,
            "Post-fix verification FAILED after all retries"
        );
        PostFixVerification {
            passed: false,
            build_id_match: false,
            edge_process_ok: false,
            attempts: max_retries,
            verified_at: Utc::now(),
        }
    }
}

/// Result of post-fix verification.
#[derive(Debug, Clone, Serialize)]
pub struct PostFixVerification {
    pub passed: bool,
    pub build_id_match: bool,
    pub edge_process_ok: bool,
    pub attempts: u32,
    pub verified_at: DateTime<Utc>,
}

// ─── FH-07: Canary Rollout ─────────────────────────────────────────────────

/// Canary rollout strategy: Pod 8 first, verify, then gradual.
///
/// Stages:
/// 1. Pod 8 (canary)
/// 2. Pods 1, 2, 3 (first wave)
/// 3. Pods 4, 5, 6, 7 (remaining)
pub struct CanaryRollout;

impl CanaryRollout {
    /// Return rollout waves in order. Each wave is a Vec of pod numbers.
    /// Excludes pods not in `target_pods`.
    pub fn waves(target_pods: &[u32]) -> Vec<Vec<u32>> {
        let canary: Vec<u32> = vec![8].into_iter().filter(|p| target_pods.contains(p)).collect();
        let wave1: Vec<u32> = vec![1, 2, 3].into_iter().filter(|p| target_pods.contains(p)).collect();
        let wave2: Vec<u32> = vec![4, 5, 6, 7].into_iter().filter(|p| target_pods.contains(p)).collect();

        let mut waves = Vec::new();
        if !canary.is_empty() {
            waves.push(canary);
        }
        if !wave1.is_empty() {
            waves.push(wave1);
        }
        if !wave2.is_empty() {
            waves.push(wave2);
        }
        waves
    }
}

// ─── FH-08: Pod Isolation Before Risky Repair ──────────────────────────────

/// Writes/clears MAINTENANCE_MODE sentinel via SSH before/after risky repairs.
pub struct PodIsolation;

impl PodIsolation {
    /// Write MAINTENANCE_MODE sentinel on a pod before a risky repair.
    pub async fn isolate(pod_number: u32) -> Result<(), FleetHealerError> {
        tracing::info!(
            target: LOG_TARGET,
            pod = pod_number,
            "Isolating pod: writing MAINTENANCE_MODE sentinel"
        );
        let result = SshDiagnosticRunner::run_command(
            pod_number,
            r#"echo fleet_healer_isolation > C:\RacingPoint\MAINTENANCE_MODE"#,
        )
        .await?;

        if result.exit_code != Some(0) {
            return Err(FleetHealerError::IsolationFailed {
                pod_id: format!("pod_{}", pod_number),
                error: format!("exit_code={:?}, stderr={}", result.exit_code, result.stderr),
            });
        }
        Ok(())
    }

    /// Clear MAINTENANCE_MODE sentinel on a pod after successful repair verification.
    pub async fn clear_isolation(pod_number: u32) -> Result<(), FleetHealerError> {
        tracing::info!(
            target: LOG_TARGET,
            pod = pod_number,
            "Clearing pod isolation: removing MAINTENANCE_MODE sentinel"
        );
        let result = SshDiagnosticRunner::run_command(
            pod_number,
            r#"del C:\RacingPoint\MAINTENANCE_MODE 2>nul & echo ok"#,
        )
        .await?;

        if result.exit_code != Some(0) {
            tracing::warn!(
                target: LOG_TARGET,
                pod = pod_number,
                stderr = %result.stderr,
                "Failed to clear MAINTENANCE_MODE (may not have existed)"
            );
        }
        Ok(())
    }
}
