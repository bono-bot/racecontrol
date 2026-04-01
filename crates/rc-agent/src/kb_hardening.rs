//! KB Hardening Pipeline — Promotion ladder for knowledge base solutions.
//!
//! Phase 278: KB-01..05
//! Promotion ladder: Observed -> Shadow -> Canary -> Quorum -> Deterministic Rule
//!
//! - Shadow mode: solution executes alongside but logs only (confidence capped at 0.5)
//! - Canary: apply on Pod 8 first, verify before fleet
//! - Quorum: 3+ successes across 2+ pods triggers promotion
//! - Deterministic Rule: promoted rules stored as typed HardenedRule structs
//!
//! Standing rules: no .unwrap(), lifecycle logging for every tokio::spawn.

use chrono::Utc;
use tokio::sync::broadcast;

use rc_common::fleet_event::FleetEvent;

use crate::knowledge_base::{KnowledgeBase, Solution, HardenedRule, KB_PATH};

const LOG_TARGET: &str = "kb-hardening";

/// Promotion status values for the ladder.
pub const STATUS_OBSERVED: &str = "observed";
pub const STATUS_SHADOW: &str = "shadow";
pub const STATUS_CANARY: &str = "canary";
pub const STATUS_QUORUM: &str = "quorum";
pub const STATUS_HARDENED: &str = "hardened";

/// Minimum applications or days in shadow before canary promotion.
const SHADOW_MIN_APPLICATIONS: i64 = 25;
const SHADOW_MIN_DAYS: i64 = 7;

/// Quorum thresholds.
const QUORUM_MIN_SUCCESSES: i64 = 3;
const QUORUM_MIN_DISTINCT_NODES: usize = 2;

/// Promotion check interval.
const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Spawn the background KB hardening promotion task.
/// Runs every 5 minutes, checking for solutions eligible for promotion.
pub fn spawn(fleet_tx: broadcast::Sender<FleetEvent>, node_id: String) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "KB hardening promoter started");

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECS)).await;

            if let Err(e) = run_promotion_cycle(&fleet_tx, &node_id) {
                tracing::warn!(target: LOG_TARGET, error = %e, "Promotion cycle failed");
            }
        }

        // Unreachable but satisfies lifecycle logging requirement
        #[allow(unreachable_code)]
        tracing::info!(target: LOG_TARGET, "KB hardening promoter exiting");
    });
}

/// Run one promotion cycle — check all ladder transitions.
fn run_promotion_cycle(
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
) -> anyhow::Result<()> {
    let kb = KnowledgeBase::open(KB_PATH)?;

    // 1. Observed -> Shadow: any solution with success_count >= 1
    promote_observed_to_shadow(&kb, fleet_tx, node_id)?;

    // 2. Shadow -> Canary: after 25 applications OR 1 week
    promote_shadow_to_canary(&kb, fleet_tx, node_id)?;

    // 3. Canary -> Quorum: after success on canary pod (pod_8)
    promote_canary_to_quorum(&kb, fleet_tx, node_id)?;

    // 4. Quorum -> Hardened: 3+ successes across 2+ distinct nodes
    promote_quorum_to_hardened(&kb, fleet_tx, node_id)?;

    Ok(())
}

fn promote_observed_to_shadow(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
) -> anyhow::Result<()> {
    let candidates = kb.get_promotion_candidates(STATUS_OBSERVED)?;
    for sol in candidates {
        if sol.success_count >= 1 {
            kb.promote_solution(&sol.problem_hash, STATUS_SHADOW)?;
            emit_promotion_event(fleet_tx, node_id, &sol, STATUS_OBSERVED, STATUS_SHADOW);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                "Promoted observed -> shadow"
            );
        }
    }
    Ok(())
}

fn promote_shadow_to_canary(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
) -> anyhow::Result<()> {
    let candidates = kb.get_promotion_candidates(STATUS_SHADOW)?;
    for sol in candidates {
        let days_in_shadow = kb.days_since_promotion(&sol.problem_hash).unwrap_or(0);
        let applications = sol.success_count + sol.fail_count;

        if applications >= SHADOW_MIN_APPLICATIONS || days_in_shadow >= SHADOW_MIN_DAYS {
            // Only promote if shadow period showed reasonable success rate
            if sol.success_count > sol.fail_count {
                kb.promote_solution(&sol.problem_hash, STATUS_CANARY)?;
                emit_promotion_event(fleet_tx, node_id, &sol, STATUS_SHADOW, STATUS_CANARY);
                tracing::info!(
                    target: LOG_TARGET,
                    problem_key = %sol.problem_key,
                    days = days_in_shadow,
                    applications = applications,
                    "Promoted shadow -> canary"
                );
            }
        }
    }
    Ok(())
}

