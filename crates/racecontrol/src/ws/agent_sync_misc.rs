//! Miscellaneous agent sync handlers — experience score, game inventory,
//! combo validation, launch timeline, config mismatch.
//!
//! Extracted from agent_sync.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::{
    GameInventory, ComboValidationResult,
    LaunchTimeline,
};

use crate::state::AppState;

/// Handle AgentMessage::ExperienceScoreReport (CX-06).
pub(crate) async fn handle_experience_score_report(
    state: &Arc<AppState>,
    pod_id: &str,
    total_score: f64,
    status: &str,
) {
    tracing::debug!(
        target: "racecontrol::ws",
        pod_id = %pod_id,
        score = total_score,
        status = %status,
        "Received experience score report from pod"
    );
    let mut fleet = state.pod_fleet_health.write().await;
    let store = fleet.entry(pod_id.to_string()).or_default();
    store.experience_score = Some(total_score);
    store.experience_status = Some(status.to_string());
}

/// Handle AgentMessage::GameInventoryUpdate (Phase 317 INV-02).
pub(crate) fn handle_game_inventory_update(
    state: &Arc<AppState>,
    inventory: &GameInventory,
) {
    tracing::info!(
        target: "fleet-inventory",
        "GameInventoryUpdate from pod {}: {} games",
        inventory.pod_id, inventory.games.len()
    );
    let state_clone = state.clone();
    let inv = inventory.clone();
    tokio::spawn(async move {
        crate::game_inventory::handle_game_inventory_update(&state_clone, inv).await;
    });
}

/// Handle AgentMessage::ComboValidationReport (Phase 317 COMBO-03/04).
pub(crate) fn handle_combo_validation_report(
    state: &Arc<AppState>,
    pod_id: &str,
    results: &[ComboValidationResult],
) {
    tracing::info!(
        target: "fleet-inventory",
        "ComboValidationReport from pod {}: {} results",
        pod_id, results.len()
    );
    let state_clone = state.clone();
    let pid = pod_id.to_string();
    let res = results.to_vec();
    tokio::spawn(async move {
        crate::game_inventory::handle_combo_validation_report(&state_clone, pid, res).await;
    });
}

/// Handle AgentMessage::LaunchTimelineReport (Phase 318 LAUNCH-05).
pub(crate) fn handle_launch_timeline_report(
    state: &Arc<AppState>,
    timeline: &LaunchTimeline,
) {
    let db = state.db.clone();
    let events_json = serde_json::to_string(&timeline.events)
        .unwrap_or_else(|_| "[]".to_string());
    let created_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let tl = timeline.clone();
    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT OR REPLACE INTO launch_timeline_spans
             (launch_id, pod_id, sim_type, preset_id, billing_session_id,
              outcome, total_duration_ms, started_at, events_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&tl.launch_id)
        .bind(&tl.pod_id)
        .bind(tl.sim_type.to_string())
        .bind(&tl.preset_id)
        .bind(&tl.billing_session_id)
        .bind(&tl.outcome)
        .bind(tl.total_duration_ms as i64)
        .bind(&tl.started_at)
        .bind(&events_json)
        .bind(&created_at)
        .execute(&db)
        .await;
        if let Err(e) = result {
            tracing::error!(
                "LAUNCH-05: Failed to insert launch_timeline_spans for {}: {}",
                tl.launch_id, e
            );
        } else {
            tracing::info!(
                "LAUNCH-05: Persisted launch timeline for {} (outcome={})",
                tl.launch_id, tl.outcome
            );
        }
    });
}

/// Handle AgentMessage::ConfigMismatchDetected.
pub(crate) async fn handle_config_mismatch_detected(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &rc_common::types::SimType,
    mismatches: &[(String, String, String)],
    timestamp: &str,
) {
    let mismatch_details: Vec<String> = mismatches.iter()
        .map(|(field, expected, actual)| {
            format!("{}: expected '{}', got '{}'", field, expected, actual)
        })
        .collect();
    let detail_str = mismatch_details.join("; ");
    tracing::warn!(
        "CONFIG MISMATCH on Pod {} ({:?}): {} [{}]",
        pod_id, sim_type, detail_str, timestamp
    );

    crate::event_archive::append_event(
        &state.db,
        "game.config_mismatch",
        "agent",
        Some(pod_id),
        serde_json::json!({
            "sim_type": format!("{:?}", sim_type),
            "mismatches": mismatches,
            "timestamp": timestamp,
        }),
        &state.config.venue.venue_id,
    );

    let alert_msg = format!(
        "\u{26a0}\u{fe0f} CONFIG MISMATCH \u{2014} Pod {}\nSim: {:?}\n{}\nTimestamp: {}\n\nCustomer may have wrong game settings. Check kiosk wizard \u{2192} race.ini pipeline.",
        pod_id, sim_type, detail_str, timestamp
    );
    crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;

    let _ = state.dashboard_tx.send(DashboardEvent::ConfigMismatch {
        pod_id: pod_id.to_string(),
        sim_type: format!("{:?}", sim_type),
        details: mismatch_details,
        timestamp: timestamp.to_string(),
    });
}
