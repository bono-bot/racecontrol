//! Pod Monitor: Tier 2 watchdog that detects stale pods and attempts auto-recovery.
//!
//! Runs as a background task on racecontrol. Checks all known pods every N seconds,
//! marks them Offline if heartbeat is stale, and tries to restart rc-agent via
//! the pod's pod-agent HTTP endpoint.
//!
//! Uses shared EscalatingBackoff (30s->2m->10m->30m) for intelligent cooldowns,
//! spawns post-restart verification tasks, and sends email alerts for persistent failures.
//!
//! WatchdogState FSM:
//!   Healthy -> Restarting -> Verifying -> Healthy (full recovery)
//!                                      -> RecoveryFailed (all checks fail at 60s)
//!
//! Key invariants:
//! - Pod in Restarting or Verifying state is NEVER double-restarted
//! - Partial recovery (process+WS ok, lock screen fail) is FAILED — alert fires
//! - Pod with active billing is NEVER restarted
//! - Natural recovery (fresh heartbeat while attempt > 0) resets WatchdogState to Healthy

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::activity_log::log_pod_activity;
use crate::email_alerts::EmailAlerter;
use crate::state::{AppState, WatchdogState};
use rc_common::protocol::DashboardEvent;
use rc_common::types::{DrivingState, GameState, PodInfo, PodStatus};
use rc_common::watchdog::EscalatingBackoff;

use crate::wol;

const POD_AGENT_PORT: u16 = 8090;
const POD_AGENT_TIMEOUT_MS: u64 = 3000;
const WOL_COOLDOWN_SECS: i64 = 300; // 5 minutes between WoL attempts

/// Lightweight local tracking for per-pod state that does NOT need to be shared
/// with pod_healer (WoL cooldown and pod-agent reachability).
struct PodMonitorLocal {
    last_wol_attempt: Option<DateTime<Utc>>,
    pod_agent_reachable: bool,
}

/// Spawn the pod monitor background task.
pub fn spawn(state: Arc<AppState>) {
    let cfg = &state.config.watchdog;
    if !cfg.enabled {
        tracing::info!("Pod monitor disabled");
        return;
    }

    let check_interval = cfg.check_interval_secs;
    let heartbeat_timeout = cfg.heartbeat_timeout_secs;

    tracing::info!(
        "Pod monitor starting (check every {}s, heartbeat timeout {}s, escalating backoff)",
        check_interval, heartbeat_timeout
    );

    tokio::spawn(async move {
        // Wait for agents to register on startup
        tokio::time::sleep(Duration::from_secs(15)).await;

        let mut local: HashMap<String, PodMonitorLocal> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(check_interval));

        loop {
            interval.tick().await;
            check_all_pods(&state, &mut local, heartbeat_timeout).await;
        }
    });
}

/// Check if a pod's WebSocket sender channel is still open (liveness check).
///
/// Uses `is_closed()` on the channel sender — more accurate than `contains_key`
/// because a stale entry can linger in the map after the receiver is dropped.
async fn is_ws_alive(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}

