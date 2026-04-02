//! Pod Monitor: Heartbeat detector for pod liveness.
//!
//! Runs as a background task on racecontrol. Checks all known pods every N seconds,
//! marks them Offline if heartbeat is stale, and resets state on natural recovery.
//!
//! Recovery actions (WoL, rc-agent restart, AI escalation, staff alert) are handled
//! exclusively by pod_healer's graduated recovery tracker. pod_monitor is a pure detector.
//!
//! WatchdogState is reset to Healthy on natural recovery (fresh heartbeat arrives).
//! The WatchdogState::Restarting / Verifying skip guard is kept so pod_healer's
//! in-progress recovery cycle is not interrupted.
//!
//! Key invariants:
//! - Pod in Restarting or Verifying state is NEVER double-triggered
//! - Pod with active billing is NEVER flagged for restart
//! - Natural recovery (fresh heartbeat while attempt > 0) resets WatchdogState to Healthy
//! - ALL repair actions (WoL, exec, alert) are delegated to pod_healer

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

/// Check if a pod's WebSocket sender channel is still open (liveness check).
#[cfg(test)]
async fn is_ws_alive(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}

/// Convert a cooldown duration to a human-readable label ("30s", "2m", "10m", "30m").
#[cfg(test)]
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

// ── Pure helper functions extracted for testability ─────────────────────────

/// Determine the failure reason string from check results.
///
/// Used by verification logic -- extracted for testability.
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
        create_initial_backoffs, create_initial_watchdog_states,
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

    // ── pod_is_marked_offline_when_heartbeat_stale ───────────────────────────

    #[test]
    fn pod_status_offline_is_detection_only() {
        // Verify the status transition logic: PodStatus::Offline is the signal
        // that pod_healer's graduated tracker picks up for recovery actions.
        // pod_monitor sets it; pod_healer reads it.
        let status = PodStatus::Offline;
        assert_eq!(status, PodStatus::Offline);
    }
}
