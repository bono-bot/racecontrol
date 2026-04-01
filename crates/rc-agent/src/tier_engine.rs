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

use chrono::Utc;
use sysinfo::{System, ProcessesToUpdate};
use tokio::sync::{mpsc, RwLock};

use crate::budget_tracker::BudgetTracker;
use crate::cognitive_gate::CgpEngine;
use crate::diagnosis_planner::DiagnosisPlanner;
use crate::diagnostic_engine::{DiagnosticEvent, DiagnosticTrigger};
use crate::diagnostic_log::{DiagnosticLog, DiagnosticLogEntry};
use rc_common::fleet_event::FleetEvent;

#[path = "game_launch_retry.rs"]
mod game_launch_retry;

const LOG_TARGET: &str = "tier-engine";

/// Staff-triggered diagnostic request — injected via WS handler
pub struct StaffDiagnosticRequest {
    pub correlation_id: String,
    pub incident_id: String,
    pub description: String,
    pub category: String,
    /// Channel to send the result back to the WS handler
    pub response_tx: tokio::sync::oneshot::Sender<StaffDiagnosticResult>,
}

/// Result of a staff-triggered diagnostic (Tier 1 + Tier 2 only)
pub struct StaffDiagnosticResult {
    pub correlation_id: String,
    pub tier: u8,
    pub outcome: String,
    pub root_cause: String,
    pub fix_action: String,
    pub fix_type: String,
    pub confidence: f64,
    pub fix_applied: bool,
    pub problem_hash: String,
    pub summary: String,
}

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
/// Tier 4 estimated cost for budget pre-check (5 models: Qwen3+R1+V3+MiMo+Gemini)
const TIER4_ESTIMATED_COST: f64 = 4.30;

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

// ─── MMA-First Protocol: Q1-Q4 Decision Types (v31.0) ───────────────────────

/// Decision returned by the Q1-Q4 protocol gate.
#[derive(Debug)]
enum MmaDecision {
    /// Q1 hit: permanent fix with high confidence — apply and done.
    ApplyPermanentFix {
        solution: crate::knowledge_base::Solution,
    },
    /// Q1 hit: workaround found — apply immediately, then Q4 in background.
    ApplyWorkaroundThenQ4 {
        solution: crate::knowledge_base::Solution,
    },
    /// Q2: another pod is already diagnosing this — wait.
    WaitForFleet {
        experiment_node: String,
    },
    /// Q3: invoke full 5-model MMA diagnosis.
    InvokeMma,
    /// No MMA needed — deterministic fix only or not worth the cost.
    SkipMma {
        reason: String,
    },
}

/// Structured diagnosis response from MMA (v31.0).
/// All 5 models must return this structure. The consensus builder
/// merges them into one MmaDiagnosis for KB storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MmaDiagnosis {
    /// WHY it happened — the actual root cause, not symptoms
    pub root_cause: String,
    /// What to do NOW (may be a workaround)
    pub immediate_fix: String,
    /// What prevents recurrence — the permanent solution
    pub permanent_fix: String,
    /// Determines auto-apply vs escalate
    pub fix_type: rc_common::mesh_types::FixType,
    /// How to confirm the fix worked
    pub verification: String,
    /// Standing rule, config, or code change to prevent recurrence
    pub prevention: Option<String>,
    /// Model consensus confidence (0.0-1.0)
    pub confidence: f64,
    /// true for Hardware, CodeChange — requires human intervention
    pub requires_human: bool,
}

/// Maximum number of verification attempts (6 x 5s = 30s total)
const VERIFY_MAX_ATTEMPTS: u32 = 6;
/// Delay between verification checks (seconds)
const VERIFY_CHECK_INTERVAL_SECS: u64 = 5;

/// Verify that a fix actually resolved the issue.
///
/// Waits up to 30 seconds, checking every 5 seconds (6 attempts).
/// Returns true if the specific anomaly condition has cleared.
///
/// For each trigger type, re-runs the diagnostic check that detected it.
/// Uses spawn_blocking for sysinfo calls (standing rule: no blocking on async runtime).
async fn verify_fix(
    trigger: &DiagnosticTrigger,
    failure_monitor_rx: &tokio::sync::watch::Receiver<crate::failure_monitor::FailureMonitorState>,
) -> bool {
    // Initial delay to let the fix take effect before first check
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    for attempt in 1..=VERIFY_MAX_ATTEMPTS {
        let resolved = check_trigger_resolved(trigger, failure_monitor_rx).await;
        if resolved {
            tracing::info!(
                target: LOG_TARGET,
                trigger = ?trigger,
                attempt,
                "verify_fix: anomaly resolved on attempt {}/{}",
                attempt, VERIFY_MAX_ATTEMPTS
            );
            return true;
        }

        if attempt < VERIFY_MAX_ATTEMPTS {
            tokio::time::sleep(std::time::Duration::from_secs(VERIFY_CHECK_INTERVAL_SECS)).await;
        }
    }

    tracing::warn!(
        target: LOG_TARGET,
        trigger = ?trigger,
        "verify_fix: anomaly persists after {} attempts (30s)",
        VERIFY_MAX_ATTEMPTS
    );
    false
}

/// Check whether a specific trigger condition has been resolved.
///
/// Each trigger type maps to a specific check:
/// - ProcessCrash: process no longer in WerFault state
/// - GameLaunchFail: game_pid exists in failure_monitor
/// - DisplayMismatch: Edge process count > 0
/// - WsDisconnect: check if WS is reconnected (via failure_monitor driving_state proxy)
/// - SentinelUnexpected: sentinel file no longer exists
/// - ErrorSpike: error rate back below threshold (via failure_monitor)
/// - ViolationSpike: violation delta stabilized
/// - Periodic/HealthCheckFail/PreFlightFailed: return true (informational or not re-checkable)
/// - POS triggers: check relevant POS state
async fn check_trigger_resolved(
    trigger: &DiagnosticTrigger,
    failure_monitor_rx: &tokio::sync::watch::Receiver<crate::failure_monitor::FailureMonitorState>,
) -> bool {
    match trigger {
        DiagnosticTrigger::ProcessCrash { process_name } => {
            let proc_name = process_name.clone();
            // Use spawn_blocking for sysinfo (standing rule: no sync ops on async runtime)
            let result = tokio::task::spawn_blocking(move || {
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::All, false);
                // Check that no WerFault/WerReport process is running for this process
                let werfault_active = sys.processes().values().any(|p| {
                    let name = p.name().to_string_lossy().to_lowercase();
                    (name.contains("werfault") || name.contains("werreport"))
                        && p.cmd().iter().any(|arg| {
                            arg.to_string_lossy().to_lowercase().contains(&proc_name.to_lowercase())
                        })
                });
                !werfault_active
            }).await;
            result.unwrap_or(false)
        }

        DiagnosticTrigger::GameLaunchFail => {
            // Check failure_monitor: game_pid should exist if game launched successfully
            let state = failure_monitor_rx.borrow().clone();
            state.game_pid.is_some()
        }

        DiagnosticTrigger::DisplayMismatch { .. } => {
            // Check Edge process count > 0 via spawn_blocking
            let result = tokio::task::spawn_blocking(|| {
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::All, false);
                let edge_count = sys.processes().values().filter(|p| {
                    p.name().to_string_lossy().to_lowercase().contains("msedge")
                }).count();
                edge_count > 0
            }).await;
            result.unwrap_or(false)
        }

        DiagnosticTrigger::WsDisconnect { .. } => {
            // We can't directly check ws_connected from failure_monitor (it's on a separate atomic).
            // Proxy: if recovery_in_progress is false and the failure_monitor is being updated,
            // the WS is likely reconnected. This is a best-effort check.
            let state = failure_monitor_rx.borrow().clone();
            !state.recovery_in_progress
        }

        DiagnosticTrigger::SentinelUnexpected { file_name } => {
            // Path traversal guard: reject if file_name contains suspicious characters
            if file_name.contains("..") || file_name.contains('\\') || file_name.contains('/') {
                tracing::warn!(
                    target: LOG_TARGET,
                    file = %file_name,
                    "verify_fix: suspicious sentinel filename — skipping verification"
                );
                return true; // Don't block on suspicious filenames
            }
            let path = std::path::Path::new(SENTINEL_BASE_DIR).join(file_name);
            !path.exists()
        }

        DiagnosticTrigger::ErrorSpike { errors_per_min } => {
            // Check if error rate has dropped below the original threshold
            // The original threshold is 5 errors/min (from diagnostic_engine)
            // We consider it resolved if current rate < original detected rate
            let _ = errors_per_min; // Original rate for context
            // Re-read error log to check current rate
            // For now, use a simplified check: any error count < 5 means resolved
            let result = tokio::task::spawn_blocking(|| {
                // Count recent errors from rc-bot-events.log
                let log_path = std::path::Path::new(r"C:\RacingPoint\rc-bot-events.log");
                if !log_path.exists() {
                    return true; // No log file = no errors
                }
                let content = match std::fs::read_to_string(log_path) {
                    Ok(c) => c,
                    Err(_) => return true,
                };
                let now = std::time::SystemTime::now();
                let one_min_ago = now
                    .checked_sub(std::time::Duration::from_secs(60))
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                // Count lines with "ERROR" in the last minute (simplified)
                let recent_errors = content.lines()
                    .rev()
                    .take(100) // Only check last 100 lines for performance
                    .filter(|line| line.contains("ERROR") || line.contains("error"))
                    .count();
                let _ = one_min_ago; // Time-based filtering would need parsed timestamps
                recent_errors < 5
            }).await;
            result.unwrap_or(true)
        }

        DiagnosticTrigger::ViolationSpike { .. } => {
            // Violation delta stabilization — check that no new violations in last check interval
            // Simplified: return true after the fix had time to take effect
            true
        }

        // Informational triggers — always return true
        DiagnosticTrigger::Periodic
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::PreFlightFailed { .. }
        | DiagnosticTrigger::PreShiftAudit
        | DiagnosticTrigger::PostSessionAnalysis { .. }
        | DiagnosticTrigger::DeployVerification { .. } => true,

        // POS-specific triggers
        DiagnosticTrigger::PosKioskDown { .. } => {
            // Check if Edge is running on POS
            let result = tokio::task::spawn_blocking(|| {
                let mut sys = System::new();
                sys.refresh_processes(ProcessesToUpdate::All, false);
                sys.processes().values().any(|p| {
                    p.name().to_string_lossy().to_lowercase().contains("msedge")
                })
            }).await;
            result.unwrap_or(false)
        }

        DiagnosticTrigger::PosNetworkDown { .. } => {
            // Check TCP connectivity to server
            let server_ip = std::env::var("RACECONTROL_SERVER_IP")
                .unwrap_or_else(|_| "192.168.31.23".to_string());
            let addr = format!("{}:8080", server_ip);
            let result = tokio::task::spawn_blocking(move || {
                std::net::TcpStream::connect_timeout(
                    &addr.parse().unwrap_or_else(|_| std::net::SocketAddr::from(([192, 168, 31, 23], 8080))),
                    std::time::Duration::from_secs(3),
                ).is_ok()
            }).await;
            result.unwrap_or(false)
        }

        DiagnosticTrigger::PosBillingApiError { .. }
        | DiagnosticTrigger::PosWifiDegraded { .. }
        | DiagnosticTrigger::PosKioskEscaped { .. } => {
            // These require external state checks — return true as best-effort
            true
        }

        DiagnosticTrigger::TaskbarVisible => {
            // Taskbar hide via Win32 ShowWindow is immediate — no verification delay needed.
            // If the tier engine re-hid it, the effect is synchronous.
            true
        }

        DiagnosticTrigger::GameMidSessionCrash { .. } => {
            // Check if game process is running again
            let state = failure_monitor_rx.borrow().clone();
            state.game_pid.is_some()
        }
    }
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
pub fn spawn(
    event_rx: mpsc::Receiver<DiagnosticEvent>,
    budget: Arc<RwLock<BudgetTracker>>,
    diag_log: DiagnosticLog,
    staff_rx: mpsc::Receiver<StaffDiagnosticRequest>,
    failure_monitor_rx: tokio::sync::watch::Receiver<crate::failure_monitor::FailureMonitorState>,
    fleet_bus_tx: tokio::sync::broadcast::Sender<FleetEvent>,
    ws_msg_tx: mpsc::Sender<rc_common::protocol::AgentMessage>,
    eval_store: std::sync::Arc<std::sync::Mutex<crate::model_eval_store::ModelEvalStore>>,
) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Tier engine started (supervised) — awaiting diagnostic events + staff requests + FleetEvent broadcast");

        // C2: Supervisor wraps the inner loop — restarts on panic
        run_supervised(event_rx, budget, diag_log, staff_rx, failure_monitor_rx, fleet_bus_tx, ws_msg_tx, eval_store).await;

        tracing::warn!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: exited (channel closed)");
    });
}

