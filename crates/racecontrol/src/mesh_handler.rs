//! Mesh Gossip Handler — processes incoming mesh messages from pod agents.
//!
//! Called from ws/mod.rs when an AgentMessage::MeshSolution*, MeshExperiment*,
//! or MeshHeartbeat is received. Stores in fleet KB and triggers broadcasts.

use std::sync::Arc;
use chrono::Utc;
use rc_common::mesh_types::*;
use rc_common::protocol::{AgentMessage, CoreToAgentMessage};
use serde_json::json;
use tokio::sync::mpsc;

use crate::fleet_kb;
use crate::state::AppState;

/// Handle an incoming mesh gossip message from a pod agent.
/// Returns an optional response to send back to the originating agent.
pub async fn handle_mesh_message(
    state: &Arc<AppState>,
    agent_msg: &AgentMessage,
    cmd_tx: &mpsc::Sender<CoreToAgentMessage>,
) {
    match agent_msg {
        AgentMessage::MeshSolutionAnnounce {
            problem_hash,
            problem_key,
            solution_version,
            confidence,
            fix_type,
            source_node,
            environment_tags,
            cost_to_diagnose,
            summary,
        } => {
            handle_solution_announce(
                state,
                problem_hash,
                problem_key,
                *solution_version,
                *confidence,
                fix_type,
                source_node,
                environment_tags,
                *cost_to_diagnose,
                summary,
            )
            .await;
        }

        AgentMessage::MeshSolutionRequest {
            problem_hash,
            requesting_node,
        } => {
            handle_solution_request(state, problem_hash, requesting_node, cmd_tx).await;
        }

        AgentMessage::MeshExperimentAnnounce {
            problem_key,
            hypothesis,
            status,
            node,
            estimated_cost,
        } => {
            handle_experiment_announce(state, problem_key, hypothesis, status, node, *estimated_cost)
                .await;
        }

        AgentMessage::MeshHeartbeat {
            node,
            kb_size,
            kb_hash,
            budget_remaining,
            last_diagnosis,
        } => {
            handle_heartbeat(state, node, *kb_size, kb_hash, *budget_remaining, last_diagnosis.as_deref())
                .await;
        }

        _ => {} // Non-mesh messages handled elsewhere
    }
}

/// Pod announces it solved a problem → store in fleet KB as candidate.
async fn handle_solution_announce(
    state: &AppState,
    problem_hash: &str,
    problem_key: &str,
    solution_version: u32,
    confidence: f64,
    fix_type: &str,
    source_node: &str,
    environment_tags: &[String],
    cost_to_diagnose: f64,
    summary: &str,
) {
    // MMA-C3: Validate source_node against known connected pods
    let is_known_node = {
        let pods = state.pods.read().await;
        pods.values().any(|p| p.id == source_node || p.name == source_node)
    };
    if !is_known_node {
        tracing::warn!(
            target: "mesh_handler",
            source = %source_node,
            "Mesh: solution announce from UNKNOWN source_node — rejected"
        );
        return;
    }

    // MMA-C3: Cap confidence to [0.0, 1.0] range
    let confidence = confidence.clamp(0.0, 1.0);

    // MMA-C3: Validate field lengths to prevent oversized payloads
    if problem_hash.len() > 256 || problem_key.len() > 512 || summary.len() > 4096 {
        tracing::warn!(
            target: "mesh_handler",
            "Mesh: solution announce with oversized fields — rejected"
        );
        return;
    }

    let now = Utc::now();
    let id = format!("sol_{}", &problem_hash[..problem_hash.len().min(16)]);

    // Check if we already have this solution
    match fleet_kb::get_solution_by_hash(&state.db, problem_hash).await {
        Ok(Some(existing)) => {
            // Update confidence on re-announcement (counts as another success)
            if let Err(e) = fleet_kb::update_confidence(&state.db, &existing.id, true).await {
                tracing::warn!("Failed to update mesh solution confidence: {e}");
            }
            tracing::info!(
                problem_key = %problem_key,
                source = %source_node,
                "Mesh: solution re-announced, confidence updated"
            );
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("Failed to lookup mesh solution: {e}");
        }
    }

    let ft = serde_json::from_str::<FixType>(&format!("\"{fix_type}\""))
        .unwrap_or(FixType::Deterministic);

    let solution = MeshSolution {
        id: id.clone(),
        problem_key: problem_key.to_string(),
        problem_hash: problem_hash.to_string(),
        symptoms: json!({"summary": summary}),
        environment: json!({"tags": environment_tags}),
        root_cause: summary.to_string(),
        fix_action: json!({"description": summary}),
        fix_type: ft,
        status: SolutionStatus::Candidate,
        success_count: 1,
        fail_count: 0,
        confidence,
        cost_to_diagnose,
        models_used: None,
        diagnosis_tier: DiagnosisTier::Deterministic, // default, updated by pod if known
        source_node: source_node.to_string(),
        venue_id: None,
        created_at: now,
        updated_at: now,
        version: solution_version,
        ttl_days: 90,
        tags: if environment_tags.is_empty() { None } else { Some(environment_tags.to_vec()) },
    };

    if let Err(e) = fleet_kb::insert_solution(&state.db, &solution).await {
        tracing::warn!("Failed to insert mesh solution: {e}");
        return;
    }

    // Log as incident
    let incident = MeshIncident {
        id: format!("inc_{}_{}", source_node, now.timestamp_millis()),
        node: source_node.to_string(),
        problem_key: problem_key.to_string(),
        severity: IncidentSeverity::Medium,
        cost: cost_to_diagnose,
        resolution: Some(summary.to_string()),
        time_to_resolve_secs: None,
        resolved_by_tier: Some(DiagnosisTier::Deterministic),
        detected_at: now,
        resolved_at: Some(now),
    };
    let _ = fleet_kb::insert_incident(&state.db, &incident).await;

    tracing::info!(
        problem_key = %problem_key,
        source = %source_node,
        confidence = %confidence,
        "Mesh: new solution stored as candidate"
    );
}

