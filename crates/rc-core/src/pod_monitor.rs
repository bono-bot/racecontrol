//! Pod Monitor: Tier 2 watchdog that detects stale pods and attempts auto-recovery.
//!
//! Runs as a background task on rc-core. Checks all known pods every N seconds,
//! marks them Offline if heartbeat is stale, and tries to restart rc-agent via
//! the pod's pod-agent HTTP endpoint.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::{DrivingState, GameState, PodInfo, PodStatus};

const POD_AGENT_PORT: u16 = 8090;
const POD_AGENT_TIMEOUT_MS: u64 = 3000;

struct PodRecoveryState {
    last_restart_attempt: Option<DateTime<Utc>>,
    consecutive_failures: u32,
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
    let restart_cooldown = cfg.restart_cooldown_secs;

    tracing::info!(
        "Pod monitor starting (check every {}s, heartbeat timeout {}s, restart cooldown {}s)",
        check_interval, heartbeat_timeout, restart_cooldown
    );

    tokio::spawn(async move {
        // Wait for agents to register on startup
        tokio::time::sleep(Duration::from_secs(15)).await;

        let mut recovery: HashMap<String, PodRecoveryState> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(check_interval));

        loop {
            interval.tick().await;
            check_all_pods(&state, &mut recovery, heartbeat_timeout, restart_cooldown).await;
        }
    });
}

async fn check_all_pods(
    state: &Arc<AppState>,
    recovery: &mut HashMap<String, PodRecoveryState>,
    heartbeat_timeout: i64,
    restart_cooldown: i64,
) {
    let now = Utc::now();

    // Snapshot current pod list
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();

    for pod in &pods {
        // Check if heartbeat is stale
        let stale = match pod.last_seen {
            Some(last) => (now - last).num_seconds() > heartbeat_timeout,
            None => {
                // Seeded but never connected — skip (don't spam recovery for unconfigured pods)
                continue;
            }
        };

        if !stale {
            // Pod is healthy — clear recovery state
            if let Some(rs) = recovery.get_mut(&pod.id) {
                if rs.consecutive_failures > 0 {
                    tracing::info!(
                        "Pod {} recovered after {} check(s)",
                        pod.id,
                        rs.consecutive_failures
                    );
                    rs.consecutive_failures = 0;
                    rs.pod_agent_reachable = true;
                }
            }
            continue;
        }

        // Pod is stale — mark offline if not already
        if pod.status != PodStatus::Offline {
            tracing::warn!(
                "Pod {} heartbeat stale (last_seen: {:?}), marking Offline",
                pod.id,
                pod.last_seen
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

        // Track recovery state
        let rs = recovery.entry(pod.id.clone()).or_insert(PodRecoveryState {
            last_restart_attempt: None,
            consecutive_failures: 0,
            pod_agent_reachable: false,
        });
        rs.consecutive_failures += 1;

        // Check cooldown
        let cooldown_elapsed = match rs.last_restart_attempt {
            Some(last) => (now - last).num_seconds() > restart_cooldown,
            None => true,
        };

        if !cooldown_elapsed {
            continue;
        }

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
                rs.pod_agent_reachable = true;
                tracing::info!(
                    "Pod {} pod-agent reachable at {} — attempting rc-agent restart",
                    pod.id,
                    pod.ip_address
                );

                // POST /exec to restart rc-agent
                let restart_cmd = r#"cd /d C:\RacingPoint & taskkill /F /IM rc-agent.exe >nul 2>&1 & timeout /t 2 /nobreak >nul & start /b rc-agent.exe"#;
                let exec_url = format!("http://{}:{}/exec", pod.ip_address, POD_AGENT_PORT);
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
                        tracing::info!("Pod {} rc-agent restart command sent successfully", pod.id);
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            "Pod {} rc-agent restart returned HTTP {}",
                            pod.id,
                            resp.status()
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Pod {} rc-agent restart exec failed: {}", pod.id, e);
                    }
                }

                rs.last_restart_attempt = Some(now);
            }
            _ => {
                rs.pod_agent_reachable = false;
                tracing::error!(
                    "Pod {} FULLY UNREACHABLE (both agents down, {} consecutive failures)",
                    pod.id,
                    rs.consecutive_failures
                );

                // Alert dashboard — staff needs to physically check this pod
                let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                    pod_id: pod.id.clone(),
                    driver_name: pod.current_driver.clone().unwrap_or_default(),
                    game: String::new(),
                    reason: format!(
                        "Pod fully unreachable ({} checks). Both agents appear down. Manual intervention required.",
                        rs.consecutive_failures
                    ),
                });

                rs.last_restart_attempt = Some(now);
            }
        }
    }
}
