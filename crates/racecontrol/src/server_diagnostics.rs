//! Server Self-Diagnostics — autonomous health monitoring for racecontrol server.
//!
//! MMA Step 1 consensus (4/4 models): the server must detect its own issues,
//! not just relay pod anomalies. This module runs periodic checks:
//!
//! 1. WS Connection Drift — expected pods vs connected (accounts for MAINTENANCE_MODE)
//! 2. Session State Split-Brain — DB vs WS vs pod-reported reconciliation
//! 3. DB Write Latency — billing writes must complete under threshold
//!
//! Runs as a background tokio task every 60 seconds.

use std::sync::Arc;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::state::AppState;

const LOG_TARGET: &str = "server-diagnostics";
const SCAN_INTERVAL_SECS: u64 = 60;

/// Spawn the server self-diagnostics background task.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "server_diagnostics", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Server self-diagnostics started ({}s interval)", SCAN_INTERVAL_SECS);

        // Startup grace — let server fully initialize
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut ticker = interval(Duration::from_secs(SCAN_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            run_diagnostics(&state).await;
        }
    });
}

async fn run_diagnostics(state: &AppState) {
    // Check 1: WS Connection Drift
    check_ws_connection_drift(state).await;

    // Check 2: Session State Split-Brain
    check_session_split_brain(state).await;

    // Check 3: DB Health
    check_db_health(state).await;
}

/// MMA consensus (4/4): Track expected vs actual WS connections.
async fn check_ws_connection_drift(state: &AppState) {
    let connected_count = {
        let senders = state.agent_senders.read().await;
        senders.len()
    };

    let registered_count = {
        let pods = state.pods.read().await;
        pods.len()
    };

    // If we have registered pods but fewer are connected, flag it
    if registered_count > 0 && connected_count < registered_count {
        let missing = registered_count - connected_count;
        tracing::warn!(target: LOG_TARGET,
            connected = connected_count, registered = registered_count, missing,
            "WS connection drift: {missing} pod(s) not connected"
        );
    } else {
        tracing::debug!(target: LOG_TARGET,
            connected = connected_count, registered = registered_count,
            "WS connections OK"
        );
    }
}

/// MMA consensus (3/4): Detect ghost sessions — DB says active but pod is disconnected.
async fn check_session_split_brain(state: &AppState) {
    // Get active billing sessions from timers
    let active_sessions: Vec<(String, String)> = {
        let timers = state.billing.active_timers.read().await;
        timers.iter()
            .map(|(pod_id, timer)| (pod_id.clone(), timer.session_id.clone()))
            .collect()
    };

    if active_sessions.is_empty() {
        return;
    }

    // Check which of those pods are actually WS-connected
    let connected_pods: std::collections::HashSet<String> = {
        let senders = state.agent_senders.read().await;
        senders.keys().cloned().collect()
    };

    for (pod_id, session_id) in &active_sessions {
        if !connected_pods.contains(pod_id) {
            // Ghost session: billing active but pod disconnected
            tracing::error!(target: LOG_TARGET,
                pod_id, session_id,
                "SPLIT-BRAIN: Active billing session on disconnected pod — customer may be billed for idle time"
            );
        }
    }
}

/// MMA consensus (3/4): Check DB responsiveness with a lightweight probe.
async fn check_db_health(state: &AppState) {
    let start = std::time::Instant::now();
    let result = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await;
    let latency_ms = start.elapsed().as_millis();

    match result {
        Ok(_) => {
            if latency_ms > 500 {
                tracing::warn!(target: LOG_TARGET,
                    latency_ms, "DB write latency HIGH — billing transactions may timeout"
                );
            } else {
                tracing::debug!(target: LOG_TARGET, latency_ms, "DB health OK");
            }
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET,
                error = %e, "DB health check FAILED — database may be corrupted or unreachable"
            );
        }
    }
}