/// Pod requests full solution details → respond with solution data.
async fn handle_solution_request(
    state: &AppState,
    problem_hash: &str,
    requesting_node: &str,
    cmd_tx: &mpsc::Sender<CoreToAgentMessage>,
) {
    match fleet_kb::get_solution_by_hash(&state.db, problem_hash).await {
        Ok(Some(sol)) => {
            let msg = CoreToAgentMessage::MeshSolutionBroadcast {
                problem_hash: sol.problem_hash,
                problem_key: sol.problem_key,
                root_cause: sol.root_cause,
                fix_action: sol.fix_action.to_string(),
                fix_type: serde_json::to_string(&sol.fix_type)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                confidence: sol.confidence,
                source_node: sol.source_node,
                promotion_status: serde_json::to_string(&sol.status)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
            };
            let _ = cmd_tx.send(msg).await;
            tracing::debug!("Mesh: sent solution to {requesting_node}");
        }
        Ok(None) => {
            tracing::debug!("Mesh: no solution found for hash {problem_hash} (requested by {requesting_node})");
        }
        Err(e) => {
            tracing::warn!("Mesh: solution lookup error: {e}");
        }
    }
}

/// Pod announces an active experiment → broadcast to prevent duplicate diagnosis.
async fn handle_experiment_announce(
    state: &AppState,
    problem_key: &str,
    hypothesis: &str,
    status: &str,
    node: &str,
    estimated_cost: f64,
) {
    let exp = MeshExperiment {
        id: format!("exp_{}_{}", node, Utc::now().timestamp_millis()),
        problem_key: problem_key.to_string(),
        hypothesis: hypothesis.to_string(),
        test_plan: format!("Node {node} testing: {hypothesis}"),
        result: match status {
            "confirmed" => Some(ExperimentResult::Confirmed),
            "eliminated" => Some(ExperimentResult::Eliminated),
            _ => None,
        },
        cost: estimated_cost,
        node: node.to_string(),
        created_at: Utc::now(),
    };

    if let Err(e) = fleet_kb::insert_experiment(&state.db, &exp).await {
        tracing::warn!("Failed to store mesh experiment: {e}");
        return;
    }

    // Broadcast to other agents so they don't duplicate this diagnosis
    let broadcast = CoreToAgentMessage::MeshExperimentBroadcast {
        problem_key: problem_key.to_string(),
        hypothesis: hypothesis.to_string(),
        node: node.to_string(),
        estimated_cost,
    };

    let senders = state.agent_senders.read().await;
    for (pod_id, tx) in senders.iter() {
        if pod_id != node {
            let _ = tx.send(broadcast.clone()).await;
        }
    }

    tracing::info!(
        problem_key = %problem_key,
        node = %node,
        "Mesh: experiment announced, broadcast to {} peers", senders.len().saturating_sub(1)
    );
}

/// Pod sends periodic heartbeat with KB digest → detect drift, track node status.
async fn handle_heartbeat(
    _state: &AppState,
    node: &str,
    kb_size: u32,
    _kb_hash: &str,
    budget_remaining: f64,
    _last_diagnosis: Option<&str>,
) {
    // For now, log heartbeat. KB drift detection and sync will be built
    // when we have a fleet_kb_hash() function to compare against.
    tracing::debug!(
        node = %node,
        kb_size = kb_size,
        budget = %budget_remaining,
        "Mesh heartbeat received"
    );
}