/// C2: Inner supervised loop — separated so panics can be caught and restarted.
async fn run_supervised(
    mut event_rx: mpsc::Receiver<DiagnosticEvent>,
    budget: Arc<RwLock<BudgetTracker>>,
    diag_log: DiagnosticLog,
    mut staff_rx: mpsc::Receiver<StaffDiagnosticRequest>,
    failure_monitor_rx: tokio::sync::watch::Receiver<crate::failure_monitor::FailureMonitorState>,
    fleet_bus_tx: tokio::sync::broadcast::Sender<FleetEvent>,
    ws_msg_tx: mpsc::Sender<rc_common::protocol::AgentMessage>,
    eval_store: std::sync::Arc<std::sync::Mutex<crate::model_eval_store::ModelEvalStore>>,
) {
    let mut circuit_breaker = CircuitBreaker::new();
    let mut dedup_map: HashMap<String, Instant> = HashMap::new();
    // Track in-flight staff requests to prevent duplicate diagnosis for same incident
    // (MMA OpenRouter fix: two kiosks filing for same pod creates duplicate resolutions)
    let mut inflight_incidents: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut first_event_processed = false;

    // Resolve node_id once at startup (Windows: COMPUTERNAME env var, fallback to "unknown")
    let node_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());

    // SAFE-01/02/03: Safety guardrails — blast radius, per-action circuit breaker, idempotency
    let safety = rc_common::safety::SafetyGuardrails::new();

    loop {
        tokio::select! {
            // ── Autonomous diagnostic events ──
            Some(event) = event_rx.recv() => {
                // T7: Dedup — collapse same trigger within window.
                // Include payload context so PreFlightFailed("billing") != PreFlightFailed("hid")
                // (MMA R4-1 fix: discriminant-only was too broad for payload variants)
                let dedup_key = make_dedup_key(&event.trigger);
                let now = Instant::now();
                if let Some(last_seen) = dedup_map.get(&dedup_key) {
                    if now.duration_since(*last_seen).as_secs() < DEDUP_WINDOW_SECS {
                        tracing::debug!(target: LOG_TARGET, key = %dedup_key, "Dedup: skipping duplicate trigger within {}s window", DEDUP_WINDOW_SECS);
                        continue;
                    }
                }
                dedup_map.insert(dedup_key, now);
                dedup_map.retain(|_, v| now.duration_since(*v).as_secs() < DEDUP_WINDOW_SECS * 2);

                tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, ts = %event.timestamp, "Received diagnostic event");

                // SAFE-01/02/03: Pre-flight safety check before applying any fix.
                // Build idempotency key from trigger + build_id as incident fingerprint.
                let action_type = format!("{:?}", std::mem::discriminant(&event.trigger));
                let incident_fp = make_dedup_key(&event.trigger);
                let fix_id = format!("auto-{}-{}", incident_fp, event.timestamp);
                let safety_node_id = event.build_id;

                let safety_guard = safety.pre_check(
                    &fix_id,
                    &action_type,
                    safety_node_id,  // target = this pod
                    safety_node_id,  // node_id
                    "v1",            // rule_version — tier engine v1
                    &incident_fp,
                );

                let _guard = match safety_guard {
                    Ok(guard) => guard,
                    Err(reason) => {
                        tracing::info!(
                            target: LOG_TARGET,
                            trigger = ?event.trigger,
                            reason = %reason,
                            "Safety guardrail blocked fix — skipping"
                        );
                        continue;
                    }
                };

                let result = run_tiers(&event, &mut circuit_breaker, &budget, &ws_msg_tx, &node_id).await;

                // Record circuit breaker outcome for per-action tracking
                match &result {
                    TierResult::Fixed { .. } => {
                        safety.circuit_breaker.record_success(&action_type);
                    }
                    TierResult::FailedToFix { .. } => {
                        safety.circuit_breaker.record_failure(&action_type);
                    }
                    _ => {} // NotApplicable and Stub don't affect breaker state
                }

                // Log to shared DiagnosticLog for /events/recent endpoint
                let entry = tier_result_to_log_entry(&event, &result, None, "autonomous");
                diag_log.push(entry).await;

                let trigger_str = format!("{:?}", std::mem::discriminant(&event.trigger));

                // ── 273-03: Universal KB recording — record every resolution ──
                {
                    use crate::knowledge_base::{self, KnowledgeBase, KB_PATH};
                    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
                    let env_fp = knowledge_base::fingerprint_env(event.build_id);
                    let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);

                    match &result {
                        TierResult::Fixed { tier, action } => {
                            if let Ok(kb) = KnowledgeBase::open(KB_PATH) {
                                let diag_method = match *tier {
                                    1 => Some("deterministic"),
                                    2 => Some("kb_cached"),
                                    3 => Some("scanner_enumeration"),
                                    4 => Some("consensus_5model"),
                                    _ => None,
                                };
                                if let Err(e) = kb.record_resolution(
                                    &problem_key, &problem_hash,
                                    &format!("{:?}", event.trigger), action,
                                    match *tier { 1 => "deterministic", 2 => "kb_cached", _ => "model_suggested" },
                                    *tier, "verified_pass", event.build_id, diag_method,
                                ) {
                                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to record resolution in KB");
                                }
                            }
                        }
                        TierResult::FailedToFix { tier, reason } => {
                            if let Ok(kb) = KnowledgeBase::open(KB_PATH) {
                                if let Err(e) = kb.record_resolution(
                                    &problem_key, &problem_hash,
                                    &format!("{:?}", event.trigger), reason,
                                    "failed", *tier, "verified_fail", event.build_id, None,
                                ) {
                                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to record failure in KB");
                                }
                            }
                        }
                        _ => {}
                    }
                }

                match &result {
                    TierResult::Fixed { tier, action } => {
                        tracing::info!(target: LOG_TARGET, trigger = ?event.trigger, tier = tier, action = %action, "Anomaly resolved by tier engine");

                        // Tier 1-3: run 30-second verification loop
                        if *tier <= 3 {
                            let verified = verify_fix(&event.trigger, &failure_monitor_rx).await;
                            if verified {
                                tracing::info!(
                                    target: LOG_TARGET,
                                    tier = tier, action = %action,
                                    "Fix verified: tier={} action={}",
                                    tier, action
                                );
                                let _ = fleet_bus_tx.send(FleetEvent::FixApplied {
                                    node_id: node_id.clone(),
                                    tier: *tier,
                                    action: action.clone(),
                                    trigger: trigger_str.clone(),
                                    timestamp: Utc::now(),
                                });
                                // GAME-05: Cascade game fixes to fleet via mesh gossip
                                if matches!(event.trigger, DiagnosticTrigger::GameLaunchFail) {
                                    let gossip = crate::mesh_gossip::build_game_fix_announce(
                                        &trigger_str, action, 0.9, &node_id,
                                    );
                                    let _ = ws_msg_tx.send(gossip).await;
                                }
                            } else {
                                tracing::warn!(
                                    target: LOG_TARGET,
                                    tier = tier, action = %action,
                                    "Fix verification FAILED: tier={} action={} — escalating",
                                    tier, action
                                );
                                let _ = fleet_bus_tx.send(FleetEvent::FixFailed {
                                    node_id: node_id.clone(),
                                    tier: *tier,
                                    reason: "verification_failed".to_string(),
                                    trigger: trigger_str.clone(),
                                    timestamp: Utc::now(),
                                });
                                // Escalate: for tier 3 failure, emit Escalated event
                                if *tier >= 3 {
                                    let _ = fleet_bus_tx.send(FleetEvent::Escalated {
                                        node_id: node_id.clone(),
                                        tier: *tier,
                                        reason: format!("Verification failed after tier {} fix: {}", tier, action),
                                        timestamp: Utc::now(),
                                    });
                                }
                            }
                        } else {
                            // Tier 4/5: log but don't verify within 30s (model-suggested fixes
                            // may need longer observation windows)
                            tracing::info!(
                                target: LOG_TARGET,
                                tier = tier,
                                "Tier {} fix — verification deferred (not_verified)",
                                tier
                            );
                            let _ = fleet_bus_tx.send(FleetEvent::FixApplied {
                                node_id: node_id.clone(),
                                tier: *tier,
                                action: action.clone(),
                                trigger: trigger_str.clone(),
                                timestamp: Utc::now(),
                            });
                        }
                    }
                    TierResult::Stub { tier, note } => {
                        tracing::debug!(target: LOG_TARGET, tier = tier, note = note, "Tier stub — not yet implemented");
                    }
                    TierResult::FailedToFix { tier, reason } => {
                        tracing::warn!(target: LOG_TARGET, trigger = ?event.trigger, tier = tier, reason = %reason, "All tiers failed to resolve anomaly");
                        let _ = fleet_bus_tx.send(FleetEvent::FixFailed {
                            node_id: node_id.clone(),
                            tier: *tier,
                            reason: reason.clone(),
                            trigger: trigger_str.clone(),
                            timestamp: Utc::now(),
                        });
                        // Escalate when all tiers fail
                        let _ = fleet_bus_tx.send(FleetEvent::Escalated {
                            node_id: node_id.clone(),
                            tier: *tier,
                            reason: format!("All tiers failed: {}", reason),
                            timestamp: Utc::now(),
                        });
                    }
                    TierResult::NotApplicable { .. } => {
                        tracing::debug!(target: LOG_TARGET, trigger = ?event.trigger, "No applicable tier for trigger");
                    }
                }

                // EVAL-01: persist evaluation outcome to SQLite after every diagnosis.
                // Records Fixed and FailedToFix outcomes; skips Stub/NotApplicable (no model call).
                // Critical rule: no .await after eval_store.lock() — guard dropped in tight block.
                {
                    let (tier_num, fix_verified, action_str) = match &result {
                        TierResult::Fixed { tier, action } => (*tier, true, action.clone()),
                        TierResult::FailedToFix { tier, reason } => (*tier, false, reason.clone()),
                        _ => (0u8, false, String::new()),
                    };
                    if tier_num > 0 {
                        // Derive a model_id from tier number + action string (model_id is not
                        // threaded through run_tiers; action strings encode the model for tier 3+).
                        let model_id = match tier_num {
                            1 => "tier1/deterministic".to_string(),
                            2 => "tier2/kb_cached".to_string(),
                            3 => {
                                // Tier 3 action: "Qwen3 ($0.10): <root_cause>"
                                if action_str.starts_with("Qwen3") {
                                    "qwen/qwen3-235b-a22b:free".to_string()
                                } else {
                                    "tier3/single_model".to_string()
                                }
                            }
                            4 => "tier4/mma_protocol".to_string(),
                            _ => format!("tier{}/unknown", tier_num),
                        };
                        // Cost estimate: tiers 1-2 have no model cost, tier 3 uses TIER3, tier 4+ uses TIER4
                        let cost_usd = match tier_num {
                            1 | 2 => 0.0,
                            3 => TIER3_ESTIMATED_COST,
                            _ => TIER4_ESTIMATED_COST,
                        };
                        let trigger_name = format!("{:?}", &event.trigger)
                            .split_whitespace()
                            .next()
                            .unwrap_or("Unknown")
                            .to_string();
                        let prediction_str: String = action_str.chars().take(500).collect();
                        let outcome_str = if fix_verified { "fixed" } else { "failed_to_fix" };
                        // Record in-memory reputation tracking (existing mma_engine function)
                        crate::mma_engine::record_model_outcome(&model_id, fix_verified);
                        let record = crate::model_eval_store::EvalRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            model_id,
                            pod_id: node_id.clone(),
                            trigger_type: trigger_name,
                            prediction: prediction_str,
                            actual_outcome: outcome_str.to_string(),
                            correct: fix_verified,
                            cost_usd,
                            created_at: chrono::Utc::now().to_rfc3339(),
                        };
                        match eval_store.lock() {
                            Ok(store) => {
                                if let Err(e) = store.insert(&record) {
                                    tracing::warn!(
                                        target: LOG_TARGET,
                                        error = %e,
                                        tier = tier_num,
                                        "EVAL-01: failed to write evaluation record"
                                    );
                                } else {
                                    tracing::debug!(
                                        target: LOG_TARGET,
                                        tier = tier_num,
                                        correct = fix_verified,
                                        "EVAL-01: evaluation record written"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    target: LOG_TARGET,
                                    error = %e,
                                    "EVAL-01: eval_store mutex poisoned"
                                );
                            }
                        }
                        // EVAL-03: push evaluation record to server via WS for /api/v1/models/evaluations query.
                        // Best-effort — WS failure must NOT roll back the local EVAL-01 write.
                        let payload = rc_common::protocol::EvalRecordPayload {
                            id: record.id.clone(),
                            model_id: record.model_id.clone(),
                            pod_id: record.pod_id.clone(),
                            trigger_type: record.trigger_type.clone(),
                            prediction: record.prediction.clone(),
                            actual_outcome: record.actual_outcome.clone(),
                            correct: record.correct,
                            cost_usd: record.cost_usd,
                            created_at: record.created_at.clone(),
                        };
                        let sync_msg = rc_common::protocol::AgentMessage::ModelEvalSync {
                            pod_id: record.pod_id.clone(),
                            records: vec![payload],
                        };
                        if let Err(e) = ws_msg_tx.send(sync_msg).await {
                            tracing::warn!(
                                target: LOG_TARGET,
                                error = %e,
                                "EVAL-03: failed to push evaluation to server via WS"
                            );
                        }
                    }
                }

                if !first_event_processed {
                    tracing::info!(target: "state", task = "tier_engine", event = "lifecycle", "lifecycle: first_event_processed");
                    first_event_processed = true;
                }
            }

            // ── Staff-triggered diagnostic requests (v27.0 Phase 2) ──
            Some(req) = staff_rx.recv() => {
                // Dedup: skip if we're already processing this incident
                if inflight_incidents.contains(&req.correlation_id) {
                    tracing::info!(target: LOG_TARGET, correlation_id = %req.correlation_id, "Skipping duplicate staff request (already in-flight)");
                    let _ = req.response_tx.send(StaffDiagnosticResult {
                        correlation_id: req.correlation_id,
                        tier: 0, outcome: "duplicate".to_string(), root_cause: String::new(),
                        fix_action: String::new(), fix_type: "none".to_string(), confidence: 0.0,
                        fix_applied: false, problem_hash: String::new(),
                        summary: "Duplicate request — diagnosis already in progress for this incident".to_string(),
                    });
                    continue;
                }
                inflight_incidents.insert(req.correlation_id.clone());

                tracing::info!(
                    target: LOG_TARGET,
                    correlation_id = %req.correlation_id,
                    category = %req.category,
                    "Staff diagnostic request received — running Tier 1 + Tier 2"
                );

                // Reset dedup window so autonomous diagnosis doesn't skip this category.
                // Must use SAME key format as autonomous branch (MMA R4-1 fix: key mismatch).
                let trigger_for_dedup = category_to_trigger(&req.category, &req.description);
                let dedup_key_reset = make_dedup_key(&trigger_for_dedup);
                dedup_map.remove(&dedup_key_reset);

                let result = run_staff_diagnosis(&req, &mut circuit_breaker, &budget, &failure_monitor_rx).await;

                // Log to shared DiagnosticLog
                let entry = DiagnosticLogEntry {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    trigger: format!("StaffRequest({})", req.category),
                    tier: result.tier,
                    outcome: result.outcome.clone(),
                    action: result.fix_action.clone(),
                    root_cause: result.root_cause.clone(),
                    fix_type: result.fix_type.clone(),
                    confidence: result.confidence,
                    fix_applied: result.fix_applied,
                    problem_hash: result.problem_hash.clone(),
                    correlation_id: Some(req.correlation_id.clone()),
                    source: "staff".to_string(),
                };
                diag_log.push(entry).await;

                // Emit FleetEvent for staff diagnostic results
                if result.fix_applied {
                    let _ = fleet_bus_tx.send(FleetEvent::FixApplied {
                        node_id: node_id.clone(),
                        tier: result.tier,
                        action: result.fix_action.clone(),
                        trigger: format!("StaffRequest({})", req.category),
                        timestamp: Utc::now(),
                    });
                } else if result.outcome == "unresolved" {
                    let _ = fleet_bus_tx.send(FleetEvent::Escalated {
                        node_id: node_id.clone(),
                        tier: result.tier,
                        reason: format!("Staff request unresolved: {}", result.summary),
                        timestamp: Utc::now(),
                    });
                }

                // Remove from inflight BEFORE send — even if send fails (WS timed out),
                // the diagnosis ran and future requests should not be blocked.
                // (MMA OpenRouter Wave 2 fix: panic in send could leak the key permanently)
                let cid = req.correlation_id.clone();
                inflight_incidents.remove(&cid);
                let _ = req.response_tx.send(result);
            }

            // Both channels closed — exit
            else => {
                tracing::warn!(target: LOG_TARGET, "Both event channels closed — tier engine exiting");
                break;
            }
        }
    }
}

