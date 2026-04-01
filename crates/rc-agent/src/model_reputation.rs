//! Model Reputation Management (REP-01..02)
//!
//! Periodic sweep of MMA model accuracy stats:
//!   REP-01: Models with accuracy < 30% across 5+ runs -> auto-demoted
//!   REP-02: Models with accuracy > 90% across 10+ runs -> auto-promoted
//!
//! Called during night ops cycle or manually.

use chrono::Utc;
use tokio::sync::broadcast;

use crate::mma_engine;
use rc_common::fleet_event::FleetEvent;

const LOG_TARGET: &str = "model-reputation";

/// Accuracy threshold below which a model is demoted (REP-01).
const DEMOTE_ACCURACY_THRESHOLD: f64 = 0.30;
/// Minimum runs before demotion applies.
const DEMOTE_MIN_RUNS: u32 = 5;

/// Accuracy threshold above which a model is promoted (REP-02).
const PROMOTE_ACCURACY_THRESHOLD: f64 = 0.90;
/// Minimum runs before promotion applies.
const PROMOTE_MIN_RUNS: u32 = 10;

/// Run a reputation sweep across all tracked MMA models.
///
/// Checks each model's accuracy and run count, then demotes or promotes
/// based on thresholds. Emits `FleetEvent::ModelReputationChange` for
/// each action taken.
pub fn run_reputation_sweep(fleet_tx: &broadcast::Sender<FleetEvent>) {
    let stats = mma_engine::get_all_model_stats();

    if stats.is_empty() {
        tracing::debug!(target: LOG_TARGET, "No model stats available for reputation sweep");
        return;
    }

    tracing::info!(
        target: LOG_TARGET,
        model_count = stats.len(),
        "Running model reputation sweep"
    );

    for (model_id, accuracy, total_runs) in &stats {
        // REP-01: Demote low-accuracy models
        if *accuracy < DEMOTE_ACCURACY_THRESHOLD && *total_runs >= DEMOTE_MIN_RUNS {
            tracing::warn!(
                target: LOG_TARGET,
                model = %model_id,
                accuracy = *accuracy,
                runs = *total_runs,
                "REP-01: Demoting model — accuracy below {}%",
                (DEMOTE_ACCURACY_THRESHOLD * 100.0) as u32
            );
            mma_engine::demote_model(model_id);
            let _ = fleet_tx.send(FleetEvent::ModelReputationChange {
                model_id: model_id.clone(),
                action: "demoted".to_string(),
                accuracy: *accuracy,
                total_runs: *total_runs,
                timestamp: Utc::now(),
            });
        }

        // REP-02: Promote high-accuracy models
        if *accuracy > PROMOTE_ACCURACY_THRESHOLD && *total_runs >= PROMOTE_MIN_RUNS {
            tracing::info!(
                target: LOG_TARGET,
                model = %model_id,
                accuracy = *accuracy,
                runs = *total_runs,
                "REP-02: Promoting model — accuracy above {}%",
                (PROMOTE_ACCURACY_THRESHOLD * 100.0) as u32
            );
            mma_engine::promote_model(model_id);
            let _ = fleet_tx.send(FleetEvent::ModelReputationChange {
                model_id: model_id.clone(),
                action: "promoted".to_string(),
                accuracy: *accuracy,
                total_runs: *total_runs,
                timestamp: Utc::now(),
            });
        }
    }
}
