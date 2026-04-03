#![allow(dead_code)]
//! Mesh Gossip — solution propagation between pods and server.
//!
//! Handles sending and receiving mesh intelligence messages over the existing
//! rc-agent ↔ racecontrol WebSocket connection.
//!
//! Pod → Server: MeshSolutionAnnounce, MeshExperimentAnnounce, MeshHeartbeat
//! Server → Pod: MeshSolutionBroadcast, MeshExperimentBroadcast, MeshSystemicAlert
//!
//! Phase 233 — Meshed Intelligence MESH-01 to MESH-06.

use rc_common::protocol::{AgentMessage, CoreToAgentMessage};

use crate::knowledge_base::{self, KnowledgeBase, Solution, KB_PATH};

const LOG_TARGET: &str = "mesh-gossip";

/// Handle a mesh message received from the server (CoreToAgentMessage variants).
/// Called by ws_handler when a mesh message arrives.
///
/// Returns true if the message was handled, false if not a mesh message.
pub fn handle_server_message(msg: &CoreToAgentMessage) -> bool {
    match msg {
        CoreToAgentMessage::MeshSolutionBroadcast {
            problem_hash,
            problem_key,
            root_cause,
            fix_action,
            fix_type,
            confidence,
            source_node,
            promotion_status,
        } => {
            handle_solution_broadcast(
                problem_hash,
                problem_key,
                root_cause,
                fix_action,
                fix_type,
                *confidence,
                source_node,
                promotion_status,
            );
            true
        }
        CoreToAgentMessage::MeshExperimentBroadcast {
            problem_key,
            hypothesis,
            node,
            estimated_cost,
        } => {
            handle_experiment_broadcast(problem_key, hypothesis, node, *estimated_cost);
            true
        }
        CoreToAgentMessage::MeshSystemicAlert {
            problem_key,
            affected_pods,
            timestamp,
        } => {
            handle_systemic_alert(problem_key, affected_pods, timestamp);
            true
        }
        CoreToAgentMessage::MeshInterimBroadcast {
            problem_key,
            step_number,
            consensus_json,
            source_node,
            cost_so_far,
        } => {
            handle_interim_broadcast(problem_key, *step_number, consensus_json, source_node, *cost_so_far);
            true
        }
        _ => false,
    }
}

/// Handle a solution broadcast from the server — store in local KB as candidate.
/// MESH-06: Environment-aware propagation — solutions from other nodes stored as candidates.
fn handle_solution_broadcast(
    problem_hash: &str,
    problem_key: &str,
    root_cause: &str,
    fix_action: &str,
    fix_type: &str,
    confidence: f64,
    source_node: &str,
    promotion_status: &str,
) {
    tracing::info!(
        target: LOG_TARGET,
        problem_key = problem_key,
        source = source_node,
        confidence = confidence,
        promotion = promotion_status,
        "Received solution broadcast from fleet"
    );

    // Store in local KB — fleet-verified solutions get full confidence,
    // candidates get 50% confidence (needs local verification)
    let local_confidence = if promotion_status == "fleet_verified" || promotion_status == "hardened" {
        confidence
    } else {
        (confidence * 0.5).min(0.79) // Below 0.8 threshold — won't auto-apply until locally verified
    };

    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, error = %e, "Cannot store fleet solution — KB unavailable");
            return;
        }
    };

    let solution = Solution {
        id: format!("fleet_{}", problem_hash),
        problem_key: problem_key.to_string(),
        problem_hash: problem_hash.to_string(),
        symptoms: "{}".to_string(),
        environment: "{}".to_string(),
        root_cause: root_cause.to_string(),
        fix_action: fix_action.to_string(),
        fix_type: fix_type.to_string(),
        success_count: 1,
        fail_count: 0,
        confidence: local_confidence,
        cost_to_diagnose: 0.0,
        models_used: None,
        source_node: source_node.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        version: 1,
        ttl_days: 90,
        tags: Some(format!("[\"fleet\", \"{}\"]", promotion_status)),
        diagnosis_method: Some("fleet_gossip".to_string()),
        fix_permanence: "workaround".to_string(),
        recurrence_count: 0,
        permanent_fix_id: None,
        last_recurrence: None,
        permanent_attempt_at: None,
    };

    if let Err(e) = kb.store_solution(&solution) {
        tracing::warn!(target: LOG_TARGET, error = %e, "Failed to store fleet solution in local KB");
    } else {
        tracing::info!(
            target: LOG_TARGET,
            problem_key = problem_key,
            local_confidence = local_confidence,
            "Fleet solution stored in local KB"
        );
    }
}

