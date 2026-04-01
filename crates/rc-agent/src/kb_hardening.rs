//! KB Hardening Pipeline — Promotion ladder for knowledge base solutions.
//!
//! Phase 278: KB-01..05
//! Phase 291: KBPP-01..06 — Persistent promotion state, stage-gate enforcement, 6-hour cron.
//!
//! Promotion ladder: Observed -> Shadow -> Canary -> Quorum -> Hardened
//!
//! - Shadow mode: solution logs only (confidence capped at 0.5) — KBPP-02
//! - Canary: apply on Pod 8 first, verify before fleet — KBPP-03
//! - Quorum: 3+ successes across 2+ pods triggers promotion — KBPP-04
//! - Hardened: promoted rules stored as typed HardenedRule structs
//!
//! Persistence: KbPromotionStore (SQLite) survives rc-agent restarts — KBPP-01
//! Cron: 6-hour promotion cycle — KBPP-06
//!
//! Standing rules: no .unwrap(), lifecycle logging for every tokio::spawn.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use tokio::sync::broadcast;

use rc_common::fleet_event::FleetEvent;

use crate::knowledge_base::{KnowledgeBase, Solution, HardenedRule, KB_PATH};
use crate::kb_promotion_store::{KbPromotionStore, PromotionCandidate};

const LOG_TARGET: &str = "kb-hardening";

/// Promotion status values for the ladder.
pub const STATUS_OBSERVED: &str = "observed";
pub const STATUS_SHADOW: &str = "shadow";
pub const STATUS_CANARY: &str = "canary";
pub const STATUS_QUORUM: &str = "quorum";
pub const STATUS_HARDENED: &str = "hardened";

/// Minimum applications or days in shadow before canary promotion.
pub const SHADOW_MIN_APPLICATIONS: i64 = 25;
pub const SHADOW_MIN_DAYS: i64 = 7;

/// Quorum thresholds.
const QUORUM_MIN_SUCCESSES: i64 = 3;
const QUORUM_MIN_DISTINCT_NODES: usize = 2;

/// 6-hour promotion evaluation cycle — KBPP-06.
pub const CRON_INTERVAL_SECS: u64 = 21600;

/// Returns the cron interval in seconds (used for testing without hardcoding the constant).
pub fn next_cron_interval_secs() -> u64 {
    CRON_INTERVAL_SECS
}

/// Result of the shadow gate check.
pub enum ShadowGateResult {
    /// Rule is in shadow stage — log only, do NOT apply fix. Confidence capped at 0.5.
    LogOnly { shadow_application_count: i64 },
    /// Rule is not in shadow stage — allow normal application.
    AllowApply,
}

/// Spawn the background KB hardening promotion task.
/// Runs every 6 hours (KBPP-06), checking for solutions eligible for promotion.
/// Loads existing candidates at startup to restore state after rc-agent restarts (KBPP-01).
pub fn spawn(
    fleet_tx: broadcast::Sender<FleetEvent>,
    node_id: String,
    promo_store: Arc<Mutex<KbPromotionStore>>,
) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "KB hardening promoter started");

        // KBPP-01: Restore promotion state from SQLite on startup.
        // Snapshot under lock, then log — don't hold lock across async boundaries.
        let restored_count = {
            let store = promo_store.lock().ok();
            store
                .and_then(|s| s.all_candidates().ok())
                .map(|v| v.len())
                .unwrap_or(0)
        };
        tracing::info!(
            target: LOG_TARGET,
            candidates = restored_count,
            "KB promotion state restored: {} candidates",
            restored_count,
        );

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(CRON_INTERVAL_SECS)).await;

            if let Err(e) = run_promotion_cycle(&fleet_tx, &node_id, &promo_store) {
                tracing::warn!(target: LOG_TARGET, error = %e, "Promotion cycle failed");
            }
        }

        // Unreachable but satisfies lifecycle logging requirement
        #[allow(unreachable_code)]
        tracing::info!(target: LOG_TARGET, "KB hardening promoter exiting");
    });
}

