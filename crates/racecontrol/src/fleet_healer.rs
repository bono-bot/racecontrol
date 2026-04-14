//! Fleet Healer — Layer 2: SSH-based remote pod healing via Tailscale.
//!
//! Phase 270 — v31.0. The server SSHes into dark/broken pods, runs diagnostics,
//! fingerprints symptoms, detects fleet-wide patterns, and applies deterministic
//! fixes autonomously — with billing safety, canary rollout, and full audit trail.
//!
//! Requirements: FH-01 through FH-12.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::state::AppState;

const LOG_TARGET: &str = "fleet-healer";

// ─── Submodules ────────────────────────────────────────────────────────────

#[path = "fleet_healer_diagnosis.rs"]
pub mod diagnosis;

#[path = "fleet_healer_repair.rs"]
pub mod repair;

#[path = "fleet_healer_audit.rs"]
pub mod audit;

// ─── Re-exports (preserve external API) ────────────────────────────────────

pub use audit::{AuditTrail, BillingSafetyCheck, SurvivalReport, SurvivalReportIngester};
pub use diagnosis::{
    DiagnosticFingerprinter, FleetPattern, FleetPatternDetector, SshCommandResult,
    SshDiagnosticRunner, Symptom,
};
pub use repair::{
    CanaryRollout, PodIsolation, PostFixVerification, PostFixVerifier, RepairAction,
    RepairDispatcher, RepairResult,
};

// ─── Pod Tailscale IP map ───────────────────────────────────────────────────

/// Map pod number (1-8) to its Tailscale IP for SSH access.
fn tailscale_ip(pod_number: u32) -> Option<&'static str> {
    match pod_number {
        1 => Some("100.92.122.89"),
        2 => Some("100.105.93.108"),
        3 => Some("100.69.231.26"),
        4 => Some("100.75.45.10"),
        5 => Some("100.110.133.87"),
        6 => Some("100.127.149.17"),
        7 => Some("100.82.196.28"),
        8 => Some("100.98.67.67"),
        _ => None,
    }
}

/// SSH user for pod connections.
const SSH_USER: &str = "User";
/// SSH connection timeout in seconds.
const SSH_TIMEOUT_SECS: u64 = 10;
/// SSH command execution timeout in seconds.
const SSH_CMD_TIMEOUT_SECS: u64 = 30;

// ─── Error Type ────────────────────────────────────────────────────────────

/// Errors specific to the fleet healer module.
#[derive(Debug, thiserror::Error)]
pub enum FleetHealerError {
    #[error("Unknown pod number: {0}")]
    UnknownPod(u32),

    #[error("SSH timeout on {pod_id} after {timeout_secs}s")]
    SshTimeout { pod_id: String, timeout_secs: u64 },

    #[error("SSH execution failed on {pod_id}: {error}")]
    SshExecFailed { pod_id: String, error: String },

    #[error("Pod isolation failed on {pod_id}: {error}")]
    IsolationFailed { pod_id: String, error: String },

    #[error("Billing session active on {pod_id} — repair not permitted")]
    BillingActive { pod_id: String },

    #[error("SSH command blocked on {pod_id}: {reason}")]
    CommandBlocked { pod_id: String, reason: String },
}

// ─── FH-12: Survival Report Endpoint ───────────────────────────────────────

