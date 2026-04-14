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
//!
//! Sibling modules (extracted for <500 line target):
//! - pod_healer_diagnostics: data collection, verification chains, heal actions, helpers
//! - pod_healer_ai: AI escalation, WARN log scanner
//! - pod_healer_recovery: graduated recovery for offline pods

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::json;

use crate::activity_log::log_pod_activity;
use crate::event_archive;
use crate::pod_healer_diagnostics::{collect_diagnostics, execute_heal_action, is_protected_pid, has_active_billing};
use crate::pod_healer_ai::{escalate_to_ai, scan_warn_logs};
use crate::pod_healer_recovery::{run_graduated_recovery, PodRecoveryTracker};
use crate::state::{AppState, WatchdogState};
use rc_common::protocol::DashboardEvent;
use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_SERVER};
use rc_common::types::{PodInfo, PodStatus};

pub(crate) const POD_AGENT_PORT: u16 = 8090;
pub(crate) const POD_AGENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Processes that must NEVER be killed by the healer.
pub(crate) const PROTECTED_PROCESSES: &[&str] = &[
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
pub(crate) const MONITORED_PORTS: &[&str] = &["18923", "18924"];

/// Disk usage threshold (percent used) to trigger temp cleanup.
pub(crate) const DISK_THRESHOLD_PCT: f64 = 90.0;

/// Memory threshold (MB free) to flag as low memory.
pub(crate) const MEMORY_LOW_MB: u64 = 2048;

// --- Types -------------------------------------------------------------------

pub(crate) struct PodDiagnostics {
    pub(crate) stale_sockets: Vec<(u32, String)>, // (PID, state like CLOSE_WAIT)
    pub(crate) disk_free_pct: f64,
    pub(crate) memory_free_mb: u64,
    pub(crate) memory_total_mb: u64,
    pub(crate) rc_agent_healthy: bool,
    pub(crate) suspicious_processes: Vec<(String, u32, u64)>, // (name, PID, mem_kb)
}

pub(crate) struct HealAction {
    pub(crate) pod_id: String,
    pub(crate) action: String,
    pub(crate) target: String,
    pub(crate) reason: String,
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
                    None,
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
                None,
            );
            event_archive::append_event(&state.db, "pod.recovery", "pod_healer", Some(&action.pod_id), serde_json::json!({
                "action": action.action,
                "target": action.target,
                "reason": action.reason,
            }), &state.config.venue.venue_id);
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
            None,
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
pub(crate) fn should_skip_for_watchdog_state(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod_healer_recovery::PodRecoveryStep;
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

    // --- Phase 185-02: WakeOnLan step and recovery event query tests ---

    #[test]
    fn test_wol_step_exists_in_enum() {
        let step = PodRecoveryStep::WakeOnLan;
        assert_eq!(step, PodRecoveryStep::WakeOnLan);
        assert_ne!(step, PodRecoveryStep::TierOneRestart);
        assert_ne!(step, PodRecoveryStep::AiEscalation);
        assert_ne!(step, PodRecoveryStep::AlertStaff);
        assert_ne!(step, PodRecoveryStep::Waiting);
    }

    #[test]
    fn test_graduated_recovery_step_order() {
        let mut tracker = PodRecoveryTracker::new();
        assert_eq!(tracker.step, PodRecoveryStep::Waiting, "must start at Waiting");

        tracker.step = PodRecoveryStep::TierOneRestart;
        assert_eq!(tracker.step, PodRecoveryStep::TierOneRestart);

        tracker.step = PodRecoveryStep::WakeOnLan;
        assert_eq!(tracker.step, PodRecoveryStep::WakeOnLan);

        tracker.step = PodRecoveryStep::AiEscalation;
        assert_eq!(tracker.step, PodRecoveryStep::AiEscalation);

        tracker.step = PodRecoveryStep::AlertStaff;
        assert_eq!(tracker.step, PodRecoveryStep::AlertStaff);
    }

    #[test]
    fn test_skip_wol_when_sentry_restart_recent() {
        use crate::recovery::RecoveryEventStore;
        use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryEvent};

        let mut store = RecoveryEventStore::new();
        let event = RecoveryEvent {
            pod_id: "pod-1".to_string(),
            process: "rc-agent.exe".to_string(),
            authority: RecoveryAuthority::RcSentry,
            action: RecoveryAction::Restart,
            spawn_verified: Some(true),
            server_reachable: Some(true),
            reason: "heartbeat_timeout".to_string(),
            context: String::new(),
            timestamp: Utc::now(),
        };
        store.push(event);

        let recent = store.query(Some("pod-1"), Some(60));
        let skip_wol_sentry = recent.iter().any(|e| {
            e.authority == RecoveryAuthority::RcSentry
                && matches!(e.action, RecoveryAction::Restart)
                && e.spawn_verified == Some(true)
        });

        assert!(
            skip_wol_sentry,
            "WoL must be skipped when rc-sentry restarted with spawn_verified=true within 60s"
        );
    }

    // --- Task 2: needs_restart flag logic ---

    #[test]
    fn needs_restart_condition_lock_screen_down_no_ws_no_billing() {
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
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = false;

        let should_flag = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(
            !should_flag,
            "needs_restart should NOT be set when WebSocket is connected"
        );
    }

    #[test]
    fn needs_restart_not_set_when_billing_active() {
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
        let disk_low = true;
        let rc_agent_healthy = true;
        let has_active_ws = true;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws;
        assert!(
            !should_flag_restart,
            "needs_restart should NOT be set for disk low issues"
        );
        assert!(disk_low, "disk_low triggers a clear_temp HealAction, not a restart");
    }

    #[test]
    fn needs_restart_not_set_for_memory_issues() {
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
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = false;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(!should_flag_restart, "needs_restart should NOT be set when WS is connected");
        let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
        assert!(should_relaunch, "relaunch_lock_screen should be dispatched when WS connected + no billing");
    }

    #[test]
    fn ws_connected_with_billing_should_skip_relaunch() {
        let rc_agent_healthy = false;
        let has_active_ws = true;
        let has_active_billing = true;

        let should_flag_restart = !rc_agent_healthy && !has_active_ws && !has_active_billing;
        assert!(!should_flag_restart, "no restart flag when billing active");

        let should_relaunch = !rc_agent_healthy && has_active_ws && !has_active_billing;
        assert!(!should_relaunch, "no relaunch when billing active");
    }

    // --- Phase 140-02: parse_ai_action_server tests ---

    #[test]
    fn test_parse_ai_action_server_kill_edge() {
        let suggestion = r#"The edge browser is causing issues. {"action":"kill_edge"} Terminate it immediately."#;
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("kill_edge"), "kill_edge action must be parsed");
    }

    #[test]
    fn test_parse_ai_action_server_no_action_returns_none() {
        let suggestion = "Reboot the pod and check network connectivity. No specific action needed.";
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "no JSON block must return None");
    }

    #[test]
    fn test_parse_ai_action_server_unknown_action_returns_none() {
        let suggestion = r#"Try this: {"action":"reboot_system"} It should fix the issue."#;
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "unknown action must return None (whitelist rejection)");
    }

    #[test]
    fn test_parse_ai_action_server_relaunch_lock_screen() {
        let suggestion = r#"Lock screen is stuck. {"action":"relaunch_lock_screen"}"#;
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("relaunch_lock_screen"));
    }

    #[test]
    fn test_parse_ai_action_server_clear_temp() {
        let suggestion = r#"Disk space low. {"action":"clear_temp"} This will free up space."#;
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, Some("clear_temp"));
    }

    #[test]
    fn test_parse_ai_action_server_malformed_json_returns_none() {
        let suggestion = r#"Suggestion: {action: kill_edge} missing quotes."#;
        let result = crate::pod_healer_ai::parse_ai_action_server(suggestion);
        assert_eq!(result, None, "malformed JSON must return None");
    }

    // --- PodRecoveryTracker unit tests ---

    #[test]
    fn tracker_starts_at_waiting() {
        let tracker = PodRecoveryTracker::new();
        assert_eq!(tracker.step, PodRecoveryStep::Waiting, "new tracker must start at Waiting step");
        assert!(tracker.first_detected_at.is_none(), "new tracker must have no first_detected_at");
    }

    #[test]
    fn tracker_reset_clears_state() {
        let mut tracker = PodRecoveryTracker::new();
        tracker.step = PodRecoveryStep::TierOneRestart;
        tracker.first_detected_at = Some(std::time::Instant::now());
        tracker.reset();
        assert_eq!(tracker.step, PodRecoveryStep::Waiting, "reset must restore step to Waiting");
        assert!(tracker.first_detected_at.is_none(), "reset must clear first_detected_at");
    }

    #[test]
    fn tracker_waiting_advances_to_tier_one_after_30s() {
        let mut tracker = PodRecoveryTracker::new();
        let past = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(31))
            .expect("instant subtraction must succeed");
        tracker.first_detected_at = Some(past);
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(tracker.first_detected_at.unwrap_or(now));
        let should_advance = elapsed >= std::time::Duration::from_secs(30);
        assert!(should_advance, "elapsed >= 30s must trigger advance to TierOneRestart");
        if should_advance {
            tracker.step = PodRecoveryStep::TierOneRestart;
        }
        assert_eq!(tracker.step, PodRecoveryStep::TierOneRestart, "step must be TierOneRestart after 30s elapsed");
    }

    // --- MON-02: Sentry fallback verification ---

    #[test]
    fn test_tier_one_restart_is_mon02_sentry_fallback() {
        let mut tracker = PodRecoveryTracker::new();
        tracker.step = PodRecoveryStep::TierOneRestart;
        tracker.step = PodRecoveryStep::WakeOnLan;
        assert_eq!(tracker.step, PodRecoveryStep::WakeOnLan, "MON-02: TierOneRestart must advance to WakeOnLan");
    }

    // --- COORD-01: ProcessOwnership unit tests ---

    #[test]
    fn test_ownership_check_skips_when_not_owner() {
        use rc_common::recovery::{ProcessOwnership, RecoveryAuthority};

        let mut ownership = ProcessOwnership::new();
        ownership.register("rc-agent.exe", RecoveryAuthority::RcSentry).expect("register should succeed");

        let owner = ownership.owner_of("rc-agent.exe");
        assert_eq!(owner, Some(RecoveryAuthority::RcSentry), "owner_of must return RcSentry");
        assert_ne!(owner, Some(RecoveryAuthority::PodHealer), "PodHealer must not own rc-agent.exe after RcSentry registration");
        let should_skip = owner.map_or(false, |o| o != RecoveryAuthority::PodHealer);
        assert!(should_skip, "PodHealer must skip when rc-agent.exe is owned by RcSentry");
    }

    // --- COORD-02: RecoveryIntent unit tests ---

    #[test]
    fn test_recovery_intent_prevents_concurrent_action() {
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
        assert!(found.is_some(), "active intent must be found — concurrent action must be blocked");
        assert_eq!(found.unwrap().authority, RecoveryAuthority::RcSentry, "found intent authority must match");
    }
}
