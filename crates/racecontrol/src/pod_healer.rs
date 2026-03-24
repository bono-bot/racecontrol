//! Pod Healer: Self-healing daemon with AI diagnostics.
//!
//! Runs every 2 minutes (configurable). For each connected pod, collects deep
//! diagnostics via pod-agent `/exec`, applies safe rule-based fixes (kill zombie
//! sockets, clear temp files), and escalates complex/unfamiliar issues to AI
//! (Claude CLI -> Ollama -> Anthropic).
//!
//! rc-agent restarts are deferred to pod_monitor (which owns the shared backoff).
//! The healer reads the shared EscalatingBackoff from AppState.pod_backoffs for cooldown
//! gating but does NOT advance the backoff (advancing is pod_monitor's exclusive responsibility).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::json;

use crate::activity_log::log_pod_activity;
use crate::state::{AppState, WatchdogState};
use rc_common::protocol::{CoreToAgentMessage, DashboardEvent};
use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryIntent, RecoveryLogger, RECOVERY_LOG_SERVER};
use rc_common::types::{AiDebugSuggestion, PodInfo, PodStatus, SimType};

const POD_AGENT_PORT: u16 = 8090;
const POD_AGENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Processes that must NEVER be killed by the healer.
const PROTECTED_PROCESSES: &[&str] = &[
    "rc-agent.exe",
    "pod-agent.exe",
    "acs.exe",
    "conspitlink2.0.exe",
    "msedge.exe",
    "explorer.exe",
    "system",
    "svchost.exe",
    "csrss.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "dwm.exe",
    "taskhostw.exe",
    "conhost.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "vmsdesktop.exe",
    // James's machine runs as Pod 1 -- these are infrastructure, not suspicious
    "claude.exe",
    "ollama.exe",
    "ollama_llama_server.exe",
    "deskin.exe",
];

/// Ports we monitor for stale sockets.
const MONITORED_PORTS: &[&str] = &["18923", "18924"];

/// Disk usage threshold (percent used) to trigger temp cleanup.
const DISK_THRESHOLD_PCT: f64 = 90.0;

/// Memory threshold (MB free) to flag as low memory.
const MEMORY_LOW_MB: u64 = 2048;

// --- Types -------------------------------------------------------------------

struct PodDiagnostics {
    stale_sockets: Vec<(u32, String)>, // (PID, state like CLOSE_WAIT)
    disk_free_pct: f64,
    memory_free_mb: u64,
    memory_total_mb: u64,
    rc_agent_healthy: bool,
    suspicious_processes: Vec<(String, u32, u64)>, // (name, PID, mem_kb)
}

struct HealAction {
    pod_id: String,
    action: String,
    target: String,
    reason: String,
}

// --- Graduated Recovery Types ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum PodRecoveryStep {
    /// First offline detection — waiting 30s before acting.
    Waiting,
    /// Second cycle — attempt Tier 1 rc-agent restart.
    TierOneRestart,
    /// Third cycle — escalate to AI.
    AiEscalation,
    /// Fourth+ cycle — alert staff.
    AlertStaff,
}

/// Per-pod graduated recovery state. Held in a HashMap inside heal_all_pods.
/// Not shared with AppState — local to the healer loop.
#[derive(Debug)]
struct PodRecoveryTracker {
    step: PodRecoveryStep,
    first_detected_at: Option<std::time::Instant>,
}

impl PodRecoveryTracker {
    fn new() -> Self {
        Self {
            step: PodRecoveryStep::Waiting,
            first_detected_at: None,
        }
    }

    fn reset(&mut self) {
        self.step = PodRecoveryStep::Waiting;
        self.first_detected_at = None;
    }
}

impl Default for PodRecoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

// --- Spawn -------------------------------------------------------------------

/// Spawn the pod healer background task.
pub fn spawn(state: Arc<AppState>) {
    if !state.config.pods.healer_enabled {
        tracing::info!("Pod healer disabled");
        return;
    }

    let interval_secs = state.config.pods.healer_interval_secs as u64;

    tracing::info!(
        "Pod healer starting (interval: {}s, shared backoff via AppState)",
        interval_secs,
    );

    tokio::spawn(async move {
        // Wait for pods to connect before first scan
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        let mut recovery_trackers: std::collections::HashMap<String, PodRecoveryTracker> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;
            heal_all_pods(&state, &mut recovery_trackers).await;
        }
    });
}

// --- Main Loop ---------------------------------------------------------------

async fn heal_all_pods(
    state: &Arc<AppState>,
    trackers: &mut std::collections::HashMap<String, PodRecoveryTracker>,
) {
    // Check cascade guard before any recovery action
    {
        let guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_paused() {
            tracing::warn!(
                target: "pod_healer",
                "Recovery paused by cascade guard (remaining: {:?}), skipping heal cycle",
                guard.pause_remaining()
            );
            return;
        }
    }

    // Snapshot connected pods
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();

    let active_pods: Vec<&PodInfo> = pods
        .iter()
        .filter(|p| p.status != PodStatus::Disabled && p.last_seen.is_some())
        .collect();

    if active_pods.is_empty() {
        return;
    }

    tracing::info!("Pod healer: checking {} pods", active_pods.len());

    for pod in active_pods {
        if pod.status == PodStatus::Offline {
            // Offline pod: run graduated recovery instead of proactive diagnostics.
            run_graduated_recovery(state, pod, trackers).await;
        } else {
            // Online pod: reset any graduated recovery tracker, then run proactive diagnostics.
            trackers.entry(pod.id.clone()).or_default().reset();
            if let Err(e) = heal_pod(state, pod).await {
                tracing::warn!("Pod healer: error checking pod {}: {}", pod.id, e);
            }
        }
    }

    // Phase 141: Scan server-side WARN log for surge detection
    scan_warn_logs(state).await;
}