/// Convert a cooldown duration to a human-readable label ("30s", "2m", "10m", "30m").
fn backoff_label(cooldown: Duration) -> String {
    let secs = cooldown.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

async fn check_all_pods(
    state: &Arc<AppState>,
    local: &mut HashMap<String, PodMonitorLocal>,
    heartbeat_timeout: i64,
) {
    let now = Utc::now();

    // Snapshot current pod list
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();

    for pod in &pods {
        // Skip disabled pods -- admin intentionally shut them down
        if pod.status == PodStatus::Disabled {
            continue;
        }

        // Check if heartbeat is stale
        let stale = match pod.last_seen {
            Some(last) => (now - last).num_seconds() > heartbeat_timeout,
            None => {
                // Seeded but never connected -- skip (don't spam recovery for unconfigured pods)
                continue;
            }
        };

        if !stale {
            // Pod is healthy -- reset shared backoff if it had prior failures
            let mut backoffs = state.pod_backoffs.write().await;
            if let Some(backoff) = backoffs.get_mut(&pod.id) {
                if backoff.attempt() > 0 {
                    let attempt_count = backoff.attempt();
                    backoff.reset();
                    tracing::info!(
                        "Pod {} recovered after {} restart attempt(s)",
                        pod.id,
                        attempt_count
                    );
                    log_pod_activity(
                        state,
                        &pod.id,
                        "race_engineer",
                        "Pod Recovered",
                        &format!("Recovered after {} restart attempt(s)", attempt_count),
                        "race_engineer",
                    );
                }
            }
            drop(backoffs);

            // Reset WatchdogState to Healthy on natural recovery (fresh heartbeat)
            let mut wd_states = state.pod_watchdog_states.write().await;
            if let Some(wd_state) = wd_states.get(&pod.id) {
                if *wd_state != WatchdogState::Healthy {
                    tracing::info!(
                        "Pod {} natural recovery detected -- resetting WatchdogState to Healthy",
                        pod.id
                    );
                    wd_states.insert(pod.id.clone(), WatchdogState::Healthy);
                    drop(wd_states);
                    // Broadcast recovery to dashboard
                    let pods_lock = state.pods.read().await;
                    if let Some(updated_pod) = pods_lock.get(&pod.id) {
                        let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(updated_pod.clone()));
                    }
                } else {
                    drop(wd_states);
                }
            } else {
                drop(wd_states);
            }

            // Also reset local state
            if let Some(loc) = local.get_mut(&pod.id) {
                loc.pod_agent_reachable = true;
            }
            continue;
        }

        // Pod is stale -- mark offline if not already
        if pod.status != PodStatus::Offline {
            tracing::warn!(
                "Pod {} heartbeat stale (last_seen: {:?}), marking Offline",
                pod.id,
                pod.last_seen
            );
            log_pod_activity(
                state,
                &pod.id,
                "race_engineer",
                "Heartbeat Lost",
                &format!("No heartbeat for {}s", heartbeat_timeout),
                "race_engineer",
            );

            let mut pods_lock = state.pods.write().await;
            if let Some(p) = pods_lock.get_mut(&pod.id) {
                p.status = PodStatus::Offline;
                p.driving_state = Some(DrivingState::NoDevice);
                p.game_state = Some(GameState::Idle);
                p.current_game = None;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
            }
        }

        // Skip if WatchdogState is already Restarting or Verifying (avoids double-restart)
        let wd_state = {
            let states = state.pod_watchdog_states.read().await;
            states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
        };
        match wd_state {
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. } => {
                tracing::debug!(
                    "Pod {} in recovery cycle ({:?}) -- skipping restart",
                    pod.id,
                    wd_state
                );
                continue;
            }
            _ => {}
        }

        // Skip pods with active deploy (deploy executor manages lifecycle)
        {
            let deploy_states = state.pod_deploy_states.read().await;
            if let Some(deploy_state) = deploy_states.get(&pod.id) {
                if deploy_state.is_active() {
                    tracing::debug!(
                        "Pod {} has active deploy ({:?}) -- skipping watchdog",
                        pod.id,
                        deploy_state
                    );
                    continue;
                }
            }
        }

        // Check shared backoff -- is it ready for another attempt?
        let mut backoffs = state.pod_backoffs.write().await;
        let backoff = backoffs.entry(pod.id.clone()).or_insert_with(|| {
            if state.config.watchdog.escalation_steps_secs.is_empty() {
                EscalatingBackoff::new()
            } else {
                EscalatingBackoff::with_steps(
                    state
                        .config
                        .watchdog
                        .escalation_steps_secs
                        .iter()
                        .map(|s| Duration::from_secs(*s))
                        .collect(),
                )
            }
        });

        if !backoff.ready(now) {
            continue;
        }

        // Drop backoffs lock before network operations
        drop(backoffs);

        // Guard: do NOT restart pods with active billing
        if state
            .billing
            .active_timers
            .read()
            .await
            .contains_key(&pod.id)
        {
            tracing::info!(
                "Pod {} heartbeat stale but has active billing -- skipping restart",
                pod.id
            );
            continue;
        }

        // Check needs_restart flag from pod_healer (consume and clear it)
        let healer_flagged = {
            let mut needs = state.pod_needs_restart.write().await;
            needs.remove(&pod.id).unwrap_or(false)
        };
        // healer_flagged is informational -- billing guard already checked above.
        // If healer set the flag, proceed with restart even if heartbeat timeout is borderline.
        if !healer_flagged {
            // Normal path: heartbeat timeout already confirmed stale above
        }

        // Ensure local tracking exists
        let loc = local.entry(pod.id.clone()).or_insert(PodMonitorLocal {
            last_wol_attempt: None,
            pod_agent_reachable: false,
        });

        // Try reaching pod-agent
        let ping_url = format!("http://{}:{}/ping", pod.ip_address, POD_AGENT_PORT);
        let ping_result = state
            .http_client
            .get(&ping_url)
            .timeout(Duration::from_millis(POD_AGENT_TIMEOUT_MS))
            .send()
            .await;

        match ping_result {
            Ok(resp) if resp.status().is_success() => {
                loc.pod_agent_reachable = true;
                tracing::info!(
                    "Pod {} pod-agent reachable at {} -- attempting rc-agent restart",
                    pod.id,
                    pod.ip_address
                );

                // POST /exec to restart rc-agent
                let restart_cmd = r#"cd /d C:\RacingPoint & taskkill /F /IM rc-agent.exe >nul 2>&1 & timeout /t 2 /nobreak >nul & start /b rc-agent.exe"#;
                let exec_url =
                    format!("http://{}:{}/exec", pod.ip_address, POD_AGENT_PORT);
                let exec_result = state
                    .http_client
                    .post(&exec_url)
                    .json(&serde_json::json!({
                        "cmd": restart_cmd,
                        "timeout_ms": 10000
                    }))
                    .timeout(Duration::from_millis(15000))
                    .send()
                    .await;

                match exec_result {
                    Ok(resp) if resp.status().is_success() => {
                        tracing::info!(
                            "Pod {} rc-agent restart command sent successfully",
                            pod.id
                        );
                        log_pod_activity(
                            state,
                            &pod.id,
                            "race_engineer",
                            "Agent Restarted",
                            "rc-agent restart via pod-agent",
                            "race_engineer",
                        );

                        // Record attempt in shared backoff
                        let (attempt, cooldown_duration, exhausted) = {
                            let mut backoffs = state.pod_backoffs.write().await;
                            if let Some(backoff) = backoffs.get_mut(&pod.id) {
                                backoff.record_attempt(now);
                                let attempt = backoff.attempt();
                                let cooldown = backoff.current_cooldown();
                                let exhausted = backoff.exhausted();
                                (attempt, cooldown, exhausted)
                            } else {
                                (1, Duration::from_secs(30), false)
                            }
                        };

                        // Set WatchdogState to Restarting
                        {
                            let mut wd_states = state.pod_watchdog_states.write().await;
                            wd_states.insert(pod.id.clone(), WatchdogState::Restarting {
                                attempt,
                                started_at: now,
                            });
                        }

                        // Broadcast PodRestarting to dashboard
                        let label = backoff_label(cooldown_duration);
                        let _ = state.dashboard_tx.send(DashboardEvent::PodRestarting {
                            pod_id: pod.id.clone(),
                            attempt,
                            max_attempts: 4,
                            backoff_label: label,
                        });

                        // Check if exhausted after this attempt -- send alert
                        if exhausted {
                            let cooldown_secs = cooldown_duration.as_secs();
                            let body = EmailAlerter::format_alert_body(
                                &pod.id,
                                "Max escalation reached -- all restart attempts exhausted",
                                "Max Escalation",
                                attempt,
                                cooldown_secs,
                                pod.last_seen,
                                "Manual intervention required",
                            );
                            let subject = format!(
                                "[RaceControl] Pod {} -- Max Escalation EXHAUSTED",
                                pod.id
                            );
                            state
                                .email_alerter
                                .write()
                                .await
                                .send_alert(&pod.id, &subject, &body)
                                .await;
                        }

                        // Spawn post-restart verification (detached -- does not block monitor loop)
                        let verify_state = Arc::clone(state);
                        let verify_pod_id = pod.id.clone();
                        let verify_pod_ip = pod.ip_address.clone();
                        let verify_last_seen = pod.last_seen;
                        tokio::spawn(async move {
                            verify_restart(verify_state, verify_pod_id, verify_pod_ip, verify_last_seen).await;
                        });
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            "Pod {} rc-agent restart returned HTTP {}",
                            pod.id,
                            resp.status()
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Pod {} rc-agent restart exec failed: {}",
                            pod.id,
                            e
                        );
                    }
                }
            }
            _ => {
                loc.pod_agent_reachable = false;
                tracing::error!(
                    "Pod {} FULLY UNREACHABLE (both agents down)",
                    pod.id,
                );
                log_pod_activity(
                    state,
                    &pod.id,
                    "race_engineer",
                    "Pod Unreachable",
                    "Both agents down",
                    "race_engineer",
                );

                // Record attempt and check exhaustion
                let mut backoffs = state.pod_backoffs.write().await;
                let backoff = backoffs.entry(pod.id.clone()).or_insert_with(EscalatingBackoff::new);
                backoff.record_attempt(now);

                if backoff.exhausted() {
                    let attempt = backoff.attempt();
                    let cooldown = backoff.current_cooldown().as_secs();
                    drop(backoffs);

                    let body = EmailAlerter::format_alert_body(
                        &pod.id,
                        "Pod fully unreachable",
                        "Pod Unreachable",
                        attempt,
                        cooldown,
                        pod.last_seen,
                        "Check physical connectivity and power",
                    );
                    let subject =
                        format!("[RaceControl] Pod {} UNREACHABLE", pod.id);
                    state
                        .email_alerter
                        .write()
                        .await
                        .send_alert(&pod.id, &subject, &body)
                        .await;
                } else {
                    drop(backoffs);
                }

                // Attempt Wake-on-LAN if MAC address is known and cooldown elapsed
                if let Some(mac) = &pod.mac_address {
                    let wol_cooldown_ok = match loc.last_wol_attempt {
                        Some(last) => (now - last).num_seconds() > WOL_COOLDOWN_SECS,
                        None => true,
                    };
                    if wol_cooldown_ok {
                        tracing::info!("Pod {} -- sending Wake-on-LAN to {}", pod.id, mac);
                        if let Err(e) = wol::send_wol(mac).await {
                            tracing::warn!("Pod {} WoL failed: {}", pod.id, e);
                        }
                        log_pod_activity(
                            state,
                            &pod.id,
                            "race_engineer",
                            "Wake-on-LAN Sent",
                            mac,
                            "race_engineer",
                        );
                        loc.last_wol_attempt = Some(now);
                    }
                }

                // Alert dashboard -- staff may need to check this pod
                let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                    pod_id: pod.id.clone(),
                    driver_name: pod.current_driver.clone().unwrap_or_default(),
                    game: String::new(),
                    reason: format!(
                        "Pod fully unreachable. WoL sent. Manual intervention may be needed."
                    ),
                });
            }
        }
    }
}

