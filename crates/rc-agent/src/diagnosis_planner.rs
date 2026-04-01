//! Diagnosis Plan Manager — breaks diagnosis into tracked atomic steps with dependency awareness.
//!
//! Generates structured plans for Tier 3 (3-5 steps) and Tier 4 (8-12 steps).
//! Persists to the same SQLite DB used by KnowledgeBase.
//! All operations are local compute ($0 cost).

use chrono::Utc;
use serde_json::json;

use crate::diagnostic_engine::DiagnosticEvent;
use crate::knowledge_base::KnowledgeBase;
use rc_common::mesh_types::{
    CgpGateResult, DiagnosisPlan, DiagnosisPlanStep, DiagnosisTier, PlanStepStatus,
};

const LOG_TARGET: &str = "diagnosis-planner";
const MAX_STEPS: usize = 15;

/// Diagnosis Plan Manager — creates and tracks structured diagnosis plans.
pub struct DiagnosisPlanner;

impl DiagnosisPlanner {
    /// Create a diagnosis plan from CGP Phase A output.
    /// Tier 3: 3-5 steps. Tier 4: 8-12 steps.
    pub fn create_plan(
        event: &DiagnosticEvent,
        cgp_phase_a: &[CgpGateResult],
        tier: DiagnosisTier,
    ) -> DiagnosisPlan {
        let problem_key = crate::knowledge_base::normalize_problem_key(&event.trigger);
        let plan_id = uuid::Uuid::new_v4().to_string();

        let steps = match tier {
            DiagnosisTier::SingleModel => Self::build_tier3_steps(event, cgp_phase_a),
            DiagnosisTier::MultiModel => Self::build_tier4_steps(event, cgp_phase_a),
            _ => vec![Self::make_step(1, "Execute deterministic fix", &[])],
        };

        let plan = DiagnosisPlan {
            plan_id,
            incident_id: format!("diag-{}", event.timestamp),
            problem_key,
            steps,
            created_at: Utc::now(),
            completed_at: None,
            tier,
        };

        tracing::info!(
            target: LOG_TARGET,
            plan_id = %plan.plan_id,
            steps = plan.steps.len(),
            tier = ?tier,
            "Created diagnosis plan with {} steps",
            plan.steps.len()
        );

        plan
    }

    /// Mark a step as InProgress.
    pub fn start_step(plan: &mut DiagnosisPlan, step_id: u8) {
        // First, check dependencies without holding a mutable borrow
        let blocked = {
            let step = plan.steps.iter().find(|s| s.step_id == step_id);
            match step {
                Some(step) => step.depends_on.iter().any(|dep_id| {
                    plan.steps.iter()
                        .find(|s| s.step_id == *dep_id)
                        .map_or(true, |dep| dep.status != PlanStepStatus::Done)
                }),
                None => return,
            }
        };

        // Now mutate
        if let Some(step) = plan.steps.iter_mut().find(|s| s.step_id == step_id) {
            if blocked {
                tracing::warn!(
                    target: LOG_TARGET,
                    step_id,
                    "Cannot start step {} — dependencies not met",
                    step_id
                );
                step.status = PlanStepStatus::Blocked;
                return;
            }
            step.status = PlanStepStatus::InProgress;
            step.started_at = Some(Utc::now());
        }
    }

    /// Mark a step as Done with output.
    pub fn complete_step(plan: &mut DiagnosisPlan, step_id: u8, output: serde_json::Value) {
        if let Some(step) = plan.steps.iter_mut().find(|s| s.step_id == step_id) {
            step.status = PlanStepStatus::Done;
            step.output = Some(output);
            step.completed_at = Some(Utc::now());
        }

        // Check if all steps are done
        if Self::is_complete(plan) {
            plan.completed_at = Some(Utc::now());
            tracing::info!(
                target: LOG_TARGET,
                plan_id = %plan.plan_id,
                "Diagnosis plan COMPLETE"
            );
        }
    }

    /// Mark a step as Blocked.
    pub fn block_step(plan: &mut DiagnosisPlan, step_id: u8, reason: &str) {
        if let Some(step) = plan.steps.iter_mut().find(|s| s.step_id == step_id) {
            step.status = PlanStepStatus::Blocked;
            step.output = Some(json!({"blocked_reason": reason}));
        }
    }

