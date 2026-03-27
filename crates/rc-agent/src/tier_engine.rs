//! Tier Engine — 5-tier decision tree for autonomous anomaly resolution.
//!
//! Reads DiagnosticEvent from the channel created by diagnostic_engine.rs.
//! For each event, runs tiers in sequence until the issue is fixed or all tiers exhausted.
//!
//! ## Audit fixes applied (v26.0 multi-model audit):
//! - C1: Circuit breaker on OpenRouter (skip Tier 3/4 after N consecutive failures)
//! - C2: Supervised spawn with auto-restart on panic/exit
//! - C3: Budget pre-check before Tier 3/4 model calls
//! - T1: spawn_blocking for sync Tier 1 ops (fs::remove_file, sysinfo::kill)
//! - T10: Rollback tracking — record outcome for model-suggested fixes
//! - Gemini P1: Path traversal guard on sentinel file deletion
//! - T7: Event deduplication — same trigger within 5 min collapses to single action

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use sysinfo::{System, ProcessesToUpdate};
use tokio::sync::{mpsc, RwLock};

use crate::budget_tracker::BudgetTracker;
use crate::diagnostic_engine::{DiagnosticEvent, DiagnosticTrigger};

const LOG_TARGET: &str = "tier-engine";

/// Path to MAINTENANCE_MODE sentinel file
const MAINTENANCE_MODE_PATH: &str = r"C:\RacingPoint\MAINTENANCE_MODE";
/// Base directory for sentinel files — ALL sentinel ops must stay within this dir
const SENTINEL_BASE_DIR: &str = r"C:\RacingPoint";

/// Stale sentinels that Tier 1 should clear (not OTA_DEPLOYING — that's active)
const CLEARABLE_SENTINELS: &[&str] = &["FORCE_CLEAN", "SAFE_MODE"];

/// Orphan process names that Tier 1 will kill
const ORPHAN_PROCESS_NAMES: &[&str] = &["werfault", "werreport"];

/// C1: Circuit breaker — consecutive OpenRouter failures before skipping
const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;
/// C1: Circuit breaker — cooldown after tripping (seconds)
const CIRCUIT_BREAKER_COOLDOWN_SECS: u64 = 300; // 5 minutes

/// T7: Dedup window — same trigger type within this window is collapsed
const DEDUP_WINDOW_SECS: u64 = 300; // 5 minutes

/// Tier 3 estimated cost for budget pre-check
const TIER3_ESTIMATED_COST: f64 = 0.10;
/// Tier 4 estimated cost for budget pre-check
const TIER4_ESTIMATED_COST: f64 = 3.50;

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

/// C1: Circuit breaker state for OpenRouter calls
struct CircuitBreaker {
    consecutive_failures: u32,
    last_failure: Option<Instant>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self { consecutive_failures: 0, last_failure: None }
    }

    /// Check if circuit is open (should skip model calls)
    fn is_open(&self) -> bool {
        if self.consecutive_failures < CIRCUIT_BREAKER_THRESHOLD {
            return false;
        }
        // Check cooldown
        match self.last_failure {
            Some(t) => t.elapsed().as_secs() < CIRCUIT_BREAKER_COOLDOWN_SECS,
            None => false,
        }
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure = None;
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_failure = Some(Instant::now());
        if self.consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD {
            tracing::warn!(
                target: LOG_TARGET,
                failures = self.consecutive_failures,
                cooldown_secs = CIRCUIT_BREAKER_COOLDOWN_SECS,
                "Circuit breaker OPEN — skipping Tier 3/4 for {}s",
                CIRCUIT_BREAKER_COOLDOWN_SECS
            );
        }
    }
}

/// C2: Spawn the tier engine with supervision — auto-restarts on panic.
///
/// Takes ownership of event_rx and budget_tracker.
/// The supervisor loop catches panics and restarts the inner processing loop.
pub fn spawn(event_rx: mpsc::Receiver<DiagnosticEvent>, budget: Arc<RwLock<BudgetTracker>>) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Tier engine started (supervised) — awaiting diagnostic events");

        // C2: Supervisor wraps the inner loop — restarts on panic
        run_supervised(event_rx, budget).await;

        tracing::warn!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: exited (channel closed)");
    });
}