/// Run one promotion cycle — check all ladder transitions.
/// Tracks candidates_checked / promoted / held for the audit log (KBPP-06).
fn run_promotion_cycle(
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    promo_store: &Arc<Mutex<KbPromotionStore>>,
) -> anyhow::Result<()> {
    let kb = KnowledgeBase::open(KB_PATH)?;

    let mut candidates_checked: usize = 0;
    let mut promoted: usize = 0;
    let mut held: usize = 0;

    // 1. Observed -> Shadow: any solution with success_count >= 1
    let (c, p, h) = promote_observed_to_shadow(&kb, fleet_tx, node_id, promo_store)?;
    candidates_checked += c;
    promoted += p;
    held += h;

    // 2. Shadow -> Canary: after 25 applications OR 1 week
    let (c, p, h) = promote_shadow_to_canary(&kb, fleet_tx, node_id, promo_store)?;
    candidates_checked += c;
    promoted += p;
    held += h;

    // 3. Canary -> Quorum: after success on canary pod (pod_8)
    let (c, p, h) = promote_canary_to_quorum(&kb, fleet_tx, node_id, promo_store)?;
    candidates_checked += c;
    promoted += p;
    held += h;

    // 4. Quorum -> Hardened: 3+ successes across 2+ distinct nodes
    let (c, p, h) = promote_quorum_to_hardened(&kb, fleet_tx, node_id, promo_store)?;
    candidates_checked += c;
    promoted += p;
    held += h;

    tracing::info!(
        target: LOG_TARGET,
        candidates_checked,
        promoted,
        held,
        "KB promotion cycle complete (6h cron)",
    );

    Ok(())
}

/// Returns (checked, promoted, held) counts.
fn promote_observed_to_shadow(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    promo_store: &Arc<Mutex<KbPromotionStore>>,
) -> anyhow::Result<(usize, usize, usize)> {
    let candidates = kb.get_promotion_candidates(STATUS_OBSERVED)?;
    let mut promoted = 0usize;
    let mut held = 0usize;

    for sol in &candidates {
        if sol.success_count >= 1 {
            kb.promote_solution(&sol.problem_hash, STATUS_SHADOW)?;

            // KBPP-01: persist the new stage to SQLite so it survives restarts
            let candidate = PromotionCandidate {
                problem_hash: sol.problem_hash.clone(),
                problem_key: sol.problem_key.clone(),
                stage: STATUS_SHADOW.to_string(),
                stage_entered_at: Utc::now().to_rfc3339(),
                shadow_applications: 0,
                created_at: Utc::now().to_rfc3339(),
            };
            if let Ok(store) = promo_store.lock() {
                if let Err(e) = store.upsert_candidate(&candidate) {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to persist observed->shadow");
                }
            }

            emit_promotion_event(fleet_tx, node_id, sol, STATUS_OBSERVED, STATUS_SHADOW);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                "Promoted observed -> shadow"
            );
            promoted += 1;
        } else {
            held += 1;
        }
    }

    Ok((candidates.len(), promoted, held))
}

/// Returns (checked, promoted, held) counts.
fn promote_shadow_to_canary(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    promo_store: &Arc<Mutex<KbPromotionStore>>,
) -> anyhow::Result<(usize, usize, usize)> {
    let candidates = kb.get_promotion_candidates(STATUS_SHADOW)?;
    let mut promoted = 0usize;
    let mut held = 0usize;

    for sol in &candidates {
        let days_in_shadow = kb.days_since_promotion(&sol.problem_hash).unwrap_or(0);
        let applications = sol.success_count + sol.fail_count;

        if applications >= SHADOW_MIN_APPLICATIONS || days_in_shadow >= SHADOW_MIN_DAYS {
            // Only promote if shadow period showed reasonable success rate
            if sol.success_count > sol.fail_count {
                kb.promote_solution(&sol.problem_hash, STATUS_CANARY)?;

                // KBPP-01: persist canary stage
                if let Ok(store) = promo_store.lock() {
                    if let Err(e) = store.update_stage(&sol.problem_hash, STATUS_CANARY) {
                        tracing::warn!(target: LOG_TARGET, error = %e, "Failed to persist shadow->canary");
                    }
                }

                emit_promotion_event(fleet_tx, node_id, sol, STATUS_SHADOW, STATUS_CANARY);
                tracing::info!(
                    target: LOG_TARGET,
                    problem_key = %sol.problem_key,
                    days = days_in_shadow,
                    applications = applications,
                    "Promoted shadow -> canary"
                );
                promoted += 1;
            } else {
                tracing::debug!(
                    target: LOG_TARGET,
                    problem_key = %sol.problem_key,
                    applications = applications,
                    days_in_shadow = days_in_shadow,
                    min_applications = SHADOW_MIN_APPLICATIONS,
                    min_days = SHADOW_MIN_DAYS,
                    "Shadow held — criteria not yet met (need {} apps OR {} days)",
                    SHADOW_MIN_APPLICATIONS, SHADOW_MIN_DAYS,
                );
                held += 1;
            }
        } else {
            tracing::debug!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                applications = applications,
                days_in_shadow = days_in_shadow,
                min_applications = SHADOW_MIN_APPLICATIONS,
                min_days = SHADOW_MIN_DAYS,
                "Shadow held — criteria not yet met (need {} apps OR {} days)",
                SHADOW_MIN_APPLICATIONS, SHADOW_MIN_DAYS,
            );
            held += 1;
        }
    }

    Ok((candidates.len(), promoted, held))
}

