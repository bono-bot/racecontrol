//! Pod healer rules — proactive diagnostics and auto-heal for online pods.
//!
//! Applies 5 rules to each connected pod: stale sockets, lock screen health,
//! disk space, memory, and suspicious processes. Executes auto-heal actions
//! (kill zombies, clear temp, relaunch lock screen) with cascade guard and
//! backoff coordination.
//!
//! Extracted from pod_healer.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::event_archive;
use crate::pod_healer::{
    should_skip_for_watchdog_state, HealAction, DISK_THRESHOLD_PCT,
    MEMORY_LOW_MB, POD_AGENT_PORT,
};
use crate::pod_healer_ai::escalate_to_ai;
use crate::pod_healer_diagnostics::{collect_diagnostics, exec_on_pod, has_active_billing, is_protected_pid};
use crate::state::{AppState, WatchdogState};
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_SERVER,
};
use rc_common::types::PodInfo;

// ─── Proactive Heal (Online Pods) ────────────────────────────────────────────

pub(crate) async fn heal_pod(
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
            let has_billing = has_active_billing(state, &pod.id).await;
            if has_billing {
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
            let has_billing = has_active_billing(state, &pod.id).await;
            if has_billing {
                issues.push(
                    "rc-agent lock screen unresponsive but pod has active billing -- NOT flagging restart"
                        .to_string(),
                );
            } else {
                // No WebSocket, no billing -- genuine rc-agent failure.
                // COORD-01: Only flag restart if PodHealer owns rc-agent.exe.
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
    } else if !actions.is_empty() {
        tracing::info!(
            "Pod healer: {} has {} pending actions but cooldown not elapsed",
            pod.id,
            actions.len()
        );
    }

    // Escalate to AI if there are complex issues that rules can't handle
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

// ─── Auto-Heal Action Execution ──────────────────────────────────────────────

pub(crate) async fn execute_heal_action(state: &Arc<AppState>, pod_ip: &str, action: &HealAction) {
    // Relaunch lock screen: send ForceRelaunchBrowser over WS — no shell exec needed
    if action.action == "relaunch_lock_screen" {
        let senders = state.agent_senders.read().await;
        if let Some(sender) = senders.get(&action.pod_id) {
            let msg = CoreToAgentMessage::ForceRelaunchBrowser {
                pod_id: action.pod_id.clone(),
            };
            match sender.send(CoreMessage::wrap(msg)).await {
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