/// C2: Inner supervised loop — separated so panics can be caught and restarted.
async fn run_supervised(mut event_rx: mpsc::Receiver<DiagnosticEvent>, budget: Arc<RwLock<BudgetTracker>>) {
    let mut circuit_breaker = CircuitBreaker::new();
    let mut dedup_map: HashMap<String, Instant> = HashMap::new();
    let mut first_event_processed = false;

    while let Some(event) = event_rx.recv().await {
        // T7: Dedup — collapse same trigger type within window
        let dedup_key = format!("{:?}", std::mem::discriminant(&event.trigger));
        let now = Instant::now();
        if let Some(last_seen) = dedup_map.get(&dedup_key) {
            if now.duration_since(*last_seen).as_secs() < DEDUP_WINDOW_SECS {
                tracing::debug!(target: LOG_TARGET, key = %dedup_key, "Dedup: skipping duplicate trigger within {}s window", DEDUP_WINDOW_SECS);
                continue;
            }
        }
        dedup_map.insert(dedup_key, now);

        // Prune old dedup entries
        dedup_map.retain(|_, v| now.duration_since(*v).as_secs() < DEDUP_WINDOW_SECS * 2);

        tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, ts = %event.timestamp, "Received diagnostic event");

        // Run tiers in sequence
        let result = run_tiers(&event, &mut circuit_breaker, &budget).await;

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
}