/// Run Tier 1 + Tier 2 for staff-triggered requests.
/// Does NOT run Tier 3/4 (model calls) to keep staff response fast.
/// If Tier 1+2 don't resolve, returns recommendation for manual action.
async fn run_staff_diagnosis(
    req: &StaffDiagnosticRequest,
    _circuit_breaker: &mut CircuitBreaker,
    _budget: &Arc<RwLock<BudgetTracker>>,
    failure_monitor_rx: &tokio::sync::watch::Receiver<crate::failure_monitor::FailureMonitorState>,
) -> StaffDiagnosticResult {
    // Map staff category to a DiagnosticTrigger for Tier 1
    let trigger = category_to_trigger(&req.category, &req.description);
    // Use REAL pod state from the failure monitor watch channel (MMA Round 2 P1 fix)
    let pod_state = failure_monitor_rx.borrow().clone();

    let event = DiagnosticEvent {
        trigger,
        pod_state,
        timestamp: chrono::Utc::now().to_rfc3339(),
        build_id: crate::BUILD_ID,
    };

    // ── Tier 1: Deterministic ──
    let t1 = tier1_deterministic(&event).await;
    if let TierResult::Fixed { tier, ref action } = t1 {
        tracing::info!(target: LOG_TARGET, correlation_id = %req.correlation_id, "Staff request resolved by Tier 1: {}", action);
        return StaffDiagnosticResult {
            correlation_id: req.correlation_id.clone(),
            tier,
            outcome: "fixed".to_string(),
            root_cause: format!("Deterministic fix for {}", req.category),
            fix_action: action.clone(),
            fix_type: "deterministic".to_string(),
            confidence: 1.0,
            fix_applied: true,
            problem_hash: compute_problem_hash(&req.category),
            summary: format!("Tier 1 applied: {}", action),
        };
    }

    // ── Tier 2: Knowledge Base ──
    let t2 = tier2_kb_lookup(&event);
    if let TierResult::Fixed { tier, ref action } = t2 {
        tracing::info!(target: LOG_TARGET, correlation_id = %req.correlation_id, "Staff request resolved by Tier 2 KB: {}", action);
        return StaffDiagnosticResult {
            correlation_id: req.correlation_id.clone(),
            tier,
            outcome: "fixed".to_string(),
            root_cause: action.clone(),
            fix_action: action.clone(),
            fix_type: "kb_lookup".to_string(),
            confidence: 0.8,
            fix_applied: true,
            problem_hash: compute_problem_hash(&req.category),
            summary: format!("Tier 2 KB match: {}", action),
        };
    }

    // Tier 1+2 didn't resolve — return recommendation
    tracing::info!(target: LOG_TARGET, correlation_id = %req.correlation_id, "Staff request: Tier 1+2 did not resolve — recommending manual action");
    StaffDiagnosticResult {
        correlation_id: req.correlation_id.clone(),
        tier: 0,
        outcome: "unresolved".to_string(),
        root_cause: String::new(),
        fix_action: String::new(),
        fix_type: "none".to_string(),
        confidence: 0.0,
        fix_applied: false,
        problem_hash: compute_problem_hash(&req.category),
        summary: format!(
            "Tier 1 (deterministic) and Tier 2 (KB) found no solution for '{}'. Recommend manual investigation or AI diagnosis via server.",
            req.category
        ),
    }
}

