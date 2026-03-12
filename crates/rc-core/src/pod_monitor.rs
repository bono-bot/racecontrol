//! Pod Monitor: Tier 2 watchdog that detects stale pods and attempts auto-recovery.
//!
//! Runs as a background task on rc-core. Checks all known pods every N seconds,
//! marks them Offline if heartbeat is stale, and tries to restart rc-agent via
//! the pod's pod-agent HTTP endpoint.
//!
//! Uses shared EscalatingBackoff (30s->2m->10m->30m) for intelligent cooldowns,
//! spawns post-restart verification tasks, and sends email alerts for persistent failures.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::activity_log::log_pod_activity;
use crate::email_alerts::EmailAlerter;
use crate::state::AppState;
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
                        let mut backoffs = state.pod_backoffs.write().await;
                        if let Some(backoff) = backoffs.get_mut(&pod.id) {
                            backoff.record_attempt(now);

                            // Check if exhausted after this attempt
                            if backoff.exhausted() {
                                let attempt = backoff.attempt();
                                let cooldown = backoff.current_cooldown().as_secs();
                                drop(backoffs);

                                let body = EmailAlerter::format_alert_body(
                                    &pod.id,
                                    "Max escalation reached -- all restart attempts exhausted",
                                    attempt,
                                    cooldown,
                                );
                                let subject = format!(
                                    "[RacingPoint] Pod {} -- max escalation EXHAUSTED",
                                    pod.id
                                );
                                state
                                    .email_alerter
                                    .write()
                                    .await
                                    .send_alert(&pod.id, &subject, &body)
                                    .await;
                            } else {
                                drop(backoffs);
                            }
                        } else {
                            drop(backoffs);
                        }

                        // Spawn post-restart verification (detached -- does not block monitor loop)
                        let verify_state = Arc::clone(state);
                        let verify_pod_id = pod.id.clone();
                        let verify_pod_ip = pod.ip_address.clone();
                        tokio::spawn(async move {
                            verify_restart(verify_state, verify_pod_id, verify_pod_ip).await;
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
                        attempt,
                        cooldown,
                    );
                    let subject =
                        format!("[RacingPoint] Pod {} UNREACHABLE", pod.id);
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
/// On full recovery, resets the shared backoff. On failure after 60s, sends email alert.
async fn verify_restart(state: Arc<AppState>, pod_id: String, pod_ip: String) {
    let check_delays = [5u64, 15, 30, 60];

    for delay in check_delays {
        tokio::time::sleep(Duration::from_secs(delay)).await;

        // 1. Process running? (via pod-agent /exec tasklist)
        let process_ok = check_process_running(&state, &pod_ip).await;
        if !process_ok {
            continue;
        }

        // 2. WebSocket connected?
        let ws_ok = state.agent_senders.read().await.contains_key(&pod_id);

        // 3. Lock screen responsive? (via pod-agent /exec PowerShell HTTP check)
        let lock_ok = check_lock_screen(&state, &pod_ip).await;

        if ws_ok && lock_ok {
            // Full recovery
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
            let mut backoffs = state.pod_backoffs.write().await;
            if let Some(b) = backoffs.get_mut(&pod_id) {
                b.reset();
            }
            return;
        }

        if ws_ok && !lock_ok {
            // Partial recovery (Session 0 known limitation)
            tracing::info!(
                "Pod {} restart partial: WebSocket connected but lock screen in Session 0 after {}s",
                pod_id,
                delay
            );
            log_pod_activity(
                &state,
                &pod_id,
                "race_engineer",
                "Restart Partial",
                &format!(
                    "WebSocket OK but lock screen in Session 0 -- will resolve on reboot ({}s)",
                    delay
                ),
                "watchdog",
            );
            // Do NOT trigger email for partial recovery -- this is expected behavior
            // Do NOT reset backoff -- partial recovery should still escalate if it happens again
            return;
        }
    }

    // All checks failed after 60s
    tracing::error!("Pod {} restart verification FAILED after 60s", pod_id);
    log_pod_activity(
        &state,
        &pod_id,
        "race_engineer",
        "Restart Failed",
        "Not healthy after 60s verification",
        "watchdog",
    );

    // Send email alert
    let backoffs = state.pod_backoffs.read().await;
    let attempt = backoffs.get(&pod_id).map(|b| b.attempt()).unwrap_or(0);
    let cooldown = backoffs
        .get(&pod_id)
        .map(|b| b.current_cooldown().as_secs())
        .unwrap_or(30);
    drop(backoffs);

    let body = EmailAlerter::format_alert_body(
        &pod_id,
        "Restart verification failed after 60s",
        attempt,
        cooldown,
    );
    let subject = format!("[RacingPoint] Pod {} restart FAILED", pod_id);
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
    let cmd = r#"powershell -NoProfile -Command "try { $r = Invoke-WebRequest -Uri 'http://127.0.0.1:18923/' -TimeoutSec 3 -UseBasicParsing; $r.StatusCode } catch { 0 }""#;
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
