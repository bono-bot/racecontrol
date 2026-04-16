#![allow(unused_imports)]
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Game Launcher ─────────────────────────────────────────────────────────

pub(crate) async fn launch_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let sim_type_str = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("");
    let launch_args_raw = body
        .get("launch_args")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if pod_id.is_empty() || sim_type_str.is_empty() {
        return Json(json!({ "error": "pod_id and sim_type are required" })).into_response();
    }

    // Act 2: Trial sessions are AC-only — reject game launches for other sims during trials
    let is_trial_session = sqlx::query_as::<_, (bool,)>(
        "SELECT pt.is_trial FROM billing_sessions bs \
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id \
         WHERE bs.pod_id = ? AND bs.status IN ('active', 'waiting_for_game') \
         ORDER BY bs.created_at DESC LIMIT 1",
    )
    .bind(pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(t,)| t)
    .unwrap_or(false);

    if is_trial_session && sim_type_str != "assetto_corsa" {
        return Json(json!({ "error": "Free trial sessions are limited to Assetto Corsa only" })).into_response();
    }

    // Inject duration_minutes from active billing session into launch_args.
    // Uses REMAINING time (not allocated) so mid-session relaunches get correct duration.
    // Ceiling division ensures AC session >= billing time (no early AC expiry).
    let launch_args = if let Some(args) = launch_args_raw {
        let session_info = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
            "SELECT allocated_seconds, driving_seconds, split_duration_minutes FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
        )
        .bind(pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let duration_minutes: u32 = match &session_info {
            // Split sessions: use fixed split duration (each segment is independent)
            Some((_, _, Some(split_mins))) if sim_type_str == "assetto_corsa" => *split_mins as u32,
            // Non-split: use remaining time with ceiling division
            Some((allocated, driven, _)) => {
                let remaining_secs = (*allocated as u32).saturating_sub(*driven as u32);
                remaining_secs.div_ceil(60)  // ceiling division — AC never expires before billing
            }
            None => 60,
        };

        let mut parsed: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);

        // SEC-01: Validate launch_args fields for INI injection chars BEFORE WS send.
        // Reject at the server boundary — 400 returned immediately, nothing reaches the agent.
        if let Err(e) = crate::api::security::validate_launch_args(&parsed) {
            return Json(json!({ "error": format!("Invalid launch_args: {}", e) })).into_response();
        }

        // SEC-02: Sanitize FFB GAIN — cap to 100 (physical motor safety).
        if let Some(ffb_str) = parsed.get("ffb").and_then(|v| v.as_str()) {
            let safe_ffb = crate::api::security::sanitize_ffb_gain(ffb_str);
            parsed["ffb"] = serde_json::json!(safe_ffb);
        }

        Some(parsed.to_string())
    } else {
        None
    };

    let sim_type: SimType = match serde_json::from_value(serde_json::Value::String(
        sim_type_str.to_string(),
    )) {
        Ok(st) => st,
        Err(_) => return Json(json!({ "error": format!("Unknown sim_type: {}", sim_type_str) })).into_response(),
    };

    // Phase 361-01: Server-side validity gate.
    //
    // Parse pod number from pod_id (e.g. "pod_1" -> 1, "1" -> 1). Load the
    // pod's inventory TOML from `config_dir`, run the validity gate, and
    // return HTTP 422 on failure with the ValidityError as JSON. This runs
    // BEFORE game_launcher::handle_dashboard_command, which is the earliest
    // point where we can inspect car/track AND the latest point before any
    // WS dispatch / lock acquisition in game_launcher.
    //
    // NOTE: The plan specified wiring into `create_session` at routes.rs:2541,
    // but that legacy handler only accepts (type, sim_type, track, car_class)
    // via a Json<Value> body — it does NOT receive pod_id, car, or ai_count.
    // The actual user-selected tuple flows through `launch_game` instead, so
    // the gate lives here. Documented in 361-01-SUMMARY.md as a deviation.
    //
    // Pre-lock placement: this block does sync I/O only (std::fs::read_to_string
    // + toml::from_str), no .await, no lock acquisition. The existing
    // `game_launcher::handle_dashboard_command(&state, cmd).await` remains the
    // first .await after this gate.
    {
        let pod_num: Option<u32> = pod_id
            .strip_prefix("pod_")
            .or(Some(pod_id))
            .and_then(|s| s.parse::<u32>().ok());
        if let Some(n) = pod_num {
            let config_dir = state.config.server.config_dir_path();
            match crate::api::pods::load_pod_inventory(n, &config_dir) {
                Ok(inv) => {
                    // Extract car/track/ai_count from the (already-validated
                    // for injection) launch_args JSON. Missing fields default
                    // to empty strings / 0 which degrade-open for games whose
                    // inventory lists are empty and otherwise are rejected
                    // with a CAR_NOT_AVAILABLE / TRACK_NOT_AVAILABLE error.
                    let args_parsed: serde_json::Value = launch_args
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(serde_json::Value::Object(Default::default()));
                    let car = args_parsed
                        .get("car")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let track = args_parsed
                        .get("track")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let ai_count = args_parsed
                        .get("ai_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    if let Err(err) = crate::validation::session_validity::validate_session_tuple(
                        sim_type_str, car, track, ai_count, &inv,
                    ) {
                        // Return the structured ValidityError as JSON. The
                        // HTTP status is 200 here for backward compatibility
                        // with existing callers that consume `error` on the
                        // body; we include `status: 422` inside the body so
                        // clients can discriminate. Phase 361-02 kiosk will
                        // switch to the status-code path once the endpoint
                        // is moved to its own route.
                        return Json(serde_json::json!({
                            "error": err.reason,
                            "reason": err.reason,
                            "suggestion": err.suggestion,
                            "code": err.code,
                            "status": 422,
                        })).into_response();
                    }
                }
                Err((code, msg)) if code == axum::http::StatusCode::NOT_FOUND => {
                    // Pod TOML missing — degrade-open (don't block launch on
                    // infra gap). Log for drift detection.
                    tracing::warn!(
                        pod_id = pod_id,
                        error = msg,
                        "pod inventory TOML missing; validity gate skipped (degrade-open)"
                    );
                }
                Err((_, msg)) => {
                    tracing::error!(
                        pod_id = pod_id,
                        error = msg,
                        "pod inventory TOML parse error; validity gate skipped"
                    );
                }
            }
        }
    }

    // INTEL-01: Query combo reliability BEFORE launching — build warning if success_rate < 70%.
    // Parse car/track from the already-injected launch_args JSON (duration_minutes was added above).
    let reliability_warning: Option<String> = {
        let args_parsed: serde_json::Value = launch_args
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Object(Default::default()));
        let car = args_parsed.get("car").and_then(|v| v.as_str());
        let track = args_parsed.get("track").and_then(|v| v.as_str());
        crate::metrics::query_combo_reliability(&state.db, pod_id, sim_type_str, car, track)
            .await
            .filter(|r| r.success_rate < 0.70)
            .map(|r| {
                format!(
                    "This combination has a {:.0}% success rate on this pod ({}/{} launches)",
                    r.success_rate * 100.0,
                    (r.success_rate * r.total_launches as f64).round() as i64,
                    r.total_launches
                )
            })
    };

    // DB-2 FIX: Extract car/track from launch_args BEFORE moving into cmd.
    // These are used to update billing_sessions after successful launch.
    let (launch_car, launch_track) = {
        let args_parsed: serde_json::Value = launch_args
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Object(Default::default()));
        (
            args_parsed.get("car").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            args_parsed.get("track").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        )
    };

    let cmd = rc_common::protocol::DashboardCommand::LaunchGame {
        pod_id: pod_id.to_string(),
        sim_type,
        launch_args,
    };

    match game_launcher::handle_dashboard_command(&state, cmd).await {
        Ok(()) => {
            // DB-2 FIX: Update billing_sessions with car/track/sim_type on successful launch.
            // Previously only launch_or_assist() (PIN auth flow) wrote these fields.
            // The /games/launch path (kiosk game selection, admin dashboard) skipped the update,
            // leaving car/track/sim_type NULL for most sessions.
            {
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET car = ?, track = ?, sim_type = ? \
                     WHERE pod_id = ? AND status IN ('active', 'waiting_for_game') \
                     AND car IS NULL \
                     ORDER BY created_at DESC LIMIT 1",
                )
                .bind(&launch_car)
                .bind(&launch_track)
                .bind(sim_type_str)
                .bind(pod_id)
                .execute(&state.db)
                .await;
            }

            // CLOSED-LOOP: Per-launch verification from the pod's game tracker.
            // Previous implementation used a global AtomicBool (last_launch_verified) that
            // returned stale results from prior launches on ANY pod — causing verified=true
            // even when the current launch failed. Now we check this pod's actual tracker state.
            let verified = {
                let games = state.game_launcher.active_games.read().await;
                games.get(pod_id)
                    .map(|t| t.game_state == rc_common::types::GameState::Running)
                    .unwrap_or(false)
            };
            let mut resp = json!({
                "ok": true,
                "verified": verified,
            });
            if !verified {
                resp["verification_warning"] = json!(
                    "Game launch command sent but process not confirmed running within 20s. Check pod manually."
                );
            }
            if let Some(w) = reliability_warning {
                resp["warning"] = json!(w);
            }
            Json(resp).into_response()
        }
        Err(e) if e.contains("No agent connected") => {
            // No local pod — try relaying to venue via Tailscale bono_relay
            relay_game_launch_to_venue(&state, pod_id, sim_type_str, &body).await.into_response()
        }
        // Phase 366 GLD-F-04: Return HTTP 409 for concurrent game launch
        Err(e) if e.contains("already has a game active") || e.contains("game still stopping") => {
            (
                axum::http::StatusCode::CONFLICT,
                Json(json!({
                    "error": "game_already_active",
                    "pod_id": pod_id,
                    "detail": e
                })),
            ).into_response()
        }
        Err(e) => Json(json!({ "ok": false, "error": e })).into_response(),
    }
}

