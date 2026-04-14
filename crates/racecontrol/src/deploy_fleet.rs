//! Fleet-level deploy orchestration: rolling deploy, session-drain, status queries.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::DeployState;

use super::{deploy_status, is_deploy_window_locked, set_deploy_state};
use super::executor::deploy_pod;

/// Rolling deploy: Pod 8 first (canary), then remaining pods.
///
/// Pods with active billing sessions are queued (WaitingSession state).
/// Their binary URL is stored in `pending_deploys` — when that pod's session
/// ends, `check_and_trigger_pending_deploy()` fires the deploy automatically.
///
/// If the canary (Pod 8) fails, the rolling deploy halts: no other pods are touched.
/// Non-canary failures are logged but do not halt the rolling deploy.
///
/// This function resolves pod IPs from AppState.pods at call time.
///
/// DEPLOY-03: Rejects deploy during weekend 18:00-22:59 IST unless force=true.
pub async fn deploy_rolling(
    state: Arc<AppState>,
    binary_url: String,
    force: bool,
    actor: &str,
) -> Result<(), String> {
    // DEPLOY-03: Check deploy window lock before touching any pods
    is_deploy_window_locked(force, actor)?;
    let canary_id = "pod_8".to_string();

    // Build ordered pod list: canary first, then 1-7 ascending.
    // Resolve IPs from AppState.pods (only deploy to known/connected pods).
    let ordered_pods: Vec<(String, String)> = {
        let pods = state.pods.read().await;
        let mut entries: Vec<(String, String)> = pods
            .iter()
            .map(|(id, p)| (id.clone(), p.ip_address.clone()))
            .collect();
        // Sort: pod_8 canary first (key=0), then ascending by pod number
        entries.sort_by_key(|(id, _)| {
            if id == "pod_8" {
                0u32
            } else {
                id.strip_prefix("pod_")
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(99)
            }
        });
        entries
    };

    tracing::info!(
        "Rolling deploy: {} pods found, canary=pod_8, binary={}",
        ordered_pods.len(),
        binary_url
    );

    // Phase 1: Deploy canary (Pod 8) synchronously — must succeed before continuing.
    let canary_ip = {
        let pods = state.pods.read().await;
        pods.get(&canary_id).map(|p| p.ip_address.clone())
    };

    let canary_ip = match canary_ip {
        Some(ip) => ip,
        None => {
            return Err("Canary pod_8 not found in AppState.pods — cannot start rolling deploy".to_string());
        }
    };

    tracing::info!("Rolling deploy: starting canary on pod_8 ({})", canary_ip);
    deploy_pod(state.clone(), canary_id.clone(), canary_ip, binary_url.clone()).await;

    // Check canary result
    {
        let deploy_states = state.pod_deploy_states.read().await;
        match deploy_states.get(&canary_id) {
            Some(DeployState::Complete) | Some(DeployState::Idle) => {
                // Idle means it reset after completion (deploy_pod resets to Idle after 10s)
                tracing::info!("Rolling deploy: canary pod_8 succeeded, proceeding to remaining pods");
            }
            Some(DeployState::Failed { reason }) => {
                return Err(format!(
                    "Canary pod_8 failed: {}. Rolling deploy halted — no other pods touched.",
                    reason
                ));
            }
            other => {
                return Err(format!(
                    "Canary pod_8 in unexpected state: {:?}. Rolling deploy halted.",
                    other
                ));
            }
        }
    }

    // Phase 2: Deploy remaining pods (1-7), respecting active billing sessions.
    for (pod_id, pod_ip) in ordered_pods.iter().filter(|(id, _)| id != &canary_id) {
        let has_active_session = {
            let timers = state.billing.active_timers.read().await;
            timers.contains_key(pod_id)
        };

        if has_active_session {
            tracing::info!(
                "Rolling deploy: {} has active billing session, queuing for session-end",
                pod_id
            );
            // Set WaitingSession state and broadcast
            set_deploy_state(&state, pod_id, DeployState::WaitingSession).await;
            // Store URL for session-end hook
            {
                let mut pending = state.pending_deploys.write().await;
                pending.insert(pod_id.clone(), binary_url.clone());
            }
            continue;
        }

        // No active session — deploy immediately
        tracing::info!("Rolling deploy: deploying to {}", pod_id);
        if let Err(e) = {
            // deploy_pod is infallible (returns ()), so we just spawn inline
            deploy_pod(state.clone(), pod_id.clone(), pod_ip.clone(), binary_url.clone()).await;
            Ok::<(), String>(())
        } {
            tracing::error!("Rolling deploy: {} failed: {} (continuing)", pod_id, e);
        }

        // Brief delay between pods to avoid concurrent disk/network load
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    // ── Fleet summary and retry (DEP-04) ────────────────────────────────────
    // NOTE: ordered_pods includes pod_8 (canary). The canary is always Idle/Complete
    // at this point — if it had failed, deploy_rolling() returned Err early and never
    // reached this block. No special canary exclusion needed in the retry loop.
    let initial_states = deploy_status(&state).await;
    let mut succeeded: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut waiting: Vec<String> = Vec::new();

    for (pod_id, _) in &ordered_pods {
        match initial_states.get(pod_id) {
            Some(DeployState::Complete) | Some(DeployState::Idle) => {
                succeeded.push(pod_id.clone());
            }
            Some(DeployState::Failed { .. }) => {
                failed.push(pod_id.clone());
            }
            Some(DeployState::WaitingSession) => {
                waiting.push(pod_id.clone());
            }
            _ => {
                // Still in progress or unknown -- treat as waiting
                waiting.push(pod_id.clone());
            }
        }
    }

    // Retry failed pods once (excluding canary -- if canary failed, we already halted above)
    if !failed.is_empty() {
        tracing::warn!(
            "Rolling deploy: retrying {} failed pod(s): {:?}",
            failed.len(),
            failed
        );

        let retry_pod_ids: Vec<String> = failed.drain(..).collect();
        for retry_id in &retry_pod_ids {
            let retry_ip = {
                let pods = state.pods.read().await;
                pods.get(retry_id).map(|p| p.ip_address.clone())
            };
            if let Some(ip) = retry_ip {
                tracing::info!("Rolling deploy: retry deploying to {}", retry_id);
                deploy_pod(state.clone(), retry_id.clone(), ip, binary_url.clone()).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        // Recheck retry results
        let retry_states = deploy_status(&state).await;
        for retry_id in &retry_pod_ids {
            match retry_states.get(retry_id) {
                Some(DeployState::Complete) | Some(DeployState::Idle) => {
                    succeeded.push(retry_id.clone());
                }
                _ => {
                    failed.push(retry_id.clone());
                }
            }
        }
    }

    // Log structured summary
    tracing::info!(
        "Rolling deploy COMPLETE: succeeded={:?} failed={:?} waiting_session={:?}",
        succeeded,
        failed,
        waiting
    );

    // Broadcast fleet summary event to dashboard
    let _ = state.dashboard_tx.send(DashboardEvent::FleetDeploySummary {
        succeeded: succeeded.clone(),
        failed: failed.clone(),
        waiting: waiting.clone(),
        timestamp: Utc::now().to_rfc3339(),
    });

    Ok(())
}

/// Called when a billing session ends on a pod.
///
/// If a pending deploy is waiting for this pod (queued during a rolling deploy
/// while the pod had an active session), this function fires `deploy_pod` immediately.
///
/// Called from billing.rs after removing the timer from `active_timers`.
///
/// DEPLOY-01: Session drain — verified. Pods with active billing sessions receive
/// DeployState::WaitingSession during deploy_rolling(). When the session ends,
/// billing.rs calls this function which fires deploy_pod() for the deferred pod.
/// Connection points verified:
///   - billing.rs tick loop: check_and_trigger_pending_deploy (expired + pause-timeout)
///   - billing.rs end_billing path: check_and_trigger_pending_deploy (stop_billing handler)
///   - billing_fsm.rs cancel path: check_and_trigger_pending_deploy (FSM-driven end)
pub async fn check_and_trigger_pending_deploy(state: &Arc<AppState>, pod_id: &str) {
    // Check for a pending deploy URL for this pod
    let binary_url = {
        let mut pending = state.pending_deploys.write().await;
        pending.remove(pod_id)
    };

    let binary_url = match binary_url {
        Some(url) => url,
        None => return, // No pending deploy for this pod
    };

    // Resolve pod IP
    let pod_ip = {
        let pods = state.pods.read().await;
        pods.get(pod_id).map(|p| p.ip_address.clone())
    };

    let pod_ip = match pod_ip {
        Some(ip) => ip,
        None => {
            tracing::warn!(
                "Pending deploy for {} triggered at session-end but pod not found",
                pod_id
            );
            return;
        }
    };

    tracing::info!(
        "Session ended on {} — triggering deferred rolling deploy: {}",
        pod_id,
        binary_url
    );

    let state = Arc::clone(state);
    let pod_id = pod_id.to_string();
    tokio::spawn(async move {
        deploy_pod(state, pod_id, pod_ip, binary_url).await;
    });
}
