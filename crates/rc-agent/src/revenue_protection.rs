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

/// IST peak hours: 12:00 - 22:00 (venue operating hours).
const PEAK_HOUR_START: u32 = 12;
const PEAK_HOUR_END: u32 = 22;

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
                    message: format!(
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
                        message: format!(
                            "Billing active for {}s without game on {}",
                            billing_idle_ticks, node_id
                        ),
                        timestamp: Utc::now(),
                    });
                }
            } else {
                billing_idle_ticks = 0;
            }

            // REV-03: Peak hour priority — check if current IST hour is in peak range
            // IST = UTC + 5:30 (computed manually per CLAUDE.md — NEVER use TZ=Asia/Kolkata)
            let utc_now = Utc::now();
            let ist_total_minutes = utc_now.timestamp() / 60 + 5 * 60 + 30;
            let ist_hour = ((ist_total_minutes % (24 * 60)) / 60) as u32;
            let is_peak = ist_hour >= PEAK_HOUR_START && ist_hour < PEAK_HOUR_END;

            if is_peak {
                // During peak hours, if pod health is degraded (no game, no billing,
                // but we expect activity), emit a higher-priority event.
                // "Degraded" here = game was recently running but now gone unexpectedly
                // while billing is still active (crash during peak).
                if state.billing_active && state.game_pid.is_none() && state.billing_paused {
                    tracing::warn!(
                        target: LOG_TARGET,
                        node = %node_id,
                        ist_hour = ist_hour,
                        "REV-03: Pod degraded during peak hours — prioritize recovery"
                    );
                    let _ = fleet_tx.send(FleetEvent::RevenueAnomaly {
                        anomaly_type: "peak_hour_degraded".to_string(),
                        node_id: node_id.clone(),
                        message: format!(
                            "Pod {} degraded during peak hour {} IST — recovery priority HIGH",
                            node_id, ist_hour
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
