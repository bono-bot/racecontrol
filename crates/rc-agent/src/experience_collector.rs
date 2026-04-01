//! Experience Score Collector — background task that subscribes to FleetEvents,
//! tracks metrics, and emits experience scores every 5 minutes.
//!
//! Phase 276 — CX-05 to CX-08: Predictive Alerts & Experience Scoring.
//!
//! Lifecycle: spawned in main.rs, subscribes to FleetEventBus, sends
//! AgentMessage::ExperienceScoreReport via WS to server every 5 minutes.

use tokio::sync::{broadcast, mpsc};

use rc_common::fleet_event::FleetEvent;
use rc_common::protocol::AgentMessage;

use crate::experience_actions;
use crate::experience_score::{self, MetricInputs};

const LOG_TARGET: &str = "experience-collector";

/// Spawn the experience collector background task.
///
/// Subscribes to the fleet event bus, accumulates metrics, and every 5 minutes:
/// 1. Calculates the experience score (CX-05)
/// 2. Emits FleetEvent::ExperienceScoreUpdate for local subscribers
/// 3. Sends AgentMessage::ExperienceScoreReport via WS to server (CX-06)
/// 4. Evaluates score thresholds for maintenance/removal actions (CX-07/CX-08)
pub fn spawn(
    mut fleet_rx: broadcast::Receiver<FleetEvent>,
    fleet_tx: broadcast::Sender<FleetEvent>,
    ws_tx: mpsc::Sender<AgentMessage>,
    node_id: String,
) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "experience_collector", event = "lifecycle", "lifecycle: started");

        let mut inputs = MetricInputs::default();
        let mut first_score_logged = false;
        let mut score_interval = tokio::time::interval(std::time::Duration::from_secs(300));
        score_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Wait 2 minutes for system to stabilize
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;

        loop {
            tokio::select! {
                // Consume fleet events to update metrics
                event_result = fleet_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            experience_score::update_metrics(&mut inputs, &event);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(target: LOG_TARGET, lagged = n, "Missed {} fleet events — slow consumer", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::warn!(target: "state", task = "experience_collector", event = "lifecycle", "lifecycle: exited (bus closed)");
                            return;
                        }
                    }
                }

                // Every 5 minutes: calculate score and report
                _ = score_interval.tick() => {
                    // Record a clean scan baseline (diagnostic scans that found nothing
                    // don't emit events, so we count them here)
                    experience_score::record_clean_scan(&mut inputs);

                    let score = experience_score::calculate_score(&inputs);

                    if !first_score_logged {
                        tracing::info!(
                            target: "state",
                            task = "experience_collector",
                            event = "lifecycle",
                            score = format!("{:.1}", score.total),
                            "lifecycle: first_score"
                        );
                        first_score_logged = true;
                    }

                    // CX-05: Emit FleetEvent for local subscribers
                    let status_str = format!("{:?}", score.status);
                    let _ = fleet_tx.send(FleetEvent::ExperienceScoreUpdate {
                        node_id: node_id.clone(),
                        total_score: score.total,
                        status: status_str.clone(),
                        timestamp: chrono::Utc::now(),
                    });

                    // CX-06: Send to server via WS for fleet health API
                    let report = AgentMessage::ExperienceScoreReport {
                        pod_id: node_id.clone(),
                        total_score: score.total,
                        game_launch: score.game_launch,
                        session_completion: score.session_completion,
                        display_stability: score.display_stability,
                        hardware_responsive: score.hardware_responsive,
                        billing_accuracy: score.billing_accuracy,
                        status: status_str,
                        scored_at: chrono::Utc::now().to_rfc3339(),
                    };
                    if let Err(e) = ws_tx.send(report).await {
                        tracing::warn!(target: LOG_TARGET, error = %e, "Failed to send experience score report via WS");
                    }

                    // CX-07/CX-08: Evaluate score thresholds for actions
                    experience_actions::evaluate_score(&score, &node_id, &ws_tx).await;

                    tracing::debug!(
                        target: LOG_TARGET,
                        score = format!("{:.1}", score.total),
                        status = ?score.status,
                        "Experience score cycle complete"
                    );

                    // Reset inputs for next window
                    inputs = MetricInputs::default();
                }
            }
        }
    });
}