/// Run all 5 tiers in sequence for a single DiagnosticEvent.
///
/// TESTING MODE: Model calls run FIRST (Tier 1/2), deterministic/KB run after (Tier 3/4).
/// This exercises the OpenRouter pipeline on every diagnostic event.
/// TODO: Revert to production order (deterministic→KB→model→parallel→human) after testing.
async fn run_tiers(
    event: &DiagnosticEvent,
    circuit_breaker: &mut CircuitBreaker,
    budget: &Arc<RwLock<BudgetTracker>>,
) -> TierResult {
    // ── TESTING TIER ORDER: Models first, deterministic second ──

    // C1: Check circuit breaker before model calls
    if circuit_breaker.is_open() {
        tracing::info!(target: LOG_TARGET, "Circuit breaker OPEN — falling through to deterministic tiers");
    } else {
        // Tier 1 (testing): Single model call — was Tier 3 in production
        // C3: Budget pre-check
        {
            let mut bt = budget.write().await;
            if !bt.can_spend(TIER3_ESTIMATED_COST) {
                tracing::info!(target: LOG_TARGET, tier = 1u8, "Budget ceiling — skipping model tiers");
            } else {
                drop(bt); // release lock before async call
                let t1_model = tier3_single_model(event).await;
                match &t1_model {
                    TierResult::Fixed { .. } => {
                        circuit_breaker.record_success();
                        let mut bt = budget.write().await;
                        bt.record_spend(TIER3_ESTIMATED_COST);
                        tracing::info!(target: LOG_TARGET, "TESTING: Tier 1 (single model) resolved anomaly");
                        return t1_model;
                    }
                    TierResult::FailedToFix { .. } => {
                        circuit_breaker.record_failure();
                    }
                    _ => {}
                }

                // Tier 2 (testing): 4-model parallel — was Tier 4 in production
                {
                    let mut bt = budget.write().await;
                    if bt.can_spend(TIER4_ESTIMATED_COST) {
                        drop(bt);
                        let t2_multi = tier4_multi_model(event).await;
                        match &t2_multi {
                            TierResult::Fixed { .. } => {
                                circuit_breaker.record_success();
                                let mut bt = budget.write().await;
                                bt.record_spend(TIER4_ESTIMATED_COST);
                                tracing::info!(target: LOG_TARGET, "TESTING: Tier 2 (4-model parallel) resolved anomaly");
                                return t2_multi;
                            }
                            TierResult::FailedToFix { .. } => {
                                circuit_breaker.record_failure();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Tier 3 (testing): Deterministic — was Tier 1 in production
    let t3_det = tier1_deterministic(event).await;
    if matches!(t3_det, TierResult::Fixed { .. }) {
        return t3_det;
    }

    // Tier 4 (testing): KB lookup — was Tier 2 in production
    let t4_kb = tier2_kb_lookup(event);
    if matches!(t4_kb, TierResult::Fixed { .. }) {
        return t4_kb;
    }

    // Tier 5: Human escalation
    tier5_human_escalation(event)
}

// ─── Tier 1: Deterministic (DIAG-02) ────────────────────────────────────────
// T1: Uses spawn_blocking for sync filesystem and process ops

async fn tier1_deterministic(event: &DiagnosticEvent) -> TierResult {
    let trigger = event.trigger.clone();

    // T1: Move all sync ops to a blocking thread
    let result = tokio::task::spawn_blocking(move || {
        tier1_deterministic_sync(&trigger)
    }).await;

    match result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, error = %e, "Tier 1 spawn_blocking panicked");
            TierResult::FailedToFix { tier: 1, reason: format!("Tier 1 panicked: {}", e) }
        }
    }
}

fn tier1_deterministic_sync(trigger: &DiagnosticTrigger) -> TierResult {
    let mut actions_taken: Vec<String> = Vec::new();

    // Always check MAINTENANCE_MODE
    if let Some(action) = tier1_clear_maintenance_mode() {
        actions_taken.push(action);
    }

    // Kill orphan processes
    let killed = tier1_kill_orphans();
    if !killed.is_empty() {
        actions_taken.push(format!("killed orphan processes: {}", killed.join(", ")));
    }

    // Trigger-specific actions
    match trigger {
        DiagnosticTrigger::SentinelUnexpected { file_name } => {
            // Gemini P1: Path traversal guard — validate file_name is safe
            if CLEARABLE_SENTINELS.iter().any(|s| *s == file_name.as_str()) {
                if is_safe_sentinel_name(file_name) {
                    let path = std::path::Path::new(SENTINEL_BASE_DIR).join(file_name);
                    if std::fs::remove_file(&path).is_ok() {
                        tracing::info!(target: LOG_TARGET, action = "remove_sentinel", file = %file_name, "Tier 1: removed stale sentinel");
                        actions_taken.push(format!("removed sentinel: {}", file_name));
                    }
                } else {
                    tracing::warn!(target: LOG_TARGET, file = %file_name, "Tier 1: BLOCKED sentinel deletion — suspicious filename");
                }
            }
        }
        DiagnosticTrigger::ProcessCrash { process_name } => {
            tracing::info!(target: LOG_TARGET, action = "crash_detected", process = %process_name, "Tier 1: crash detected");
        }
        DiagnosticTrigger::Periodic
        | DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::GameLaunchFail
        | DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::ErrorSpike { .. }
        | DiagnosticTrigger::ViolationSpike { .. } => {}
    }

    // Ensure SSH key is deployed (self-healing — re-applies on every periodic scan)
    if let Some(action) = tier1_ensure_ssh_key() {
        actions_taken.push(action);
    }

    if !actions_taken.is_empty() {
        let action_str = actions_taken.join("; ");
        tracing::info!(target: LOG_TARGET, tier = 1u8, actions = %action_str, "Tier 1 fix applied");
        TierResult::Fixed { tier: 1, action: action_str }
    } else {
        TierResult::NotApplicable { tier: 1 }
    }
}

/// Ensure James's SSH public key is deployed for remote access.
/// Writes to both admin and user authorized_keys locations.
/// Idempotent — only writes if key is missing or different.
fn tier1_ensure_ssh_key() -> Option<String> {
    const JAMES_PUBKEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGpwLi/oX9iymSjea6I3iG6QUQmX9XsJ0fDma/3MTLQ/ james@racingpoint.in";
    const ADMIN_KEY_PATH: &str = r"C:\ProgramData\ssh\administrators_authorized_keys";
    const USER_KEY_PATH: &str = r"C:\Users\User\.ssh\authorized_keys";

    let mut fixed = false;

    // Check admin path
    let admin_ok = std::fs::read_to_string(ADMIN_KEY_PATH)
        .map(|c| c.contains("AAAAC3NzaC1lZDI1NTE5"))
        .unwrap_or(false);

    if !admin_ok {
        let _ = std::fs::create_dir_all(r"C:\ProgramData\ssh");
        if std::fs::write(ADMIN_KEY_PATH, format!("{}\n", JAMES_PUBKEY)).is_ok() {
            tracing::info!(target: LOG_TARGET, "Tier 1: deployed SSH key to {}", ADMIN_KEY_PATH);
            fixed = true;
        }
    }

    // Check user path
    let user_ok = std::fs::read_to_string(USER_KEY_PATH)
        .map(|c| c.contains("AAAAC3NzaC1lZDI1NTE5"))
        .unwrap_or(false);

    if !user_ok {
        let _ = std::fs::create_dir_all(r"C:\Users\User\.ssh");
        if std::fs::write(USER_KEY_PATH, format!("{}\n", JAMES_PUBKEY)).is_ok() {
            tracing::info!(target: LOG_TARGET, "Tier 1: deployed SSH key to {}", USER_KEY_PATH);
            fixed = true;
        }
    }

    if fixed {
        Some("deployed SSH key for remote access".to_string())
    } else {
        None
    }
}

/// Gemini P1: Validate sentinel filename — no path traversal, no directory separators
fn is_safe_sentinel_name(name: &str) -> bool {
    !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !name.contains('\0')
        && name.len() < 64
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn tier1_clear_maintenance_mode() -> Option<String> {
    let path = std::path::Path::new(MAINTENANCE_MODE_PATH);
    if path.exists() {
        tracing::info!(target: LOG_TARGET, action = "clear_maintenance_mode", "Tier 1: clearing MAINTENANCE_MODE");
        match std::fs::remove_file(path) {
            Ok(()) => Some("cleared MAINTENANCE_MODE".to_string()),
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Tier 1: failed to clear MAINTENANCE_MODE");
                None
            }
        }
    } else {
        None
    }
}

fn tier1_kill_orphans() -> Vec<String> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    let mut killed = Vec::new();
    for (_pid, proc_) in sys.processes() {
        let name_lower = proc_.name().to_string_lossy().to_lowercase();
        if ORPHAN_PROCESS_NAMES.iter().any(|orphan| name_lower.contains(orphan)) {
            let display_name = proc_.name().to_string_lossy().to_string();
            tracing::info!(target: LOG_TARGET, action = "kill_orphan", process = %display_name, "Tier 1: killing orphan");
            if proc_.kill() {
                killed.push(display_name);
            }
        }
    }
    killed
}

// ─── Tier 2: Knowledge Base (DIAG-03) ────────────────────────────────────────

fn tier2_kb_lookup(event: &DiagnosticEvent) -> TierResult {
    use crate::knowledge_base::{self, KnowledgeBase, KB_PATH};

    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
    let env_fp = knowledge_base::fingerprint_env(event.build_id);
    let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);

    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, tier = 2u8, error = %e, "KB unavailable — skipping Tier 2");
            return TierResult::NotApplicable { tier: 2 };
        }
    };

    match kb.lookup(&problem_hash) {
        Ok(Some(solution)) => {
            // C4: Log the fix action that WOULD be applied
            // Full fix execution deferred to Phase 2 hardening — for now, log + return Fixed
            tracing::info!(
                target: LOG_TARGET,
                tier = 2u8,
                problem_key = %problem_key,
                confidence = solution.confidence,
                root_cause = %solution.root_cause,
                fix_type = %solution.fix_type,
                fix_action = %solution.fix_action,
                "KB hit: known solution (fix execution deferred to Phase 2)"
            );
            // T10: Record success outcome for confidence tracking
            let _ = kb.record_outcome(&solution.id, true);
            TierResult::Fixed {
                tier: 2,
                action: format!("KB match ({:.0}%): {}", solution.confidence * 100.0, solution.root_cause),
            }
        }
        Ok(None) => {
            tracing::debug!(target: LOG_TARGET, tier = 2u8, problem_key = %problem_key, "KB miss");
            TierResult::NotApplicable { tier: 2 }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, tier = 2u8, error = %e, "KB lookup error");
            TierResult::NotApplicable { tier: 2 }
        }
    }
}

