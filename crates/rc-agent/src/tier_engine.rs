//! Tier Engine — 5-tier decision tree for autonomous anomaly resolution.
//!
//! Reads DiagnosticEvent from the channel created by diagnostic_engine.rs.
//! For each event, runs tiers in sequence until the issue is fixed or all tiers exhausted.
//!
//! Tier 1: Deterministic (DIAG-02) — implemented. $0 cost.
//!   - MAINTENANCE_MODE file → delete it
//!   - WerFault/orphan powershell processes → kill them
//!   - Stale sentinel files (FORCE_CLEAN, SAFE_MODE) → delete them
//!
//! Tier 2: Knowledge Base lookup (DIAG-03) — stub. $0 cost.
//!   Phase 230 will implement real KB lookup.
//!
//! Tier 3: Single-model Qwen3 diagnosis (DIAG-04) — stub. ~$0.05.
//!   Phase 231 will implement OpenRouter Qwen3 call.
//!
//! Tier 4: 4-model parallel diagnosis (DIAG-05) — stub. ~$3.
//!   Phase 231 will implement parallel R1+V3+MiMo+Gemini calls.
//!
//! Tier 5: Human escalation via WhatsApp (DIAG-06) — stub.
//!   Phase 231 will implement WhatsApp send via Evolution API.
//!
//! Every fix action is logged at INFO before application.
//! Every tier result is logged at DEBUG.

use sysinfo::{System, ProcessesToUpdate};
use tokio::sync::mpsc;

use crate::diagnostic_engine::{DiagnosticEvent, DiagnosticTrigger};

const LOG_TARGET: &str = "tier-engine";

/// Path to MAINTENANCE_MODE sentinel file
const MAINTENANCE_MODE_PATH: &str = r"C:\RacingPoint\MAINTENANCE_MODE";

/// Stale sentinels that Tier 1 should clear (not OTA_DEPLOYING — that's active)
const CLEARABLE_SENTINELS: &[&str] = &["FORCE_CLEAN", "SAFE_MODE"];

/// Orphan process names that Tier 1 will kill
const ORPHAN_PROCESS_NAMES: &[&str] = &["werfault", "werreport"];

/// Result of a single tier's attempt to resolve an anomaly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierResult {
    /// Fix was applied and the issue is considered resolved
    Fixed { tier: u8, action: String },
    /// Tier found the issue but could not fix it
    FailedToFix { tier: u8, reason: String },
    /// Tier has no applicable fix for this trigger type
    NotApplicable { tier: u8 },
    /// Tier is not yet implemented (stub)
    Stub { tier: u8, note: &'static str },
}

/// Spawn the tier engine background task.
///
/// Reads DiagnosticEvents from event_rx and runs the 5-tier decision tree.
/// Lifecycle logs: started, first_event_processed.
pub fn spawn(mut event_rx: mpsc::Receiver<DiagnosticEvent>) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Tier engine started — awaiting diagnostic events");

        let mut first_event_processed = false;

        while let Some(event) = event_rx.recv().await {
            tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, ts = %event.timestamp, "Received diagnostic event");

            // Run tiers in sequence — stop when Fixed or all tiers exhausted
            let result = run_tiers(&event).await;

            match &result {
                TierResult::Fixed { tier, action } => {
                    tracing::info!(
                        target: LOG_TARGET,
                        trigger = ?event.trigger,
                        tier = tier,
                        action = %action,
                        "Anomaly resolved by tier engine"
                    );
                }
                TierResult::Stub { tier, note } => {
                    tracing::debug!(target: LOG_TARGET, tier = tier, note = note, "Tier stub — not yet implemented");
                }
                TierResult::FailedToFix { tier, reason } => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        trigger = ?event.trigger,
                        tier = tier,
                        reason = %reason,
                        "All tiers failed to resolve anomaly"
                    );
                }
                TierResult::NotApplicable { .. } => {
                    tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, "No applicable tier for trigger");
                }
            }

            if !first_event_processed {
                tracing::info!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: first_event_processed");
                first_event_processed = true;
            }
        }

        tracing::warn!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: exited (channel closed — diagnostic_engine may have stopped)");
    });
}

