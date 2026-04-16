//! Game launcher support functions — alerting, logging, parsing, health checks.
//!
//! Extracted from game_launcher.rs (Phase 385, v49.0 Architecture Completion).
//! Contains send_staff_launch_alert, check_game_health, log_game_event,
//! extract_launch_fields, and classify_error_taxonomy.

use std::sync::Arc;

use chrono::Utc;

use crate::activity_log::log_pod_activity;
use crate::game_launcher_state::handle_game_state_update;
use crate::metrics;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::{GameLaunchInfo, GameState, SimType};

pub async fn send_staff_launch_alert(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &str,
    error_taxonomy: &str,
    exit_codes: &[Option<i32>],
    suggested_action: &str,
) {
    let exit_codes_str = if exit_codes.is_empty() {
        "none recorded".to_string()
    } else {
        exit_codes.iter()
            .map(|c| c.map(|n| n.to_string()).unwrap_or_else(|| "unknown".to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let msg = format!(
        "Launch Failure - Pod {}\nGame: {}\nError: {}\nAttempts: 2/2 exhausted\nExit codes: {}\nSuggested: {}\nAction: Check pod + try different car/track",
        pod_id, sim_type, error_taxonomy, exit_codes_str, suggested_action
    );
    tracing::warn!("LAUNCH-15: Staff alert for pod {} — {}", pod_id, msg);

    // Access Evolution API config (same pattern as billing.rs WhatsApp receipt)
    let (evo_url, evo_key, evo_instance) = match &state.config.auth {
        auth => match (auth.evolution_url.as_deref(), auth.evolution_api_key.as_deref(), auth.evolution_instance.as_deref()) {
            (Some(url), Some(key), Some(inst)) => (url.to_string(), key.to_string(), inst.to_string()),
            _ => {
                tracing::warn!("LAUNCH-15: Evolution API not configured — skipping WhatsApp staff alert for pod {}", pod_id);
                return;
            }
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("LAUNCH-15: Failed to build HTTP client for staff alert: {}", e);
            return;
        }
    };

    let payload = serde_json::json!({
        "number": "917075778180",
        "text": msg,
    });

    match client
        .post(format!("{}/message/sendText/{}", evo_url, evo_instance))
        .header("apikey", evo_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            tracing::info!("LAUNCH-15: Staff WhatsApp alert sent for pod {} (status: {})", pod_id, resp.status());
        }
        Err(e) => {
            tracing::error!("LAUNCH-15: Failed to send staff WhatsApp alert for pod {}: {}", pod_id, e);
        }
    }
}

/// Periodic health check: detect stale Launching states.
/// AC with CSP mods can take 90s+ to load; use 120s timeout for AC, 60s for others.
pub async fn check_game_health(state: &Arc<AppState>) {
    let now = Utc::now();
    let mut timed_out = Vec::new();

    let mut needs_launched_at = Vec::new();

    {
        let games = state.game_launcher.active_games.read().await;
        for (pod_id, tracker) in games.iter() {
            if tracker.game_state == GameState::Launching {
                if let Some(launched_at) = tracker.launched_at {
                    let elapsed = now.signed_duration_since(launched_at);
                    let elapsed_secs = elapsed.num_seconds();
                    // LAUNCH-08: Use stored dynamic timeout, fall back to game-specific default
                    let timeout_secs = tracker.dynamic_timeout_secs.unwrap_or(match tracker.sim_type {
                        SimType::AssettoCorsa | SimType::AssettoCorsaRally | SimType::AssettoCorsaEvo => 120,
                        _ => 90,
                    });
                    let mut already_timed_out = false;
                    if elapsed_secs > timeout_secs {
                        timed_out.push((pod_id.clone(), tracker.sim_type, timeout_secs));
                        already_timed_out = true;
                    }
                    // GSTATE-01: Hard cap at 180s regardless of dynamic timeout
                    if !already_timed_out && elapsed_secs > 180 {
                        tracing::warn!("GSTATE-01: Hard-cap 180s timeout for pod {} (dynamic was {}s)", pod_id, timeout_secs);
                        timed_out.push((pod_id.clone(), tracker.sim_type, 180));
                    }
                } else {
                    // GSTATE-01: No launched_at = likely reconstructed from reconnect.
                    // Backfill with current time so the next health tick starts the countdown.
                    needs_launched_at.push(pod_id.clone());
                }
            }
            // STATE-01 edge case: detect stale Stopping state from server restart
            // (the in-memory timeout spawn is gone after restart, so we need this catch)
            // GAME-05 fix: use last_state_change (if available) or launched_at + game duration
            // to avoid force-erroring long-running games. Only timeout Stopping if the game
            // has been in Stopping state for >60s (not since launch).
            if tracker.game_state == GameState::Stopping
                && let Some(launched_at) = tracker.launched_at {
                    let since_launch = now.signed_duration_since(launched_at).num_seconds();
                    // GAME-05 MMA iter2: two tiers —
                    // 1) Short-lived (30-90s since launch): likely reconstructed post-restart
                    // 2) Long-stuck (>300s since launch): catch genuinely stuck Stopping states
                    // Normal in-memory 30s spawn handles non-restart cases in between.
                    if (since_launch > 30 && since_launch < 90) || since_launch > 300 {
                        timed_out.push((pod_id.clone(), tracker.sim_type, 30));
                    }
                }
        }
    }

    // GSTATE-01: Backfill launched_at for trackers that had None (e.g., from reconnect reconciliation).
    // Next check_game_health tick (5s) will start the real countdown.
    if !needs_launched_at.is_empty() {
        let mut games = state.game_launcher.active_games.write().await;
        for pod_id in &needs_launched_at {
            if let Some(tracker) = games.get_mut(pod_id)
                && tracker.game_state == GameState::Launching && tracker.launched_at.is_none() {
                    tracker.launched_at = Some(now);
                    tracing::warn!("GSTATE-01: Backfilled launched_at for pod {} (was None, likely from reconnect)", pod_id);
                }
        }
    }

    for (pod_id, sim_type, timeout_secs) in timed_out {
        tracing::warn!("Game launch timed out on pod {} ({}s limit for {:?})", pod_id, timeout_secs, sim_type);
        log_pod_activity(state, &pod_id, "game", "Launch Timeout", &format!("{} failed to start within {}s", sim_type, timeout_secs), "core", None);

        let timeout_msg = format!("Launch timed out ({}s)", timeout_secs);
        let info = GameLaunchInfo {
            pod_id: pod_id.clone(),
            sim_type,
            game_state: GameState::Error,
            pid: None,
            launched_at: None,
            error_message: Some(timeout_msg.clone()),
            diagnostics: None,
            exit_code: None,
            playable_at: None,
            ready_delay_ms: None,
            session_id: None, launch_stage: None,
        };

        // Update tracker
        if let Some(tracker) = state
            .game_launcher
            .active_games
            .write()
            .await
            .get_mut(&pod_id)
        {
            tracker.game_state = GameState::Error;
            tracker.error_message = Some(timeout_msg);
        }

        // Log (legacy table)
        log_game_event(state, &pod_id, &sim_type.to_string(), "timeout", None, None).await;

        // Rich timeout event recording
        {
            let timeout_event = metrics::LaunchEvent {
                id: uuid::Uuid::new_v4().to_string(),
                pod_id: pod_id.clone(),
                sim_type: sim_type.to_string(),
                car: None,
                track: None,
                session_type: None,
                timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                outcome: metrics::LaunchOutcome::Timeout,
                error_taxonomy: Some(metrics::ErrorTaxonomy::LaunchTimeout),
                duration_to_playable_ms: Some(timeout_secs * 1000),
                error_details: Some(format!("Launch timed out after {}s", timeout_secs)),
                launch_args_hash: None,
                attempt_number: 1,
                db_fallback: None,
                session_id: None,
            };
            metrics::record_launch_event(&state.db, &timeout_event, &state.config.venue.venue_id).await;
        }

        // LAUNCH-18: Route timeout through handle_game_state_update so Race Engineer
        // auto-relaunch logic fires on timeout (same path as crash events).
        // handle_game_state_update handles dashboard broadcast too — do NOT broadcast here.
        handle_game_state_update(state, info).await;

        // LAUNCH-01 (Phase 318): Notify agent so it can feed GameLaunchTimeout into
        // its tier engine (Game Doctor runs, attempts recovery).
        // Standing rule: snapshot the sender BEFORE awaiting — never hold a lock across .await.
        let sender_opt = {
            let senders = state.agent_senders.read().await;
            senders.get(&pod_id).cloned()
        };
        if let Some(tx) = sender_opt {
            let msg = CoreMessage::wrap(CoreToAgentMessage::LaunchTimedOut {
                sim_type,
                elapsed_secs: timeout_secs as u64,
            });
            if let Err(e) = tx.send(msg).await {
                tracing::warn!(
                    "LAUNCH-01: Failed to send LaunchTimedOut to pod {}: {}",
                    pod_id, e
                );
            }
        }
    }
}

pub async fn log_game_event(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &str,
    event_type: &str,
    pid: Option<u32>,
    error_message: Option<&str>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let result = sqlx::query(
        "INSERT INTO game_launch_events (id, pod_id, sim_type, event_type, pid, error_message, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(pod_id)
    .bind(sim_type)
    .bind(event_type)
    .bind(pid.map(|p| p as i64))
    .bind(error_message)
    .bind(&now)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        tracing::error!("game_launch_event insert failed for pod {pod_id}: {e}");
        // JSONL fallback — write to launch-events.jsonl so the event is never lost
        let fallback_event = crate::metrics::LaunchEvent {
            id,
            pod_id: pod_id.to_string(),
            sim_type: sim_type.to_string(),
            car: None,
            track: None,
            session_type: None,
            timestamp: now,
            outcome: crate::metrics::LaunchOutcome::Error,
            error_taxonomy: None,
            duration_to_playable_ms: None,
            error_details: error_message.map(|s| s.to_string()),
            launch_args_hash: None,
            attempt_number: 1,
            db_fallback: Some(true),
            session_id: None,
        };
        crate::metrics::record_launch_event_jsonl_only(&fallback_event).await;
    }
}

/// Extract car, track, session_type, and a hash from an optional launch_args JSON string.
/// Returns (car, track, session_type, hash) — all None if args absent or unparseable.
pub fn extract_launch_fields(
    launch_args: &Option<String>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let Some(args_json) = launch_args else {
        return (None, None, None, None);
    };
    let hash = Some(metrics::hash_launch_args(args_json));
    match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(v) => {
            let car = v.get("car").and_then(|x| x.as_str()).map(str::to_string);
            let track = v.get("track").and_then(|x| x.as_str()).map(str::to_string);
            let session_type = v
                .get("session_type")
                .and_then(|x| x.as_str())
                .map(str::to_string);
            (car, track, session_type, hash)
        }
        Err(_) => (None, None, None, hash),
    }
}

/// Classify an error message into a structured ErrorTaxonomy entry.
/// Uses simple heuristics — no heavy parsing needed at this phase.
pub fn classify_error_taxonomy(error_message: Option<&str>, exit_code: Option<i32>) -> metrics::ErrorTaxonomy {
    // Check typed exit_code first - more reliable than string parsing
    if let Some(code) = exit_code {
        return metrics::ErrorTaxonomy::ProcessCrash { exit_code: code as i64 };
    }
    let Some(msg) = error_message else {
        return metrics::ErrorTaxonomy::Unknown;
    };
    let msg_lower = msg.to_ascii_lowercase();
    if msg_lower.contains("shader") || msg_lower.contains("shader compilation") {
        metrics::ErrorTaxonomy::ShaderCompilationFail
    } else if msg_lower.contains("out of memory") || msg_lower.contains("oom") {
        metrics::ErrorTaxonomy::OutOfMemory
    } else if msg_lower.contains("anticheat") || msg_lower.contains("anti-cheat") {
        metrics::ErrorTaxonomy::AntiCheatKick
    } else if msg_lower.contains("config") && msg_lower.contains("corrupt") {
        metrics::ErrorTaxonomy::ConfigCorrupt
    } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
        metrics::ErrorTaxonomy::LaunchTimeout
    } else if msg_lower.contains("content manager") || msg_lower.contains("hang") {
        metrics::ErrorTaxonomy::ContentManagerHang
    } else if msg_lower.contains("missing") || msg_lower.contains("not found") {
        metrics::ErrorTaxonomy::MissingDependency
    } else if msg_lower.contains("billing") {
        metrics::ErrorTaxonomy::BillingGateRejected
    } else if msg_lower.contains("agent") || msg_lower.contains("disconnected") {
        metrics::ErrorTaxonomy::AgentDisconnected
    } else {
        metrics::ErrorTaxonomy::Unknown
    }
}
