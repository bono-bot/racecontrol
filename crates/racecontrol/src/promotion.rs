//! Promotion Pipeline + Systemic Pattern Detection — v26.0 Meshed Intelligence.
//!
//! Background tokio task runs every 60s:
//! 1. Scans candidates → promotes to fleet_verified/hardened based on thresholds
//! 2. Detects systemic patterns (3+ pods, same problem, within 5 min)
//! 3. Expires stale solutions past their TTL
//! 4. Broadcasts promoted solutions to connected agents

use std::sync::Arc;
use chrono::Utc;
use tokio::time::{interval, Duration};
use rc_common::mesh_types::*;
use rc_common::protocol::CoreToAgentMessage;

use crate::fleet_kb;
use crate::state::AppState;

/// Spawn the promotion pipeline background task.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tracing::info!("Mesh promotion pipeline started (60s interval)");

        loop {
            ticker.tick().await;
            if let Err(e) = run_cycle(&state).await {
                tracing::warn!("Mesh promotion cycle error: {e}");
            }
        }
    });
}

/// Run one promotion + detection + expiry cycle.
async fn run_cycle(state: &AppState) -> anyhow::Result<()> {
    let pool = &state.db;

    // 1. Promote candidates
    let candidates = fleet_kb::get_candidates(pool).await?;
    for sol in &candidates {
        let unique_nodes = fleet_kb::count_unique_nodes(pool, &sol.problem_key).await?;

        // Candidate → fleet_verified: 3+ successes across 2+ unique pods
        if sol.success_count >= 3 && unique_nodes >= 2 && sol.status == SolutionStatus::Candidate {
            fleet_kb::update_status(pool, &sol.id, SolutionStatus::FleetVerified).await?;
            tracing::info!(
                problem_key = %sol.problem_key,
                "Mesh: promoted to fleet_verified (successes={}, nodes={})",
                sol.success_count, unique_nodes
            );
            broadcast_solution(state, &sol.id, FleetUpdateReason::Promoted).await;
        }
    }

    // Check fleet_verified → hardened: 10+ successes, zero failures
    let verified = fleet_kb::list_solutions(pool, Some("fleet_verified"), 100, 0).await?;
    for sol in &verified {
        if sol.success_count >= 10 && sol.fail_count == 0 {
            fleet_kb::update_status(pool, &sol.id, SolutionStatus::Hardened).await?;
            tracing::info!(
                problem_key = %sol.problem_key,
                "Mesh: hardened (successes={}, zero failures)", sol.success_count
            );
            broadcast_solution(state, &sol.id, FleetUpdateReason::Hardened).await;
        }
        // Demote if confidence drops below threshold
        if sol.confidence < 0.5 {
            fleet_kb::update_status(pool, &sol.id, SolutionStatus::Demoted).await?;
            tracing::warn!(
                problem_key = %sol.problem_key,
                "Mesh: demoted (confidence={:.2})", sol.confidence
            );
        }
    }

    // 2. Detect systemic patterns
    detect_systemic_patterns(state).await?;

    // 3. Expire stale solutions
    let expired = fleet_kb::expire_stale_solutions(pool).await?;
    if expired > 0 {
        tracing::info!("Mesh: expired {expired} stale solutions");
    }

    Ok(())
}

/// Detect systemic patterns: 3+ pods reporting the same problem_key within 5 minutes.
async fn detect_systemic_patterns(state: &AppState) -> anyhow::Result<()> {
    let pool = &state.db;

    // Get all recent incidents grouped by problem_key
    let recent = fleet_kb::list_incidents(pool, 50, 0).await?;

    // Group by problem_key, only consider incidents from last 5 min
    let cutoff = Utc::now() - chrono::Duration::minutes(5);
    let mut problem_nodes: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for inc in &recent {
        if inc.detected_at >= cutoff {
            problem_nodes
                .entry(inc.problem_key.clone())
                .or_default()
                .push(inc.node.clone());
        }
    }

    for (problem_key, nodes) in &problem_nodes {
        // Deduplicate nodes
        let mut unique: Vec<String> = nodes.clone();
        unique.sort();
        unique.dedup();

        if unique.len() >= 3 {
            let severity = if unique.len() >= 5 {
                SystemicSeverity::Emergency
            } else {
                SystemicSeverity::Critical
            };

            tracing::error!(
                problem_key = %problem_key,
                affected = ?unique,
                "SYSTEMIC PATTERN DETECTED: {} pods affected", unique.len()
            );

            // Broadcast systemic alert to all connected agents
            let alert_msg = CoreToAgentMessage::MeshSystemicAlert {
                problem_key: problem_key.clone(),
                affected_pods: unique.clone(),
                timestamp: Utc::now().to_rfc3339(),
            };
            broadcast_to_agents(state, &alert_msg).await;

            // WhatsApp alert to Uday
            let severity_str = match severity {
                SystemicSeverity::Emergency => "EMERGENCY",
                SystemicSeverity::Critical => "CRITICAL",
                SystemicSeverity::Warning => "WARNING",
            };
            let wa_msg = format!(
                "🔴 MESH {} ALERT\n\nProblem: {}\nAffected: {} pods ({})\nTime: {}",
                severity_str,
                problem_key,
                unique.len(),
                unique.join(", "),
                Utc::now().format("%H:%M IST")
            );
            crate::whatsapp_alerter::send_whatsapp(&state.config, &wa_msg).await;
        }
    }

    Ok(())
}

/// Broadcast a promoted/hardened solution to all connected agents.
async fn broadcast_solution(state: &AppState, solution_id: &str, reason: FleetUpdateReason) {
    let sol = match fleet_kb::get_solution(&state.db, solution_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    let status_str = match reason {
        FleetUpdateReason::Promoted => "fleet_verified",
        FleetUpdateReason::Hardened => "hardened",
        FleetUpdateReason::Retired => "retired",
        FleetUpdateReason::Updated => "updated",
    };

    let msg = CoreToAgentMessage::MeshSolutionBroadcast {
        problem_hash: sol.problem_hash,
        problem_key: sol.problem_key,
        root_cause: sol.root_cause,
        fix_action: sol.fix_action.to_string(),
        fix_type: serde_json::to_string(&sol.fix_type).unwrap_or_default().trim_matches('"').to_string(),
        confidence: sol.confidence,
        source_node: sol.source_node,
        promotion_status: status_str.to_string(),
    };

    broadcast_to_agents(state, &msg).await;
}

/// Send a CoreToAgentMessage to all connected agents.
async fn broadcast_to_agents(state: &AppState, msg: &CoreToAgentMessage) {
    let senders = state.agent_senders.read().await;
    let json = match serde_json::to_string(msg) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Failed to serialize mesh broadcast: {e}");
            return;
        }
    };
    tracing::debug!("Mesh broadcast to {} agents: {}", senders.len(), &json[..json.len().min(100)]);
    for (pod_id, tx) in senders.iter() {
        if tx.send(msg.clone()).await.is_err() {
            tracing::debug!("Mesh broadcast to {pod_id} failed (channel closed)");
        }
    }
}