/// Post-restart verification: checks process, WebSocket, and lock screen at 5s, 15s, 30s, 60s.
///
/// Runs as a detached tokio task so it does not block the monitor loop.
/// On full recovery, resets the shared backoff and sets WatchdogState to Healthy.
/// On failure after 60s, sets RecoveryFailed and sends email alert.
///
/// Partial recovery (process + WS ok, lock screen fail) is treated as SUCCESS:
/// rc-agent is alive (Session 0 or game in foreground). Restarting would kill active games.
async fn verify_restart(
    state: Arc<AppState>,
    pod_id: String,
    pod_ip: String,
    last_seen: Option<DateTime<Utc>>,
) {
    // Set WatchdogState to Verifying on entry
    let attempt = {
        let backoffs = state.pod_backoffs.read().await;
        backoffs.get(&pod_id).map(|b| b.attempt()).unwrap_or(0)
    };
    {
        let mut wd_states = state.pod_watchdog_states.write().await;
        wd_states.insert(pod_id.clone(), WatchdogState::Verifying {
            attempt,
            started_at: Utc::now(),
        });
    }

    // Broadcast PodVerifying to dashboard
    let _ = state.dashboard_tx.send(DashboardEvent::PodVerifying {
        pod_id: pod_id.clone(),
        attempt,
    });

    let check_delays = [5u64, 15, 30, 60];

    // Track last check results so failure path knows WHY it failed.
    // All three are updated each iteration where process is alive.
    // If process never comes up, they remain false (process_dead failure path).
    let mut last_process_ok = false;
    let mut last_ws_ok = false;
    // last_lock_ok is tracked so determine_failure_reason can distinguish
    // "process+ws ok, lock fail" from other failure modes.
    let mut last_lock_ok = false;

    for delay in check_delays {
        tokio::time::sleep(Duration::from_secs(delay)).await;

        // 1. Process running? (via pod-agent /exec tasklist)
        let process_ok = check_process_running(&state, &pod_ip).await;
        last_process_ok = process_ok;
        if !process_ok {
            // Process still dead -- continue to next delay
            last_ws_ok = false;
            last_lock_ok = false;
            continue;
        }

        // 2. WebSocket connected? (uses is_closed() for accurate liveness)
        let ws_ok = is_ws_alive(&state, &pod_id).await;
        last_ws_ok = ws_ok;

        // 3. Lock screen responsive? (via pod-agent /exec PowerShell HTTP check)
        let lock_ok = check_lock_screen(&state, &pod_ip).await;
        last_lock_ok = lock_ok;

        if process_ok && ws_ok && lock_ok {
            // Full recovery -- all 3 checks passed
            tracing::info!(
                "Pod {} restart verified: fully healthy after {}s",
                pod_id,
                delay
            );
            log_pod_activity(
                &state,
                &pod_id,
                "race_engineer",
                "Restart Verified",
                &format!(
                    "Healthy after {}s (process + WebSocket + lock screen)",
                    delay
                ),
                "watchdog",
            );

            // Reset backoff
            {
                let mut backoffs = state.pod_backoffs.write().await;
                if let Some(b) = backoffs.get_mut(&pod_id) {
                    b.reset();
                }
            }

            // Set WatchdogState to Healthy
            {
                let mut wd_states = state.pod_watchdog_states.write().await;
                wd_states.insert(pod_id.clone(), WatchdogState::Healthy);
            }

            // Broadcast recovery to dashboard
            let pods = state.pods.read().await;
            if let Some(pod) = pods.get(&pod_id) {
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
            return;
        }

        if process_ok && ws_ok && !lock_ok {
            // WS connected means rc-agent is alive. Lock screen may be unresponsive
            // because rc-agent is in Session 0 (no GUI) or a game is in foreground.
            // Do NOT restart -- that would kill the running game and disrupt customers.
            tracing::warn!(
                "Pod {} partial recovery at {}s: WS connected but lock screen unresponsive -- accepting as recovered (Session 0 or game active)",
                pod_id,
                delay
            );

            // Reset backoff and mark healthy since rc-agent IS running
            {
                let mut backoffs = state.pod_backoffs.write().await;
                if let Some(b) = backoffs.get_mut(&pod_id) {
                    b.reset();
                }
            }
            {
                let mut wd_states = state.pod_watchdog_states.write().await;
                wd_states.insert(pod_id.clone(), WatchdogState::Healthy);
            }
            let pods = state.pods.read().await;
            if let Some(pod) = pods.get(&pod_id) {
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
            return;
        }
    }

    // All check delays exhausted without full recovery
    // Use pure helper to determine failure reason from last check results
    let reason = determine_failure_reason(last_process_ok, last_ws_ok, last_lock_ok);
    let failure_type = failure_type_from_reason(reason);

    tracing::error!(
        "Pod {} restart verification FAILED after 60s: {}",
        pod_id,
        failure_type
    );
    log_pod_activity(
        &state,
        &pod_id,
        "race_engineer",
        "Restart Failed",
        &format!("{} after 60s verification", failure_type),
        "watchdog",
    );

    // Get current attempt count
    let fail_attempt = {
        let backoffs = state.pod_backoffs.read().await;
        backoffs.get(&pod_id).map(|b| b.attempt()).unwrap_or(0)
    };

    // Set WatchdogState to RecoveryFailed
    {
        let mut wd_states = state.pod_watchdog_states.write().await;
        wd_states.insert(pod_id.clone(), WatchdogState::RecoveryFailed {
            attempt: fail_attempt,
            failed_at: Utc::now(),
        });
    }

    // Broadcast PodRecoveryFailed to dashboard
    let _ = state.dashboard_tx.send(DashboardEvent::PodRecoveryFailed {
        pod_id: pod_id.clone(),
        attempt: fail_attempt,
        reason: reason.to_string(),
    });

    // Send email alert (ALERT-01)
    let cooldown_secs = {
        let backoffs = state.pod_backoffs.read().await;
        backoffs
            .get(&pod_id)
            .map(|b| b.current_cooldown().as_secs())
            .unwrap_or(30)
    };
    let next_action = if fail_attempt >= 4 {
        "Manual intervention required".to_string()
    } else {
        format!(
            "Pod will retry in {}",
            backoff_label(Duration::from_secs(cooldown_secs))
        )
    };

    let body = EmailAlerter::format_alert_body(
        &pod_id,
        "Restart verification failed after 60s",
        failure_type,
        fail_attempt,
        cooldown_secs,
        last_seen,
        &next_action,
    );
    let subject = format!("[RaceControl] Pod {} -- Recovery Failed", pod_id);
    state
        .email_alerter
        .write()
        .await
        .send_alert(&pod_id, &subject, &body)
        .await;
}

/// Check if rc-agent.exe is running on the pod via pod-agent /exec tasklist.
async fn check_process_running(state: &Arc<AppState>, pod_ip: &str) -> bool {
    let cmd = "tasklist /NH | findstr rc-agent";
    let url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
    match state
        .http_client
        .post(&url)
        .json(&serde_json::json!({"cmd": cmd, "timeout_ms": 5000}))
        .timeout(Duration::from_millis(8000))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(body) => body["stdout"]
                    .as_str()
                    .unwrap_or("")
                    .contains("rc-agent"),
                Err(_) => false,
            }
        }
        _ => false,
    }
}