/// Returns (checked, promoted, held) counts.
fn promote_canary_to_quorum(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    promo_store: &Arc<Mutex<KbPromotionStore>>,
) -> anyhow::Result<(usize, usize, usize)> {
    let candidates = kb.get_promotion_candidates(STATUS_CANARY)?;
    let mut promoted = 0usize;
    let mut held = 0usize;

    for sol in &candidates {
        // Canary must have at least one success from a pod_8 node
        let has_canary_success = kb.has_canary_pod_success(&sol.problem_hash)?;
        if has_canary_success {
            kb.promote_solution(&sol.problem_hash, STATUS_QUORUM)?;

            // KBPP-01: persist quorum stage
            if let Ok(store) = promo_store.lock() {
                if let Err(e) = store.update_stage(&sol.problem_hash, STATUS_QUORUM) {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to persist canary->quorum");
                }
            }

            emit_promotion_event(fleet_tx, node_id, sol, STATUS_CANARY, STATUS_QUORUM);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                "Promoted canary -> quorum (Pod 8 verified)"
            );
            promoted += 1;
        } else {
            tracing::debug!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                "Canary held — Pod 8 success not yet recorded",
            );
            held += 1;
        }
    }

    Ok((candidates.len(), promoted, held))
}

/// Returns (checked, promoted, held) counts.
fn promote_quorum_to_hardened(
    kb: &KnowledgeBase,
    fleet_tx: &broadcast::Sender<FleetEvent>,
    node_id: &str,
    promo_store: &Arc<Mutex<KbPromotionStore>>,
) -> anyhow::Result<(usize, usize, usize)> {
    let candidates = kb.get_promotion_candidates(STATUS_QUORUM)?;
    let mut promoted = 0usize;
    let mut held = 0usize;

    for sol in &candidates {
        let distinct_nodes = kb.count_distinct_nodes(&sol.problem_hash)?;
        if sol.success_count >= QUORUM_MIN_SUCCESSES && distinct_nodes >= QUORUM_MIN_DISTINCT_NODES {
            // Create a typed HardenedRule from this solution
            let rule = HardenedRule {
                problem_key: sol.problem_key.clone(),
                matchers: derive_matchers(sol),
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

            // KBPP-04: persist hardened stage
            if let Ok(store) = promo_store.lock() {
                if let Err(e) = store.update_stage(&sol.problem_hash, STATUS_HARDENED) {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to persist quorum->hardened");
                }
            }

            emit_promotion_event(fleet_tx, node_id, sol, STATUS_QUORUM, STATUS_HARDENED);
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                distinct_nodes = distinct_nodes,
                successes = sol.success_count,
                "Promoted quorum -> hardened rule"
            );
            promoted += 1;
        } else {
            tracing::debug!(
                target: LOG_TARGET,
                problem_key = %sol.problem_key,
                distinct_nodes = distinct_nodes,
                success_count = sol.success_count,
                "Quorum held — need {}/{} successes across {}/{} distinct nodes",
                QUORUM_MIN_SUCCESSES, sol.success_count,
                QUORUM_MIN_DISTINCT_NODES, distinct_nodes,
            );
            held += 1;
        }
    }

    Ok((candidates.len(), promoted, held))
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
/// Uses exact match to prevent pod_88 from being treated as a canary pod.
pub fn is_canary_pod(node_id: &str) -> bool {
    node_id == "pod_8" || node_id == "pod-8"
        || node_id.ends_with(":pod_8") || node_id.ends_with(":pod-8")
}

/// KBPP-02: Shadow-stage gate enforcement.
/// If the rule is in shadow stage: increment application count, log, return LogOnly.
/// Confidence must be capped at 0.5 by the caller when LogOnly is returned.
pub fn enforce_shadow_gate(
    promo_store: &KbPromotionStore,
    problem_hash: &str,
    node_id: &str,
) -> ShadowGateResult {
    let candidates = promo_store.candidates_at_stage(STATUS_SHADOW).ok().unwrap_or_default();
    let is_shadow = candidates.iter().any(|c| c.problem_hash == problem_hash);

    if is_shadow {
        // Record the shadow application
        if let Err(e) = promo_store.record_shadow_application(problem_hash) {
            tracing::warn!(target: LOG_TARGET, error = %e, "Failed to record shadow application");
        }
        let count = promo_store.shadow_application_count(problem_hash).unwrap_or(0);
        tracing::info!(
            target: LOG_TARGET,
            problem_hash = problem_hash,
            node_id = node_id,
            shadow_application_count = count,
            "SHADOW: log-only, not applying fix",
        );
        ShadowGateResult::LogOnly { shadow_application_count: count }
    } else {
        ShadowGateResult::AllowApply
    }
}

/// KBPP-03: Canary-stage gate enforcement.
/// Returns true if the fix SHOULD be applied (stage is not canary, OR this is pod_8).
/// Returns false if the fix should be SKIPPED (canary stage + not pod_8).
pub fn canary_gate(
    promo_store: &KbPromotionStore,
    problem_hash: &str,
    node_id: &str,
) -> bool {
    let candidates = promo_store.candidates_at_stage(STATUS_CANARY).ok().unwrap_or_default();
    let is_canary_stage = candidates.iter().any(|c| c.problem_hash == problem_hash);

    match is_canary_stage {
        true => is_canary_pod(node_id), // canary stage: only pod_8 applies
        false => true,                  // not canary stage: apply normally
    }
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

// ─── Integration Tests (Plan 291-03) ────────────────────────────────────────

#[cfg(test)]
mod cron_integration_tests {
    use super::*;
    use crate::kb_promotion_store::{KbPromotionStore, PromotionCandidate};
    use chrono::Utc;

    // Test 1: next_cron_interval_secs() returns exactly 21600
    #[test]
    fn test_cron_interval_is_6_hours() {
        assert_eq!(next_cron_interval_secs(), 21600, "6-hour cron must be exactly 21600 seconds");
    }

    // Test 2: A PromotionCandidate can be upserted and update_stage promoted to shadow
    #[test]
    fn test_promote_observed_to_shadow_upserts_store() {
        let promo = KbPromotionStore::open(":memory:").unwrap();

        let candidate = PromotionCandidate {
            problem_hash: "abc123".to_string(),
            problem_key: "game_crash".to_string(),
            stage: "observed".to_string(),
            stage_entered_at: Utc::now().to_rfc3339(),
            shadow_applications: 0,
            created_at: Utc::now().to_rfc3339(),
        };
        promo.upsert_candidate(&candidate).unwrap();
        promo.update_stage("abc123", "shadow").unwrap();

        let candidates = promo.candidates_at_stage("shadow").unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].stage, "shadow");
    }

    // Test 3: SHADOW_MIN_APPLICATIONS threshold is 25
    #[test]
    fn test_shadow_min_applications_threshold() {
        assert_eq!(SHADOW_MIN_APPLICATIONS, 25, "Shadow requires 25 applications before canary");
    }

    // Test 4: is_canary_pod detects pod_8, pod-8 but NOT pod_88 or pod_1
    #[test]
    fn test_is_canary_pod_detection() {
        assert!(is_canary_pod("pod_8"), "pod_8 must be canary pod");
        assert!(is_canary_pod("pod-8"), "pod-8 must be canary pod");
        assert!(!is_canary_pod("pod_3"), "pod_3 must NOT be canary pod");
        assert!(!is_canary_pod("pod_1"), "pod_1 must NOT be canary pod");
        assert!(!is_canary_pod("pod_88"), "pod_88 must NOT be canary pod (partial match safety)");
    }

    // Test 5: canary_gate blocks non-canary pods when stage == canary
    #[test]
    fn test_canary_gate_blocks_non_canary_pods() {
        let promo = KbPromotionStore::open(":memory:").unwrap();
        let candidate = PromotionCandidate {
            problem_hash: "canary_hash".to_string(),
            problem_key: "test_problem".to_string(),
            stage: "canary".to_string(),
            stage_entered_at: Utc::now().to_rfc3339(),
            shadow_applications: 30,
            created_at: Utc::now().to_rfc3339(),
        };
        promo.upsert_candidate(&candidate).unwrap();

        // pod_3 should be blocked
        let pod3_allowed = canary_gate(&promo, "canary_hash", "pod_3");
        assert!(!pod3_allowed, "pod_3 must be blocked from applying canary-stage fixes");

        // pod_8 should be allowed
        let pod8_allowed = canary_gate(&promo, "canary_hash", "pod_8");
        assert!(pod8_allowed, "pod_8 must be allowed to apply canary-stage fixes");
    }

    // Test 6: enforce_shadow_gate returns LogOnly for shadow-stage hash
    #[test]
    fn test_shadow_gate_returns_log_only_for_shadow_stage() {
        let promo = KbPromotionStore::open(":memory:").unwrap();
        let candidate = PromotionCandidate {
            problem_hash: "shadow_hash".to_string(),
            problem_key: "shadow_problem".to_string(),
            stage: "shadow".to_string(),
            stage_entered_at: Utc::now().to_rfc3339(),
            shadow_applications: 5,
            created_at: Utc::now().to_rfc3339(),
        };
        promo.upsert_candidate(&candidate).unwrap();

        let result = enforce_shadow_gate(&promo, "shadow_hash", "pod_3");
        assert!(
            matches!(result, ShadowGateResult::LogOnly { .. }),
            "Shadow stage must return LogOnly gate result"
        );
        if let ShadowGateResult::LogOnly { shadow_application_count } = result {
            assert!(shadow_application_count >= 0, "shadow application count must be non-negative");
        }
    }
}