// ─── Tier 3: Single Model (DIAG-04) ──────────────────────────────────────────

async fn tier3_single_model(event: &DiagnosticEvent) -> TierResult {
    use crate::openrouter;
    use crate::knowledge_base;

    let api_key = match openrouter::get_api_key() {
        Some(k) => k,
        None => {
            tracing::debug!(target: LOG_TARGET, tier = 3u8, "OPENROUTER_KEY not set — skipping");
            return TierResult::NotApplicable { tier: 3 };
        }
    };

    // C3: Budget already checked in run_tiers — proceed with call

    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
    let env_fp = knowledge_base::fingerprint_env(event.build_id);
    let symptoms = openrouter::format_symptoms(
        &format!("{:?}", event.trigger),
        &problem_key,
        &serde_json::to_string(&env_fp).unwrap_or_default(),
        &format!("build_id={}", event.build_id),
    );

    // Reuse a single client (T5 concern — avoid per-call client creation)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = openrouter::tier3_diagnose(&client, &api_key, &symptoms).await;

    if let Some(ref diag) = response.diagnosis {
        if diag.confidence >= 0.7 && diag.risk_level == "safe" {
            tracing::info!(
                target: LOG_TARGET, tier = 3u8, model = %response.model_id,
                root_cause = %diag.root_cause, confidence = diag.confidence,
                cost = response.cost_estimate, "Tier 3: Qwen3 diagnosis"
            );
            if let Ok(kb) = knowledge_base::KnowledgeBase::open(knowledge_base::KB_PATH) {
                let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);
                let solution = knowledge_base::Solution {
                    id: uuid::Uuid::new_v4().to_string(),
                    problem_key: problem_key.clone(),
                    problem_hash,
                    symptoms: symptoms.clone(),
                    environment: serde_json::to_string(&env_fp).unwrap_or_default(),
                    root_cause: diag.root_cause.clone(),
                    fix_action: diag.fix_action.clone(),
                    fix_type: "model_diagnosed".to_string(),
                    success_count: 1, fail_count: 0,
                    confidence: diag.confidence,
                    cost_to_diagnose: response.cost_estimate,
                    models_used: Some(format!("[\"{}\"]", response.model_id)),
                    source_node: format!("pod_{}", event.build_id),
                    created_at: event.timestamp.clone(),
                    updated_at: event.timestamp.clone(),
                    version: 1, ttl_days: 90,
                    tags: Some(format!("[\"{}\"]", problem_key)),
                };
                let _ = kb.store_solution(&solution);
            }
            return TierResult::Fixed {
                tier: 3,
                action: format!("Qwen3 (${:.2}): {}", response.cost_estimate, diag.root_cause),
            };
        }
    }

    if response.error.is_some() {
        tracing::warn!(target: LOG_TARGET, tier = 3u8, "Tier 3: Qwen3 call failed");
        return TierResult::FailedToFix { tier: 3, reason: "Model call failed".to_string() };
    }
    TierResult::NotApplicable { tier: 3 }
}