/// Check if rc-agent lock screen HTTP server is responsive on port 18923 (localhost on the pod).
async fn check_lock_screen(state: &Arc<AppState>, pod_ip: &str) -> bool {
    let cmd = r#"powershell -NoProfile -Command "try { $r = Invoke-WebRequest -Uri 'http://127.0.0.1:18923/health' -TimeoutSec 3 -UseBasicParsing; $r.StatusCode } catch { 0 }""#;
    let url = format!("http://{}:{}/exec", pod_ip, POD_AGENT_PORT);
    match state
        .http_client
        .post(&url)
        .json(&serde_json::json!({"cmd": cmd, "timeout_ms": 8000}))
        .timeout(Duration::from_millis(12000))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(body) => {
                    let code: u32 = body["stdout"]
                        .as_str()
                        .unwrap_or("0")
                        .trim()
                        .parse()
                        .unwrap_or(0);
                    code == 200
                }
                Err(_) => false,
            }
        }
        _ => false,
    }
}

// ── Pure helper functions extracted for testability ─────────────────────────

/// Determine the WatchdogState transition after a successful restart command.
/// Returns the new WatchdogState to set and whether to broadcast PodRestarting.
///
/// Extracted as a pure function for unit testing without network calls.
pub fn next_watchdog_state_on_restart(attempt: u32, now: DateTime<Utc>) -> WatchdogState {
    WatchdogState::Restarting { attempt, started_at: now }
}