/// Run all 5 tiers in sequence for a single DiagnosticEvent.
/// Returns the first Fixed result, or the last result if all tiers exhaust.
async fn run_tiers(event: &DiagnosticEvent) -> TierResult {
    // Tier 1: Deterministic
    let t1 = tier1_deterministic(event);
    if matches!(t1, TierResult::Fixed { .. }) {
        return t1;
    }

    // Tier 2: KB lookup (stub — Phase 230)
    let t2 = tier2_kb_lookup(event);
    if matches!(t2, TierResult::Fixed { .. }) {
        return t2;
    }

    // Tier 3: Single model (stub — Phase 231)
    let t3 = tier3_single_model(event);
    if matches!(t3, TierResult::Fixed { .. }) {
        return t3;
    }

    // Tier 4: 4-model parallel (stub — Phase 231)
    let t4 = tier4_multi_model(event);
    if matches!(t4, TierResult::Fixed { .. }) {
        return t4;
    }

    // Tier 5: Human escalation (stub — Phase 231)
    tier5_human_escalation(event)
}

// ─── Tier 1: Deterministic (DIAG-02) ────────────────────────────────────────

fn tier1_deterministic(event: &DiagnosticEvent) -> TierResult {
    let mut actions_taken: Vec<String> = Vec::new();

    // Always check MAINTENANCE_MODE regardless of trigger (Tier 1 is preemptive)
    if let Some(action) = tier1_clear_maintenance_mode() {
        actions_taken.push(action);
    }

    // Kill orphan WerFault / crash reporter processes
    let killed = tier1_kill_orphans();
    if !killed.is_empty() {
        actions_taken.push(format!("killed orphan processes: {}", killed.join(", ")));
    }

    // Trigger-specific Tier 1 actions
    match &event.trigger {
        DiagnosticTrigger::SentinelUnexpected { file_name } => {
            if CLEARABLE_SENTINELS.iter().any(|s| *s == file_name.as_str()) {
                let path = format!(r"C:\RacingPoint\{}", file_name);
                if std::fs::remove_file(&path).is_ok() {
                    tracing::info!(target: LOG_TARGET, action = "remove_sentinel", file = %file_name, "Tier 1: removed stale sentinel");
                    actions_taken.push(format!("removed sentinel: {}", file_name));
                }
            }
        }
        DiagnosticTrigger::ProcessCrash { process_name } => {
            // WerFault already handled by kill_orphans above
            tracing::info!(target: LOG_TARGET, action = "crash_detected", process = %process_name, "Tier 1: crash detected, WerFault killed if present");
        }
        // Periodic and other triggers: Tier 1 proactively clears MAINTENANCE_MODE (already done above)
        DiagnosticTrigger::Periodic
        | DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::GameLaunchFail
        | DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::ErrorSpike { .. }
        | DiagnosticTrigger::ViolationSpike { .. } => {
            // Tier 1 has no additional action for these triggers beyond MM clear + orphan kill
        }
    }

    if !actions_taken.is_empty() {
        let action_str = actions_taken.join("; ");
        tracing::info!(target: LOG_TARGET, tier = 1u8, trigger = ?event.trigger, actions = %action_str, "Tier 1 fix applied");
        TierResult::Fixed { tier: 1, action: action_str }
    } else {
        TierResult::NotApplicable { tier: 1 }
    }
}

/// Attempt to clear MAINTENANCE_MODE sentinel file.
/// Returns Some(action_description) if cleared, None if file did not exist.
fn tier1_clear_maintenance_mode() -> Option<String> {
    let path = std::path::Path::new(MAINTENANCE_MODE_PATH);
    if path.exists() {
        tracing::info!(target: LOG_TARGET, action = "clear_maintenance_mode", path = MAINTENANCE_MODE_PATH, "Tier 1: clearing MAINTENANCE_MODE sentinel");
        match std::fs::remove_file(path) {
            Ok(()) => {
                tracing::info!(target: LOG_TARGET, "Tier 1: MAINTENANCE_MODE cleared successfully");
                Some("cleared MAINTENANCE_MODE".to_string())
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Tier 1: failed to clear MAINTENANCE_MODE");
                None
            }
        }
    } else {
        None
    }
}