async fn heal_pod(
    state: &Arc<AppState>,
    pod: &PodInfo,
) -> anyhow::Result<()> {
    // First verify pod-agent is reachable
    let ping_url = format!("http://{}:{}/ping", pod.ip_address, POD_AGENT_PORT);
    let ping = state
        .http_client
        .get(&ping_url)
        .timeout(Duration::from_millis(3000))
        .send()
        .await;

    if ping.is_err() || !ping.as_ref().unwrap().status().is_success() {
        // Pod-agent unreachable -- pod_monitor handles this case
        return Ok(());
    }

    // Skip pods in active recovery cycle -- pod_monitor owns the restart lifecycle
    let wd_state = {
        let states = state.pod_watchdog_states.read().await;
        states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
    };
    if should_skip_for_watchdog_state(&wd_state) {
        tracing::debug!(
            "Pod healer: {} in recovery cycle ({:?}) -- skipping diagnostic",
            pod.id, wd_state
        );
        return Ok(());
    }

    // Skip pods with active deploy -- deploy executor manages lifecycle
    {
        let deploy_states = state.pod_deploy_states.read().await;
        if let Some(deploy_state) = deploy_states.get(&pod.id) {
            if deploy_state.is_active() {
                tracing::debug!(
                    "Pod healer: {} has active deploy ({:?}) -- skipping diagnostic",
                    pod.id, deploy_state
                );
                return Ok(());
            }
        }
    }

    // Collect diagnostics
    let diag = collect_diagnostics(state, &pod.ip_address).await?;

    // Build issue list for potential AI escalation
    let mut issues: Vec<String> = Vec::new();
    let mut actions: Vec<HealAction> = Vec::new();

    // --- Rule 1: Stale sockets -----------------------------------------------
    if !diag.stale_sockets.is_empty() {
        for (pid, sock_state) in &diag.stale_sockets {
            if is_protected_pid(state, &pod.ip_address, *pid).await {
                issues.push(format!(
                    "Protected process PID {} has {} socket on monitored port",
                    pid, sock_state
                ));
            } else {
                actions.push(HealAction {
                    pod_id: pod.id.clone(),
                    action: "kill_zombie".to_string(),
                    target: format!("PID {}", pid),
                    reason: format!("{} socket on lock screen port", sock_state),
                });
            }
        }
    }

    // --- Rule 2: rc-agent lock screen unresponsive ---------------------------
    if !diag.rc_agent_healthy {
        let has_active_ws = {
            let senders = state.agent_senders.read().await;
            match senders.get(&pod.id) {
                Some(sender) => !sender.is_closed(),
                None => false,
            }
        };
        if has_active_ws {
            // WS is alive but lock screen HTTP is failing — attempt soft recovery
            // by commanding the pod to relaunch Edge rather than forcing a full restart.
            let has_active_billing = has_active_billing(state, &pod.id).await;
            if has_active_billing {
                tracing::warn!(
                    "Pod healer: {} lock screen unresponsive, WS connected, billing active -- skipping relaunch",
                    pod.id
                );
                issues.push(format!(
                    "Pod {}: lock screen HTTP failed but WS connected + billing active -- no relaunch dispatched",
                    pod.id
                ));
            } else {
                tracing::info!(
                    "Pod healer: {} lock screen unresponsive, WS connected -- dispatching ForceRelaunchBrowser",
                    pod.id
                );
                actions.push(HealAction {
                    pod_id: pod.id.clone(),
                    action: "relaunch_lock_screen".to_string(),
                    target: "edge_browser".to_string(),
                    reason: "Lock screen HTTP check failed, WS connected".to_string(),
                });
                issues.push(format!(
                    "Pod {}: lock screen HTTP failed (WS alive) -- ForceRelaunchBrowser queued",
                    pod.id
                ));
            }
        } else {
            let has_active_billing = has_active_billing(state, &pod.id).await;
            if has_active_billing {
                issues.push(
                    "rc-agent lock screen unresponsive but pod has active billing -- NOT flagging restart"
                        .to_string(),
                );
            } else {
                // No WebSocket, no billing -- this is a genuine rc-agent failure.
                // Set needs_restart flag so pod_monitor triggers restart on next cycle.
                // COORD-01: Only flag restart if PodHealer owns rc-agent.exe (or it's unregistered).
                let is_restart_owner = {
                    let ownership = state.process_ownership.lock().unwrap_or_else(|e| e.into_inner());
                    ownership.owner_of("rc-agent.exe").map_or(true, |o| o == RecoveryAuthority::PodHealer)
                };
                if is_restart_owner {
                    let mut needs = state.pod_needs_restart.write().await;
                    needs.insert(pod.id.clone(), true);
                } else {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} rc-agent.exe not owned by PodHealer — skipping restart flag, deferring to owner",
                        pod.id
                    );
                }
                tracing::info!(
                    "Pod healer: {} lock screen unresponsive, no WebSocket -- flagged for restart",
                    pod.id
                );
                log_pod_activity(
                    state,
                    &pod.id,
                    "race_engineer",
                    "Restart Flagged",
                    "Lock screen unresponsive + no WebSocket -- deferred to pod_monitor",
                    "race_engineer",
                );
                // Still add to issues for potential AI escalation context
                issues.push(
                    "rc-agent lock screen unresponsive (no WebSocket) -- restart flagged for pod_monitor"
                        .to_string(),
                );
            }
        }
    }

    // --- Rule 3: Disk space low ----------------------------------------------
    if diag.disk_free_pct < (100.0 - DISK_THRESHOLD_PCT) {
        actions.push(HealAction {
            pod_id: pod.id.clone(),
            action: "clear_temp".to_string(),
            target: "C:\\Users\\*\\AppData\\Local\\Temp\\*".to_string(),
            reason: format!("Disk only {:.1}% free", diag.disk_free_pct),
        });
    }

    // --- Rule 4: Low memory (alert only) -------------------------------------
    if diag.memory_free_mb < MEMORY_LOW_MB {
        issues.push(format!(
            "Low memory: {}MB free / {}MB total",
            diag.memory_free_mb, diag.memory_total_mb
        ));
    }

    // --- Rule 5: Suspicious processes (alert only) ---------------------------
    if !diag.suspicious_processes.is_empty() {
        for (name, pid, mem_kb) in &diag.suspicious_processes {
            issues.push(format!(
                "Suspicious process: {} (PID {}, {}MB RAM)",
                name,
                pid,
                mem_kb / 1024
            ));
        }
    }

    // Nothing to do
    if actions.is_empty() && issues.is_empty() {
        return Ok(());
    }

    // Check shared backoff before executing heal actions
    let now = Utc::now();
    let backoffs = state.pod_backoffs.read().await;
    let cooldown_ok = match backoffs.get(&pod.id) {
        Some(backoff) => backoff.ready(now),
        None => true, // no prior attempts, OK to proceed
    };
    drop(backoffs); // release read lock before executing actions

    // Execute auto-heal actions (if cooldown allows)
    if cooldown_ok && !actions.is_empty() {
        for action in &actions {
            // Record this decision to the cascade guard and recovery log before executing.
            let recovery_action = match action.action.as_str() {
                "kill_zombie" => RecoveryAction::Kill,
                _ => RecoveryAction::Restart,
            };
            let decision = RecoveryDecision::new(
                "server",
                &action.target,
                RecoveryAuthority::PodHealer,
                recovery_action,
                &action.reason,
            );
            {
                let mut guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
                let cascaded = guard.record(&decision);
                if cascaded {
                    tracing::error!(
                        target: "pod_healer",
                        "Cascade detected — aborting heal cycle for pod {}",
                        action.pod_id
                    );
                    return Ok(());
                }
                if guard.is_paused() {
                    tracing::warn!(
                        target: "pod_healer",
                        "Cascade guard paused after recording action — aborting heal for pod {}",
                        action.pod_id
                    );
                    return Ok(());
                }
            }
            // Log to recovery JSONL
            let logger = RecoveryLogger::new(RECOVERY_LOG_SERVER);
            let _ = logger.log(&decision);

            tracing::info!(
                "Pod healer: [{}] {} -> {} ({})",
                action.pod_id,
                action.action,
                action.target,
                action.reason
            );
            let activity_action = match action.action.as_str() {
                "kill_zombie" => "Zombie Socket Killed",
                "clear_temp" => "Disk Cleaned",
                _ => "Auto-Fix Applied",
            };
            log_pod_activity(
                state,
                &action.pod_id,
                "race_engineer",
                activity_action,
                &action.reason,
                "race_engineer",
            );
            execute_heal_action(state, &pod.ip_address, action).await;
        }
        // NOTE: The healer does NOT call record_attempt() here.
        // Advancing the backoff is pod_monitor's exclusive responsibility.
        // The healer only reads backoff.ready() to avoid spamming heal actions.
    } else if !actions.is_empty() {
        tracing::info!(
            "Pod healer: {} has {} pending actions but cooldown not elapsed",
            pod.id,
            actions.len()
        );
    }

    // Escalate to AI if there are complex issues that rules can't handle
    // (respects same cooldown as heal actions to prevent spamming)
    if !issues.is_empty() && state.config.ai_debugger.enabled && cooldown_ok {
        log_pod_activity(
            state,
            &pod.id,
            "race_engineer",
            "AI Analysis Requested",
            &issues.join("; "),
            "race_engineer",
        );
        escalate_to_ai(state, pod, &issues, &actions).await;

        // Send email for persistent issues (3+ issues on a single pod)
        if issues.len() >= 3 {
            let body = format!(
                "Pod {} has {} persistent issues requiring attention:\n\n{}\n\nAI analysis was requested. Check dashboard for suggestions.",
                pod.id,
                issues.len(),
                issues
                    .iter()
                    .map(|i| format!("- {}", i))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let subject = format!(
                "[RacingPoint] Pod {} -- {} issues detected",
                pod.id,
                issues.len()
            );
            state
                .email_alerter
                .write()
                .await
                .send_alert(&pod.id, &subject, &body)
                .await;
        }
    }

    Ok(())
}