/// Handle experiment broadcast — record that another pod is diagnosing this issue.
/// MESH-05: First-responder rule — don't start diagnosis if another pod is already on it.
fn handle_experiment_broadcast(
    problem_key: &str,
    hypothesis: &str,
    node: &str,
    estimated_cost: f64,
) {
    tracing::info!(
        target: LOG_TARGET,
        problem_key = problem_key,
        node = node,
        hypothesis = hypothesis,
        cost = estimated_cost,
        "Another pod is diagnosing this issue — will wait for result"
    );

    // Store as an open experiment in local KB so tier3/4 can check before spending
    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, error = %e, "Cannot record fleet experiment — KB unavailable");
            return;
        }
    };

    let exp = knowledge_base::Experiment {
        id: format!("fleet_exp_{}_{}", node, problem_key),
        problem_key: problem_key.to_string(),
        hypothesis: hypothesis.to_string(),
        test_plan: format!("Waiting for {} to complete diagnosis", node),
        result: None, // Open experiment — node is still working on it
        cost: estimated_cost,
        node: node.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let _ = kb.record_experiment(&exp);
}

/// Handle systemic alert — 3+ pods have the same issue.
fn handle_systemic_alert(problem_key: &str, affected_pods: &[String], timestamp: &str) {
    tracing::warn!(
        target: LOG_TARGET,
        problem_key = problem_key,
        affected_count = affected_pods.len(),
        timestamp = timestamp,
        "SYSTEMIC ALERT: fleet-wide issue detected"
    );
    // Future: trigger immediate Tier 4 diagnosis + WhatsApp to Uday
}

/// Handle an interim MMA result broadcast from another pod.
///
/// Stores the consensus in the local MMA cache so that if this pod encounters
/// the same problem, it can skip already-completed MMA steps.
fn handle_interim_broadcast(
    problem_key: &str,
    step_number: u8,
    consensus_json: &str,
    source_node: &str,
    cost_so_far: f64,
) {
    tracing::info!(
        target: LOG_TARGET,
        problem_key,
        step_number,
        source_node,
        cost_so_far,
        "Received interim MMA result (step {}) from {} — caching locally",
        step_number, source_node
    );

    // Store in local MMA cache so this pod can skip Steps 1-3 if the same problem occurs.
    // We use a synthetic problem_hash since we only have problem_key.
    // The cache will match when this pod's run_protocol() computes the same hash.
    if let Ok(cache) = crate::mma_cache::MmaCache::open(KB_PATH) {
        // Use problem_key as a simple hash — the real run_protocol will compute the proper
        // stable_hash and may or may not match. This is a best-effort optimization.
        let build_id = std::env::var("RACECONTROL_BUILD_ID").unwrap_or_default();
        if let Err(e) = cache.put(
            &format!("interim_{}_{}", problem_key, step_number),
            consensus_json,
            cost_so_far,
            &build_id,
        ) {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "Failed to cache interim result from fleet"
            );
        }
    }
}

/// Build an AgentMessage::MeshSolutionAnnounce from a local solution.
/// Called by tier_engine after a successful Tier 3/4 diagnosis.
pub fn build_solution_announce(
    solution: &Solution,
    build_id: &str,
) -> AgentMessage {
    AgentMessage::MeshSolutionAnnounce {
        problem_hash: solution.problem_hash.clone(),
        problem_key: solution.problem_key.clone(),
        solution_version: solution.version as u32,
        confidence: solution.confidence,
        fix_type: solution.fix_type.clone(),
        source_node: solution.source_node.clone(),
        environment_tags: vec![
            format!("build:{}", build_id),
            "pod".to_string(),
        ],
        cost_to_diagnose: solution.cost_to_diagnose,
        summary: solution.root_cause.clone(),
    }
}

/// Build an AgentMessage::MeshExperimentAnnounce.
/// Called BEFORE starting a Tier 3/4 diagnosis to prevent fleet-wide duplicate work.
/// MESH-03: Experiment announcement.
pub fn build_experiment_announce(
    problem_key: &str,
    tier: u8,
    node_id: &str,
    estimated_cost: f64,
) -> AgentMessage {
    AgentMessage::MeshExperimentAnnounce {
        problem_key: problem_key.to_string(),
        hypothesis: format!("Tier {} diagnosis in progress", tier),
        status: "testing".to_string(),
        node: node_id.to_string(),
        estimated_cost,
    }
}

/// Build an AgentMessage::MeshSolutionAnnounce for a game launch fix (GAME-05).
/// Convenience wrapper that creates a minimal Solution and delegates to build_solution_announce.
pub fn build_game_fix_announce(
    cause: &str,
    fix: &str,
    confidence: f64,
    node_id: &str,
) -> AgentMessage {
    let now = chrono::Utc::now().to_rfc3339();
    let solution = Solution {
        id: format!("game_fix_{}_{}", node_id, chrono::Utc::now().timestamp()),
        problem_key: "game_launch_fail".to_string(),
        problem_hash: format!("game_launch:{}", cause),
        symptoms: "game launch failure".to_string(),
        environment: "{}".to_string(),
        root_cause: cause.to_string(),
        fix_action: fix.to_string(),
        fix_type: "deterministic".to_string(),
        success_count: 1,
        fail_count: 0,
        confidence,
        cost_to_diagnose: 0.0,
        models_used: None,
        source_node: node_id.to_string(),
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        ttl_days: 90,
        tags: Some("[\"game_launch\", \"auto_retry\"]".to_string()),
        diagnosis_method: Some("game_doctor_retry".to_string()),
        fix_permanence: "workaround".to_string(),
        recurrence_count: 0,
        permanent_fix_id: None,
        last_recurrence: None,
        permanent_attempt_at: None,
    };
    // Use the node_id as build_id for game fixes (actual build_id not available here)
    build_solution_announce(&solution, node_id)
}

