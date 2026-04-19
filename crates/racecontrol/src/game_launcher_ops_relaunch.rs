//! Game relaunch operations — extracted from game_launcher_ops.rs (v49.0 Architecture Completion).

use std::sync::Arc;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::game_launcher_support::{extract_launch_fields, log_game_event};
use crate::metrics;
use crate::state::AppState;
use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::GameState;

/// CRASH-04: Relaunch a crashed game using stored launch_args from GameTracker.
/// Resets auto_relaunch_count so Race Engineer gets fresh attempts.
pub async fn relaunch_game(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), String> {
    // Normalize pod_id to canonical form at function entry
    let pod_id_owned = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_owned.as_str();

    // Get stored launch info from tracker
    let (sim_type, launch_args) = {
        let games = state.game_launcher.active_games.read().await;
        let tracker = games.get(pod_id).ok_or("No game tracker for pod")?;
        if tracker.game_state != GameState::Error {
            return Err(format!("Pod game is {:?}, not Error — cannot relaunch", tracker.game_state));
        }
        // LAUNCH-16: Guard null launch_args — externally tracked games don't have original args
        if tracker.externally_tracked || tracker.launch_args.is_none() {
            return Err(format!(
                "Cannot relaunch pod {} — original launch args unavailable (externally tracked or null). Please relaunch from kiosk.",
                pod_id
            ));
        }
        (tracker.sim_type, tracker.launch_args.clone())
    };

    // Verify billing is still active
    let has_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id);
    if !has_billing {
        return Err("No active billing session — cannot relaunch".into());
    }

    // Reset relaunch counter for fresh Race Engineer attempts
    {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.auto_relaunch_count = 0;
            tracker.game_state = GameState::Launching;
            tracker.error_message = None;
            tracker.launched_at = Some(Utc::now());
        }
    }

    // Extract launch fields for metrics BEFORE consuming launch_args in the send
    let (relaunch_car, relaunch_track, relaunch_session_type, relaunch_args_hash) =
        extract_launch_fields(&launch_args);

    // Send LaunchGame to agent (canonical pod_id guaranteed by normalization at entry)
    let senders = state.agent_senders.read().await;
    let tx = senders.get(pod_id).ok_or("Pod not connected")?;
    tx.send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
        sim_type,
        launch_args,
        force_clean: true,
        duration_minutes: None,
        launch_id: None,
    }))
    .await
    .map_err(|e| format!("Failed to send launch command: {}", e))?;

    // Log + broadcast
    log_game_event(state, pod_id, &sim_type.to_string(), "relaunched", None, Some("Manual relaunch from kiosk"), None).await;

    // Rich launch event recording for relaunch
    {
        let relaunch_event = metrics::LaunchEvent {
            id: uuid::Uuid::new_v4().to_string(),
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: relaunch_car,
            track: relaunch_track,
            session_type: relaunch_session_type,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            outcome: metrics::LaunchOutcome::Success,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: Some("Manual relaunch from kiosk".to_string()),
            launch_args_hash: relaunch_args_hash,
            attempt_number: 2,
            db_fallback: None,
            session_id: None,
        };
        metrics::record_launch_event(&state.db, &relaunch_event, &state.config.venue.venue_id).await;
    }
    let info = {
        let games = state.game_launcher.active_games.read().await;
        games.get(pod_id).map(|t| t.to_info())
    };
    if let Some(info) = info {
        let pod_id_for_warn = pod_id.to_string();
        if let Err(e) = state
            .dashboard_tx
            .send(DashboardEvent::GameStateChanged(info))
        {
            tracing::warn!("dashboard broadcast failed for pod {}: {}", pod_id_for_warn, e);
        }
    }

    log_pod_activity(state, pod_id, "game", "Game Relaunched", "Manual relaunch from kiosk", "core", None);

    Ok(())
}