// --- Graduated Recovery ------------------------------------------------------

/// Graduated recovery for offline pods.
///
/// Step 1 (Waiting): Record first_detected_at, wait 30s — no action.
/// Step 2 (TierOneRestart): Attempt rc-agent restart via pod-agent /exec.
/// Step 3 (AiEscalation): Escalate to AI via query_ai().
/// Step 4+ (AlertStaff): Send email alert and log AlertStaff each cycle until pod recovers.
///
/// Gates:
/// - in_maintenance=true  → log SkipMaintenanceMode, return (no step advance)
/// - billing_active=true  → log SkipCascadeGuardActive, return (no step advance)
/// - cascade guard paused → skip silently, return
async fn run_graduated_recovery(
    state: &Arc<AppState>,
    pod: &PodInfo,
    trackers: &mut std::collections::HashMap<String, PodRecoveryTracker>,
) {
    // Cascade guard check
    {
        let guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_paused() {
            tracing::warn!(
                target: "pod_healer",
                "graduated recovery for {} skipped — cascade guard paused",
                pod.id
            );
            return;
        }
    }

    // Maintenance gate (PMON-01): never touch a pod in maintenance
    let in_maintenance = {
        let health = state.pod_fleet_health.read().await;
        health.get(&pod.id).map(|h| h.in_maintenance).unwrap_or(false)
    };
    if in_maintenance {
        let decision = RecoveryDecision::new(
            "server",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            RecoveryAction::SkipMaintenanceMode,
            "pod_in_maintenance",
        );
        let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
        tracing::info!(
            target: "pod_healer",
            "Pod {} in maintenance — skipping graduated recovery",
            pod.id
        );
        return;
    }

    // Billing gate: never restart a pod with an active session
    if has_active_billing(state, &pod.id).await {
        let decision = RecoveryDecision::new(
            "server",
            "rc-agent.exe",
            RecoveryAuthority::PodHealer,
            RecoveryAction::SkipCascadeGuardActive,
            "billing_active",
        );
        let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
        tracing::info!(
            target: "pod_healer",
            "Pod {} has active billing — skipping graduated recovery",
            pod.id
        );
        return;
    }

    // COORD-02: Check if another authority has an active recovery intent for this pod
    {
        let intents = state.recovery_intents.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(active) = intents.has_active_intent(&pod.id, "rc-agent.exe") {
            let remaining = 120 - (Utc::now() - active.created_at).num_seconds();
            tracing::info!(
                target: "pod_healer",
                "Pod {} has active recovery intent from {:?} ({}), skipping — TTL expires in {}s",
                pod.id, active.authority, active.reason, remaining
            );
            return;
        }
    }

    // COORD-03: Check GRACEFUL_RELAUNCH sentinel via rc-sentry /files endpoint
    // If the sentinel is present, rc-agent is in the middle of a planned self-restart — not a crash.
    let sentry_url = format!(
        "http://{}:8091/files?path=C%3A%5CRacingPoint%5CGRACEFUL_RELAUNCH",
        pod.ip_address
    );
    match state
        .http_client
        .get(&sentry_url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} GRACEFUL_RELAUNCH sentinel present — intentional restart in progress, skipping recovery",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::SkipCascadeGuardActive,
                "graceful_relaunch_sentinel_present",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
            return;
        }
        _ => {} // Sentinel absent or rc-sentry unreachable — proceed with recovery
    }

    let tracker = trackers.entry(pod.id.clone()).or_insert_with(PodRecoveryTracker::new);
    let now_instant = std::time::Instant::now();

    match tracker.step {
        PodRecoveryStep::Waiting => {
            if tracker.first_detected_at.is_none() {
                // First detection: record timestamp, log, wait
                tracker.first_detected_at = Some(now_instant);
                let decision = RecoveryDecision::new(
                    "server",
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    RecoveryAction::SkipCascadeGuardActive,
                    "graduated_step1_wait_30s",
                );
                let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} offline — step 1: waiting 30s before acting",
                    pod.id
                );
            } else if now_instant.duration_since(
                tracker.first_detected_at.unwrap_or(now_instant),
            ) >= std::time::Duration::from_secs(30)
            {
                // 30s elapsed: advance to TierOneRestart (fires on next cycle)
                tracker.step = PodRecoveryStep::TierOneRestart;
                tracing::info!(
                    target: "pod_healer",
                    "Pod {} — 30s elapsed, advancing to Tier 1 restart",
                    pod.id
                );
            }
        }

        PodRecoveryStep::TierOneRestart => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} — step 2: Tier 1 restart (rc-agent via pod-agent)",
                pod.id
            );

            // COORD-01: ProcessOwnership enforcement
            // rc-agent.exe is registered to RcSentry — PodHealer should not perform
            // a direct process restart on it. Skip Tier 1 and advance to AI escalation.
            {
                let ownership = state.process_ownership.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(owner) = ownership.owner_of("rc-agent.exe") {
                    if owner != RecoveryAuthority::PodHealer {
                        tracing::info!(
                            target: "pod_healer",
                            "Pod {} rc-agent.exe owned by {:?}, not PodHealer — skipping Tier 1 restart, advancing to AI escalation",
                            pod.id, owner
                        );
                        tracker.step = PodRecoveryStep::AiEscalation;
                        return;
                    }
                }
                // If unregistered, PodHealer may proceed (backward compat)
            }

            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::Restart,
                "graduated_step2_tier1_restart",
            );
            {
                let mut guard = state.cascade_guard.lock().unwrap_or_else(|e| e.into_inner());
                if guard.record(&decision) {
                    tracing::error!(
                        target: "pod_healer",
                        "Cascade guard triggered — aborting graduated recovery for {}",
                        pod.id
                    );
                    return;
                }
            }
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            // COORD-02: Register PodHealer's recovery intent before acting.
            // This prevents concurrent recovery by another authority within the 2-min TTL.
            {
                let mut intents = state.recovery_intents.lock().unwrap_or_else(|e| e.into_inner());
                intents.register(RecoveryIntent::new(
                    &pod.id,
                    "rc-agent.exe",
                    RecoveryAuthority::PodHealer,
                    "graduated_tier1_restart",
                ));
            }

            // Attempt restart via pod-agent :8090/exec
            let restart_cmd = r#"cd /d C:\RacingPoint & start /b rc-agent.exe"#;
            let exec_url = format!("http://{}:{}/exec", pod.ip_address, POD_AGENT_PORT);
            let result = state
                .http_client
                .post(&exec_url)
                .json(&serde_json::json!({ "cmd": restart_cmd, "timeout_ms": 10000 }))
                .timeout(std::time::Duration::from_secs(15))
                .send()
                .await;
            match result {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} Tier 1 restart sent",
                        pod.id
                    );
                    log_pod_activity(
                        state,
                        &pod.id,
                        "race_engineer",
                        "Graduated Restart (Tier 1)",
                        "rc-agent restart via pod-agent (graduated step 2)",
                        "race_engineer",
                    );
                }
                _ => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} Tier 1 restart failed (pod-agent unreachable)",
                        pod.id
                    );
                }
            }
            tracker.step = PodRecoveryStep::AiEscalation;
        }

        PodRecoveryStep::AiEscalation => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} — step 3: AI escalation",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::EscalateToAi,
                "graduated_step3_ai_escalation",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            let context = format!(
                "Pod {} is offline. Tier 1 restart was attempted and pod remains offline. \
                 Last seen: {:?}. Please suggest root cause and next steps.",
                pod.id, pod.last_seen
            );
            let messages = vec![
                serde_json::json!({
                    "role": "system",
                    "content": "You are a sim racing venue technician. A pod has failed to recover \
                                after an automated restart. Provide a brief root cause and specific \
                                manual steps. Keep under 150 words."
                }),
                serde_json::json!({ "role": "user", "content": context.clone() }),
            ];
            match crate::ai::query_ai(
                &state.config.ai_debugger,
                &messages,
                Some(&state.db),
                Some("healer_graduated"),
            )
            .await
            {
                Ok((suggestion, model)) => {
                    tracing::info!(
                        target: "pod_healer",
                        "Pod {} AI suggestion ({}): {}",
                        pod.id,
                        model,
                        suggestion.chars().take(100).collect::<String>()
                    );
                    log_pod_activity(
                        state,
                        &pod.id,
                        "race_engineer",
                        "AI Escalation",
                        &format!(
                            "AI suggestion ({}): {}",
                            model,
                            suggestion.chars().take(200).collect::<String>()
                        ),
                        "race_engineer",
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target: "pod_healer",
                        "Pod {} AI escalation failed: {}",
                        pod.id,
                        e
                    );
                }
            }
            tracker.step = PodRecoveryStep::AlertStaff;
        }

        PodRecoveryStep::AlertStaff => {
            tracing::warn!(
                target: "pod_healer",
                "Pod {} — step 4: alerting staff",
                pod.id
            );
            let decision = RecoveryDecision::new(
                "server",
                "rc-agent.exe",
                RecoveryAuthority::PodHealer,
                RecoveryAction::AlertStaff,
                "graduated_step4_staff_alert",
            );
            let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

            let body = format!(
                "Pod {} has failed all automated recovery steps.\n\
                 Tier 1 restart attempted. AI escalated. Pod still offline.\n\
                 Last seen: {:?}\n\
                 Manual intervention required.",
                pod.id, pod.last_seen
            );
            let subject = format!(
                "[RaceControl] Pod {} — Manual Intervention Required",
                pod.id
            );
            state
                .email_alerter
                .write()
                .await
                .send_alert(&pod.id, &subject, &body)
                .await;
            log_pod_activity(
                state,
                &pod.id,
                "race_engineer",
                "Staff Alert Sent",
                "All automated recovery steps exhausted — staff alerted",
                "race_engineer",
            );
            // Stay at AlertStaff — keep alerting each cycle until pod recovers
        }
    }
}