/// Check if another node is already diagnosing this problem.
/// MESH-05: First-responder rule.
/// Returns true if we should SKIP diagnosis (another node is on it).
pub fn should_skip_diagnosis(problem_key: &str) -> bool {
    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(_) => return false, // Can't check — proceed with diagnosis
    };

    match kb.get_open_experiment(problem_key) {
        Ok(Some(exp)) => {
            tracing::info!(
                target: LOG_TARGET,
                problem_key = problem_key,
                diagnosing_node = %exp.node,
                "First-responder rule: another node is already diagnosing — skipping"
            );
            true
        }
        _ => false,
    }
}

/// Build a MeshHeartbeat message with current KB state.
/// MESH-04: Periodic KB digest for sync detection.
pub fn build_heartbeat(node_id: &str, budget_remaining: f64) -> AgentMessage {
    let (kb_size, kb_hash) = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => {
            let size = kb.solution_count().unwrap_or(0) as u32;
            // Simple hash: solution count as string — Phase 234 can upgrade to bloom filter
            let hash = format!("count:{}", size);
            (size, hash)
        }
        Err(_) => (0, "unavailable".to_string()),
    };

    AgentMessage::MeshHeartbeat {
        node: node_id.to_string(),
        kb_size,
        kb_hash,
        budget_remaining,
        last_diagnosis: None, // TODO: track in budget_tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_solution_announce() {
        let sol = Solution {
            id: "test".to_string(),
            problem_key: "ws_disconnect".to_string(),
            problem_hash: "abc123".to_string(),
            symptoms: "{}".to_string(),
            environment: "{}".to_string(),
            root_cause: "MAINTENANCE_MODE blocking".to_string(),
            fix_action: "clear sentinel".to_string(),
            fix_type: "deterministic".to_string(),
            success_count: 3,
            fail_count: 0,
            confidence: 0.95,
            cost_to_diagnose: 0.05,
            models_used: None,
            source_node: "pod_3".to_string(),
            created_at: "2026-03-27".to_string(),
            updated_at: "2026-03-27".to_string(),
            version: 1,
            ttl_days: 90,
            tags: None,
            diagnosis_method: Some("consensus_5model".to_string()),
            fix_permanence: "workaround".to_string(),
            recurrence_count: 0,
            permanent_fix_id: None,
            last_recurrence: None,
            permanent_attempt_at: None,
        };

        let msg = build_solution_announce(&sol, "abc123");
        match msg {
            AgentMessage::MeshSolutionAnnounce {
                problem_hash,
                confidence,
                source_node,
                ..
            } => {
                assert_eq!(problem_hash, "abc123");
                assert!((confidence - 0.95).abs() < f64::EPSILON);
                assert_eq!(source_node, "pod_3");
            }
            _ => panic!("Expected MeshSolutionAnnounce"),
        }
    }

    #[test]
    fn test_build_experiment_announce() {
        let msg = build_experiment_announce("ws_disconnect", 3, "pod_5", 0.05);
        match msg {
            AgentMessage::MeshExperimentAnnounce {
                problem_key,
                node,
                estimated_cost,
                ..
            } => {
                assert_eq!(problem_key, "ws_disconnect");
                assert_eq!(node, "pod_5");
                assert!((estimated_cost - 0.05).abs() < f64::EPSILON);
            }
            _ => panic!("Expected MeshExperimentAnnounce"),
        }
    }

    #[test]
    fn test_build_heartbeat() {
        let msg = build_heartbeat("pod_7", 8.50);
        match msg {
            AgentMessage::MeshHeartbeat {
                node,
                budget_remaining,
                ..
            } => {
                assert_eq!(node, "pod_7");
                assert!((budget_remaining - 8.50).abs() < 0.001);
            }
            _ => panic!("Expected MeshHeartbeat"),
        }
    }

    #[test]
    fn test_should_skip_diagnosis_no_kb() {
        // With no KB file, should return false (proceed with diagnosis)
        // This test works because C:\RacingPoint\mesh_kb.db doesn't exist in test env
        let result = should_skip_diagnosis("nonexistent_key");
        assert!(!result, "Should not skip when KB is unavailable");
    }
}