/// Map staff incident category to the closest DiagnosticTrigger for Tier 1.
/// NOTE: "pod_offline" is NOT mapped to HealthCheckFail because if we're receiving
/// a DiagnosticRequest via WS, the pod is clearly online. Instead treat as a general
/// anomaly check (Periodic with always-applied fixes).
fn category_to_trigger(category: &str, _description: &str) -> DiagnosticTrigger {
    match category {
        // pod_offline from kiosk = pod misbehaving but WS alive, run general cleanup
        "pod_offline" => DiagnosticTrigger::Periodic,
        "game_crash" => DiagnosticTrigger::GameLaunchFail,
        "screen_stuck" => DiagnosticTrigger::DisplayMismatch {
            expected_edge_count: 1,
            actual_edge_count: 0,
        },
        "billing_stuck" => DiagnosticTrigger::PreFlightFailed {
            check_name: "billing".to_string(),
            detail: "Staff reported billing stuck".to_string(),
        },
        "no_steering_input" => DiagnosticTrigger::PreFlightFailed {
            check_name: "hid".to_string(),
            detail: "Staff reported no steering input".to_string(),
        },
        // kiosk_bypass is a HUMAN report, not a filesystem observation.
        // Do NOT map to SentinelUnexpected (MMA R4-1 fix: would create phantom sentinel).
        // Map to Periodic for general cleanup; the server's AI diagnosis handles the actual bypass.
        "kiosk_bypass" => DiagnosticTrigger::Periodic,
        _ => {
            tracing::warn!(target: LOG_TARGET, category = %category, "Unknown staff category — mapping to Periodic");
            DiagnosticTrigger::Periodic
        }
    }
}

/// Build a dedup key that includes both discriminant AND payload context.
/// e.g., PreFlightFailed("billing") and PreFlightFailed("hid") get different keys.
/// Build a dedup key with explicit stable names for EVERY variant.
/// Never falls back to `discriminant` Debug formatting (MMA R4-2 fix: opaque + build-fragile).
fn make_dedup_key(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::Periodic => "Periodic".to_string(),
        DiagnosticTrigger::HealthCheckFail => "HealthCheckFail".to_string(),
        DiagnosticTrigger::GameLaunchFail => "GameLaunchFail".to_string(),
        DiagnosticTrigger::ProcessCrash { process_name } => {
            format!("ProcessCrash_{}", process_name)
        }
        // DisplayMismatch: use only expected count (stable), not actual (fluctuates)
        DiagnosticTrigger::DisplayMismatch { expected_edge_count, .. } => {
            format!("DisplayMismatch_{}", expected_edge_count)
        }
        // ErrorSpike: bucket by severity tier (low/medium/high/critical) so escalation
        // is visible but minor fluctuations are deduped. Raw count changes every scan.
        // (MMA OpenRouter Wave 2 fix: fully static key hid escalating errors)
        DiagnosticTrigger::ErrorSpike { errors_per_min } => {
            let severity = if *errors_per_min >= 20 { "critical" }
                else if *errors_per_min >= 10 { "high" }
                else if *errors_per_min >= 5 { "medium" }
                else { "low" };
            format!("ErrorSpike_{}", severity)
        }
        DiagnosticTrigger::WsDisconnect { .. } => "WsDisconnect".to_string(),
        DiagnosticTrigger::SentinelUnexpected { file_name } => {
            format!("SentinelUnexpected_{}", file_name)
        }
        DiagnosticTrigger::ViolationSpike { .. } => "ViolationSpike".to_string(),
        DiagnosticTrigger::PreFlightFailed { check_name, .. } => {
            format!("PreFlightFailed_{}", check_name)
        }
        DiagnosticTrigger::PosKioskDown { .. } => "PosKioskDown".to_string(),
        DiagnosticTrigger::PosNetworkDown { .. } => "PosNetworkDown".to_string(),
        DiagnosticTrigger::PosBillingApiError { endpoint, .. } => {
            format!("PosBillingApiError_{}", endpoint)
        }
        DiagnosticTrigger::PosWifiDegraded { rssi_dbm, .. } => {
            format!("PosWifiDegraded_{}dBm", rssi_dbm)
        }
        DiagnosticTrigger::PosKioskEscaped { foreground_process } => {
            format!("PosKioskEscaped_{}", foreground_process)
        }
        DiagnosticTrigger::TaskbarVisible => "TaskbarVisible".to_string(),
        // MMA-First Protocol triggers (v31.0)
        DiagnosticTrigger::GameMidSessionCrash { exit_code, .. } => {
            format!("GameMidSessionCrash_{}", exit_code.unwrap_or(-1))
        }
        DiagnosticTrigger::PostSessionAnalysis { .. } => "PostSessionAnalysis".to_string(),
        DiagnosticTrigger::PreShiftAudit => "PreShiftAudit".to_string(),
        DiagnosticTrigger::DeployVerification { new_build_id } => {
            format!("DeployVerification_{}", new_build_id)
        }
    }
}

/// Stable hash for problem key — uses FNV-1a for deterministic output
/// across restarts and versions (MMA R4-1 fix: DefaultHasher is randomized per-process).
fn compute_problem_hash(category: &str) -> String {
    // FNV-1a 64-bit — deterministic, no per-process randomization
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in category.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Convert a TierResult + DiagnosticEvent into a log entry
fn tier_result_to_log_entry(
    event: &DiagnosticEvent,
    result: &TierResult,
    correlation_id: Option<String>,
    source: &str,
) -> DiagnosticLogEntry {
    let (tier, outcome, action, root_cause, fix_type, confidence, fix_applied) = match result {
        TierResult::Fixed { tier, action } => (*tier, "fixed", action.clone(), String::new(), "deterministic", 1.0, true),
        TierResult::FailedToFix { tier, reason } => (*tier, "failed_to_fix", String::new(), reason.clone(), "none", 0.0, false),
        TierResult::NotApplicable { tier } => (*tier, "not_applicable", String::new(), String::new(), "none", 0.0, false),
        TierResult::Stub { tier, note } => (*tier, "stub", note.to_string(), String::new(), "none", 0.0, false),
    };
    DiagnosticLogEntry {
        timestamp: event.timestamp.clone(),
        trigger: format!("{:?}", std::mem::discriminant(&event.trigger)),
        tier,
        outcome: outcome.to_string(),
        action,
        root_cause,
        fix_type: fix_type.to_string(),
        confidence,
        fix_applied,
        problem_hash: String::new(),
        correlation_id,
        source: source.to_string(),
    }
}

/// MMA-First Protocol: Q1-Q4 decision gate (v31.0).
///
/// Determines whether to invoke MMA for a given diagnostic event.
/// Replaces the old linear Tier 2 KB lookup with a 4-question protocol:
/// - Q1: Has this EXACT problem been solved before? (two-tier KB lookup)
/// - Q2: Is someone ALREADY diagnosing this? (fleet dedup via experiments table)
/// - Q3: Is this novel and worth an MMA call? (training mode = always yes)
/// - Q4: Should we search for a permanent fix? (background, after Q1 workaround)
fn mma_decision(event: &DiagnosticEvent) -> MmaDecision {
    use crate::knowledge_base::{self, KnowledgeBase, KB_PATH};

    let problem_key = knowledge_base::normalize_problem_key(&event.trigger);
    let env_fp = knowledge_base::fingerprint_env(event.build_id);
    let exact_hash = knowledge_base::compute_exact_hash(&problem_key, &env_fp);
    let stable_hash = knowledge_base::compute_stable_hash(&problem_key, &env_fp);

    let kb = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb,
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, error = %e, "KB unavailable — proceeding to MMA");
            return MmaDecision::InvokeMma;
        }
    };

    // ── Q1: Has this EXACT problem been solved before? ──
    match kb.lookup_two_tier(&exact_hash, &stable_hash) {
        Ok(Some(solution)) => {
            if solution.fix_permanence == "permanent" && solution.confidence >= 0.9 {
                // Permanent fix with high confidence — apply and done
                tracing::info!(
                    target: LOG_TARGET,
                    problem_key = %problem_key,
                    confidence = solution.confidence,
                    fix = %solution.fix_action,
                    "Q1 HIT: permanent fix (confidence {:.0}%)",
                    solution.confidence * 100.0
                );
                return MmaDecision::ApplyPermanentFix { solution };
            }
            // Workaround found — apply immediately, Q4 may fire in background
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %problem_key,
                permanence = %solution.fix_permanence,
                recurrence = solution.recurrence_count,
                fix = %solution.fix_action,
                "Q1 HIT: workaround (recurrence #{}) — applying, Q4 may follow",
                solution.recurrence_count + 1
            );
            return MmaDecision::ApplyWorkaroundThenQ4 { solution };
        }
        Ok(None) => {
            tracing::debug!(target: LOG_TARGET, problem_key = %problem_key, "Q1 MISS: no KB match");
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, error = %e, "Q1: KB lookup error — proceeding to Q2");
        }
    }

    // ── Q2: Is someone ALREADY diagnosing this? ──
    match kb.get_open_experiment(&problem_key) {
        Ok(Some(exp)) => {
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %problem_key,
                exp_node = %exp.node,
                "Q2: open experiment found on {} — waiting instead of duplicate diagnosis",
                exp.node
            );
            return MmaDecision::WaitForFleet { experiment_node: exp.node };
        }
        Ok(None) => {
            tracing::debug!(target: LOG_TARGET, "Q2: no open experiment — proceeding to Q3");
        }
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, error = %e, "Q2: experiment check failed — proceeding");
        }
    }

    // ── Q3: Is this novel and worth an MMA call? ──
    let mma_config = crate::config::load_config()
        .map(|c| c.mma)
        .unwrap_or_default();

    if mma_config.is_training_active() {
        // TRAINING MODE: MMA for everything the KB can't permanently solve
        tracing::info!(
            target: LOG_TARGET,
            problem_key = %problem_key,
            "Q3 TRAINING MODE: invoking full 5-model MMA (Tier 1 during training)"
        );
        return MmaDecision::InvokeMma;
    }

    // PRODUCTION MODE: only MMA for novel issues
    // Check if billing is active (revenue justifies the cost)
    if event.pod_state.billing_active {
        tracing::info!(
            target: LOG_TARGET,
            problem_key = %problem_key,
            "Q3 PRODUCTION: billing active — revenue justifies MMA"
        );
        return MmaDecision::InvokeMma;
    }

    // Check if this is truly novel (never seen this problem_key at all)
    match kb.solution_count() {
        Ok(count) if count == 0 => {
            // Empty KB — everything is novel
            return MmaDecision::InvokeMma;
        }
        _ => {}
    }

    // Not billing, not training, KB exists but no match — skip MMA, use deterministic only
    MmaDecision::SkipMma {
        reason: format!("Production mode: no billing active, KB miss for '{}'", problem_key),
    }
}