/// Relay a game launch command to venue server via Tailscale bono_relay.
/// Called when cloud has no local agent connected for the target pod.
pub(crate) async fn relay_game_launch_to_venue(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type_str: &str,
    body: &Value,
) -> Json<Value> {
    let bono = &state.config.bono;
    if !bono.enabled {
        return Json(json!({ "ok": false, "error": "No local agent and venue relay not configured" }));
    }

    let relay_ip = match &bono.tailscale_bind_ip {
        Some(ip) => ip.clone(),
        None => return Json(json!({ "ok": false, "error": "No venue Tailscale IP configured" })),
    };
    let relay_secret = bono.relay_secret.as_deref().unwrap_or("");
    let relay_url = format!("http://{}:{}/relay/command", relay_ip, bono.relay_port);

    // Resolve pod_id to pod_number for the relay command
    let pod_number = {
        let pods = state.pods.read().await;
        pods.values()
            .find(|p| p.id == pod_id)
            .map(|p| p.number)
    };

    let pod_number = match pod_number {
        Some(n) => n,
        None => {
            // Try parsing pod_id as "pod-N" format
            match pod_id.strip_prefix("pod-").and_then(|n| n.parse::<u32>().ok()) {
                Some(n) if n > 0 => n,
                _ => {
                    tracing::warn!("Venue relay: cannot resolve pod_id '{}' to pod number — pod not found in registry and id format unrecognized", pod_id);
                    return Json(json!({ "ok": false, "error": format!("Cannot resolve pod_id '{}' to pod number for venue relay. Pod may be offline or not registered.", pod_id) }));
                }
            }
        }
    };

    let track = body.get("launch_args")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("track").and_then(|t| t.as_str()).map(|s| s.to_string()));
    let car = body.get("launch_args")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("car").and_then(|c| c.as_str()).map(|s| s.to_string()));

    let relay_cmd = json!({
        "type": "launch_game",
        "data": {
            "pod_number": pod_number,
            "game": sim_type_str,
            "track": track,
            "car": car
        }
    });

    tracing::info!(
        "Relaying game launch to venue: pod_number={}, game={}, relay_url={}",
        pod_number, sim_type_str, relay_url
    );

    match state.http_client
        .post(&relay_url)
        .header("X-Relay-Secret", relay_secret)
        .json(&relay_cmd)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or_default();
            Json(json!({ "ok": true, "relayed": true, "venue_response": body }))
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("Venue relay returned {}: {}", status, body);
            Json(json!({ "ok": false, "error": format!("Venue relay returned {}: {}", status, body) }))
        }
        Err(e) => {
            tracing::error!("Venue relay request failed: {}", e);
            Json(json!({ "ok": false, "error": format!("Cannot reach venue: {}", e) }))
        }
    }
}