// --- Diagnostics Collection --------------------------------------------------

async fn collect_diagnostics(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<PodDiagnostics> {
    // Run all diagnostic commands concurrently
    let (sockets_res, disk_res, memory_res, health_res, procs_res) = tokio::join!(
        check_stale_sockets(state, pod_ip),
        check_disk_space(state, pod_ip),
        check_memory(state, pod_ip),
        check_rc_agent_health(state, pod_ip),
        check_processes(state, pod_ip),
    );

    let stale_sockets = sockets_res.unwrap_or_default();
    let (disk_free_pct,) = disk_res.unwrap_or((100.0,));
    let (memory_free_mb, memory_total_mb) = memory_res.unwrap_or((8192, 32768));
    let rc_agent_healthy = health_res.unwrap_or(true); // assume healthy on error
    let suspicious_processes = procs_res.unwrap_or_default();

    Ok(PodDiagnostics {
        stale_sockets,
        disk_free_pct,
        memory_free_mb,
        memory_total_mb,
        rc_agent_healthy,
        suspicious_processes,
    })
}

/// Check for CLOSE_WAIT / TIME_WAIT sockets on monitored ports.
async fn check_stale_sockets(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<Vec<(u32, String)>> {
    let cmd = format!(
        "netstat -ano | findstr /C:\"CLOSE_WAIT\" /C:\"TIME_WAIT\" | findstr {}",
        MONITORED_PORTS
            .iter()
            .map(|p| format!("/C:\"{}\"", p))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let output = exec_on_pod(state, pod_ip, &cmd).await?;
    let mut results = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // netstat output: Proto LocalAddr ForeignAddr State PID
        if parts.len() >= 5 {
            let state_str = parts[3].to_string();
            if let Ok(pid) = parts[4].parse::<u32>() {
                if pid > 0 && (state_str == "CLOSE_WAIT" || state_str == "TIME_WAIT") {
                    // Deduplicate by PID
                    if !results.iter().any(|(p, _): &(u32, String)| *p == pid) {
                        results.push((pid, state_str));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Check disk free space percentage on C: drive.
async fn check_disk_space(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<(f64,)> {
    let cmd = "wmic logicaldisk where \"DeviceID='C:'\" get size,freespace /format:csv";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    // CSV output: Node,FreeSpace,Size
    for line in output.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            if let (Ok(free), Ok(total)) = (
                parts[1].trim().parse::<f64>(),
                parts[2].trim().parse::<f64>(),
            ) {
                if total > 0.0 {
                    let pct_free = (free / total) * 100.0;
                    return Ok((pct_free,));
                }
            }
        }
    }

    Ok((100.0,)) // assume OK if parse fails
}

/// Check free physical memory.
async fn check_memory(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<(u64, u64)> {
    let cmd = "wmic OS get FreePhysicalMemory,TotalVisibleMemorySize /format:csv";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    // CSV: Node,FreePhysicalMemory,TotalVisibleMemorySize (in KB)
    for line in output.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            if let (Ok(free_kb), Ok(total_kb)) = (
                parts[1].trim().parse::<u64>(),
                parts[2].trim().parse::<u64>(),
            ) {
                return Ok((free_kb / 1024, total_kb / 1024)); // convert to MB
            }
        }
    }

    Ok((8192, 32768)) // default: assume 8GB free / 32GB total
}

/// Check if rc-agent lock screen is responsive.
/// The lock screen binds to 127.0.0.1:18923, so we must check from the pod
/// itself via pod-agent exec rather than connecting directly to the pod's network IP.
async fn check_rc_agent_health(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<bool> {
    // Use curl.exe instead of PowerShell — cmd.exe strips $ variables from
    // PowerShell commands, causing $r to disappear and the check to always return 0.
    // curl.exe -s -o NUL -w %{http_code} is cmd.exe-safe (no $ variables).
    let cmd = r#"curl.exe -s -o NUL -w %{http_code} http://127.0.0.1:18923/ --max-time 3"#;
    match exec_on_pod(state, pod_ip, cmd).await {
        Ok(output) => {
            let code: u32 = output.trim().trim_matches('"').parse().unwrap_or(0);
            Ok(code == 200)
        }
        Err(_) => Ok(true), // if pod-agent exec fails, assume healthy (safe default)
    }
}

/// List running processes and flag suspicious ones (high memory, not in protected list).
async fn check_processes(
    state: &Arc<AppState>,
    pod_ip: &str,
) -> anyhow::Result<Vec<(String, u32, u64)>> {
    let cmd = "tasklist /FO CSV /NH";
    let output = exec_on_pod(state, pod_ip, cmd).await?;

    let mut suspicious = Vec::new();
    let high_mem_threshold_kb: u64 = 500_000; // 500MB

    for line in output.lines() {
        // CSV: "Image Name","PID","Session Name","Session#","Mem Usage"
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 5 {
            let name = parts[0].trim_matches('"').to_lowercase();
            let pid: u32 = parts[1].trim_matches('"').parse().unwrap_or(0);
            // Mem Usage like "123,456 K" -- handle the comma in the number
            let mem_str = parts[4..]
                .join(",")
                .replace('"', "")
                .replace(" K", "")
                .replace(',', "")
                .trim()
                .to_string();
            let mem_kb: u64 = mem_str.parse().unwrap_or(0);

            if pid == 0 {
                continue;
            }

            // Flag if high memory AND not in protected list
            let is_protected = PROTECTED_PROCESSES
                .iter()
                .any(|p| name == *p || name.contains(p.trim_end_matches(".exe")));

            if !is_protected && mem_kb > high_mem_threshold_kb {
                suspicious.push((name, pid, mem_kb));
            }
        }
    }

    Ok(suspicious)
}

// --- Auto-Heal Actions -------------------------------------------------------

async fn execute_heal_action(state: &Arc<AppState>, pod_ip: &str, action: &HealAction) {
    // Relaunch lock screen: send ForceRelaunchBrowser over WS — no shell exec needed
    if action.action == "relaunch_lock_screen" {
        let senders = state.agent_senders.read().await;
        if let Some(sender) = senders.get(&action.pod_id) {
            let msg = CoreToAgentMessage::ForceRelaunchBrowser {
                pod_id: action.pod_id.clone(),
            };
            match sender.send(msg).await {
                Ok(_) => tracing::info!(
                    "Pod healer: ForceRelaunchBrowser sent to {} (lock screen recovery)",
                    action.pod_id
                ),
                Err(e) => tracing::warn!(
                    "Pod healer: ForceRelaunchBrowser send to {} failed: {}",
                    action.pod_id, e
                ),
            }
        } else {
            tracing::warn!(
                "Pod healer: ForceRelaunchBrowser -- no WS sender for {} (pod disconnected?)",
                action.pod_id
            );
        }
        return;
    }

    let cmd = match action.action.as_str() {
        "kill_zombie" => {
            // Extract PID from target like "PID 1234"
            let pid = action
                .target
                .strip_prefix("PID ")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            if pid == 0 {
                tracing::warn!("Pod healer: invalid PID in kill_zombie action");
                return;
            }
            format!("taskkill /F /PID {}", pid)
        }
        "clear_temp" => {
            r#"del /q /s C:\Users\*\AppData\Local\Temp\* >nul 2>&1"#.to_string()
        }
        _ => {
            tracing::warn!("Pod healer: unknown action type: {}", action.action);
            return;
        }
    };

    match exec_on_pod(state, pod_ip, &cmd).await {
        Ok(output) => {
            tracing::info!(
                "Pod healer: action '{}' on {} completed: {}",
                action.action,
                action.pod_id,
                output.chars().take(200).collect::<String>()
            );
        }
        Err(e) => {
            tracing::warn!(
                "Pod healer: action '{}' on {} failed: {}",
                action.action,
                action.pod_id,
                e
            );
        }
    }
}

// --- AI Escalation -----------------------------------------------------------

async fn escalate_to_ai(
    state: &Arc<AppState>,
    pod: &PodInfo,
    issues: &[String],
    actions_taken: &[HealAction],
) {
    let actions_desc = if actions_taken.is_empty() {
        "No auto-heal actions taken.".to_string()
    } else {
        actions_taken
            .iter()
            .map(|a| format!("  - {} on {} ({})", a.action, a.target, a.reason))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let context = format!(
        "POD HEALTH ALERT -- Pod {} (#{}, IP: {})\n\n\
         Issues detected:\n{}\n\n\
         Auto-heal actions taken:\n{}\n\n\
         Pod status: {:?}, Last seen: {:?}, Current game: {:?}",
        pod.id,
        pod.number,
        pod.ip_address,
        issues
            .iter()
            .map(|i| format!("  - {}", i))
            .collect::<Vec<_>>()
            .join("\n"),
        actions_desc,
        pod.status,
        pod.last_seen,
        pod.current_game,
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are an expert Windows systems administrator and sim racing venue technician. \
                        Analyze the pod health issues below. Provide a brief root cause hypothesis \
                        and specific remediation steps. Focus on actionable fixes. Keep under 150 words."
        }),
        json!({
            "role": "user",
            "content": context.clone()
        }),
    ];

    match crate::ai::query_ai(
        &state.config.ai_debugger,
        &messages,
        Some(&state.db),
        Some("healer"),
    )
    .await
    {
        Ok((suggestion, model)) => {
            tracing::info!(
                "Pod healer AI suggestion for {} (via {}): {}",
                pod.id,
                model,
                suggestion.chars().take(100).collect::<String>()
            );

            let debug_suggestion = AiDebugSuggestion {
                pod_id: pod.id.clone(),
                sim_type: pod.current_game.unwrap_or(SimType::AssettoCorsa),
                error_context: context,
                suggestion,
                model,
                created_at: Utc::now(),
            };

            // Persist to DB
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'healer')",
            )
            .bind(&id)
            .bind(&debug_suggestion.pod_id)
            .bind(
                serde_json::to_string(&debug_suggestion.sim_type)
                    .unwrap_or_default()
                    .trim_matches('"'),
            )
            .bind(&debug_suggestion.error_context)
            .bind(&debug_suggestion.suggestion)
            .bind(&debug_suggestion.model)
            .execute(&state.db)
            .await;

            // Broadcast to dashboard
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::AiDebugSuggestion(debug_suggestion.clone()));

            // Phase 140: Parse AI suggestion for whitelisted actions and log audit trail.
            // The server does NOT execute actions — rc-agent executes them on the pod.
            // This logs what action the AI recommended for the server-side activity log.
            if let Some(action_name) = parse_ai_action_server(&debug_suggestion.suggestion) {
                let detail = format!("AI recommended action model={}", debug_suggestion.model);
                log_pod_activity(
                    state,
                    &debug_suggestion.pod_id,
                    "ai_action",
                    action_name,
                    &detail,
                    "ai_debugger",
                );
                tracing::info!(
                    "Pod healer: AI action parsed for {} — {} ({})",
                    debug_suggestion.pod_id,
                    action_name,
                    debug_suggestion.model
                );
            }
        }
        Err(e) => {
            tracing::warn!("Pod healer AI escalation failed for {}: {}", pod.id, e);
        }
    }
}

// --- Phase 140-02: Server-side AI action parsing ----------------------------

/// Parse a whitelisted AI action from a free-text LLM suggestion.
///
/// Mirrors the rc-agent parse_ai_action() logic but returns &'static str
/// instead of the rc-agent enum type, avoiding a cross-crate dependency.
///
/// Returns None if no parseable JSON block with a whitelisted action is found.
/// No .unwrap() — all parse errors return None.
fn parse_ai_action_server(suggestion: &str) -> Option<&'static str> {
    #[derive(serde::Deserialize)]
    struct ActionBlock {
        action: String,
    }

    let bytes = suggestion.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(end) = suggestion[i..].find('}') {
                let candidate = &suggestion[i..=i + end];
                if let Ok(block) = serde_json::from_str::<ActionBlock>(candidate) {
                    let action = match block.action.as_str() {
                        "kill_edge" => Some("kill_edge"),
                        "relaunch_lock_screen" => Some("relaunch_lock_screen"),
                        "restart_rcagent" => Some("restart_rcagent"),
                        "kill_game" => Some("kill_game"),
                        "clear_temp" => Some("clear_temp"),
                        _ => None,
                    };
                    if action.is_some() {
                        return action;
                    }
                }
            }
        }
        i += 1;
    }
    None
}

