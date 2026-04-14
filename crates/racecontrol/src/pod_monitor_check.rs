/// Pod monitor: spawn + heartbeat check loop.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::bono_relay::BonoEvent;
use crate::state::{AppState, WatchdogState};
use rc_common::protocol::DashboardEvent;
use rc_common::types::{DrivingState, GameState, PodInfo, PodStatus};
use rc_common::watchdog::EscalatingBackoff;

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
        "Pod monitor starting (check every {}s, heartbeat timeout {}s, detection only — recovery delegated to pod_healer)",
        check_interval, heartbeat_timeout
    );

    tokio::spawn(async move {
        // Wait for agents to register on startup
        tokio::time::sleep(Duration::from_secs(15)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(check_interval));
        // Phase 310-fix: Track when each pod was first seen stale.
        // Only mark Offline after 2 consecutive stale checks (skip-once pattern).
        // Prevents false "offline" from transient network blips.
        let mut first_stale_at: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;
            check_all_pods(&state, heartbeat_timeout, &mut first_stale_at).await;
        }
    });
}

async fn check_all_pods(
    state: &Arc<AppState>,
    heartbeat_timeout: i64,
    first_stale_at: &mut std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
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
            // Clear stale tracking — pod is alive
            first_stale_at.remove(&pod.id);
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
                        None,
                    );
                    // Emit PodOnline event to Bono relay (pod transitioned offline -> online)
                    let _ = state.bono_event_tx.send(BonoEvent::PodOnline {
                        pod_number: pod.number,
                        ip: pod.ip_address.clone(),
                        tailscale_ip: None,
                    });

                    // MI Bridge: Auto-resolve open pod_offline incidents on recovery
                    let db = state.db.clone();
                    let pod_id_for_resolve = pod.id.clone();
                    tokio::spawn(async move {
                        let now = chrono::Utc::now().to_rfc3339();
                        let result = sqlx::query(
                            "UPDATE fleet_incidents SET resolved_at = ?1, resolution = 'auto_recovered'
                             WHERE problem_key = ?2 AND resolved_at IS NULL"
                        )
                        .bind(&now)
                        .bind(&format!("pod_offline:{}", pod_id_for_resolve))
                        .execute(&db)
                        .await;
                        if let Ok(r) = result {
                            if r.rows_affected() > 0 {
                                tracing::info!(
                                    "MI Bridge: Resolved {} pod_offline incident(s) for {}",
                                    r.rows_affected(), pod_id_for_resolve
                                );
                            }
                        }
                    });
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

            continue;
        }

        // Pod is stale. Apply skip-once pattern: first stale → record timestamp,
        // second consecutive stale → mark Offline. Prevents false offline from
        // transient network blips (standing rule: never conclude offline from single probe).
        if !first_stale_at.contains_key(&pod.id) {
            first_stale_at.insert(pod.id.clone(), now);
            tracing::debug!(
                "Pod {} heartbeat stale (first detection, skip-once) — will confirm next cycle",
                pod.id
            );
            continue; // Skip this cycle — confirm on next check
        }

        // Second+ consecutive stale detection — proceed with Offline marking.
        // Recovery actions (WoL, rc-agent restart, AI escalation, staff alert)
        // are handled by pod_healer's graduated recovery tracker (see pod_healer.rs).
        // pod_monitor's role here is detection only.

        // Mark offline if not already
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
                None,
            );

            let mut pods_lock = state.pods.write().await;
            if let Some(p) = pods_lock.get_mut(&pod.id) {
                p.status = PodStatus::Offline;
                p.driving_state = Some(DrivingState::NoDevice);
                p.game_state = Some(GameState::Idle);
                p.current_game = None;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
            }
            drop(pods_lock);

            // Emit PodOffline event to Bono relay (pod transitioned online -> offline)
            let _ = state.bono_event_tx.send(BonoEvent::PodOffline {
                pod_number: pod.number,
                ip: pod.ip_address.clone(),
                last_seen_secs_ago: 0,
            });

            // MI Bridge: Log fleet incident for pod going offline.
            // Severity depends on venue state — High if open (revenue impact), Low if closed.
            let incident = rc_common::mesh_types::MeshIncident {
                id: format!("inc_pod_offline_{}_{}", pod.id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                node: pod.id.clone(),
                problem_key: format!("pod_offline:{}", pod.id),
                severity: if crate::venue_state::venue_is_open() {
                    rc_common::mesh_types::IncidentSeverity::High
                } else {
                    rc_common::mesh_types::IncidentSeverity::Low
                },
                cost: 0.0,
                resolution: None,
                time_to_resolve_secs: None,
                resolved_by_tier: None,
                detected_at: chrono::Utc::now(),
                resolved_at: None,
            };
            let db = state.db.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::fleet_kb::insert_incident(&db, &incident).await {
                    tracing::warn!("MI Bridge: Failed to log pod offline incident: {}", e);
                }
            });
        }

        // Skip if WatchdogState is already Restarting or Verifying (avoids double-restart)
        // pod_healer sets Restarting when it begins a recovery action.
        let wd_state = {
            let states = state.pod_watchdog_states.read().await;
            states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
        };
        match wd_state {
            WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. } => {
                tracing::debug!(
                    "Pod {} in recovery cycle ({:?}) -- skipping",
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
        // (pod_healer reads this same backoff to gate its graduated recovery)
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

        // Drop backoffs lock before any further processing
        drop(backoffs);

        // Guard: do NOT flag pods with active billing
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

        // Pod is offline, backoff ready, no active billing.
        // pod_healer's graduated tracker will handle recovery on its next cycle.
        tracing::debug!(
            "Pod {} is offline and ready for recovery — pod_healer will handle",
            pod.id
        );
    }
}