    /// Check if all non-skipped steps are Done.
    pub fn is_complete(plan: &DiagnosisPlan) -> bool {
        plan.steps.iter().all(|s| {
            matches!(s.status, PlanStepStatus::Done | PlanStepStatus::Skipped)
        })
    }

    /// Persist plan to SQLite via typed KB method (MMA-F2: no raw SQL).
    pub fn save(plan: &DiagnosisPlan, kb: &KnowledgeBase) {
        let steps_json = serde_json::to_string(&plan.steps).unwrap_or_default();
        let tier_str = serde_json::to_string(&plan.tier).unwrap_or_default();
        let completed_str = plan.completed_at.map(|t| t.to_rfc3339());

        if let Err(e) = kb.save_diagnosis_plan(
            &plan.plan_id,
            &plan.incident_id,
            &plan.problem_key,
            &steps_json,
            &plan.created_at.to_rfc3339(),
            completed_str.as_deref(),
            &tier_str,
        ) {
            tracing::warn!(target: LOG_TARGET, error = %e, "Failed to save diagnosis plan");
        }
    }

    /// Save a full audit trail to SQLite via typed KB method (MMA-F2: no raw SQL).
    pub fn save_audit(audit: &rc_common::mesh_types::StructuredDiagnosisAudit, kb: &KnowledgeBase) {
        let audit_json = serde_json::to_string(audit).unwrap_or_default();

        if let Err(e) = kb.save_diagnosis_audit(
            &audit.incident_id,
            &audit_json,
            &audit.timestamp.to_rfc3339(),
        ) {
            tracing::warn!(target: LOG_TARGET, error = %e, "Failed to save diagnosis audit");
        }
    }

    // ─── Plan Builders ──────────────────────────────────────────────────────

    /// Tier 3 plan: 3-5 steps (gather context → model call → verify → store).
    fn build_tier3_steps(event: &DiagnosticEvent, cgp_phase_a: &[CgpGateResult]) -> Vec<DiagnosisPlanStep> {
        let mut steps = Vec::new();

        steps.push(Self::make_step(1, "Gather diagnostic context (pod state, logs, trigger details)", &[]));
        steps.push(Self::make_step(2, "Call single AI model for diagnosis", &[1]));
        steps.push(Self::make_step(3, "Apply recommended fix", &[2]));
        steps.push(Self::make_step(4, "Verify fix resolved the anomaly", &[3]));
        steps.push(Self::make_step(5, "Store solution in local KB", &[4]));

        // Annotate step 2 with G7 tool selection
        if let Some(g7) = cgp_phase_a.iter().find(|g| g.gate == rc_common::mesh_types::CgpGateId::G7ToolVerification) {
            if let Some(step2) = steps.iter_mut().find(|s| s.step_id == 2) {
                let tool = g7.evidence["tool"].as_str().unwrap_or("Qwen3");
                step2.description = format!("Call {} for diagnosis", tool);
            }
        }

        let _ = event; // Used for context in future enhancements
        steps.truncate(MAX_STEPS);
        steps
    }

    /// Tier 4 plan: 8-12 steps (gather → DIAGNOSE → PLAN → EXECUTE → VERIFY → store → gossip).
    fn build_tier4_steps(event: &DiagnosticEvent, cgp_phase_a: &[CgpGateResult]) -> Vec<DiagnosisPlanStep> {
        let mut steps = Vec::new();

        // Pre-MMA
        steps.push(Self::make_step(1, "Gather diagnostic context (pod state, logs, trigger details)", &[]));
        steps.push(Self::make_step(2, "Evaluate competing hypotheses from G5", &[1]));

        // MMA Step 1: DIAGNOSE
        steps.push(Self::make_step(3, "MMA DIAGNOSE: 5-model parallel root cause analysis", &[2]));
        steps.push(Self::make_step(4, "Build consensus from DIAGNOSE results (3/5 majority)", &[3]));

        // MMA Step 2: PLAN
        steps.push(Self::make_step(5, "MMA PLAN: 5-model fix plan design", &[4]));
        steps.push(Self::make_step(6, "Select best fix plan (smallest reversible change)", &[5]));

        // MMA Step 3: EXECUTE
        steps.push(Self::make_step(7, "MMA EXECUTE: Apply selected fix", &[6]));

        // MMA Step 4: VERIFY
        steps.push(Self::make_step(8, "Deterministic verification (verify_fix check)", &[7]));
        steps.push(Self::make_step(9, "MMA VERIFY: 3-model adversarial review", &[8]));

        // Post-MMA
        steps.push(Self::make_step(10, "Store solution in local KB with full provenance", &[9]));
        steps.push(Self::make_step(11, "Gossip solution to server for fleet learning", &[10]));

        // Annotate step 3 with G7 domain roster
        if let Some(g7) = cgp_phase_a.iter().find(|g| g.gate == rc_common::mesh_types::CgpGateId::G7ToolVerification) {
            if let Some(step3) = steps.iter_mut().find(|s| s.step_id == 3) {
                let tool = g7.evidence["tool"].as_str().unwrap_or("default roster");
                step3.description = format!("MMA DIAGNOSE: {} parallel root cause analysis", tool);
            }
        }

        let _ = event; // Used for context
        steps.truncate(MAX_STEPS);
        steps
    }