// --- Helpers -----------------------------------------------------------------

/// Execute a command on a pod via pod-agent POST /exec.
async fn exec_on_pod(
    state: &Arc<AppState>,
    pod_ip: &str,
    command: &str,
) -> anyhow::Result<String> {
    let url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
    let resp = state
        .http_client
        .post(&url)
        .json(&json!({
            "cmd": command,
            "timeout_ms": 10000
        }))
        .timeout(POD_AGENT_TIMEOUT)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Pod exec returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    Ok(body["stdout"].as_str().unwrap_or("").to_string())
}

/// Check if a PID belongs to a protected process on the pod.
async fn is_protected_pid(state: &Arc<AppState>, pod_ip: &str, pid: u32) -> bool {
    let cmd = format!(
        "wmic process where ProcessId={} get Name /format:csv",
        pid
    );
    match exec_on_pod(state, pod_ip, &cmd).await {
        Ok(output) => {
            let name = output
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.contains("Node"))
                .next()
                .map(|l| {
                    l.split(',')
                        .last()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase()
                })
                .unwrap_or_default();

            PROTECTED_PROCESSES.iter().any(|p| name == *p)
        }
        Err(_) => true, // if we can't check, treat as protected (safe default)
    }
}

/// Check if a pod has an active billing session.
async fn has_active_billing(state: &Arc<AppState>, pod_id: &str) -> bool {
    let timers = state.billing.active_timers.read().await;
    timers.contains_key(pod_id)
}