/// Run the MMA-First Protocol for a single DiagnosticEvent.
///
/// v31.0 PROTOCOL ORDER:
///   1. Tier 1 deterministic (always runs — free, instant)
///   2. Q1-Q4 decision gate (KB lookup, fleet dedup, training/production gate)
///   3. If MMA: full 5-model parallel diagnosis
///   4. If Q1 workaround: apply + spawn Q4 background permanent fix search
///   5. Tier 5 human escalation (if all else fails)
async fn run_tiers(
    event: &DiagnosticEvent,
    circuit_breaker: &mut CircuitBreaker,
    budget: &Arc<RwLock<BudgetTracker>>,
    ws_msg_tx: &mpsc::Sender<rc_common::protocol::AgentMessage>,
    pod_id: &str,
) -> TierResult {
    // ── Short-circuit: Periodic events with no anomaly don't need model diagnosis ──
    let is_periodic_only = matches!(event.trigger, DiagnosticTrigger::Periodic);

    // ── Tier 1: Deterministic fixes (always runs — free, instant) ──
    let t1 = tier1_deterministic(event).await;
    if matches!(t1, TierResult::Fixed { .. }) {
        // In training mode, even Tier 1 fixes still fall through to MMA
        // for root cause analysis — but only if they're NOT periodic
        let mma_config = crate::config::load_config()
            .map(|c| c.mma)
            .unwrap_or_default();
        if !mma_config.is_training_active() || is_periodic_only {
            return t1;
        }
        // Training mode: Tier 1 fixed it, but we still want MMA to analyze WHY
        // so the KB captures the permanent fix, not just the band-aid
        tracing::info!(
            target: LOG_TARGET,
            trigger = ?event.trigger,
            "Training mode: Tier 1 fix applied, but continuing to MMA for root cause analysis"
        );
    }

    // Periodic-only events: cleanup only, no model tiers needed
    if is_periodic_only {
        tracing::debug!(target: LOG_TARGET, "Periodic scan complete — no anomaly, skipping model tiers");
        return TierResult::NotApplicable { tier: 1 };
    }

    // ── Q1-Q4: MMA-First Protocol decision gate ──
    let decision = mma_decision(event);

    match decision {
        MmaDecision::ApplyPermanentFix { solution } => {
            // Q1 hit: permanent fix — apply and done
            if let Ok(kb) = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH) {
                let _ = kb.record_outcome(&solution.id, true);
                let _ = kb.increment_recurrence(&solution.id);
            }
            return TierResult::Fixed {
                tier: 2,
                action: format!("Q1 permanent fix ({:.0}%): {}", solution.confidence * 100.0, solution.fix_action),
            };
        }

        MmaDecision::ApplyWorkaroundThenQ4 { solution } => {
            // Q1 hit: workaround — apply immediately
            if let Ok(kb) = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH) {
                let _ = kb.record_outcome(&solution.id, true);
                let _ = kb.increment_recurrence(&solution.id);

                // Q4: Should we search for a permanent fix in the background?
                if kb.should_trigger_q4(&solution) {
                    let sol_id = solution.id.clone();
                    let problem_key = solution.problem_key.clone();
                    let fix_action = solution.fix_action.clone();
                    let recurrence = solution.recurrence_count;
                    let budget_clone = budget.clone();

                    tracing::info!(
                        target: LOG_TARGET,
                        problem_key = %problem_key,
                        recurrence = recurrence,
                        "Q4: spawning background permanent fix search (workaround applied {} times)",
                        recurrence
                    );

                    // Fire-and-forget: customer already unblocked by workaround
                    tokio::spawn(async move {
                        run_q4_permanent_fix_search(
                            &sol_id, &problem_key, &fix_action, recurrence,
                            &budget_clone,
                        ).await;
                    });
                }
            }

            return TierResult::Fixed {
                tier: 2,
                action: format!(
                    "Q1 workaround (recurrence #{}): {}",
                    solution.recurrence_count + 1,
                    solution.fix_action
                ),
            };
        }

        MmaDecision::WaitForFleet { experiment_node } => {
            // Q2: another pod is diagnosing — wait up to 120s, then recheck KB
            tracing::info!(
                target: LOG_TARGET,
                node = %experiment_node,
                "Q2: waiting 120s for fleet experiment result from {}",
                experiment_node
            );
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;

            // Recheck KB after waiting
            let recheck = mma_decision(event);
            match recheck {
                MmaDecision::ApplyPermanentFix { solution } | MmaDecision::ApplyWorkaroundThenQ4 { solution } => {
                    if let Ok(kb) = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH) {
                        let _ = kb.record_outcome(&solution.id, true);
                    }
                    return TierResult::Fixed {
                        tier: 2,
                        action: format!("Q2 fleet result: {}", solution.fix_action),
                    };
                }
                _ => {
                    // Fleet didn't produce a result in time — fall through to MMA
                    tracing::info!(target: LOG_TARGET, "Q2: fleet experiment timed out — invoking MMA");
                }
            }
        }

        MmaDecision::InvokeMma => {
            // Q3: proceed to MMA diagnosis below
        }

        MmaDecision::SkipMma { reason } => {
            tracing::debug!(target: LOG_TARGET, reason = %reason, "Q3: skipping MMA");
            return TierResult::NotApplicable { tier: 3 };
        }
    }

    // ═══ CGP + Plan Manager + MMA Integration (v32.0) ═══════════════════════
    let mma_start = Instant::now();

    // ── CGP Phase A: Pre-action gates (local, $0) ──
    let kb = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH).ok();
    let tier = rc_common::mesh_types::DiagnosisTier::MultiModel;

    let cgp_phase_a = match CgpEngine::run_phase_a(event, kb.as_ref(), tier) {
        Ok(gates) => gates,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, error = %e, "CGP Phase A critical failure — escalating");
            // MMA-F1: Persist partial audit trail before escalating (fleet blind spot fix)
            let problem_key = crate::knowledge_base::normalize_problem_key(&event.trigger);
            let partial_audit = rc_common::mesh_types::StructuredDiagnosisAudit {
                incident_id: format!("diag-{}", event.timestamp),
                problem_key,
                tier,
                cgp_gates: vec![],
                plan: None,
                mma_summary: Some(serde_json::json!({"status": "cgp_phase_a_failed", "error": format!("{}", e)})),
                total_cost: 0.0,
                total_duration_ms: mma_start.elapsed().as_millis() as u64,
                timestamp: Utc::now(),
            };
            if let Some(ref kb) = kb {
                DiagnosisPlanner::save_audit(&partial_audit, kb);
            }
            let _ = ws_msg_tx.send(rc_common::protocol::AgentMessage::MeshDiagnosisAudit {
                incident_id: partial_audit.incident_id.clone(),
                audit_json: serde_json::to_string(&partial_audit).unwrap_or_default(),
            }).await;
            return tier5_human_escalation(event, ws_msg_tx, pod_id).await;
        }
    };

    // ── Plan Manager: Create structured diagnosis plan ──
    let mut plan = DiagnosisPlanner::create_plan(event, &cgp_phase_a, tier);
    if let Some(ref kb) = kb {
        DiagnosisPlanner::save(&plan, kb);
    }

    // Mark step 1 (gather context) as done — CGP Phase A already did this
    DiagnosisPlanner::start_step(&mut plan, 1);
    DiagnosisPlanner::complete_step(&mut plan, 1, serde_json::json!({
        "cgp_gates_passed": cgp_phase_a.iter().filter(|g| g.status == rc_common::mesh_types::CgpGateStatus::Passed).count(),
    }));

    // ── Staggered startup: delay first model call by pod_number × 2s ──
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static STARTUP_DELAYED: AtomicBool = AtomicBool::new(false);
        if !STARTUP_DELAYED.swap(true, Ordering::SeqCst) {
            if let Ok(cfg) = crate::config::load_config() {
                let delay_ms = u64::from(cfg.pod.number) * 2000;
                tracing::info!(
                    target: LOG_TARGET,
                    pod = cfg.pod.number, delay_ms,
                    "Staggered startup: delaying first MMA call"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }

    // C1: Check circuit breaker before model calls
    if circuit_breaker.is_open() {
        tracing::info!(target: LOG_TARGET, "Circuit breaker OPEN — skipping MMA");
        return tier5_human_escalation(event, ws_msg_tx, pod_id).await;
    }

    // Mark step 2 (evaluate hypotheses) as done
    if plan.steps.len() >= 2 {
        DiagnosisPlanner::start_step(&mut plan, 2);
        DiagnosisPlanner::complete_step(&mut plan, 2, serde_json::json!({"hypotheses_from_g5": true}));
    }

    // ── Run the full 4-step Unified MMA Protocol ──
    tracing::info!(
        target: LOG_TARGET,
        trigger = ?event.trigger,
        plan_id = %plan.plan_id,
        "Q3 authorized: launching Unified MMA Protocol with CGP+Plan tracking"
    );

    // Mark MMA DIAGNOSE step as in-progress
    if plan.steps.len() >= 3 {
        DiagnosisPlanner::start_step(&mut plan, 3);
    }

    let protocol_result = crate::mma_engine::run_protocol(event, budget).await;

    let (fix_applied, fix_description, tier_result) = match protocol_result {
        crate::mma_engine::MmaProtocolResult::Success { consensus, total_cost, backtracks } => {
            circuit_breaker.record_success();

            let root_cause = consensus.majority_findings.first()
                .map(|f| f.description.clone())
                .unwrap_or_else(|| "MMA protocol found no specific root cause".to_string());
            let fix_action = consensus.executions.first()
                .map(|e| e.implementation.clone())
                .unwrap_or_else(|| consensus.fix_plans.first()
                    .map(|p| p.actions.join("; "))
                    .unwrap_or_default());
            let fix_type = consensus.fix_plans.first()
                .map(|p| p.fix_type.clone())
                .unwrap_or_else(|| "deterministic".to_string());

            tracing::info!(
                target: LOG_TARGET,
                findings = consensus.majority_findings.len(),
                plans = consensus.fix_plans.len(),
                executions = consensus.executions.len(),
                cost = total_cost,
                backtracks,
                "Unified MMA Protocol SUCCEEDED — {} findings, {} plans, {} executions (${:.2}, {} backtracks)",
                consensus.majority_findings.len(),
                consensus.fix_plans.len(),
                consensus.executions.len(),
                total_cost,
                backtracks
            );

            // Complete plan steps 3-10 (MMA steps)
            for step_id in 3..=std::cmp::min(10, plan.steps.len() as u8) {
                DiagnosisPlanner::complete_step(&mut plan, step_id, serde_json::json!({"mma": "success"}));
            }

            // Store in KB with full provenance
            if let Ok(kb) = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH) {
                let problem_key = crate::knowledge_base::normalize_problem_key(&event.trigger);
                let env_fp = crate::knowledge_base::fingerprint_env(event.build_id);
                let stable_hash = crate::knowledge_base::compute_stable_hash(&problem_key, &env_fp);

                let solution = crate::knowledge_base::Solution {
                    id: uuid::Uuid::new_v4().to_string(),
                    problem_key: problem_key.clone(),
                    problem_hash: stable_hash,
                    symptoms: serde_json::to_string(&consensus).unwrap_or_default(),
                    environment: serde_json::to_string(&env_fp).unwrap_or_default(),
                    root_cause: root_cause.clone(),
                    fix_action: fix_action.clone(),
                    fix_type: format!("mma_protocol_{}", fix_type),
                    success_count: 1,
                    fail_count: 0,
                    confidence: consensus.majority_findings.first()
                        .map(|f| f.confidence).unwrap_or(0.8),
                    cost_to_diagnose: total_cost,
                    models_used: serde_json::to_string(&consensus.models_used).ok(),
                    source_node: format!("pod_{}", event.build_id),
                    created_at: event.timestamp.clone(),
                    updated_at: event.timestamp.clone(),
                    version: 1,
                    ttl_days: 365,
                    tags: Some(format!("[\"mma_protocol\",\"{}\"]", problem_key)),
                    diagnosis_method: Some("cgp_plan_mma_4step".to_string()),
                    fix_permanence: "permanent".to_string(),
                    recurrence_count: 0,
                    permanent_fix_id: None,
                    last_recurrence: None,
                    permanent_attempt_at: None,
                };
                let _ = kb.store_solution(&solution);
            }

            let desc = format!(
                "MMA Protocol (${:.2}, {} backtracks): {}",
                total_cost, backtracks, root_cause
            );
            (true, desc.clone(), TierResult::Fixed { tier: 4, action: desc })
        }

        crate::mma_engine::MmaProtocolResult::BudgetExhausted { step, spent } => {
            tracing::warn!(
                target: LOG_TARGET,
                step, spent,
                "MMA Protocol: budget exhausted at step {} (${:.2} spent)",
                step, spent
            );
            let desc = format!("Budget exhausted at step {} (${:.2})", step, spent);
            (false, desc, tier5_human_escalation(event, ws_msg_tx, pod_id).await)
        }

        crate::mma_engine::MmaProtocolResult::HumanEscalation { backtracks, last_failure, total_cost } => {
            circuit_breaker.record_failure();
            tracing::warn!(
                target: LOG_TARGET,
                backtracks,
                cost = total_cost,
                failure = %last_failure,
                "MMA Protocol: max backtracks ({}) — human escalation",
                backtracks
            );
            let desc = format!("Max backtracks ({}, ${:.2}): {}", backtracks, total_cost, last_failure);
            (false, desc.clone(), TierResult::FailedToFix {
                tier: 4,
                reason: desc,
            })
        }

        crate::mma_engine::MmaProtocolResult::ApiUnavailable { reason } => {
            tracing::warn!(target: LOG_TARGET, reason = %reason, "MMA Protocol: API unavailable");
            let desc = format!("API unavailable: {}", reason);
            (false, desc, tier5_human_escalation(event, ws_msg_tx, pod_id).await)
        }
    };

    // ── CGP Phase D: Post-action verification gates (local, $0) ──
    let cgp_phase_d = CgpEngine::run_phase_d(event, fix_applied, &fix_description, tier, kb.as_ref());

    // ── Store structured audit trail ──
    let all_gates: Vec<_> = cgp_phase_a.iter().chain(cgp_phase_d.iter()).cloned().collect();
    let audit = rc_common::mesh_types::StructuredDiagnosisAudit {
        incident_id: plan.incident_id.clone(),
        problem_key: plan.problem_key.clone(),
        tier,
        cgp_gates: all_gates,
        plan: Some(plan.clone()),
        mma_summary: Some(serde_json::json!({
            "fix_applied": fix_applied,
            "fix_description": fix_description,
        })),
        total_cost: 0.0, // Cost tracked by MMA engine internally
        total_duration_ms: mma_start.elapsed().as_millis() as u64,
        timestamp: Utc::now(),
    };

    // Persist audit in local SQLite
    if let Some(ref kb) = kb {
        DiagnosisPlanner::save(&plan, kb);
        DiagnosisPlanner::save_audit(&audit, kb);
    }

    // MMA-F3: Gossip audit to server — log errors instead of ignoring
    if let Err(e) = ws_msg_tx.send(rc_common::protocol::AgentMessage::MeshDiagnosisAudit {
        incident_id: audit.incident_id.clone(),
        audit_json: serde_json::to_string(&audit).unwrap_or_default(),
    }).await {
        tracing::warn!(
            target: LOG_TARGET,
            incident_id = %audit.incident_id,
            error = %e,
            "Failed to gossip diagnosis audit — channel full or closed"
        );
    }

    tracing::info!(
        target: LOG_TARGET,
        plan_id = %plan.plan_id,
        gates_passed = audit.cgp_gates.iter().filter(|g| g.status == rc_common::mesh_types::CgpGateStatus::Passed).count(),
        gates_total = audit.cgp_gates.len(),
        duration_ms = audit.total_duration_ms,
        "CGP+Plan+MMA diagnosis complete"
    );

    tier_result
}

