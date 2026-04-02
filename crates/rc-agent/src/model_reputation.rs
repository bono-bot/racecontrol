//! Model Reputation Management (REP-01..02, MREP-01..04)
//!
//! Periodic sweep of MMA model accuracy using 7-day eval window from SQLite:
//!   MREP-01: Persist accuracy and run counts to model_reputation table after each sweep.
//!   MREP-02: Models with 7-day accuracy < 30% across 5+ runs -> auto-demoted + persisted.
//!   MREP-03: Models with 7-day accuracy > 90% across 10+ runs -> auto-promoted + persisted.
//!   MREP-04: Push current reputation state to server via AgentMessage::ModelReputationSync.
//!
//! Called during night ops cycle or manually.
//!
//! Previously read from in-memory get_all_model_stats() — now queries model_evaluations
//! directly (Phase 290 data) for a durable 7-day window that survives restarts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use tokio::sync::broadcast;

use crate::mma_engine;
use crate::model_eval_store::ModelEvalStore;
use crate::model_reputation_store::ModelReputationStore;
use rc_common::fleet_event::FleetEvent;

const LOG_TARGET: &str = "model-reputation";

/// Accuracy threshold below which a model is demoted (MREP-02).
const DEMOTE_ACCURACY_THRESHOLD: f64 = 0.30;
/// Minimum runs before demotion applies.
const DEMOTE_MIN_RUNS: u32 = 5;

/// Accuracy threshold above which a model is promoted (MREP-03).
const PROMOTE_ACCURACY_THRESHOLD: f64 = 0.90;
/// Minimum runs before promotion applies.
const PROMOTE_MIN_RUNS: u32 = 10;

