//! Revenue Protection Monitor (REV-01..03)
//!
//! Independent monitoring task that polls FailureMonitorState every 10 seconds
//! to detect revenue anomalies:
//!   REV-01: Game running without active billing session
//!   REV-02: Billing session active but no game running (with grace period)
//!   REV-03: Pod down during peak hours (12-22 IST) gets priority recovery
//!
//! Completely independent of billing_guard.rs — separate task, separate state polling.

use std::time::Duration;

use chrono::Utc;
use tokio::sync::{broadcast, watch};

use crate::failure_monitor::FailureMonitorState;
use rc_common::fleet_event::FleetEvent;

const LOG_TARGET: &str = "revenue-protection";

/// How often to poll state (seconds).
const POLL_INTERVAL_SECS: u64 = 10;

/// Grace period before flagging billing-without-game (seconds).
/// Allows time for game launch after billing starts.
const BILLING_IDLE_GRACE_SECS: u64 = 120;

// Removed hardcoded peak hours (was 12-22 IST).
// Revenue protection is now always active when billing is running —
// if billing_active=true, a customer is being charged, so degradation
// is always revenue-impacting regardless of time.

/// Spawn the revenue protection background task.
///
/// Polls `FailureMonitorState` every 10s and emits `FleetEvent::RevenueAnomaly`
/// when revenue-leaking conditions are detected.
pub fn spawn(
    state_rx: watch::Receiver<FailureMonitorState>,
    fleet_tx: broadcast::Sender<FleetEvent>,
    node_id: String,
) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "Revenue protection monitor started");

        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
        let mut first_check = true;
        // Track how long billing has been active without a game (for grace period).
        let mut billing_idle_ticks: u64 = 0;

        loop {
            interval.tick().await;

            let state = state_rx.borrow().clone();

            if first_check {
                tracing::info!(target: LOG_TARGET, "First check completed");
                first_check = false;
            }

            // Skip checks during recovery — another system is handling it.
            if state.recovery_in_progress {
                billing_idle_ticks = 0;
                continue;
            }

            // REV-01: Game running without billing
            if state.game_pid.is_some() && !state.billing_active {
                tracing::warn!(
                    target: LOG_TARGET,
                    node = %node_id,
                    game_pid = ?state.game_pid,
                    "REV-01: Game running without active billing session"
                );
                let _ = fleet_tx.send(FleetEvent::RevenueAnomaly {
                    anomaly_type: "game_without_billing".to_string(),
                    node_id: node_id.clone(),
                    detail: format!(
                        "Game PID {:?} running without billing on {}",
                        state.game_pid, node_id
                    ),
                    timestamp: Utc::now(),
                });
            }

            // REV-02: Billing active but no game (with grace period)
            if state.billing_active && state.game_pid.is_none() && !state.billing_paused {
                billing_idle_ticks += POLL_INTERVAL_SECS;
                if billing_idle_ticks > BILLING_IDLE_GRACE_SECS {
                    tracing::warn!(
                        target: LOG_TARGET,
                        node = %node_id,
                        idle_secs = billing_idle_ticks,
                        "REV-02: Billing active for {}s without game",
                        billing_idle_ticks
                    );
                    let _ = fleet_tx.send(FleetEvent::RevenueAnomaly {
                        anomaly_type: "billing_without_game".to_string(),
                        node_id: node_id.clone(),
                        detail: format!(
                            "Billing active for {}s without game on {}",
                            billing_idle_ticks, node_id
                        ),
                        timestamp: Utc::now(),
                    });
                }
            } else {
                billing_idle_ticks = 0;
            }

            // REV-03: Revenue-at-risk detection — billing active + pod degraded.
            // Replaced hardcoded peak hours (12-22 IST) with billing state check.
            // If billing is active, a customer is being charged — degradation is
            // ALWAYS revenue-impacting regardless of time of day.
            {
                if state.billing_active && state.game_pid.is_none() && state.billing_paused {
                    tracing::warn!(
                        target: LOG_TARGET,
                        node = %node_id,
                        "REV-03: Pod degraded while billing active — prioritize recovery"
                    );
                    let _ = fleet_tx.send(FleetEvent::RevenueAnomaly {
                        anomaly_type: "billing_active_degraded".to_string(),
                        node_id: node_id.clone(),
                        detail: format!(
                            "Pod {} degraded while billing active — recovery priority HIGH",
                            node_id
                        ),
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        // Unreachable, but log if the loop ever exits.
        #[allow(unreachable_code)]
        tracing::error!(target: LOG_TARGET, "Revenue protection monitor exited unexpectedly");
    });
}