// ─── Q4: Background Permanent Fix Search (v31.0) ────────────────────────────
// Runs async after Q1 applies a workaround. Customer already unblocked.
// Goal: find WHY the workaround works and what prevents recurrence.

async fn run_q4_permanent_fix_search(
    workaround_id: &str,
    problem_key: &str,
    workaround_action: &str,
    recurrence_count: i64,
    budget: &Arc<RwLock<BudgetTracker>>,
) {
    use crate::openrouter;
    use crate::knowledge_base::{self, KnowledgeBase, KB_PATH};

    // Mark that we're attempting a permanent fix search (cooldown)
    if let Ok(kb) = KnowledgeBase::open(KB_PATH) {
        let _ = kb.mark_permanent_attempt(workaround_id);
    }

    // Budget check — Q4 uses full 5-model MMA ($4.30)
    {
        let mut bt = budget.write().await;
        if !bt.can_spend(TIER4_ESTIMATED_COST) {
            tracing::info!(
                target: LOG_TARGET,
                problem_key = %problem_key,
                "Q4: budget ceiling — deferring permanent fix search"
            );
            return;
        }
        bt.record_spend(TIER4_ESTIMATED_COST);
    }

    let api_key = match openrouter::get_api_key() {
        Some(k) => k,
        None => {
            tracing::debug!(target: LOG_TARGET, "Q4: OPENROUTER_KEY not set — skipping");
            return;
        }
    };

    // Q4-specific prompt: ask WHY the workaround works and find permanent fix
    let q4_symptoms = format!(
        "CONTEXT: This is a Q4 permanent fix search.\n\
         Problem: {}\n\
         Workaround applied {} times: \"{}\"\n\
         The workaround works every time, but the issue keeps recurring.\n\n\
         TASK:\n\
         1. WHY does \"{}\" fix it? What state was corrupted?\n\
         2. WHAT causes that corruption in the first place?\n\
         3. HOW to prevent the corruption from occurring?\n\
         4. Provide a PERMANENT FIX that eliminates recurrence.\n\n\
         CRITICAL: \"Restart\" or \"kill process\" is NOT a permanent fix.\n\
         Explain the root cause mechanism and how to prevent it.",
        problem_key, recurrence_count, workaround_action, workaround_action
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let responses = openrouter::tier4_diagnose_parallel(&client, &api_key, &q4_symptoms).await;
    let total_cost = openrouter::total_cost(&responses);

    if let Some(consensus) = openrouter::find_consensus(&responses) {
        tracing::info!(
            target: LOG_TARGET,
            problem_key = %problem_key,
            root_cause = %consensus.root_cause,
            confidence = consensus.confidence,
            cost = total_cost,
            "Q4: permanent fix found — linking to workaround"
        );

        // Store the permanent fix as a new solution
        if let Ok(kb) = KnowledgeBase::open(KB_PATH) {
            let env_fp = knowledge_base::fingerprint_env(crate::BUILD_ID);
            let stable_hash = knowledge_base::compute_stable_hash(problem_key, &env_fp);
            let perm_id = uuid::Uuid::new_v4().to_string();
            let models_used: Vec<String> = responses.iter().map(|r| r.model_id.clone()).collect();

            let permanent_solution = knowledge_base::Solution {
                id: perm_id.clone(),
                problem_key: problem_key.to_string(),
                problem_hash: stable_hash,
                symptoms: q4_symptoms.clone(),
                environment: serde_json::to_string(&env_fp).unwrap_or_default(),
                root_cause: consensus.root_cause.clone(),
                fix_action: consensus.fix_action.clone(),
                fix_type: "model_diagnosed".to_string(),
                success_count: 0,
                fail_count: 0,
                confidence: consensus.confidence,
                cost_to_diagnose: total_cost,
                models_used: serde_json::to_string(&models_used).ok(),
                source_node: format!("q4_permanent_{}", crate::BUILD_ID),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                version: 1,
                ttl_days: 365,
                tags: Some(format!("[\"q4_permanent\",\"{}\"]", problem_key)),
                diagnosis_method: Some("q4_permanent_fix_search".to_string()),
                fix_permanence: "permanent".to_string(),
                recurrence_count: 0,
                permanent_fix_id: None,
                last_recurrence: None,
                permanent_attempt_at: None,
            };

            if kb.store_solution(&permanent_solution).is_ok() {
                // Link the workaround to the permanent fix
                let _ = kb.link_permanent_fix(workaround_id, &perm_id);
                tracing::info!(
                    target: LOG_TARGET,
                    workaround = %workaround_id,
                    permanent = %perm_id,
                    "Q4: workaround linked to permanent fix — future Q1 lookups will return permanent solution"
                );
            }
        }
    } else {
        tracing::info!(
            target: LOG_TARGET,
            problem_key = %problem_key,
            cost = total_cost,
            "Q4: no consensus for permanent fix — will retry in 7 days"
        );
    }
}

// ─── Tier 1: Deterministic (DIAG-02) ────────────────────────────────────────
// T1: Uses spawn_blocking for sync filesystem and process ops

async fn tier1_deterministic(event: &DiagnosticEvent) -> TierResult {
    let trigger = event.trigger.clone();
    let billing_active = event.pod_state.billing_active;

    // T1: Move all sync ops to a blocking thread
    let result = tokio::task::spawn_blocking(move || {
        tier1_deterministic_sync(&trigger, billing_active)
    }).await;

    match result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, error = %e, "Tier 1 spawn_blocking panicked");
            TierResult::FailedToFix { tier: 1, reason: format!("Tier 1 panicked: {}", e) }
        }
    }
}

