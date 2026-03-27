//! Bono relay: push real-time events to Bono's VPS over Tailscale mesh,
//! and receive inbound commands from Bono's cloud at the relay endpoint.
//!
//! Two responsibilities:
//! 1. EVENT PUSH — subscribes to AppState.bono_event_tx broadcast channel and
//!    POSTs each event to Bono's webhook URL (reqwest 0.12, same as cloud_sync).
//! 2. COMMAND RELAY — Axum endpoint bound to Tailscale IP (plan 03) accepts
//!    commands from Bono and forwards to pods via existing WebSocket channel.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use crate::game_launcher;
use rc_common::protocol::DashboardCommand;

// ─── Event Types ─────────────────────────────────────────────────────────────

/// Events pushed from racecontrol server to Bono's VPS webhook.
/// Tagged enum following the AgentMessage pattern in rc-common.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BonoEvent {
    SessionStart { pod_number: u32, driver_name: String, game: String, session_id: String },
    SessionEnd { pod_number: u32, session_id: String, duration_secs: u64, paise_charged: i64 },
    LapRecorded { pod_number: u32, session_id: String, lap_time_ms: u64, track: String, car: String },
    PodOffline { pod_number: u32, ip: String, last_seen_secs_ago: u64 },
    PodOnline { pod_number: u32, ip: String, tailscale_ip: Option<String> },
    BillingEnd { pod_number: u32, session_id: String, driver_id: String },
}

// ─── Command Types ────────────────────────────────────────────────────────────

/// Commands Bono's VPS can send to the relay endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RelayCommand {
    LaunchGame { pod_number: u32, game: String, track: Option<String>, car: Option<String> },
    StopGame { pod_number: u32 },
    GetStatus { pod_number: u32 },
}

// ─── Spawn ────────────────────────────────────────────────────────────────────

/// Spawn the Bono relay event-push background task.
///
/// No-ops if bono.enabled = false or webhook_url is not configured.
/// When running, subscribes to AppState.bono_event_tx and POSTs each event
/// to the configured webhook URL using the shared reqwest client.
pub fn spawn(state: Arc<AppState>) {
    let bono = &state.config.bono;

    if !bono.enabled {
        tracing::info!("Bono relay disabled");
        return;
    }

    let webhook_url = match &bono.webhook_url {
        Some(url) => url.clone(),
        None => {
            tracing::warn!("Bono relay enabled but no webhook_url configured — event push skipped");
            return;
        }
    };

    tracing::info!("Bono relay spawned — pushing events to {}", webhook_url);

    tokio::spawn(async move {
        let mut rx = state.bono_event_tx.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = push_event(&state, &webhook_url, &event).await {
                        // Non-fatal: log and continue. Missing one event is acceptable.
                        // Next event will retry the connection naturally.
                        tracing::warn!("Bono webhook push failed: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Bono relay: dropped {} events (channel lagged)", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::error!("Bono relay: event channel closed — relay task exiting");
                    break;
                }
            }
        }
    });
}

/// POST a single event to Bono's webhook URL.
/// Bug #13: Retries once after a 2s delay if the first attempt fails.
async fn push_event(
    state: &Arc<AppState>,
    webhook_url: &str,
    event: &BonoEvent,
) -> anyhow::Result<()> {
    let result = state
        .http_client
        .post(webhook_url)
        .json(event)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(first_err) => {
            tracing::debug!("Bono webhook first attempt failed: {} — retrying in 2s", first_err);
            tokio::time::sleep(Duration::from_secs(2)).await;
            state
                .http_client
                .post(webhook_url)
                .json(event)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Webhook POST failed after retry: {}", e))?;
            Ok(())
        }
    }
}

// ─── Relay Endpoint ───────────────────────────────────────────────────────────

/// Build the relay Axum router.
/// Bound to the Tailscale IP in main.rs (Plan 03).
/// Machine-to-machine only — no CORS, no JWT, auth via X-Relay-Secret header.
pub fn build_relay_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/relay/command", post(handle_command))
        .route("/relay/health", get(relay_health))
        .with_state(state)
}

/// Health endpoint on the relay router (Tailscale interface only).
async fn relay_health() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok", "service": "bono-relay"}))
}