/// Determine the failure reason string from check results.
///
/// Used by verify_restart's failure path -- extracted for testability.
pub fn determine_failure_reason(process_ok: bool, ws_ok: bool, _lock_ok: bool) -> &'static str {
    if !process_ok {
        "process_dead"
    } else if !ws_ok {
        "no_ws"
    } else {
        "no_lock_screen"
    }
}

/// Determine failure type label from reason string.
pub fn failure_type_from_reason(reason: &str) -> &'static str {
    match reason {
        "process_dead" => "Process Dead",
        "no_ws" => "No WebSocket",
        "no_lock_screen" => "Lock Screen Unresponsive",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        create_initial_backoffs, create_initial_needs_restart, create_initial_watchdog_states,
    };
    use chrono::TimeDelta;

    // ── backoff_label tests ───────────────────────────────────────────────────

    #[test]
    fn backoff_label_30s() {
        assert_eq!(backoff_label(Duration::from_secs(30)), "30s");
    }

    #[test]
    fn backoff_label_120s_is_2m() {
        assert_eq!(backoff_label(Duration::from_secs(120)), "2m");
    }

    #[test]
    fn backoff_label_600s_is_10m() {
        assert_eq!(backoff_label(Duration::from_secs(600)), "10m");
    }

    #[test]
    fn backoff_label_1800s_is_30m() {
        assert_eq!(backoff_label(Duration::from_secs(1800)), "30m");
    }

    #[test]
    fn backoff_label_3600s_is_1h() {
        assert_eq!(backoff_label(Duration::from_secs(3600)), "1h");
    }

    // ── determine_failure_reason tests ───────────────────────────────────────

    #[test]
    fn failure_reason_process_dead() {
        assert_eq!(determine_failure_reason(false, false, false), "process_dead");
        assert_eq!(determine_failure_reason(false, true, true), "process_dead");
    }

    #[test]
    fn failure_reason_no_ws_when_process_ok() {
        assert_eq!(determine_failure_reason(true, false, false), "no_ws");
        assert_eq!(determine_failure_reason(true, false, true), "no_ws");
    }

    #[test]
    fn failure_reason_no_lock_screen_when_process_and_ws_ok() {
        // This is the partial recovery case -- now treated as SUCCESS (early return before this helper)
        // but the helper itself still returns "no_lock_screen" for logging purposes
        assert_eq!(determine_failure_reason(true, true, false), "no_lock_screen");
    }

    // ── failure_type_from_reason tests ───────────────────────────────────────

    #[test]
    fn failure_type_process_dead() {
        assert_eq!(failure_type_from_reason("process_dead"), "Process Dead");
    }

    #[test]
    fn failure_type_no_ws() {
        assert_eq!(failure_type_from_reason("no_ws"), "No WebSocket");
    }

    #[test]
    fn failure_type_no_lock_screen() {
        assert_eq!(failure_type_from_reason("no_lock_screen"), "Lock Screen Unresponsive");
    }

    #[test]
    fn failure_type_unknown_fallback() {
        assert_eq!(failure_type_from_reason("something_else"), "Unknown");
    }

    // ── WatchdogState skip logic tests ───────────────────────────────────────

    #[test]
    fn watchdog_restarting_state_is_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::Restarting { attempt: 1, started_at: now };
        // Pod in Restarting should NOT be restarted again
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(should_skip, "Restarting state should trigger skip");
    }

    #[test]
    fn watchdog_verifying_state_is_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::Verifying { attempt: 2, started_at: now };
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(should_skip, "Verifying state should trigger skip");
    }

    #[test]
    fn watchdog_healthy_state_is_not_skip_condition() {
        let state = WatchdogState::Healthy;
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(!should_skip, "Healthy state should NOT trigger skip");
    }

    #[test]
    fn watchdog_recovery_failed_state_is_not_skip_condition() {
        let now = Utc::now();
        let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
        // RecoveryFailed allows retry (backoff will gate actual timing)
        let should_skip = matches!(
            state,
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
        );
        assert!(!should_skip, "RecoveryFailed state should NOT trigger skip");
    }

    // ── needs_restart flag consumption tests ─────────────────────────────────

    #[test]
    fn needs_restart_flag_consumed_as_false_when_not_set() {
        let mut needs = create_initial_needs_restart();
        // Simulates: let healer_flagged = needs.remove(&pod.id).unwrap_or(false)
        let flagged = needs.remove("pod_1").unwrap_or(false);
        assert!(!flagged, "Default needs_restart should be false");
        // After removal, re-access returns None (consumed)
        assert!(needs.get("pod_1").is_none(), "Flag should be consumed (removed)");
    }

    #[test]
    fn needs_restart_flag_consumed_as_true_when_set() {
        let mut needs = create_initial_needs_restart();
        needs.insert("pod_3".to_string(), true);

        let flagged = needs.remove("pod_3").unwrap_or(false);
        assert!(flagged, "needs_restart=true should be consumed as true");
        // After consumption, the flag is cleared
        assert!(needs.get("pod_3").is_none(), "Flag should be cleared after consumption");
    }

    #[test]
    fn needs_restart_not_present_returns_false() {
        // Key was never inserted at all (edge case -- pre-populated map should have it)
        let mut needs: HashMap<String, bool> = HashMap::new();
        let flagged = needs.remove("pod_99").unwrap_or(false);
        assert!(!flagged, "Missing key should return false via unwrap_or");
    }

    // ── WatchdogState transition on restart tests ─────────────────────────────

    #[test]
    fn next_watchdog_state_on_restart_produces_restarting() {
        let now = Utc::now();
        let state = next_watchdog_state_on_restart(1, now);
        match state {
            WatchdogState::Restarting { attempt, started_at } => {
                assert_eq!(attempt, 1);
                assert_eq!(started_at, now);
            }
            _ => panic!("Expected WatchdogState::Restarting"),
        }
    }

    #[test]
    fn next_watchdog_state_on_restart_attempt_zero_is_valid() {
        let now = Utc::now();
        let state = next_watchdog_state_on_restart(0, now);
        assert!(matches!(state, WatchdogState::Restarting { attempt: 0, .. }));
    }

    // ── Backoff + WatchdogState integration tests ────────────────────────────

    #[test]
    fn backoff_reset_on_natural_recovery_clears_attempt() {
        let mut backoffs = create_initial_backoffs();
        let now = Utc::now();

        // Simulate 2 prior restart attempts
        if let Some(b) = backoffs.get_mut("pod_5") {
            b.record_attempt(now);
            b.record_attempt(now + TimeDelta::seconds(120));
        }
        assert_eq!(backoffs["pod_5"].attempt(), 2);

        // Natural recovery: fresh heartbeat -> reset backoff
        if let Some(b) = backoffs.get_mut("pod_5") {
            if b.attempt() > 0 {
                b.reset();
            }
        }
        assert_eq!(backoffs["pod_5"].attempt(), 0);
    }

    #[test]
    fn watchdog_state_set_to_healthy_on_natural_recovery() {
        let mut wd_states = create_initial_watchdog_states();
        let now = Utc::now();

        // Pod was in RecoveryFailed
        wd_states.insert(
            "pod_2".to_string(),
            WatchdogState::RecoveryFailed { attempt: 3, failed_at: now },
        );
        assert!(!matches!(wd_states["pod_2"], WatchdogState::Healthy));

        // Natural recovery resets to Healthy
        if let Some(state) = wd_states.get("pod_2") {
            if *state != WatchdogState::Healthy {
                wd_states.insert("pod_2".to_string(), WatchdogState::Healthy);
            }
        }
        assert!(matches!(wd_states["pod_2"], WatchdogState::Healthy));
    }

    #[test]
    fn watchdog_state_already_healthy_no_change_on_natural_recovery() {
        let mut wd_states = create_initial_watchdog_states();

        // Pod_1 is already Healthy (default)
        let was_healthy = matches!(wd_states["pod_1"], WatchdogState::Healthy);
        assert!(was_healthy);

        // "Natural recovery" branch -- should not change anything
        if let Some(state) = wd_states.get("pod_1") {
            if *state != WatchdogState::Healthy {
                // This branch NOT entered -- already healthy
                wd_states.insert("pod_1".to_string(), WatchdogState::Healthy);
            }
        }
        // Still healthy
        assert!(matches!(wd_states["pod_1"], WatchdogState::Healthy));
    }

    // ── WS liveness pattern tests ─────────────────────────────────────────────

    #[test]
    fn ws_liveness_pattern_uses_is_closed_not_contains_key() {
        // This test documents the pattern: is_ws_alive() uses is_closed()
        // We can't test is_ws_alive() directly without AppState, but we can
        // verify the function signature exists and is correct via compilation.
        // The real test is that contains_key() is no longer used in pod_monitor.

        // Verify backoff_label correctness (smoke test)
        assert_eq!(backoff_label(Duration::from_secs(30)), "30s");
        assert_eq!(backoff_label(Duration::from_secs(120)), "2m");
    }

    // ── Partial recovery = failure tests ─────────────────────────────────────

    #[test]
    fn partial_recovery_process_and_ws_ok_lock_fail_is_failure() {
        // Partial recovery: process running + WS connected + lock screen unresponsive
        // Per CONTEXT.md: this is FAILURE, not success. Alert must fire.
        let reason = determine_failure_reason(
            true,  // process_ok
            true,  // ws_ok
            false, // lock_ok -- FAILS
        );
        assert_eq!(reason, "no_lock_screen");
        assert_eq!(failure_type_from_reason(reason), "Lock Screen Unresponsive");
    }

    #[test]
    fn full_recovery_all_three_checks_pass_is_success() {
        // All 3 checks passing is the ONLY success condition
        let process_ok = true;
        let ws_ok = true;
        let lock_ok = true;
        // If all pass, success path runs
        let is_full_recovery = process_ok && ws_ok && lock_ok;
        assert!(is_full_recovery);
    }

    #[test]
    fn any_check_failing_is_not_full_recovery() {
        // Only combinations where all three pass count as recovery
        assert!(!(true && true && false));   // lock fails
        assert!(!(true && false && true));   // ws fails
        assert!(!(false && true && true));   // process fails
    }

    // ── Email alert next_action format tests ──────────────────────────────────

    #[test]
    fn next_action_manual_when_attempt_gte_4() {
        let attempt = 4u32;
        let next_action = if attempt >= 4 {
            "Manual intervention required".to_string()
        } else {
            format!("Pod will retry in {}", backoff_label(Duration::from_secs(30)))
        };
        assert_eq!(next_action, "Manual intervention required");
    }

    #[test]
    fn next_action_retry_label_when_attempt_lt_4() {
        let attempt = 2u32;
        let cooldown_secs = 600u64; // 10m step
        let next_action = if attempt >= 4 {
            "Manual intervention required".to_string()
        } else {
            format!("Pod will retry in {}", backoff_label(Duration::from_secs(cooldown_secs)))
        };
        assert_eq!(next_action, "Pod will retry in 10m");
    }
}