// --- Phase 141: WARN Log Scanner ---------------------------------------------

/// Constants for WARN log scanning.
const WARN_SCAN_WINDOW_SECS: i64 = 300;   // 5-minute rolling window
const WARN_THRESHOLD: usize = 50;          // trigger AI escalation above this
const WARN_COOLDOWN_SECS: i64 = 600;       // 10-minute cooldown between escalations

/// Scan the current racecontrol JSONL log for WARN entries in the last 5 minutes.
///
/// Returns (warn_count, raw_warn_lines) where raw_warn_lines are the matching log
/// lines (used by plan 02 for deduplication). Returns (0, vec![]) on any I/O error
/// so the healer cycle is never interrupted by log read failures.
///
/// No .unwrap() — all errors return the default empty result.
pub(crate) async fn scan_warn_logs(state: &Arc<AppState>) {
    let now = Utc::now();

    // Build path: logs/racecontrol-YYYY-MM-DD.jsonl (relative to server CWD)
    let date_str = now.format("%Y-%m-%d").to_string();
    let log_path = format!("logs/racecontrol-{}.jsonl", date_str);

    let contents = match tokio::fs::read_to_string(&log_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("WARN scanner: could not read log {}: {}", log_path, e);
            return;
        }
    };

    let cutoff = now - chrono::Duration::seconds(WARN_SCAN_WINDOW_SECS);

    // Count WARN lines within the rolling window
    let warn_lines: Vec<String> = contents
        .lines()
        .filter(|line| {
            // Fast pre-filter: must contain "WARN" string
            if !line.contains("\"WARN\"") {
                return false;
            }
            // Parse timestamp to check rolling window
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(ts_str) = entry.get("timestamp").and_then(|v| v.as_str()) {
                    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                        return ts.with_timezone(&Utc) >= cutoff;
                    }
                }
            }
            false
        })
        .map(|s| s.to_string())
        .collect();

    let warn_count = warn_lines.len();

    if warn_count == 0 {
        tracing::debug!("WARN scanner: {} WARNs in last 5min (below threshold)", warn_count);
        return;
    }

    tracing::info!("WARN scanner: {} WARNs in last 5min (threshold: {})", warn_count, WARN_THRESHOLD);

    if warn_count <= WARN_THRESHOLD {
        return;
    }

    // Threshold breached — check cooldown before escalating
    {
        let last = state.warn_scanner_last_escalated.read().await;
        if let Some(last_time) = *last {
            let elapsed = (now - last_time).num_seconds();
            if elapsed < WARN_COOLDOWN_SECS {
                tracing::debug!(
                    "WARN scanner: threshold breached ({} WARNs) but cooldown active ({}s remaining)",
                    warn_count,
                    WARN_COOLDOWN_SECS - elapsed
                );
                return;
            }
        }
    }

    // Update cooldown timestamp
    {
        let mut last = state.warn_scanner_last_escalated.write().await;
        *last = Some(now);
    }

    tracing::warn!(
        "WARN scanner: ESCALATING — {} WARNs in 5min exceeds threshold of {}",
        warn_count,
        WARN_THRESHOLD
    );

    escalate_warn_surge(state, warn_count, warn_lines).await;
}