/// Axum handler for POST /api/v1/pods/{id}/survival-report.
/// Watchdog processes on pods POST their survival reports here.
pub async fn survival_report_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(pod_id): axum::extract::Path<String>,
    axum::Json(report): axum::Json<SurvivalReport>,
) -> axum::Json<Value> {
    let normalized = rc_common::pod_id::normalize_pod_id(&pod_id)
        .unwrap_or_else(|_| pod_id.clone());

    tracing::info!(
        target: LOG_TARGET,
        pod_id = %normalized,
        source = %report.source_layer,
        status = %report.status,
        "Received survival report"
    );

    // Log to audit trail
    AuditTrail::log_repair(
        &state.db,
        &format!("sr-{}", Utc::now().timestamp_millis()),
        &normalized,
        "survival_report_ingested",
        report.status == "healthy" || report.status == "ok",
        Some(&serde_json::to_string(&report).unwrap_or_default()),
    )
    .await;

    axum::Json(json!({
        "status": "accepted",
        "pod_id": normalized,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Build the sub-router for fleet healer endpoints.
pub fn fleet_healer_routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new().route(
        "/pods/{pod_id}/survival-report",
        axum::routing::post(survival_report_handler),
    )
}

// ─── Orchestrator (ties FH-01 through FH-12 together) ──────────────────────

/// High-level orchestrator that coordinates all fleet healer subsystems.
/// Typically invoked from a background task or API trigger.
pub struct FleetHealerOrchestrator;

impl FleetHealerOrchestrator {
    /// Heal a single pod end-to-end:
    /// 1. Check billing safety (FH-11)
    /// 2. Run SSH diagnostics (FH-01)
    /// 3. Fingerprint symptoms (FH-02)
    /// 4. Look up fixes in fleet KB
    /// 5. Gate on confidence (FH-04)
    /// 6. Isolate pod (FH-08) if risky
    /// 7. Dispatch fix (FH-05)
    /// 8. Verify (FH-06)
    /// 9. Clear isolation
    /// 10. Log everything (FH-09)
    pub async fn heal_pod(
        state: &Arc<AppState>,
        pod_number: u32,
    ) -> Result<HealPodOutcome, FleetHealerError> {
        let pod_id = format!("pod_{}", pod_number);
        let action_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            "Starting fleet heal for pod"
        );

        // FH-11: Billing safety check
        if !BillingSafetyCheck::is_safe_to_repair(state, &pod_id).await {
            AuditTrail::log_repair(
                &state.db,
                &action_id,
                &pod_id,
                "heal_blocked_billing",
                false,
                Some("Active billing session on pod"),
            )
            .await;
            return Ok(HealPodOutcome {
                pod_id,
                action_id,
                stage: "billing_check".to_string(),
                success: false,
                blocked_reason: Some("Active billing session".to_string()),
                symptoms: Vec::new(),
                repair_applied: false,
                verification_passed: false,
            });
        }

        // FH-01: SSH Diagnostics
        let diag_results = SshDiagnosticRunner::run_diagnostics(pod_number).await?;

        // FH-09: Log every diagnostic command
        for result in &diag_results {
            AuditTrail::log_ssh_command(&state.db, &action_id, result, "diagnostic").await;
        }

        // FH-02: Fingerprint
        let symptoms = DiagnosticFingerprinter::fingerprint(&diag_results);

        if symptoms.is_empty() {
            tracing::info!(
                target: LOG_TARGET,
                action_id = %action_id,
                pod = pod_number,
                "No symptoms detected — pod appears healthy via SSH"
            );
            AuditTrail::log_repair(
                &state.db,
                &action_id,
                &pod_id,
                "heal_no_symptoms",
                true,
                None,
            )
            .await;
            return Ok(HealPodOutcome {
                pod_id,
                action_id,
                stage: "fingerprint".to_string(),
                success: true,
                blocked_reason: None,
                symptoms: Vec::new(),
                repair_applied: false,
                verification_passed: true,
            });
        }

        tracing::info!(
            target: LOG_TARGET,
            action_id = %action_id,
            pod = pod_number,
            symptom_count = symptoms.len(),
            "Symptoms detected: {:?}",
            symptoms
        );

        // FH-04/FH-05: Look up KB solutions and attempt repair for each symptom
        let mut repair_applied = false;
        let mut verification_passed = false;

        for symptom in &symptoms {
            let problem_key = format!("{}:{}", symptom.category, symptom.detail);

            // Query fleet KB for matching solutions
            let kb_solution = crate::fleet_kb::get_solution_by_hash(
                &state.db,
                &problem_key,
            )
            .await
            .ok()
            .flatten();

            if let Some(solution) = kb_solution {
                let fix_type_str = serde_json::to_string(&solution.fix_type)
                    .unwrap_or_else(|_| "\"Unknown\"".to_string())
                    .trim_matches('"')
                    .to_string();

                // FH-04: Confidence gate
                if RepairDispatcher::should_dispatch(solution.confidence, &fix_type_str) {
                    let repair = RepairAction {
                        action_id: action_id.clone(),
                        pod_id: pod_id.clone(),
                        ssh_command: solution.fix_action.to_string(),
                        description: format!(
                            "KB fix for {}: {}",
                            problem_key, solution.root_cause
                        ),
                        fix_type: fix_type_str.clone(),
                        confidence: solution.confidence,
                    };

                    // FH-08: Isolate if not deterministic
                    let needs_isolation = fix_type_str != "Deterministic";
                    if needs_isolation {
                        if let Err(e) = PodIsolation::isolate(pod_number).await {
                            tracing::warn!(
                                target: LOG_TARGET,
                                action_id = %action_id,
                                pod = pod_number,
                                error = %e,
                                "Failed to isolate pod — skipping repair"
                            );
                            continue;
                        }
                    }

                    // FH-05: Dispatch
                    let result = RepairDispatcher::dispatch(pod_number, &repair).await;

                    // FH-09: Log repair
                    if let Some(ssh_result) = &result.ssh_result {
                        AuditTrail::log_ssh_command(
                            &state.db,
                            &action_id,
                            ssh_result,
                            "repair",
                        )
                        .await;
                    }

                    repair_applied = true;

                    // FH-06: Post-fix verification
                    if result.success {
                        let ip = tailscale_ip(pod_number).unwrap_or("127.0.0.1");
                        let verification = PostFixVerifier::verify(
                            &state.http_client,
                            ip,
                            None,
                            3,
                            Duration::from_secs(10),
                        )
                        .await;

                        verification_passed = verification.passed;

                        AuditTrail::log_repair(
                            &state.db,
                            &action_id,
                            &pod_id,
                            "post_fix_verification",
                            verification.passed,
                            Some(
                                &serde_json::to_string(&verification).unwrap_or_default(),
                            ),
                        )
                        .await;
                    }

                    // FH-08: Clear isolation after verification
                    if needs_isolation {
                        let _ = PodIsolation::clear_isolation(pod_number).await;
                    }

                    // Break after first successful repair — don't stack fixes
                    if verification_passed {
                        break;
                    }
                }
            }
        }

        let outcome = HealPodOutcome {
            pod_id: pod_id.clone(),
            action_id: action_id.clone(),
            stage: "complete".to_string(),
            success: verification_passed || symptoms.is_empty(),
            blocked_reason: None,
            symptoms: symptoms.clone(),
            repair_applied,
            verification_passed,
        };

        AuditTrail::log_repair(
            &state.db,
            &action_id,
            &pod_id,
            "heal_complete",
            outcome.success,
            Some(&serde_json::to_string(&outcome).unwrap_or_default()),
        )
        .await;

        Ok(outcome)
    }
}

/// Outcome of a pod heal attempt.
#[derive(Debug, Clone, Serialize)]
pub struct HealPodOutcome {
    pub pod_id: String,
    pub action_id: String,
    pub stage: String,
    pub success: bool,
    pub blocked_reason: Option<String>,
    pub symptoms: Vec<Symptom>,
    pub repair_applied: bool,
    pub verification_passed: bool,
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "fleet_healer_tests.rs"]
mod tests;