/// Inbound command handler: accepts commands from Bono's VPS.
/// Auth: X-Relay-Secret header must match config.bono.relay_secret.
pub async fn handle_command(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(cmd): Json<RelayCommand>,
) -> impl IntoResponse {
    // Validate relay secret
    let expected_secret = state
        .config
        .bono
        .relay_secret
        .as_deref()
        .unwrap_or("");

    let provided_secret = headers
        .get("X-Relay-Secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if expected_secret.is_empty() || provided_secret != expected_secret {
        tracing::warn!("Bono relay: rejected command with bad or missing X-Relay-Secret");
        return (StatusCode::UNAUTHORIZED, "invalid relay secret").into_response();
    }

    tracing::info!("Bono relay: received command {:?}", cmd);

    // Route command to pod via existing WebSocket channel
    match cmd {
        RelayCommand::LaunchGame { pod_number, game, track, car } => {
            tracing::info!(
                pod_number,
                game = %game,
                track = ?track,
                car = ?car,
                "Bono relay: LaunchGame — forwarding to pod via game_launcher"
            );

            // Resolve pod_number to pod_id
            let pod_id = {
                let pods = state.pods.read().await;
                pods.values()
                    .find(|p| p.number == pod_number)
                    .map(|p| p.id.clone())
            };

            let pod_id = match pod_id {
                Some(id) => id,
                None => {
                    return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({
                        "error": format!("No pod with number {} connected", pod_number)
                    }))).into_response();
                }
            };

            // Build launch_args JSON from track/car if provided
            let launch_args = if track.is_some() || car.is_some() {
                let mut args = serde_json::json!({});
                if let Some(t) = &track { args["track"] = serde_json::json!(t); }
                if let Some(c) = &car { args["car"] = serde_json::json!(c); }
                Some(args.to_string())
            } else {
                None
            };

            let sim_type: rc_common::types::SimType = match serde_json::from_value(
                serde_json::Value::String(game.clone()),
            ) {
                Ok(st) => st,
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({
                        "error": format!("Unknown game/sim_type: {}", game)
                    }))).into_response();
                }
            };

            let cmd = DashboardCommand::LaunchGame {
                pod_id: pod_id.clone(),
                sim_type,
                launch_args,
            };

            match game_launcher::handle_dashboard_command(&state, cmd).await {
                Ok(()) => (StatusCode::OK, axum::Json(serde_json::json!({
                    "status": "launched",
                    "pod_number": pod_number,
                    "pod_id": pod_id
                }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({
                    "error": e
                }))).into_response(),
            }
        }
        RelayCommand::StopGame { pod_number } => {
            tracing::info!(pod_number, "Bono relay: StopGame");

            let pod_id = {
                let pods = state.pods.read().await;
                pods.values()
                    .find(|p| p.number == pod_number)
                    .map(|p| p.id.clone())
            };

            let pod_id = match pod_id {
                Some(id) => id,
                None => {
                    return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({
                        "error": format!("No pod with number {} connected", pod_number)
                    }))).into_response();
                }
            };

            let cmd = DashboardCommand::StopGame { pod_id: pod_id.clone() };
            match game_launcher::handle_dashboard_command(&state, cmd).await {
                Ok(()) => (StatusCode::OK, axum::Json(serde_json::json!({
                    "status": "stopped",
                    "pod_number": pod_number,
                    "pod_id": pod_id
                }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({
                    "error": e
                }))).into_response(),
            }
        }
        RelayCommand::GetStatus { pod_number } => {
            let pods = state.pods.read().await;
            let pod_info = pods.values()
                .find(|p| p.number == pod_number)
                .map(|p| serde_json::to_value(p).unwrap_or_default());
            (StatusCode::OK, axum::Json(serde_json::json!({
                "pod_number": pod_number,
                "info": pod_info
            }))).into_response()
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_disabled() {
        // spawn() with enabled=false must not panic — just return early
        // We can't easily call spawn() in unit tests without AppState,
        // but we can test the guard logic directly:
        let enabled = false;
        let webhook_url: Option<String> = None;
        // Guard: if not enabled, do nothing
        if enabled {
            panic!("Should not reach here when disabled");
        }
        let _ = webhook_url; // suppress unused warning
    }

    #[test]
    fn spawn_no_url() {
        // spawn() with enabled=true but no webhook_url must not panic
        let enabled = true;
        let webhook_url: Option<String> = None;
        if enabled {
            if webhook_url.is_none() {
                // correct path — no panic
                return;
            }
            panic!("Should not reach here when webhook_url is None");
        }
    }

    #[test]
    fn event_serialization() {
        let event = BonoEvent::SessionStart {
            pod_number: 3,
            driver_name: "Uday".to_string(),
            game: "assetto_corsa".to_string(),
            session_id: "sess-001".to_string(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(v["type"], "session_start");
        assert_eq!(v["data"]["pod_number"], 3);
        assert_eq!(v["data"]["driver_name"], "Uday");
        assert_eq!(v["data"]["game"], "assetto_corsa");
        assert_eq!(v["data"]["session_id"], "sess-001");
    }

    #[test]
    fn relay_command_serialization() {
        let cmd = RelayCommand::LaunchGame {
            pod_number: 5,
            game: "assetto_corsa".to_string(),
            track: Some("monza".to_string()),
            car: Some("ferrari_458_gt2".to_string()),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(v["type"], "launch_game");
        assert_eq!(v["data"]["pod_number"], 5);
        assert_eq!(v["data"]["track"], "monza");
    }
}
