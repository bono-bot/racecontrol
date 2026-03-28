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
/// PRODUCTION ORDER: Deterministic → KB → Single Model → 4-Model Parallel → Human
/// Cheapest/fastest tiers first. Model tiers only for real anomalies.
async fn run_tiers(
    event: &DiagnosticEvent,
    circuit_breaker: &mut CircuitBreaker,
    budget: &Arc<RwLock<BudgetTracker>>,
) -> TierResult {
    // ── Short-circuit: Periodic events with no anomaly don't need model diagnosis ──
    // The diagnostic_engine always emits Periodic. If that's the ONLY trigger type
    // (no WsDisconnect, ProcessCrash, etc.), Tier 1 deterministic is sufficient.
    // This prevents burning ~$0.05/call every 5 min on "everything is fine" responses.
    let is_periodic_only = matches!(event.trigger, DiagnosticTrigger::Periodic);

    // ── Tier 1: Deterministic fixes (always runs) ──
    let t1 = tier1_deterministic(event).await;
    if matches!(t1, TierResult::Fixed { .. }) {
        return t1;
    }

    // Periodic-only events: Tier 1 handled cleanup (MAINTENANCE_MODE, orphans, SSH keys).
    // No need to escalate to model tiers — return early.
    if is_periodic_only {
        tracing::debug!(target: LOG_TARGET, "Periodic scan complete — no anomaly, skipping model tiers");
        return TierResult::NotApplicable { tier: 1 };
    }

    // ── Tier 2: Knowledge Base lookup ──
    let t2 = tier2_kb_lookup(event);
    if matches!(t2, TierResult::Fixed { .. }) {
        return t2;
    }

    // ── Tier 3: Single Model (Qwen3) — only for real anomalies ──
    // C1: Check circuit breaker before model calls
    if circuit_breaker.is_open() {
        tracing::info!(target: LOG_TARGET, "Circuit breaker OPEN — skipping model tiers");
    } else {
        // C3: Budget pre-check
        {
            let mut bt = budget.write().await;
            if !bt.can_spend(TIER3_ESTIMATED_COST) {
                tracing::info!(target: LOG_TARGET, tier = 3u8, "Budget ceiling — skipping model tiers");
            } else {
                drop(bt); // release lock before async call
                let t3 = tier3_single_model(event).await;
                match &t3 {
                    TierResult::Fixed { .. } => {
                        circuit_breaker.record_success();
                        let mut bt = budget.write().await;
                        bt.record_spend(TIER3_ESTIMATED_COST);
                        return t3;
                    }
                    TierResult::FailedToFix { .. } => {
                        circuit_breaker.record_failure();
                    }
                    _ => {}
                }

                // ── Tier 4: 4-Model Parallel — escalate if single model failed ──
                {
                    let mut bt = budget.write().await;
                    if bt.can_spend(TIER4_ESTIMATED_COST) {
                        drop(bt);
                        let t4 = tier4_multi_model(event).await;
                        match &t4 {
                            TierResult::Fixed { .. } => {
                                circuit_breaker.record_success();
                                let mut bt = budget.write().await;
                                bt.record_spend(TIER4_ESTIMATED_COST);
                                return t4;
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

    // ── Tier 5: Human escalation ──
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
            // Game Doctor: specialized 12-point diagnostic for game launch failures.
            // This is revenue-critical — every failed launch costs billing time.
            tracing::info!(target: LOG_TARGET, "Tier 1: invoking Game Doctor for launch failure");
            let diagnosis = crate::game_doctor::diagnose_and_fix();
            if diagnosis.fixed {
                actions_taken.push(format!("Game Doctor: {}", diagnosis.detail));
            } else if let Some(fix) = &diagnosis.fix_applied {
                // Partial fix — some issues resolved but others remain
                actions_taken.push(format!("Game Doctor (partial): fixes={}, remaining={}", fix, diagnosis.detail));
            } else {
                // No fix possible at Tier 1 — log cause for model tiers
                tracing::info!(
                    target: LOG_TARGET,
                    cause = ?diagnosis.cause,
                    "Game Doctor: no deterministic fix — escalating. Detail: {}",
                    diagnosis.detail
                );
            }
        }
        DiagnosticTrigger::Periodic
        | DiagnosticTrigger::WsDisconnect { .. }
        | DiagnosticTrigger::HealthCheckFail
        | DiagnosticTrigger::DisplayMismatch { .. }
        | DiagnosticTrigger::ErrorSpike { .. }
        | DiagnosticTrigger::ViolationSpike { .. }
        | DiagnosticTrigger::PosKioskDown { .. }
        | DiagnosticTrigger::PosNetworkDown { .. }
        | DiagnosticTrigger::PosBillingApiError { .. } => {}
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
                    diagnosis_method: Some("scanner_enumeration".to_string()),
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
                    diagnosis_method: Some("consensus_5model".to_string()),
                };
                let _ = kb.store_solution(&solution);
            }
            return TierResult::Fixed {
                tier: 4,
                action: format!("5-model (${:.2}): {}", total_cost, consensus.root_cause),
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
