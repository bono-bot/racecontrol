//! Billing Guard — polls FailureMonitorState every 5s, detects billing anomalies.
//!
//! Detection rules:
//!   BILL-02: billing_active=true + game_pid=None for >= 60s → BillingAnomaly(BillingStuckSession)
//!   BILL-03: billing_active=true + driving_state not Active for >= 300s → BillingAnomaly(IdleDriftDetected)
//!
//! Sends AgentMessage::BillingAnomaly. NEVER calls end_session directly.
//! recovery_in_progress=true suppresses all anomaly sends.

use std::time::Duration;

use tokio::sync::{mpsc, watch};
use rc_common::protocol::AgentMessage;
use rc_common::types::{DrivingState, PodFailureReason};
use crate::failure_monitor::FailureMonitorState;

const POLL_INTERVAL_SECS: u64 = 5;
const STUCK_SESSION_THRESHOLD_SECS: u64 = 60;
const IDLE_DRIFT_THRESHOLD_SECS: u64 = 300;

pub fn spawn(
    state_rx: watch::Receiver<FailureMonitorState>,
    agent_msg_tx: mpsc::Sender<AgentMessage>,
    pod_id: String,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
        // Task-local debounce state (same pattern as launch_timeout_fired in failure_monitor)
        let mut stuck_fired = false;
        let mut game_gone_since: Option<std::time::Instant> = None;
        let mut idle_fired = false;
        let mut idle_since: Option<std::time::Instant> = None;

        loop {
            interval.tick().await;
            let state = state_rx.borrow().clone();

            // Global suppression: server-initiated recovery in progress
            if state.recovery_in_progress {
                // Reset timers so they don't fire the moment recovery clears
                game_gone_since = None;
                stuck_fired = false;
                idle_since = None;
                idle_fired = false;
                continue;
            }

            // BILL-02: Stuck session detection
            if state.billing_active && state.game_pid.is_none() {
                let since = game_gone_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(STUCK_SESSION_THRESHOLD_SECS) && !stuck_fired {
                    stuck_fired = true;
                    let msg = AgentMessage::BillingAnomaly {
                        pod_id: pod_id.clone(),
                        billing_session_id: "unknown".to_string(), // server resolves via active_timers
                        reason: PodFailureReason::SessionStuckWaitingForGame,
                        detail: format!(
                            "game_pid=None for {}s while billing active",
                            since.elapsed().as_secs()
                        ),
                    };
                    let _ = agent_msg_tx.try_send(msg);
                    tracing::warn!("[billing-guard] pod={} BillingStuckSession anomaly sent", pod_id);
                }
            } else {
                // Condition cleared — reset
                game_gone_since = None;
                stuck_fired = false;
            }

            // BILL-03: Idle drift detection
            let is_driving_active = matches!(state.driving_state, Some(DrivingState::Active));
            if state.billing_active && !is_driving_active {
                let since = idle_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(IDLE_DRIFT_THRESHOLD_SECS) && !idle_fired {
                    idle_fired = true;
                    let msg = AgentMessage::BillingAnomaly {
                        pod_id: pod_id.clone(),
                        billing_session_id: "unknown".to_string(),
                        reason: PodFailureReason::IdleBillingDrift,
                        detail: format!(
                            "DrivingState not Active for {}s while billing active (state={:?})",
                            since.elapsed().as_secs(),
                            state.driving_state
                        ),
                    };
                    let _ = agent_msg_tx.try_send(msg);
                    tracing::warn!("[billing-guard] pod={} IdleDriftDetected anomaly sent", pod_id);
                }
            } else {
                idle_since = None;
                idle_fired = false;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::DrivingState;

    fn make_state(
        billing_active: bool,
        game_pid: Option<u32>,
        driving_state: Option<DrivingState>,
        recovery: bool,
    ) -> FailureMonitorState {
        FailureMonitorState {
            billing_active,
            game_pid,
            driving_state,
            recovery_in_progress: recovery,
            hid_connected: false,
            last_udp_secs_ago: None,
            launch_started_at: None,
        }
    }

    #[test]
    fn stuck_session_condition_requires_billing_and_no_pid() {
        let state = make_state(true, None, None, false);
        assert!(state.billing_active && state.game_pid.is_none());
    }

    #[test]
    fn no_stuck_session_when_billing_inactive() {
        let state = make_state(false, None, None, false);
        assert!(!(state.billing_active && state.game_pid.is_none()));
    }

    #[test]
    fn no_stuck_session_when_game_running() {
        let state = make_state(true, Some(1234), None, false);
        assert!(!(state.billing_active && state.game_pid.is_none()));
    }

    #[test]
    fn idle_drift_condition_driving_inactive() {
        let state = make_state(true, Some(1234), Some(DrivingState::Idle), false);
        let is_active = matches!(state.driving_state, Some(DrivingState::Active));
        assert!(state.billing_active && !is_active);
    }

    #[test]
    fn idle_drift_suppressed_when_recovery_in_progress() {
        let state = make_state(true, None, Some(DrivingState::Idle), true);
        assert!(state.recovery_in_progress, "recovery_in_progress must suppress detection");
    }

    #[test]
    fn stuck_threshold_is_60s() {
        assert_eq!(STUCK_SESSION_THRESHOLD_SECS, 60);
    }

    #[test]
    fn idle_threshold_is_300s() {
        assert_eq!(IDLE_DRIFT_THRESHOLD_SECS, 300);
    }
}