/// Run a reputation sweep across all tracked MMA models using a 7-day eval window.
///
/// Queries `model_evaluations` (Phase 290 data) for the past 7 days instead of reading
/// in-memory counters that reset on restart. Persists demotion/promotion decisions via
/// `ModelReputationStore` so the roster survives restarts (MREP-01/02/03).
/// After sweep, pushes the full reputation state to the server via ws_msg_tx (MREP-04).
///
/// Note: This is a sync function — it is called from an async context via
/// `tokio::task::block_in_place` so rusqlite sync calls don't block the async runtime.
pub fn run_reputation_sweep(
    fleet_tx: &broadcast::Sender<FleetEvent>,
    eval_store: Arc<Mutex<ModelEvalStore>>,
    rep_store: Arc<Mutex<ModelReputationStore>>,
    ws_msg_tx: tokio::sync::mpsc::Sender<rc_common::protocol::AgentMessage>,
) {
    // ─── Step 1: Query 7-day eval window from SQLite ───────────────────────────
    let now = Utc::now();
    let seven_days_ago = now - chrono::Duration::days(7);
    let from_str = seven_days_ago.to_rfc3339();

    // Acquire eval_store, query, drop lock before any further processing.
    let records = {
        match eval_store.lock() {
            Ok(store) => match store.query_all(Some(&from_str), None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "Failed to query eval records for reputation sweep — skipping"
                    );
                    return;
                }
            },
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    error = %e,
                    "eval_store mutex poisoned in reputation sweep — skipping"
                );
                return;
            }
        }
        // eval_store MutexGuard dropped here
    };

    if records.is_empty() {
        tracing::debug!(
            target: LOG_TARGET,
            "No eval records in 7-day window — skipping reputation sweep"
        );
        return;
    }

    // ─── Step 2: Aggregate per-model stats from 7-day records ─────────────────
    let mut per_model: HashMap<String, (u32, u32)> = HashMap::new(); // (correct, total)
    let mut per_model_cost: HashMap<String, (f64, u32)> = HashMap::new(); // (total_cost, correct_count)
    for rec in &records {
        let entry = per_model.entry(rec.model_id.clone()).or_insert((0, 0));
        if rec.correct {
            entry.0 += 1;
        }
        entry.1 += 1;
        let cost_entry = per_model_cost.entry(rec.model_id.clone()).or_insert((0.0, 0));
        cost_entry.0 += rec.cost_usd;
        if rec.correct {
            cost_entry.1 += 1;
        }
    }

    tracing::info!(
        target: LOG_TARGET,
        model_count = per_model.len(),
        record_count = records.len(),
        window_days = 7,
        "Running model reputation sweep (7-day window)"
    );

    // ─── Step 3: Persist updated counts (MREP-01) and update in-memory stats ──
    for (model_id, (correct, total)) in &per_model {
        // Persist to DB for restart durability
        if let Ok(store) = rep_store.lock() {
            if let Err(e) = store.save_outcome(model_id, *correct, *total) {
                tracing::warn!(
                    target: LOG_TARGET,
                    model = %model_id,
                    error = %e,
                    "Failed to persist reputation outcome — continuing"
                );
            }
        }
        // Also update in-memory counts for immediate use by stratified_select
        mma_engine::set_model_counts(model_id, *correct, *total);
    }

    // ─── Step 4: Apply demotion/promotion rules + persist decisions ───────────
    for (model_id, (correct, total)) in &per_model {
        let accuracy = if *total > 0 {
            *correct as f64 / *total as f64
        } else {
            0.5
        };

        // MREP-02: Demote low-accuracy models
        if accuracy < DEMOTE_ACCURACY_THRESHOLD && *total >= DEMOTE_MIN_RUNS {
            tracing::warn!(
                target: LOG_TARGET,
                model = %model_id,
                accuracy = accuracy,
                runs = *total,
                "REP-01: Demoting model — accuracy below {}% over 7-day window",
                (DEMOTE_ACCURACY_THRESHOLD * 100.0) as u32
            );
            mma_engine::demote_model(model_id);
            // MREP-02: persist demotion so it survives restart
            if let Ok(store) = rep_store.lock() {
                if let Err(e) = store.save_demotion(model_id) {
                    tracing::warn!(
                        target: LOG_TARGET,
                        model = %model_id,
                        error = %e,
                        "Failed to persist demotion — continuing"
                    );
                }
            }
            let _ = fleet_tx.send(FleetEvent::ModelReputationChange {
                model_id: model_id.clone(),
                action: "demoted".to_string(),
                accuracy,
                total_runs: *total,
                timestamp: Utc::now(),
            });
        }

        // MREP-03: Promote high-accuracy models
        if accuracy > PROMOTE_ACCURACY_THRESHOLD && *total >= PROMOTE_MIN_RUNS {
            tracing::info!(
                target: LOG_TARGET,
                model = %model_id,
                accuracy = accuracy,
                runs = *total,
                "REP-02: Promoting model — accuracy above {}% over 7-day window",
                (PROMOTE_ACCURACY_THRESHOLD * 100.0) as u32
            );
            mma_engine::promote_model(model_id);
            // MREP-03: persist promotion so it survives restart
            if let Ok(store) = rep_store.lock() {
                if let Err(e) = store.save_promotion(model_id) {
                    tracing::warn!(
                        target: LOG_TARGET,
                        model = %model_id,
                        error = %e,
                        "Failed to persist promotion — continuing"
                    );
                }
            }
            let _ = fleet_tx.send(FleetEvent::ModelReputationChange {
                model_id: model_id.clone(),
                action: "promoted".to_string(),
                accuracy,
                total_runs: *total,
                timestamp: Utc::now(),
            });
        }
    }

    tracing::info!(
        target: LOG_TARGET,
        model_count = per_model.len(),
        "Reputation sweep complete — counts persisted, demotion/promotion applied"
    );

    // ─── Step 5: Build reputation payload and push to server (MREP-04) ────────
    let mut rep_rows: Vec<rc_common::protocol::ReputationPayload> = Vec::new();
    for (model_id, (correct, total)) in &per_model {
        let accuracy = if *total > 0 {
            *correct as f64 / *total as f64
        } else {
            0.5
        };
        let status = if mma_engine::is_demoted(model_id) {
            "demoted"
        } else if mma_engine::is_promoted(model_id) {
            "promoted"
        } else {
            "active"
        };
        // Compute cost_per_correct from per_model_cost tracking
        let cost_per_correct = if let Some((total_cost, correct_count)) = per_model_cost.get(model_id) {
            if *correct_count > 0 {
                total_cost / *correct_count as f64
            } else {
                0.0
            }
        } else {
            0.0
        };
        rep_rows.push(rc_common::protocol::ReputationPayload {
            model_id: model_id.clone(),
            correct_count: *correct,
            total_count: *total,
            accuracy,
            status: status.to_string(),
            cost_per_correct_usd: cost_per_correct,
            updated_at: Utc::now().to_rfc3339(),
        });
    }

    if !rep_rows.is_empty() {
        let sync_msg = rc_common::protocol::AgentMessage::ModelReputationSync {
            pod_id: "local".to_string(),
            rows: rep_rows,
        };
        // ws_msg_tx is a tokio mpsc Sender. Since we're inside block_in_place (sync context),
        // we must use blocking_send instead of .await.
        if let Err(e) = ws_msg_tx.blocking_send(sync_msg) {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "MREP-04: failed to push reputation sync to server via WS — sweep still complete"
            );
        } else {
            tracing::debug!(
                target: LOG_TARGET,
                model_count = per_model.len(),
                "MREP-04: reputation sync sent to server"
            );
        }
    }
}
