//! Fleet deploy core orchestration logic.
//!
//! Split from fleet_deploy.rs — contains the `run_fleet_deploy` function
//! and its private helpers (`resolve_pod_ips`, `append_pod_result`).

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::AppState;
use rc_common::types::DeployState;

use super::{
    FleetDeploySession, DeployOverallStatus, WaveDeployStatus,
    PodDeployResult, RollbackEvent, now_ist_rfc3339,
};

// ---------------------------------------------------------------------------
// Core orchestration
// ---------------------------------------------------------------------------

/// Run a fleet deploy session end-to-end.
///
/// The caller creates a `FleetDeploySession` via `create_session()`, writes it to
/// `session_lock`, then calls this function in a `tokio::spawn` task.
///
/// # Wave semantics
/// - Wave 1 (canary): failure halts the entire deploy and triggers rollback.
/// - Wave 2/3: per-pod failure triggers per-pod rollback but the wave continues.
///
/// # Lock discipline
/// The `session_lock` guard is NEVER held across `.await` — state mutations use
/// the `{ let mut g = lock.write().await; ...; } // g dropped` pattern.
pub async fn run_fleet_deploy(
    state: Arc<AppState>,
    session_lock: Arc<RwLock<Option<FleetDeploySession>>>,
) {
    // Mark session as Running.
    {
        let mut guard = session_lock.write().await;
        if let Some(ref mut s) = *guard {
            s.overall_status = DeployOverallStatus::Running;
        }
    }

    // Collect wave data needed for orchestration without holding the lock.
    let (binary_url, _binary_hash, wave_delay_secs, waves_snapshot) = {
        let guard = session_lock.read().await;
        if let Some(ref s) = *guard {
            (s.binary_url.clone(), s.binary_hash.clone(), s.wave_delay_secs, s.waves.clone())
        } else {
            return; // Session was cleared externally.
        }
    };

    // Collect all target pod IDs for OTA sentinel.
    let all_pod_ids: Vec<String> = waves_snapshot.iter().flat_map(|w| w.pods.clone()).collect();
    let all_pod_ips = resolve_pod_ips(&state, &all_pod_ids).await;

    // Set OTA sentinel + kill switch on all target pods.
    crate::ota_pipeline::set_ota_sentinel(&state.http_client, &all_pod_ips).await;
    crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, true).await;

    let wave_count = waves_snapshot.len();
    let mut deploy_halted = false;

    for wave_idx in 0..wave_count {
        let wave_num = waves_snapshot[wave_idx].wave_number;
        let is_canary = wave_num == 1;
        let pod_ids_in_wave = waves_snapshot[wave_idx].pods.clone();

        // Mark wave as Running.
        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                s.current_wave = wave_num;
                s.waves[wave_idx].status = WaveDeployStatus::Running;
                s.waves[wave_idx].started_at = Some(now_ist_rfc3339());
            }
        } // guard dropped

        let mut wave_failed = false;

        for pod_id in &pod_ids_in_wave {
            // Resolve pod IP — if pod not connected, mark as skipped.
            let pod_ip = {
                let pods = state.pods.read().await;
                pods.get(pod_id).map(|p| p.ip_address.clone())
            };

            let pod_ip = match pod_ip {
                Some(ip) => ip,
                None => {
                    // Pod not connected — skip.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "skipped".to_string(),
                        detail: Some("pod not connected".to_string()),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;
                    continue;
                }
            };

            // Billing drain check — must not hold lock across .await.
            let has_active_session = {
                let timers = state.billing.active_timers.read().await;
                timers.contains_key(pod_id)
            };

            if has_active_session {
                // Defer this pod — it will be triggered by check_and_trigger_pending_deploy
                // when the billing session ends.
                {
                    let mut deploy_states = state.pod_deploy_states.write().await;
                    deploy_states.insert(pod_id.clone(), DeployState::WaitingSession);
                }
                {
                    let mut pending = state.pending_deploys.write().await;
                    pending.insert(pod_id.clone(), binary_url.clone());
                }
                let result = PodDeployResult {
                    pod_id: pod_id.clone(),
                    status: "waiting_session".to_string(),
                    detail: Some("active billing session — deferred".to_string()),
                };
                append_pod_result(&session_lock, wave_idx, result).await;
                continue;
            }

            // Deploy this pod (infallible — returns ()).
            crate::deploy::deploy_pod(
                state.clone(),
                pod_id.clone(),
                pod_ip.clone(),
                binary_url.clone(),
            )
            .await;

            // Read result immediately after deploy_pod returns.
            let deploy_state = {
                let states = state.pod_deploy_states.read().await;
                states.get(pod_id).cloned()
            };

            let succeeded = matches!(deploy_state, Some(DeployState::Complete));
            let failure_reason = if let Some(DeployState::Failed { ref reason }) = deploy_state {
                Some(reason.clone())
            } else if !succeeded {
                Some("unknown deploy state".to_string())
            } else {
                None
            };

            if succeeded {
                let result = PodDeployResult {
                    pod_id: pod_id.clone(),
                    status: "complete".to_string(),
                    detail: None,
                };
                append_pod_result(&session_lock, wave_idx, result).await;
            } else {
                let reason = failure_reason.unwrap_or_else(|| "deploy failed".to_string());

                // Trigger rollback for this pod.
                let rollback_pod_ips = vec![(pod_id.clone(), pod_ip.clone())];
                let sentry_key = state.config.pods.sentry_service_key.as_deref();
                crate::ota_pipeline::rollback_wave(&state.http_client, &rollback_pod_ips, sentry_key).await;

                let rb_outcome = "success"; // rollback_wave is infallible from our perspective.

                let rollback_event = RollbackEvent {
                    wave: wave_num,
                    pod_id: pod_id.clone(),
                    reason: reason.clone(),
                    rolled_back_at: now_ist_rfc3339(),
                    outcome: rb_outcome.to_string(),
                };
                {
                    let mut guard = session_lock.write().await;
                    if let Some(ref mut s) = *guard {
                        s.rollback_events.push(rollback_event);
                    }
                } // guard dropped

                if is_canary {
                    // Canary failure — halt entire deploy.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "rolled_back".to_string(),
                        detail: Some(reason.clone()),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;

                    {
                        let mut guard = session_lock.write().await;
                        if let Some(ref mut s) = *guard {
                            s.waves[wave_idx].status = WaveDeployStatus::Failed;
                            s.waves[wave_idx].completed_at = Some(now_ist_rfc3339());
                            s.overall_status = DeployOverallStatus::Failed;
                        }
                    }

                    // Cleanup sentinels before returning.
                    crate::ota_pipeline::clear_ota_sentinel(&state.http_client, &all_pod_ips).await;
                    crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, false).await;
                    deploy_halted = true;
                    break; // stop processing pods in this wave
                } else {
                    // Non-canary: record rolled_back result, continue to next pod.
                    let result = PodDeployResult {
                        pod_id: pod_id.clone(),
                        status: "rolled_back".to_string(),
                        detail: Some(reason),
                    };
                    append_pod_result(&session_lock, wave_idx, result).await;
                    wave_failed = true;
                }
            }
        }

        if deploy_halted {
            break;
        }

        // Mark wave complete.
        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                let status = if wave_failed { WaveDeployStatus::Failed } else { WaveDeployStatus::Passed };
                s.waves[wave_idx].status = status;
                s.waves[wave_idx].completed_at = Some(now_ist_rfc3339());
            }
        }

        // Inter-wave delay (skip after last wave).
        if wave_idx + 1 < wave_count {
            tokio::time::sleep(tokio::time::Duration::from_secs(wave_delay_secs)).await;
        }
    }

    if !deploy_halted {
        // All waves processed — cleanup and mark complete.
        crate::ota_pipeline::clear_ota_sentinel(&state.http_client, &all_pod_ips).await;
        crate::ota_pipeline::set_kill_switch(&state.http_client, &all_pod_ips, false).await;

        {
            let mut guard = session_lock.write().await;
            if let Some(ref mut s) = *guard {
                s.overall_status = DeployOverallStatus::Completed;
            }
        }
    }

    // Log final status.
    let final_status = {
        let guard = session_lock.read().await;
        guard.as_ref().map(|s| format!("{:?}", s.overall_status))
    };
    tracing::info!("Fleet deploy finished: {:?}", final_status);
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Resolve (pod_id, pod_ip) pairs for a list of pod IDs from AppState.
async fn resolve_pod_ips(state: &Arc<AppState>, pod_ids: &[String]) -> Vec<(String, String)> {
    let pods = state.pods.read().await;
    pod_ids
        .iter()
        .filter_map(|id| pods.get(id).map(|p| (id.clone(), p.ip_address.clone())))
        .collect()
}

/// Append a `PodDeployResult` to a wave in the session without holding the lock across await.
async fn append_pod_result(
    session_lock: &Arc<RwLock<Option<FleetDeploySession>>>,
    wave_idx: usize,
    result: PodDeployResult,
) {
    let mut guard = session_lock.write().await;
    if let Some(ref mut s) = *guard {
        if wave_idx < s.waves.len() {
            s.waves[wave_idx].pod_results.push(result);
        }
    }
}