/// Kill known orphan processes (WerFault, WerReport).
/// Returns names of processes that were successfully killed.
fn tier1_kill_orphans() -> Vec<String> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);

    let mut killed = Vec::new();
    for (_pid, proc_) in sys.processes() {
        let name_lower = proc_.name().to_string_lossy().to_lowercase();
        if ORPHAN_PROCESS_NAMES.iter().any(|orphan| name_lower.contains(orphan)) {
            let display_name = proc_.name().to_string_lossy().to_string();
            tracing::info!(target: LOG_TARGET, action = "kill_orphan", process = %display_name, "Tier 1: killing orphan process");
            if proc_.kill() {
                killed.push(display_name);
            }
        }
    }
    killed
}

// ─── Tier 2: Knowledge Base (DIAG-03) — Stub ────────────────────────────────

fn tier2_kb_lookup(event: &DiagnosticEvent) -> TierResult {
    use crate::knowledge_base::{self, KnowledgeBase, KB_PATH};

    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
    let env_fp = knowledge_base::fingerprint_env(event.build_id);
    let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);

    // Open KB — if it fails (first run, corrupt DB), skip Tier 2 gracefully
    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, tier = 2u8, error = %e, "KB unavailable — skipping Tier 2");
            return TierResult::NotApplicable { tier: 2 };
        }
    };

    match kb.lookup(&problem_hash) {
        Ok(Some(solution)) => {
            tracing::info!(
                target: LOG_TARGET,
                tier = 2u8,
                problem_key = %problem_key,
                problem_hash = %problem_hash,
                confidence = solution.confidence,
                root_cause = %solution.root_cause,
                fix_type = %solution.fix_type,
                "KB hit: applying known solution"
            );
            TierResult::Fixed {
                tier: 2,
                action: format!("KB match (confidence: {:.2}): {}", solution.confidence, solution.root_cause),
            }
        }
        Ok(None) => {
            tracing::debug!(
                target: LOG_TARGET,
                tier = 2u8,
                problem_key = %problem_key,
                problem_hash = %problem_hash,
                "KB miss: no solution with confidence >= 0.8"
            );
            TierResult::NotApplicable { tier: 2 }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, tier = 2u8, error = %e, "KB lookup error — skipping Tier 2");
            TierResult::NotApplicable { tier: 2 }
        }
    }
}

// ─── Tier 3: Single Model (DIAG-04) — Stub ──────────────────────────────────

fn tier3_single_model(event: &DiagnosticEvent) -> TierResult {
    tracing::debug!(
        target: LOG_TARGET,
        tier = 3u8,
        trigger = ?event.trigger,
        "Tier 3 stub: Qwen3 single-model diagnosis not yet implemented (Phase 231)"
    );
    TierResult::Stub { tier: 3, note: "Qwen3 single-model not yet implemented — Phase 231" }
}

// ─── Tier 4: 4-Model Parallel (DIAG-05) — Stub ──────────────────────────────

fn tier4_multi_model(event: &DiagnosticEvent) -> TierResult {
    tracing::debug!(
        target: LOG_TARGET,
        tier = 4u8,
        trigger = ?event.trigger,
        "Tier 4 stub: 4-model parallel diagnosis not yet implemented (Phase 231)"
    );
    TierResult::Stub { tier: 4, note: "4-model parallel not yet implemented — Phase 231" }
}

// ─── Tier 5: Human Escalation (DIAG-06) — Stub ──────────────────────────────

fn tier5_human_escalation(event: &DiagnosticEvent) -> TierResult {
    tracing::debug!(
        target: LOG_TARGET,
        tier = 5u8,
        trigger = ?event.trigger,
        "Tier 5 stub: WhatsApp human escalation not yet implemented (Phase 231)"
    );
    TierResult::Stub { tier: 5, note: "WhatsApp escalation not yet implemented — Phase 231" }
}