fn promote_canary_to_quorum(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
) -> anyhow::Result<()> {
    let candidates = kb.get_promotion_candidates(STATUS_CANARY)?;
    for sol in candidates {
        // Canary must have at least one success from a pod_8 node
        let has_canary_success = kb.has_canary_pod_success(&sol.problem_hash)?;
        if has_canary_success {
            kb.promote_solution(&sol.problem_hash, STATUS_QUORUM)?;
            emit_promotion_event(fleet_tx, node_id, &sol, STATUS_CANARY, STATUS_QUORUM);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                "Promoted canary -> quorum (Pod 8 verified)"
            );
        }
    }
    Ok(())
}

fn promote_quorum_to_hardened(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
) -> anyhow::Result<()> {
    let candidates = kb.get_promotion_candidates(STATUS_QUORUM)?;
    for sol in candidates {
        let distinct_nodes = kb.count_distinct_nodes(&sol.problem_hash)?;
        if sol.success_count >= QUORUM_MIN_SUCCESSES && distinct_nodes >= QUORUM_MIN_DISTINCT_NODES {
            // Create a typed HardenedRule from this solution
            let rule = HardenedRule {
                problem_key: sol.problem_key.clone(),
                matchers: derive_matchers(&sol),
                action: sol.fix_action.clone(),
                verifier: format!("kb_verify:{}", sol.problem_hash),
                ttl_secs: sol.ttl_days * 86400,
                confidence: sol.confidence,
                provenance: format!(
                    "promoted from {} via {}/{} successes across {} nodes",
                    sol.id, sol.success_count, sol.success_count + sol.fail_count, distinct_nodes
                ),
            };

            kb.store_hardened_rule(&rule)?;
            kb.promote_solution(&sol.problem_hash, STATUS_HARDENED)?;
            emit_promotion_event(fleet_tx, node_id, &sol, STATUS_QUORUM, STATUS_HARDENED);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                distinct_nodes = distinct_nodes,
                successes = sol.success_count,
                "Promoted quorum -> hardened rule"
            );
        }
    }
    Ok(())
}

/// Derive matchers from solution metadata.
/// Matchers are patterns that identify when this rule should fire.
fn derive_matchers(sol: &Solution) -> Vec<String> {
    let mut matchers = Vec::new();
    matchers.push(format!("problem_key:{}", sol.problem_key));
    matchers.push(format!("problem_hash:{}", sol.problem_hash));
    if !sol.symptoms.is_empty() {
        matchers.push(format!("symptoms:{}", sol.symptoms));
    }
    matchers
}

/// Check if a solution is in shadow mode — confidence should be capped at 0.5.
pub fn is_shadow_mode(kb: &KnowledgeBase, problem_hash: &str) -> bool {
    kb.get_promotion_status(problem_hash)
        .map(|s| s == STATUS_SHADOW)
        .unwrap_or(false)
}

/// Check if a solution is in canary mode — only apply on pod_8.
pub fn is_canary_mode(kb: &KnowledgeBase, problem_hash: &str) -> bool {
    kb.get_promotion_status(problem_hash)
        .map(|s| s == STATUS_CANARY)
        .unwrap_or(false)
}

/// Check if this node is the canary pod (pod_8 or pod-8).
pub fn is_canary_pod(node_id: &str) -> bool {
    node_id.contains("pod_8") || node_id.contains("pod-8")
}

fn emit_promotion_event(
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    sol: &Solution,
    from: &str,
    to: &str,
) {
    let _ = fleet_tx.send(FleetEvent::FixApplied {
        node_id: node_id.to_string(),
        tier: 0, // KB hardening is tier 0 (infrastructure)
        action: format!("kb_promotion:{}>{}", from, to),
        trigger: sol.problem_key.clone(),
        timestamp: Utc::now(),
    });
}