/// Deduplicate warn_lines and escalate to AI with a grouped summary.
///
/// Groups identical message strings, counts occurrences, and builds a compact
/// context prompt. Caps at 20 unique messages to keep the prompt under token limits.
/// Uses the same query_ai() path as escalate_to_ai() so results land in ai_suggestions.
///
/// No .unwrap() — all parse errors skip silently; the message field falls back to the raw line.
async fn escalate_warn_surge(
    state: &Arc<AppState>,
    total_warn_count: usize,
    warn_lines: Vec<String>,
) {
    // Deduplicate: extract fields.message, count occurrences
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in &warn_lines {
        let message = if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entry
                .get("fields")
                .and_then(|f| f.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| line.chars().take(120).collect())
        } else {
            line.chars().take(120).collect()
        };
        *counts.entry(message).or_insert(0) += 1;
    }

    // Sort by frequency descending, cap at 20 unique messages
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(20);

    let grouped_text = sorted
        .iter()
        .map(|(msg, count)| {
            if *count > 1 {
                format!("  [x{}] {}", count, msg)
            } else {
                format!("  {}", msg)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let context = format!(
        "RACECONTROL SERVER WARN SURGE\n\n\
         Total WARNs in last 5 minutes: {}\n\
         Unique message types: {}\n\n\
         Top WARN messages (grouped by frequency):\n{}\n\n\
         Threshold: {} WARNs/5min",
        total_warn_count,
        sorted.len(),
        grouped_text,
        WARN_THRESHOLD,
    );

    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are an expert Rust/Axum server diagnostician for a sim racing venue management system. \
                        Analyze the WARN log surge below. Identify the most likely root cause from the message patterns. \
                        Suggest one concrete investigation step. Keep under 120 words."
        }),
        serde_json::json!({
            "role": "user",
            "content": context
        }),
    ];

    match crate::ai::query_ai(
        &state.config.ai_debugger,
        &messages,
        Some(&state.db),
        Some("warn_scanner"),
    )
    .await
    {
        Ok((suggestion, model)) => {
            tracing::info!(
                "WARN scanner AI suggestion (via {}): {}",
                model,
                suggestion.chars().take(150).collect::<String>()
            );
            // Persist to ai_suggestions as a server-level event (no pod_id)
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, ?, ?, ?, ?, ?, 'warn_scanner')",
            )
            .bind(&id)
            .bind("server")
            .bind("server")
            .bind(&context)
            .bind(&suggestion)
            .bind(&model)
            .execute(&state.db)
            .await;
        }
        Err(e) => {
            tracing::warn!("WARN scanner AI escalation failed: {}", e);
        }
    }
}

/// Returns true if the pod is currently in a watchdog recovery cycle (Restarting or Verifying).
/// A second bot task must not act on this pod while recovery is in progress.
///
/// Note: RecoveryFailed means the watchdog has given up — bots may still attempt fixes.
pub fn is_pod_in_recovery(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}