/// CRASH-04: Relaunch a crashed game using stored launch_args
pub(crate) async fn relaunch_game(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    match game_launcher::relaunch_game(&state, &pod_id).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// ─── Pod Controls (split to game_pod_controls.rs) ─────────────────────────
#[path = "game_pod_controls.rs"]
mod game_pod_controls;
pub(crate) use game_pod_controls::{set_pod_transmission, set_pod_ffb, set_pod_assists, get_pod_assist_state};

pub(crate) async fn stop_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");

    if pod_id.is_empty() {
        return Json(json!({ "error": "pod_id is required" }));
    }

    let cmd = rc_common::protocol::DashboardCommand::StopGame {
        pod_id: pod_id.to_string(),
    };

    let _ = game_launcher::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

/// Returns the full game catalog — authoritative source for all UI game lists.
/// Each entry includes the sim_type id (snake_case), display name, and abbreviation.
/// Pods filter this list against their `installed_games` field.
pub(crate) async fn games_catalog(State(state): State<Arc<AppState>>) -> Json<Value> {
    let all_games = [
        SimType::AssettoCorsa,
        SimType::AssettoCorsaEvo,
        SimType::AssettoCorsaRally,
        SimType::IRacing,
        SimType::LeMansUltimate,
        SimType::F125,
        SimType::Forza,
        SimType::ForzaHorizon5,
    ];

    // Count how many pods have each game installed
    let pods = state.pods.read().await;
    let mut install_counts: std::collections::HashMap<SimType, usize> = std::collections::HashMap::new();
    for pod in pods.values() {
        for game in &pod.installed_games {
            *install_counts.entry(*game).or_insert(0) += 1;
        }
    }

    let catalog: Vec<Value> = all_games.iter().map(|sim| {
        let id = serde_json::to_value(sim).unwrap_or(json!("unknown"));
        let id_str = id.as_str().unwrap_or("unknown");
        let abbr = match sim {
            SimType::AssettoCorsa => "AC",
            SimType::AssettoCorsaEvo => "ACE",
            SimType::AssettoCorsaRally => "ACR",
            SimType::IRacing => "iR",
            SimType::LeMansUltimate => "LMU",
            SimType::F125 => "F1",
            SimType::Forza => "FM",
            SimType::ForzaHorizon5 => "FH5",
        };
        json!({
            "id": id_str,
            "name": sim.to_string(),
            "abbr": abbr,
            "installed_pod_count": install_counts.get(sim).unwrap_or(&0),
        })
    }).collect();

    Json(json!({ "games": catalog }))
}