    /// Helper to create a plan step.
    fn make_step(step_id: u8, description: &str, depends_on: &[u8]) -> DiagnosisPlanStep {
        DiagnosisPlanStep {
            step_id,
            description: description.to_string(),
            status: PlanStepStatus::Todo,
            depends_on: depends_on.to_vec(),
            output: None,
            started_at: None,
            completed_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic_engine::DiagnosticTrigger;
    use crate::failure_monitor::FailureMonitorState;

    fn make_test_event(trigger: DiagnosticTrigger) -> DiagnosticEvent {
        DiagnosticEvent {
            trigger,
            pod_state: FailureMonitorState::default(),
            timestamp: "2026-04-01T10:00:00+05:30".to_string(),
            build_id: "test1234",
        }
    }

    #[test]
    fn tier3_plan_has_3_to_5_steps() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);
        let plan = DiagnosisPlanner::create_plan(&event, &[], DiagnosisTier::SingleModel);
        assert!(plan.steps.len() >= 3 && plan.steps.len() <= 5,
            "Tier 3 plan should have 3-5 steps, got {}", plan.steps.len());
    }

    #[test]
    fn tier4_plan_has_8_to_12_steps() {
        let event = make_test_event(DiagnosticTrigger::WsDisconnect { disconnected_secs: 60 });
        let plan = DiagnosisPlanner::create_plan(&event, &[], DiagnosisTier::MultiModel);
        assert!(plan.steps.len() >= 8 && plan.steps.len() <= 12,
            "Tier 4 plan should have 8-12 steps, got {}", plan.steps.len());
    }

    #[test]
    fn step_completion_tracking() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);
        let mut plan = DiagnosisPlanner::create_plan(&event, &[], DiagnosisTier::SingleModel);

        assert!(!DiagnosisPlanner::is_complete(&plan));

        // Complete all steps
        for i in 1..=plan.steps.len() as u8 {
            DiagnosisPlanner::start_step(&mut plan, i);
            DiagnosisPlanner::complete_step(&mut plan, i, json!({"result": "ok"}));
        }

        assert!(DiagnosisPlanner::is_complete(&plan));
        assert!(plan.completed_at.is_some());
    }

    #[test]
    fn dependency_blocking() {
        let event = make_test_event(DiagnosticTrigger::GameLaunchFail);
        let mut plan = DiagnosisPlanner::create_plan(&event, &[], DiagnosisTier::SingleModel);

        // Try to start step 2 before step 1 is done — should be blocked
        DiagnosisPlanner::start_step(&mut plan, 2);
        assert_eq!(plan.steps[1].status, PlanStepStatus::Blocked);

        // Now complete step 1 and retry step 2
        DiagnosisPlanner::start_step(&mut plan, 1);
        DiagnosisPlanner::complete_step(&mut plan, 1, json!({"result": "gathered context"}));
        // Reset step 2 status for retry
        plan.steps[1].status = PlanStepStatus::Todo;
        DiagnosisPlanner::start_step(&mut plan, 2);
        assert_eq!(plan.steps[1].status, PlanStepStatus::InProgress);
    }
}
