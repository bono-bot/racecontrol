use std::sync::Arc;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Game String Parsing ─────────────────────────────────────────────────

/// Parse a game string (from kiosk_experiences, custom launch args, etc.) into a SimType.
/// Returns AssettoCorsa as default fallback for unknown game strings.
pub fn parse_sim_type(game: &str) -> rc_common::types::SimType {
    use rc_common::types::SimType;
    match game {
        "assetto_corsa" | "ac" => SimType::AssettoCorsa,
        "assetto_corsa_evo" | "ace" => SimType::AssettoCorsaEvo,
        "assetto_corsa_rally" | "ea_wrc" | "acr" | "wrc" => SimType::AssettoCorsaRally,
        "iracing" => SimType::IRacing,
        "f1_25" | "f1" => SimType::F125,
        "le_mans_ultimate" | "lmu" => SimType::LeMansUltimate,
        "forza" => SimType::Forza,
        "forza_horizon_5" | "fh5" => SimType::ForzaHorizon5,
        _ => SimType::AssettoCorsa,
    }
}

// ─── Game Availability Check ──────────────────────────────────────────────

/// Check if a game is available given a list of installed games.
/// Returns true if installed_games is empty (backward compat with old agents that don't report games).
pub fn check_pod_has_game(installed_games: &[rc_common::types::SimType], sim_type: rc_common::types::SimType) -> bool {
    if installed_games.is_empty() {
        true // backward compat: old agents don't report games -> assume available
    } else {
        installed_games.contains(&sim_type)
    }
}

/// Check if the pod has this game installed (from agent registration).
/// Returns true if pod is not found or has no installed_games data (backward compat with old agents).
async fn pod_has_game(state: &Arc<AppState>, pod_id: &str, sim_type: rc_common::types::SimType) -> bool {
    let pods = state.pods.read().await;
    match pods.get(pod_id) {
        Some(pod) => check_pod_has_game(&pod.installed_games, sim_type),
        None => false,
    }
}

/// Auto-launch game or show assistance screen depending on game type.
/// Returns the game name if an experience was linked.
pub(crate) async fn launch_or_assist(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_session_id: &str,
    experience_id: &Option<String>,
    custom_launch_args: &Option<String>,
    driver_name: &str,
) -> Option<String> {
    // Determine game/track/car from either custom launch args or experience
    let (game, track, car, launch_args_json) = if let Some(custom_args) = custom_launch_args {
        // Custom booking — parse the stored launch_args JSON
        let parsed: serde_json::Value = serde_json::from_str(custom_args).ok()?;
        let game = parsed["game"].as_str().unwrap_or("assetto_corsa").to_string();
        let track = parsed["track"].as_str().unwrap_or("").to_string();
        let car = parsed["car"].as_str().unwrap_or("").to_string();
        (game, track, car, custom_args.clone())
    } else if let Some(exp_id) = experience_id.as_ref() {
        // Pre-defined experience
        let exp = sqlx::query_as::<_, (String, String, String)>(
            "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
        )
        .bind(exp_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()?;
        let launch_args = serde_json::json!({
            "car": exp.2,
            "track": exp.1,
            "driver": driver_name,
            "transmission": "auto",
            "aids": { "abs": 1, "tc": 1, "stability": 1, "autoclutch": 1, "ideal_line": 1 },
            "conditions": { "damage": 0 }
        })
        .to_string();
        (exp.0, exp.1, exp.2, launch_args)
    } else {
        return None;
    };

    // Look up billing session duration — use remaining time (for crash relaunches)
    let duration_minutes: u32 = sqlx::query_as::<_, (i64, i64)>(
        "SELECT allocated_seconds, driving_seconds FROM billing_sessions WHERE id = ?",
    )
    .bind(billing_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(alloc, driven)| {
        let remaining = (alloc as u32).saturating_sub(driven as u32);
        remaining.div_ceil(60) // round up to nearest minute
    })
    .unwrap_or(60);

    // Inject duration_minutes into launch_args JSON
    let launch_args_json = {
        let mut parsed: serde_json::Value = serde_json::from_str(&launch_args_json).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);
        parsed.to_string()
    };

    let sim_type = parse_sim_type(&game);

    // Clone sender, drop lock before .await — prevents deadlock
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(pod_id).cloned()
    };
    if let Some(sender) = sender {
        if !pod_has_game(state, pod_id, sim_type).await {
            // Game not installed on this pod — show assistance screen
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_name.to_string(),
                    message: format!("{} is not installed on this pod — staff will assist", game),
                }))
                .await;

            // Broadcast assistance needed to kiosk dashboards
            let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                pod_id: pod_id.to_string(),
                driver_name: driver_name.to_string(),
                game: game.clone(),
                reason: format!("{} is not installed on this pod", game),
            });

            tracing::info!(
                "Game {} not installed on pod {} — assistance needed (driver: {})",
                game, pod_id, driver_name
            );
        } else {
            // Auto-spawn game
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                    sim_type,
                    launch_args: Some(launch_args_json),
                    force_clean: false,
                    duration_minutes: None,
                    launch_id: None,
                }))
                .await;

            tracing::info!(
                "Auto-launching {} on pod {} (car: {}, track: {})",
                game, pod_id, car, track
            );
        }
    }

    // Update billing session with experience info
    let exp_id = experience_id.as_deref().unwrap_or("");
    let _ = sqlx::query(
        "UPDATE billing_sessions SET experience_id = ?, car = ?, track = ?, sim_type = ? WHERE id = ?",
    )
    .bind(exp_id)
    .bind(&car)
    .bind(&track)
    .bind(&game)
    .bind(billing_session_id)
    .execute(&state.db)
    .await;

    Some(game)
}