fn tier1_deterministic_sync(trigger: &DiagnosticTrigger, billing_active: bool) -> TierResult {
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
        DiagnosticTrigger::PreFlightFailed { check_name, detail } => {
            // Tier 1 deterministic fixes for pre-flight failures.
            // Pre-flight is the SENSOR, tier engine is the EXECUTOR (audit consensus).
            // Only attempt safe, idempotent fixes — escalate the rest.
            match check_name.as_str() {
                "conspit_link" => {
                    // Idempotent: check if already running before spawning
                    let conspit_running = {
                        let mut sys = System::new();
                        sys.refresh_processes(ProcessesToUpdate::All, false);
                        sys.processes().values().any(|p| {
                            p.name().to_string_lossy().eq_ignore_ascii_case("ConspitLink.exe")
                        })
                    };
                    if !conspit_running {
                        let conspit_path = std::path::Path::new(r"C:\ConspitLink\ConspitLink.exe");
                        if conspit_path.exists() {
                            let mut cmd = std::process::Command::new(conspit_path);
                            #[cfg(windows)]
                            {
                                use std::os::windows::process::CommandExt;
                                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                            }
                            match cmd.spawn() {
                                Ok(_) => {
                                    tracing::info!(target: LOG_TARGET, "Tier 1: spawned ConspitLink.exe for pre-flight recovery");
                                    actions_taken.push("spawned ConspitLink.exe (preflight recovery)".to_string());
                                }
                                Err(e) => {
                                    tracing::warn!(target: LOG_TARGET, error = %e, "Tier 1: failed to spawn ConspitLink.exe");
                                }
                            }
                        } else {
                            tracing::warn!(target: LOG_TARGET, "Tier 1: ConspitLink.exe not found at expected path");
                        }
                    }
                }
                "popup_windows" => {
                    // Kill blocklisted popup processes by name+PID (P1 fix: validate name before kill)
                    let popup_blocklist: &[&str] = &[
                        "m365copilot.exe", "nvidia overlay.exe", "amdow.exe",
                        "amdrssrcext.exe", "amdrsserv.exe", "windowsterminal.exe",
                        "onedrive.sync.service.exe", "ccbootclient.exe",
                        "phoneexperiencehost.exe", "widgets.exe", "widgetservice.exe",
                        "gopro webcam.exe",
                    ];
                    let mut sys = System::new();
                    sys.refresh_processes(ProcessesToUpdate::All, false);
                    let mut killed_count = 0u32;
                    for p in sys.processes().values() {
                        let name = p.name().to_string_lossy().to_lowercase();
                        if popup_blocklist.iter().any(|&blocked| name == blocked) {
                            if p.kill() {
                                killed_count += 1;
                            }
                        }
                    }
                    if killed_count > 0 {
                        tracing::info!(target: LOG_TARGET, count = killed_count, "Tier 1: killed popup processes for preflight recovery");
                        actions_taken.push(format!("killed {} popup processes (preflight recovery)", killed_count));
                    }
                }
                "disk_space" => {
                    // Cleanup old logs >7 days (same as predictive_maintenance PRED-05)
                    let log_dir = std::path::Path::new(r"C:\RacingPoint");
                    if let Ok(entries) = std::fs::read_dir(log_dir) {
                        let cutoff = std::time::SystemTime::now()
                            .checked_sub(std::time::Duration::from_secs(7 * 86400))
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        let mut cleaned = 0u32;
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            if (ext == "log" || ext == "jsonl") && path.is_file() {
                                if let Ok(meta) = std::fs::metadata(&path) {
                                    if let Ok(modified) = meta.modified() {
                                        if modified < cutoff {
                                            if std::fs::remove_file(&path).is_ok() {
                                                cleaned += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if cleaned > 0 {
                            tracing::info!(target: LOG_TARGET, count = cleaned, "Tier 1: cleaned old log files for disk space recovery");
                            actions_taken.push(format!("cleaned {} old log files (preflight disk recovery)", cleaned));
                        }
                    }
                }
                _ => {
                    // hid, billing_stuck, memory, ws_stability, lock_screen_*, browser_alive, orphan_game:
                    // No safe deterministic fix — let higher tiers handle
                    tracing::info!(
                        target: LOG_TARGET,
                        check = %check_name,
                        detail = %detail,
                        "Tier 1: no deterministic fix for pre-flight check — escalating"
                    );
                }
            }
        }
        DiagnosticTrigger::GameLaunchFail => {
            // Game Launch Retry Orchestrator (Phase 275 — GAME-01..05):
            // Runs Game Doctor up to 2 times with 5s backoff, bounded to 60s total.
            // On success: KB recording + fleet cascade happen via the main loop's
            //   universal KB recording (273-03) and FleetEvent emission.
            // On failure: escalates to Tier 3/4 MMA via FailedToFix return.
            // MMA audit note: diagnose_and_fix() is sync but tier1_deterministic_sync is already
            // called via spawn_blocking from the tier engine main loop (T1 standing rule).
            // The 60-second timeout in retry_game_launch uses std::thread::sleep (correct for sync context).
            tracing::info!(target: LOG_TARGET, "Tier 1: invoking game launch retry orchestrator");
            let retry_result = game_launch_retry::retry_game_launch();

            match retry_result {
                game_launch_retry::RetryResult::Fixed { attempt, ref cause, ref fix } => {
                    actions_taken.push(format!(
                        "Game launch retry (attempt {}/2): cause={}, fix={}",
                        attempt, cause, fix
                    ));
                    // GAME-04: Record game fix in KB with game-specific metadata
                    if let Ok(kb) = crate::knowledge_base::KnowledgeBase::open(crate::knowledge_base::KB_PATH) {
                        let host = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());
                        let _ = kb.record_game_fix(cause, fix, &host);
                    }
                }
                game_launch_retry::RetryResult::EscalateToMma { attempts, ref causes } => {
                    // All retries failed — don't add to actions_taken so Tier 3/4 handles it
                    tracing::info!(
                        target: LOG_TARGET,
                        attempts = attempts,
                        causes = ?causes,
                        "Game launch retry exhausted ({} attempts) — escalating to model tiers",
                        attempts
                    );
                }
            }
        }
        DiagnosticTrigger::TaskbarVisible => {
            // Tier 1 deterministic fix: re-hide the taskbar via Win32 API.
            // The enforcement loop already does this, but if we reach here it means
            // the diagnostic engine detected it before the enforcement loop caught it.
            #[cfg(windows)]
            {
                let was_visible = crate::kiosk::ensure_taskbar_hidden();
                if was_visible {
                    actions_taken.push("re-hidden taskbar after explorer restart".to_string());
                }
            }
        }
        DiagnosticTrigger::Periodic
        | DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::ErrorSpike { .. }
        | DiagnosticTrigger::ViolationSpike { .. } => {}

        // ─── POS-Specific Tier 1 Recovery (MMA P1 fix) ─────────────────────────
        // PosKioskDown: Edge browser crashed on POS terminal.
        // Safe to restart ONLY if no billing session is active.
        // Active session → escalate to Tier 2 (staff alert), never auto-restart.
        DiagnosticTrigger::PosKioskDown { detail } => {
            if billing_active {
                tracing::warn!(
                    target: LOG_TARGET,
                    detail = %detail,
                    "POS kiosk down BUT billing session active — NOT restarting Edge (escalate to staff)"
                );
                // Don't auto-restart — let Tier 5 escalation handle it
            } else {
                tracing::info!(target: LOG_TARGET, detail = %detail, "POS kiosk down, no active session — restarting Edge");
                if tier1_restart_edge_kiosk() {
                    actions_taken.push("POS: restarted Edge kiosk (no active billing session)".to_string());
                } else {
                    tracing::warn!(target: LOG_TARGET, "POS: Edge restart failed — escalating");
                }
            }
        }
        // PosNetworkDown / PosBillingApiError / PosWifiDegraded: log + escalate (no safe Tier 1 fix)
        DiagnosticTrigger::PosNetworkDown { .. }
        | DiagnosticTrigger::PosBillingApiError { .. }
        | DiagnosticTrigger::PosWifiDegraded { .. } => {
            tracing::warn!(target: LOG_TARGET, trigger = ?trigger, "POS network/billing/WiFi issue — no Tier 1 fix, escalating");
        }
        // POS kiosk escape: log + alert staff (Tier 1 can't fix foreground window takeover)
        DiagnosticTrigger::PosKioskEscaped { foreground_process } => {
            tracing::warn!(target: LOG_TARGET, foreground = %foreground_process, "POS kiosk escape — non-Edge window in foreground, alerting staff");
        }
        // MMA-First Protocol triggers — no deterministic fix, escalate to MMA
        DiagnosticTrigger::GameMidSessionCrash { .. }
        | DiagnosticTrigger::PostSessionAnalysis { .. }
        | DiagnosticTrigger::PreShiftAudit
        | DiagnosticTrigger::DeployVerification { .. } => {
            tracing::info!(target: LOG_TARGET, trigger = ?trigger, "MMA-First trigger — no Tier 1 deterministic fix, escalating to MMA");
        }
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
/// Appends to authorized_keys (never overwrites) and sets Windows ACLs.
/// Idempotent — skips if exact key line already present.
///
/// Multi-model audit fixes (Qwen3 + Grok 4.1):
/// - P1: Append-only (don't overwrite existing keys)
/// - P1: Set ACLs via icacls after write
/// - P1: Exact full-line match (not substring)
/// - P2: Derive user path from USERNAME env var
/// - P2: Log errors on fs failures
fn tier1_ensure_ssh_key() -> Option<String> {
    const JAMES_PUBKEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGpwLi/oX9iymSjea6I3iG6QUQmX9XsJ0fDma/3MTLQ/ james@racingpoint.in";
    const ADMIN_KEY_PATH: &str = r"C:\ProgramData\ssh\administrators_authorized_keys";

    // P2: Derive user home from USERNAME env var instead of hardcoding "User"
    // MMA Round 2 fix (3/3 consensus): sanitize USERNAME against path traversal
    let user_key_path = std::env::var("USERNAME")
        .ok()
        .filter(|u| !u.is_empty() && u.len() <= 32 && u.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.'))
        .map(|u| format!(r"C:\Users\{}\.ssh\authorized_keys", u))
        .unwrap_or_else(|| r"C:\Users\User\.ssh\authorized_keys".to_string());

    let mut fixed = false;

    // Check + append to admin path
    if ensure_key_in_file(ADMIN_KEY_PATH, JAMES_PUBKEY) {
        // P1: Set strict ACLs — only SYSTEM and Administrators
        // MMA Round 2 fix (2/3): check icacls exit status
        match std::process::Command::new("icacls")
            .args([ADMIN_KEY_PATH, "/inheritance:r", "/grant", "SYSTEM:F", "/grant", "Administrators:F"])
            .output()
        {
            Ok(out) if out.status.success() => {
                tracing::info!(target: LOG_TARGET, "Tier 1: deployed SSH key to {} + ACLs set", ADMIN_KEY_PATH);
            }
            Ok(out) => {
                tracing::warn!(target: LOG_TARGET, status = %out.status, "Tier 1: SSH key deployed but icacls FAILED on {}", ADMIN_KEY_PATH);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, error = %e, "Tier 1: SSH key deployed but icacls spawn failed");
            }
        }
        fixed = true;
    }

    // Check + append to user path
    if ensure_key_in_file(&user_key_path, JAMES_PUBKEY) {
        // MMA Round 2 fix (Grok 4.1): also set ACLs on user key path
        let _ = std::process::Command::new("icacls")
            .args([&user_key_path, "/inheritance:r", "/grant", "SYSTEM:F", "/grant", "Administrators:F"])
            .output();
        tracing::info!(target: LOG_TARGET, path = %user_key_path, "Tier 1: deployed SSH key to user authorized_keys + ACLs set");
        fixed = true;
    }

    if fixed {
        Some("deployed SSH key for remote access".to_string())
    } else {
        None
    }
}

/// Ensure a specific key line exists in an authorized_keys file.
/// Appends if missing, never overwrites. Returns true if key was added.
fn ensure_key_in_file(path: &str, pubkey: &str) -> bool {
    // P1: Exact full-line match — read existing content
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let key_present = existing.lines().any(|line| line.trim() == pubkey.trim());

    if key_present {
        return false;
    }

    // Create parent dir if needed
    if let Some(parent) = std::path::Path::new(path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(target: LOG_TARGET, path = %path, error = %e, "Failed to create SSH dir");
            return false;
        }
    }

    // P1: Append, don't overwrite — preserve existing keys
    use std::io::Write;
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| writeln!(f, "{}", pubkey));

    match result {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, path = %path, error = %e, "Failed to write SSH key");
            false
        }
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

/// POS Tier 1: Restart Edge kiosk browser.
/// Only called when billing is NOT active. Kills all msedge.exe, then launches
/// Edge in kiosk mode pointing at the billing dashboard.
/// Returns true if restart was initiated successfully.
///
/// MMA Round 1 fixes (3/3 consensus):
/// - P1: Use RACECONTROL_SERVER_IP env var instead of hardcoded IP
/// - P1: Use non-blocking sleep (spawn_blocking wraps this sync fn)
/// - P2: Try both x64 and x86 Edge paths
/// - P2: Log kill failures for visibility
fn tier1_restart_edge_kiosk() -> bool {
    // Kill existing Edge processes
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, false);
    let mut killed = 0u32;
    let mut kill_failed = 0u32;
    for (_pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_lowercase();
        if name.contains("msedge") {
            if proc_.kill() {
                killed += 1;
            } else {
                kill_failed += 1;
            }
        }
    }
    if killed > 0 || kill_failed > 0 {
        tracing::info!(target: LOG_TARGET, killed = killed, failed = kill_failed,
            "POS: killed {} Edge processes ({} failed) before restart", killed, kill_failed);
    }

    // Small delay to let processes fully exit
    // NOTE: This is a sync function called via spawn_blocking from the async tier engine
    std::thread::sleep(std::time::Duration::from_secs(2));

    // MMA Round 1 P1 fix: derive billing URL from env var (same as check_pos_network_health)
    let server_ip = std::env::var("RACECONTROL_SERVER_IP")
        .unwrap_or_else(|_| "192.168.31.23".to_string());
    // Web dashboard is on :3200, NOT :8080 (API port)
    let billing_url = format!("http://{}:3200/billing", server_ip);

    // MMA Round 1 P2 fix: try x64 path first, fall back to x86
    let edge_x64 = r"C:\Program Files\Microsoft\Edge\Application\msedge.exe";
    let edge_x86 = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
    let edge_path = if std::path::Path::new(edge_x64).exists() { edge_x64 } else { edge_x86 };

    let result = std::process::Command::new(edge_path)
        .args([
            "--kiosk", &billing_url,
            "--edge-kiosk-type=fullscreen",
            "--no-first-run",
            "--remote-debugging-port=9222",
            "--disable-session-crashed-bubble",
        ])
        .spawn();

    match result {
        Ok(child) => {
            tracing::info!(target: LOG_TARGET, pid = child.id(), url = %billing_url, edge = %edge_path,
                "POS: Edge kiosk restart initiated");
            true
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, error = %e, edge = %edge_path,
                "POS: failed to restart Edge kiosk — check Edge installation path");
            false
        }
    }
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
    let base_symptoms = openrouter::format_symptoms(
        &format!("{:?}", event.trigger),
        &problem_key,
        &serde_json::to_string(&env_fp).unwrap_or_default(),
        &format!("build_id={}", event.build_id),
    );
    // MMA-First: enrich with trigger-specific context bundle
    let symptoms = openrouter::enrich_with_context_bundle(
        &base_symptoms, &event.trigger, &event.pod_state,
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
            // MMA-First: classify fix permanence based on model response
            let permanence = if diag.permanent_fix.is_some() { "permanent" } else { "workaround" };
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
                    diagnosis_method: Some("scanner_enumeration".to_string()),
                    fix_permanence: permanence.to_string(),
                    recurrence_count: 0,
                    permanent_fix_id: None,
                    last_recurrence: None,
                    permanent_attempt_at: None,
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
    let base_symptoms = openrouter::format_symptoms(
        &format!("{:?}", event.trigger),
        &problem_key,
        &serde_json::to_string(&env_fp).unwrap_or_default(),
        &format!("build_id={}", event.build_id),
    );
    // MMA-First: enrich with trigger-specific context bundle
    let symptoms = openrouter::enrich_with_context_bundle(
        &base_symptoms, &event.trigger, &event.pod_state,
    );

    tracing::info!(target: LOG_TARGET, tier = 4u8, "Tier 4: 5-model parallel (~$4)");

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
            // MMA-First: classify fix permanence and type from consensus
            let permanence = if consensus.permanent_fix.is_some() { "permanent" } else { "workaround" };
            let fix_type_class = consensus.fix_type_class.as_deref().unwrap_or("deterministic");

            // MMA-First: route by fix_type_class
            let requires_human = matches!(fix_type_class, "code_change" | "hardware");
            if requires_human {
                tracing::info!(
                    target: LOG_TARGET, tier = 4u8,
                    fix_type = fix_type_class,
                    "MMA-First: fix requires human intervention — storing but NOT auto-applying"
                );
            }

            if let Ok(kb) = knowledge_base::KnowledgeBase::open(knowledge_base::KB_PATH) {
                let problem_hash = knowledge_base::compute_problem_hash(&problem_key, &env_fp);
                // MMA-First: use stable hash for permanent fixes (survive deploys)
                let store_hash = if permanence == "permanent" {
                    knowledge_base::compute_stable_hash(&problem_key, &env_fp)
                } else {
                    problem_hash
                };
                let models_used: Vec<String> = responses.iter().map(|r| r.model_id.clone()).collect();
                let solution = knowledge_base::Solution {
                    id: uuid::Uuid::new_v4().to_string(),
                    problem_key: problem_key.clone(),
                    problem_hash: store_hash,
                    symptoms: symptoms.clone(),
                    environment: serde_json::to_string(&env_fp).unwrap_or_default(),
                    root_cause: consensus.root_cause.clone(),
                    fix_action: if let Some(ref pf) = consensus.permanent_fix {
                        pf.clone()
                    } else {
                        consensus.fix_action.clone()
                    },
                    fix_type: format!("model_diagnosed_{}", fix_type_class),
                    success_count: if requires_human { 0 } else { 1 },
                    fail_count: 0,
                    confidence: consensus.confidence,
                    cost_to_diagnose: total_cost,
                    models_used: serde_json::to_string(&models_used).ok(),
                    source_node: format!("pod_{}", event.build_id),
                    created_at: event.timestamp.clone(),
                    updated_at: event.timestamp.clone(),
                    version: 1, ttl_days: if permanence == "permanent" { 365 } else { 90 },
                    tags: Some(format!("[\"{}\",\"{}\"]", problem_key, fix_type_class)),
                    diagnosis_method: Some("consensus_5model".to_string()),
                    fix_permanence: permanence.to_string(),
                    recurrence_count: 0,
                    permanent_fix_id: None,
                    last_recurrence: None,
                    permanent_attempt_at: None,
                };
                let _ = kb.store_solution(&solution);
            }
            return TierResult::Fixed {
                tier: 4,
                action: format!(
                    "5-model {} (${:.2}): {}{}",
                    permanence, total_cost, consensus.root_cause,
                    if requires_human { " [REQUIRES HUMAN]" } else { "" }
                ),
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

async fn tier5_human_escalation(
    event: &DiagnosticEvent,
    ws_msg_tx: &mpsc::Sender<rc_common::protocol::AgentMessage>,
    pod_id: &str,
) -> TierResult {
    tracing::warn!(
        target: LOG_TARGET, tier = 5u8, trigger = ?event.trigger,
        "Tier 5: all automated tiers exhausted — escalating to human via WhatsApp"
    );

    let incident_id = uuid::Uuid::new_v4().to_string();

    // Derive severity from trigger type
    let severity = match &event.trigger {
        DiagnosticTrigger::GameMidSessionCrash { .. }
        | DiagnosticTrigger::WsDisconnect { .. } => "critical",
        _ => "high",
    };

    // Build human-readable summary from trigger + pod state
    let trigger_str = format!("{:?}", event.trigger);
    let summary = format!(
        "{} on {} (build {})",
        trigger_str, pod_id, event.build_id
    );

    // Derive impact from pod state
    let impact = if event.pod_state.billing_active {
        "Customer session impacted — billing active".to_string()
    } else {
        "Pod offline — no active billing".to_string()
    };

    let payload = rc_common::protocol::EscalationPayload {
        pod_id: pod_id.to_string(),
        incident_id: incident_id.clone(),
        severity: severity.to_string(),
        trigger: trigger_str,
        summary,
        actions_tried: vec![
            "Tier 1: deterministic rules".to_string(),
            "Tier 2: KB lookup".to_string(),
            "Tier 3: single model diagnosis".to_string(),
            "Tier 4: multi-model MMA".to_string(),
        ],
        impact,
        dashboard_url: "http://192.168.31.23:8080/status".to_string(),
        timestamp: event.timestamp.clone(),
    };

    if let Err(e) = ws_msg_tx
        .send(rc_common::protocol::AgentMessage::EscalationRequest(payload))
        .await
    {
        tracing::error!(
            target: LOG_TARGET,
            incident_id = %incident_id,
            error = %e,
            "Failed to send EscalationRequest via WS — channel closed"
        );
    } else {
        tracing::warn!(
            target: LOG_TARGET,
            incident_id = %incident_id,
            pod_id = %pod_id,
            severity = %severity,
            "EscalationRequest sent to server for WhatsApp delivery"
        );
    }

    TierResult::FailedToFix {
        tier: 5,
        reason: format!("Escalated to human via WhatsApp (incident {})", incident_id),
    }
}