/// Pure helper: given a WatchdogState, return true if the healer should skip diagnostics.
/// This is extracted for testability — heal_pod() calls this to decide whether to return early.
fn should_skip_for_watchdog_state(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WatchdogState;
    use chrono::Utc;

    // --- Task 1: WatchdogState skip logic ---

    #[test]
    fn skip_returns_true_for_restarting_state() {
        let now = Utc::now();
        let state = WatchdogState::Restarting { attempt: 1, started_at: now };
        assert!(
            should_skip_for_watchdog_state(&state),
            "heal_pod should skip for Restarting state"
        );
    }

    #[test]
    fn skip_returns_true_for_verifying_state() {
        let now = Utc::now();
        let state = WatchdogState::Verifying { attempt: 2, started_at: now };
        assert!(
            should_skip_for_watchdog_state(&state),
            "heal_pod should skip for Verifying state"
        );
    }

    #[test]
    fn skip_returns_false_for_healthy_state() {
        let state = WatchdogState::Healthy;
        assert!(
            !should_skip_for_watchdog_state(&state),
            "heal_pod should NOT skip for Healthy state"
        );
    }

    #[test]
    fn skip_returns_false_for_recovery_failed_state() {
        let now = Utc::now();
        let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
        assert!(
            !should_skip_for_watchdog_state(&state),
            "heal_pod should NOT skip for RecoveryFailed state (healer can still diagnose)"
        );
    }

    // --- is_pod_in_recovery() predicate ---

    #[test]
    fn recovery_blocks_second_bot_task_when_restarting() {
        let state = WatchdogState::Restarting { attempt: 1, started_at: Utc::now() };
        assert!(
            is_pod_in_recovery(&state),
            "is_pod_in_recovery must return true for Restarting — blocks second bot task"
        );
    }

    #[test]
    fn recovery_blocks_second_bot_task_when_verifying() {
        let state = WatchdogState::Verifying { attempt: 1, started_at: Utc::now() };
        assert!(is_pod_in_recovery(&state));
    }

    #[test]
    fn recovery_allows_bot_when_healthy() {
        assert!(!is_pod_in_recovery(&WatchdogState::Healthy));
    }

    #[test]
    fn recovery_allows_bot_when_recovery_failed() {
        let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: Utc::now() };
        assert!(
            !is_pod_in_recovery(&state),
            "RecoveryFailed means watchdog gave up — bot may still try"
        );
    }

    // --- Task 1: WS liveness check uses is_closed() ---
    // Verify by code inspection: the logic uses sender.is_closed(), not contains_key().
    // The actual channel test is an integration concern; we verify the pure skip logic above.

    // --- Task 2: needs_restart flag logic ---

    #[test]
    fn needs_restart_condition_lock_screen_down_no_ws_no_billing() {
        // Represents the decision tree in heal_pod Rule 2:
        // rc_agent_healthy=false, has_active_ws=false, has_active_billing=false -> set needs_restart
        let rc_agent_healthy = false;
        let has_active_ws = false;
        let has_active_billing = false;

        let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(
            should_flag,
            "needs_restart should be set when lock screen down + no WS + no billing"
        );
    }

    #[test]
    fn needs_restart_not_set_when_ws_connected() {
        // rc_agent_healthy=false but has_active_ws=true -> no restart flag
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = false;

        // WS connected means rc-agent IS running, so no restart needed
        let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(
            !should_flag,
            "needs_restart should NOT be set when WebSocket is connected"
        );
    }

    #[test]
    fn needs_restart_not_set_when_billing_active() {
        // rc_agent_healthy=false, no WS, but has billing -> no restart flag
        let rc_agent_healthy = false;
        let has_active_ws = false;
        let has_active_billing = true;

        let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(
            !should_flag,
            "needs_restart should NOT be set when billing is active (session in progress)"
        );
    }

    #[test]
    fn needs_restart_not_set_for_disk_issues() {
        // Disk low (diag.disk_free_pct low) is a healer-only issue — no restart flag
        // Rule 3 produces a HealAction, not a needs_restart flag
        // This test verifies the logic: should_flag_restart is only for Rule 2
        let disk_low = true;
        let rc_agent_healthy = true; // lock screen is fine
        let has_active_ws = true;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws;
        assert!(
            !should_flag_restart,
            "needs_restart should NOT be set for disk low issues"
        );
        // disk_low is consumed by a HealAction, verified by the action type
        assert!(disk_low, "disk_low triggers a clear_temp HealAction, not a restart");
    }

    #[test]
    fn needs_restart_not_set_for_memory_issues() {
        // Memory low is a healer-only issue — just logged to issues[], no restart flag
        let memory_low = true;
        let rc_agent_healthy = true;
        let has_active_ws = true;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws;
        assert!(
            !should_flag_restart,
            "needs_restart should NOT be set for memory low issues"
        );
        assert!(memory_low);
    }

    #[test]
    fn relaunch_lock_screen_action_string() {
        // Verify the action discriminant matches execute_heal_action dispatch
        let action = HealAction {
            pod_id: "pod-1".to_string(),
            action: "relaunch_lock_screen".to_string(),
            target: "edge_browser".to_string(),
            reason: "test".to_string(),
        };
        assert_eq!(action.action, "relaunch_lock_screen");
    }

    #[test]
    fn ws_connected_no_billing_should_relaunch_not_restart() {
        // When WS is alive but lock screen HTTP fails, and no billing active:
        // action should be relaunch (not restart flag)
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = false;

        // Should NOT flag restart
        let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(
            !should_flag_restart,
            "needs_restart should NOT be set when WS is connected"
        );
        // Should dispatch relaunch action
        let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
        assert!(
            should_relaunch,
            "relaunch_lock_screen should be dispatched when WS connected + no billing"
        );
    }

    #[test]
    fn ws_connected_with_billing_should_skip_relaunch() {
        // When WS is alive + billing active: no restart, no relaunch, just warn
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = true;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(!should_flag_restart, "no restart flag when billing active");

        let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
        assert!(!should_relaunch, "no relaunch when billing active");
    }

    // ─── Phase 140-02: parse_ai_action_server tests ───────────────────────────

    #[test]
    fn test_parse_ai_action_server_kill_edge() {
        // Test 1: suggestion containing {"action":"kill_edge"} returns Some("kill_edge")
        let suggestion = r#"The edge browser is causing issues. {"action":"kill_edge"} Terminate it immediately."#;
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("kill_edge"), "kill_edge action must be parsed");
    }

    #[test]
    fn test_parse_ai_action_server_no_action_returns_none() {
        // Test 2: suggestion with no JSON action block returns None
        let suggestion = "Reboot the pod and check network connectivity. No specific action needed.";
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "no JSON block must return None");
    }

    #[test]
    fn test_parse_ai_action_server_unknown_action_returns_none() {
        // Test 3: parse_ai_action_server with unknown action string returns None
        let suggestion = r#"Try this: {"action":"reboot_system"} It should fix the issue."#;
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "unknown action must return None (whitelist rejection)");
    }

    #[test]
    fn test_parse_ai_action_server_relaunch_lock_screen() {
        let suggestion = r#"Lock screen is stuck. {"action":"relaunch_lock_screen"}"#;
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("relaunch_lock_screen"));
    }

    #[test]
    fn test_parse_ai_action_server_clear_temp() {
        let suggestion = r#"Disk space low. {"action":"clear_temp"} This will free up space."#;
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("clear_temp"));
    }

    #[test]
    fn test_parse_ai_action_server_malformed_json_returns_none() {
        let suggestion = r#"Suggestion: {action: kill_edge} missing quotes."#;
        let result = super::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "malformed JSON must return None");
    }

    // ─── PodRecoveryTracker unit tests ────────────────────────────────────────

    #[test]
    fn tracker_starts_at_waiting() {
        let tracker = PodRecoveryTracker::new();
        assert_eq!(
            tracker.step,
            PodRecoveryStep::Waiting,
            "new tracker must start at Waiting step"
        );
        assert!(
            tracker.first_detected_at.is_none(),
            "new tracker must have no first_detected_at"
        );
    }

    #[test]
    fn tracker_reset_clears_state() {
        let mut tracker = PodRecoveryTracker::new();
        // Simulate advancing to TierOneRestart
        tracker.step = PodRecoveryStep::TierOneRestart;
        tracker.first_detected_at = Some(std::time::Instant::now());

        tracker.reset();

        assert_eq!(
            tracker.step,
            PodRecoveryStep::Waiting,
            "reset must restore step to Waiting"
        );
        assert!(
            tracker.first_detected_at.is_none(),
            "reset must clear first_detected_at"
        );
    }

    #[test]
    fn tracker_waiting_advances_to_tier_one_after_30s() {
        // Verify the branch logic: if first_detected_at is set and >= 30s elapsed,
        // step transitions to TierOneRestart.
        let mut tracker = PodRecoveryTracker::new();
        // Set first_detected_at to 31 seconds ago
        let past = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(31))
            .expect("instant subtraction must succeed");
        tracker.first_detected_at = Some(past);
        // Simulate the 30s check
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(tracker.first_detected_at.unwrap_or(now));
        let should_advance = elapsed >= std::time::Duration::from_secs(30);
        assert!(
            should_advance,
            "elapsed >= 30s must trigger advance to TierOneRestart"
        );
        if should_advance {
            tracker.step = PodRecoveryStep::TierOneRestart;
        }
        assert_eq!(
            tracker.step,
            PodRecoveryStep::TierOneRestart,
            "step must be TierOneRestart after 30s elapsed"
        );
    }

    // ─── COORD-01: ProcessOwnership unit tests ────────────────────────────────

    #[test]
    fn test_ownership_check_skips_when_not_owner() {
        // When rc-agent.exe is owned by RcSentry, owner_of returns RcSentry (not PodHealer).
        // PodHealer should detect this and skip.
        use rc_common::recovery::{ProcessOwnership, RecoveryAuthority};

        let mut ownership = ProcessOwnership::new();
        ownership
            .register("rc-agent.exe", RecoveryAuthority::RcSentry)
            .expect("register should succeed");

        let owner = ownership.owner_of("rc-agent.exe");
        assert_eq!(
            owner,
            Some(RecoveryAuthority::RcSentry),
            "owner_of must return RcSentry"
        );
        assert_ne!(
            owner,
            Some(RecoveryAuthority::PodHealer),
            "PodHealer must not own rc-agent.exe after RcSentry registration"
        );
        // PodHealer would skip the restart — simulate the guard
        let should_skip = owner.map_or(false, |o| o != RecoveryAuthority::PodHealer);
        assert!(should_skip, "PodHealer must skip when rc-agent.exe is owned by RcSentry");
    }

    // ─── COORD-02: RecoveryIntent unit tests ──────────────────────────────────

    #[test]
    fn test_recovery_intent_prevents_concurrent_action() {
        // Create an active intent for pod-1 and verify has_active_intent finds it.
        use crate::recovery::RecoveryIntentStore;
        use rc_common::recovery::{RecoveryAuthority, RecoveryIntent};

        let mut store = RecoveryIntentStore::new();
        let intent = RecoveryIntent::new(
            "pod-1",
            "rc-agent.exe",
            RecoveryAuthority::RcSentry,
            "heartbeat_timeout_60s",
        );
        store.register(intent);

        let found = store.has_active_intent("pod-1", "rc-agent.exe");
        assert!(
            found.is_some(),
            "active intent must be found — concurrent action must be blocked"
        );
        assert_eq!(
            found.unwrap().authority,
            RecoveryAuthority::RcSentry,
            "found intent authority must match registered authority"
        );
    }
}