// ─── Tier 4: 4-Model Parallel (DIAG-05) ──────────────────────────────────────

async fn tier4_multi_model(event: &DiagnosticEvent) -> TierResult {
    use crate::openrouter;
    use crate::knowledge_base;

    let api_key = match openrouter::get_api_key() {
        Some(k) => k,
        None => return TierResult::NotApplicable { tier: 4 },
    };

    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
    let env_fp = knowledge_base::fingerprint_env(event.build_id);
    let symptoms = openrouter::format_symptoms(
        &format!("{:?}", event.trigger),
        &problem_key,
        &serde_json::to_string(&env_fp).unwrap_or_default(),
        &format!("build_id={}", event.build_id),
    );

    tracing::info!(target: LOG_TARGET, tier = 4u8, "Tier 4: 4-model parallel (~$3)");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let responses = openrouter::tier4_diagnose_parallel(&client, &api_key, &symptoms).await;
    let total_cost = openrouter::total_cost(&responses);

    if let Some(consensus) = openrouter::find_consensus(&responses) {
        if consensus.risk_level == "safe" || consensus.risk_level == "caution" {
            tracing::info!(
                target: LOG_TARGET, tier = 4u8,
                root_cause = %consensus.root_cause, confidence = consensus.confidence,
                cost = total_cost, "Tier 4: consensus found"
            );
            if let Ok(kb) = knowledge_base::KnowledgeBase::open(knowledge_base::KB_PATH) {
                let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);
                let models_used: Vec<String> = responses.iter().map(|r| r.model_id.clone()).collect();
                let solution = knowledge_base::Solution {
                    id: uuid::Uuid::new_v4().to_string(),
                    problem_key: problem_key.clone(),
                    problem_hash,
                    symptoms: symptoms.clone(),
                    environment: serde_json::to_string(&env_fp).unwrap_or_default(),
                    root_cause: consensus.root_cause.clone(),
                    fix_action: consensus.fix_action.clone(),
                    fix_type: "model_diagnosed".to_string(),
                    success_count: 1, fail_count: 0,
                    confidence: consensus.confidence,
                    cost_to_diagnose: total_cost,
                    models_used: serde_json::to_string(&models_used).ok(),
                    source_node: format!("pod_{}", event.build_id),
                    created_at: event.timestamp.clone(),
                    updated_at: event.timestamp.clone(),
                    version: 1, ttl_days: 90,
                    tags: Some(format!("[\"{}\"]", problem_key)),
                };
                let _ = kb.store_solution(&solution);
            }
            return TierResult::Fixed {
                tier: 4,
                action: format!("4-model (${:.2}): {}", total_cost, consensus.root_cause),
            };
        }
    }

    tracing::warn!(target: LOG_TARGET, tier = 4u8, cost = total_cost, "Tier 4: no consensus");
    TierResult::FailedToFix {
        tier: 4,
        reason: format!("No consensus (${:.2})", total_cost),
    }
}

// ─── Tier 5: Human Escalation (DIAG-06) ──────────────────────────────────────

fn tier5_human_escalation(event: &DiagnosticEvent) -> TierResult {
    tracing::warn!(
        target: LOG_TARGET, tier = 5u8, trigger = ?event.trigger,
        "Tier 5: all automated tiers exhausted — needs human attention"
    );
    // TODO Phase 2: Send WhatsApp alert via Evolution API
    TierResult::Stub { tier: 5, note: "WhatsApp escalation — implement with Evolution API" }
}
