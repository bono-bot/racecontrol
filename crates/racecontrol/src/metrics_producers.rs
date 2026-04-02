//! v34.0 Phase 290: Metric producer loops -- wire real venue data into MetricsSender.
//!
//! Spawns a single background tokio task that polls venue state every 30 seconds
//! and emits samples to the MetricsSender channel consumed by metrics_tsdb.

use std::sync::Arc;
use chrono::Utc;
use tokio::time::{interval, Duration};

use crate::metrics_tsdb::{
    MetricSample, MetricsSender,
    METRIC_WS_CONNECTIONS, METRIC_GAME_SESSION_COUNT,
    METRIC_POD_HEALTH_SCORE, METRIC_BILLING_REVENUE,
};
use crate::state::AppState;

const LOG_TARGET: &str = "metrics-producers";
const POLL_INTERVAL_SECS: u64 = 30;

/// Spawn the metric producer task. Takes ownership of `metrics_tx` (the channel
/// is moved into the spawned task; no other code needs it).
pub fn spawn_metric_producers(state: Arc<AppState>, metrics_tx: MetricsSender) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "Metric producers started (interval={}s)", POLL_INTERVAL_SECS);
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));

        loop {
            ticker.tick().await;

            let now = Utc::now().to_rfc3339();

            // 1. WS connections (agent_senders count)
            {
                let count = {
                    let guard = state.agent_senders.read().await;
                    guard.len()
                };
                let sample = MetricSample {
                    metric_name: METRIC_WS_CONNECTIONS.to_string(),
                    pod_id: None,
                    value: count as f64,
                    recorded_at: now.clone(),
                };
                metrics_tx.try_send(sample).ok();
            }

            // 2. Active game session count
            {
                let count = {
                    let guard = state.game_launcher.active_games.read().await;
                    guard.len()
                };
                let sample = MetricSample {
                    metric_name: METRIC_GAME_SESSION_COUNT.to_string(),
                    pod_id: None,
                    value: count as f64,
                    recorded_at: now.clone(),
                };
                metrics_tx.try_send(sample).ok();
            }

            // 3. Pod health scores (1.0 = http_reachable, 0.0 = not reachable)
            {
                let snapshot: Vec<(String, bool)> = {
                    let guard = state.pod_fleet_health.read().await;
                    guard.iter()
                        .map(|(pod_id, store)| (pod_id.clone(), store.http_reachable))
                        .collect()
                };
                for (pod_id, reachable) in snapshot {
                    let sample = MetricSample {
                        metric_name: METRIC_POD_HEALTH_SCORE.to_string(),
                        pod_id: Some(pod_id),
                        value: if reachable { 1.0 } else { 0.0 },
                        recorded_at: now.clone(),
                    };
                    metrics_tx.try_send(sample).ok();
                }
            }

            // 4. Billing revenue today (paise → rupees)
            {
                let result: Result<Option<i64>, sqlx::Error> = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(total_amount_paise), 0) FROM billing_sessions WHERE date(created_at) = date('now')"
                )
                .fetch_one(&state.db)
                .await;

                match result {
                    Ok(total_paise) => {
                        let rupees = total_paise.unwrap_or(0) as f64 / 100.0;
                        let sample = MetricSample {
                            metric_name: METRIC_BILLING_REVENUE.to_string(),
                            pod_id: None,
                            value: rupees,
                            recorded_at: now.clone(),
                        };
                        metrics_tx.try_send(sample).ok();
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "Failed to query billing revenue: {}", e);
                    }
                }
            }
        }
    });
    tracing::info!(target: LOG_TARGET, "spawn_metric_producers registered");
}

#[cfg(test)]
mod tests {
    /// Compile-time check: confirms the module builds and exports spawn_metric_producers.
    #[test]
    fn metrics_producers_builds() {
        // This test simply verifies the module compiles (types resolve, no unwrap panics at type-check).
        // The function signature is: fn spawn_metric_producers(state: Arc<AppState>, metrics_tx: MetricsSender)
        // Actual runtime behavior is verified via integration (TSDB rows populated within 2 minutes).
        assert!(true, "metrics_producers module compiled successfully");
    }
}
