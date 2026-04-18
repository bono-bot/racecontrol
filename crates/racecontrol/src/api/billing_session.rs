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

/// Returns valid session split options for a given total duration.
/// AC-specific: customers can divide their session into shorter sub-sessions.
pub(crate) async fn get_split_options(
    Path(duration_minutes): Path<u32>,
) -> Json<Value> {
    let options = compute_split_options(duration_minutes);
    Json(json!({ "duration_minutes": duration_minutes, "options": options }))
}

/// Compute valid split configurations for a given total duration.
/// Rules: each sub-session must be at least 10 minutes, count * sub_duration == total.
pub(crate) fn compute_split_options(total_minutes: u32) -> Vec<serde_json::Value> {
    let mut options = Vec::new();
    // Always include the unsplit option
    options.push(json!({ "count": 1, "duration_minutes": total_minutes, "label": format!("1 × {} min", total_minutes) }));

    // Find all valid splits where sub-session >= 10 min
    for count in 2..=6 {
        if total_minutes.is_multiple_of(count) {
            let sub = total_minutes / count;
            if sub >= 10 {
                options.push(json!({
                    "count": count,
                    "duration_minutes": sub,
                    "label": format!("{} × {} min", count, sub),
                }));
            }
        }
    }
    options
}

/// Continue to next sub-session in a split session. No wallet debit — already paid.
pub(crate) async fn continue_split(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = match body.get("pod_id").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(json!({ "error": "pod_id required" })),
    };
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa").to_string();
    let launch_args = body.get("launch_args").and_then(|v| v.as_str()).unwrap_or("{}").to_string();

    // Find active reservation for this pod
    let reservation = match crate::pod_reservation::get_active_reservation_for_pod(&state, &pod_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation on this pod" })),
    };

    // Must not have an active billing session on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&pod_id) {
            return Json(json!({ "error": "A session is still active on this pod" }));
        }
    }

    // Look up the original split session details from the reservation
    let original = match sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT split_count, split_duration_minutes, pricing_tier_id, driver_id
         FROM billing_sessions
         WHERE reservation_id = ?
         ORDER BY started_at ASC LIMIT 1",
    )
    .bind(&reservation.id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "No billing sessions found for this reservation" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let total_splits = original.0 as u32;
    let split_duration_minutes = original.1 as u32;
    let pricing_tier_id = original.2;
    let driver_id = original.3;

    // Count completed sessions in this reservation
    let completed = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions WHERE reservation_id = ? AND status IN ('completed', 'ended_early')",
    )
    .bind(&reservation.id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0 as u32)
    .unwrap_or(0);

    if completed >= total_splits {
        // All splits used — end reservation
        let _ = crate::pod_reservation::end_reservation(&state, &reservation.id).await;
        return Json(json!({ "error": "All splits already used", "completed": completed, "total": total_splits }));
    }

    let current_split_number = completed + 1;
    let is_last_split = current_split_number >= total_splits;

    // Touch reservation
    crate::pod_reservation::touch_reservation(&state, &reservation.id).await;

    // Start billing session with split duration — NO wallet debit
    let billing_session_id = match billing::start_billing_session(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        Some(0), // custom_price_paise = 0 (no charge for continuation)
        Some(split_duration_minutes), // custom_duration_minutes
        None, // staff_id
        Some(total_splits), // split_count
        Some(split_duration_minutes), // split_duration_minutes
    )
    .await
    {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Link this billing session to the reservation
    let _ = sqlx::query(
        "UPDATE billing_sessions SET reservation_id = ?, wallet_debit_paise = 0 WHERE id = ?",
    )
    .bind(&reservation.id)
    .bind(&billing_session_id)
    .execute(&state.db)
    .await;

    // Update the timer's current_split_number
    {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.current_split_number = current_split_number;
        }
    }

    // If this is the last split, end the reservation so the final timer expiry
    // triggers SessionEnded (full end) instead of SubSessionEnded
    if is_last_split {
        let _ = crate::pod_reservation::end_reservation(&state, &reservation.id).await;
        tracing::info!("Last split ({}/{}) — reservation {} ended", current_split_number, total_splits, reservation.id);
    }

    // Launch the game — inject split_duration_minutes into launch args
    let game_launched = {
        let mut parsed: serde_json::Value = serde_json::from_str(&launch_args).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(split_duration_minutes);

        let sim: rc_common::types::SimType = match serde_json::from_value(serde_json::Value::String(sim_type.clone())) {
            Ok(st) => st,
            Err(_) => return Json(json!({ "error": format!("Unknown sim_type: {}", sim_type) })),
        };

        let cmd = rc_common::protocol::DashboardCommand::LaunchGame {
            pod_id: pod_id.clone(),
            sim_type: sim,
            launch_args: Some(parsed.to_string()),
        };
        game_launcher::handle_dashboard_command(&state, cmd).await.is_ok()
    };

    tracing::info!(
        "Continue split {}/{} on pod {} — session {}",
        current_split_number, total_splits, pod_id, billing_session_id
    );

    Json(json!({
        "ok": true,
        "billing_session_id": billing_session_id,
        "current_split_number": current_split_number,
        "total_splits": total_splits,
        "is_last_split": is_last_split,
        "game_launched": game_launched,
    }))
}

pub(crate) async fn stop_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    // FATM-02: Idempotency — if session already ended, return ok (not error)
    // Check for an existing ended_early or cancelled billing event for this session.
    let already_ended = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM billing_events \
         WHERE billing_session_id = ? AND event_type IN ('ended_early', 'cancelled', 'completed') \
         LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if already_ended.is_some() {
        return Json(json!({ "ok": true, "idempotent_replay": true }));
    }

    // Optional body may carry idempotency_key for future use — currently informational
    let _idempotency_key = body
        .as_ref()
        .and_then(|b| b.get("idempotency_key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // E2E-FIX: Check DB status — if waiting_for_game, use CancelledNoPlayable path
    // instead of EndedEarly (FSM rejects EndEarly from WaitingForGame).
    let db_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM billing_sessions WHERE id = ? AND ended_at IS NULL",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Phase 414 Plan 04 Task 2b: branch staff-triggered stop_billing on elapsed_seconds for waiting_for_game.
    // Two distinct paths:
    //   - elapsed_seconds == 0  → first-wait (customer never drove). Existing CancelledNoPlayable + full refund.
    //   - elapsed_seconds  > 0  → mid-stream WaitingForGame (customer drove, game stopped, staff/customer
    //                              ends from kiosk). STAFF-TRIGGERED EndEarly → EndedEarly + bill cumulative.
    // CRITICAL (B4): staff-stop uses BillingEvent::EndEarly → EndedEarly (NOT End → Completed).
    // The auto-end loop in tick_all_timers (Task 2a) is the only path that uses End → Completed.
    if db_status.as_deref() == Some("waiting_for_game") {
        // Read elapsed_seconds from the in-memory timer first (most accurate; BillingTimer.elapsed_seconds
        // ticks every second), falling back to the persisted DB column (synced every 60s by sync_timers_to_db).
        let elapsed_in_memory: Option<u32> = {
            let timers = state.billing.active_timers.read().await;
            timers
                .values()
                .find(|t| t.session_id == id)
                .map(|t| t.elapsed_seconds)
        };
        let elapsed_db: Option<i64> = sqlx::query_scalar(
            "SELECT elapsed_seconds FROM billing_sessions WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        let elapsed_seconds: u32 = elapsed_in_memory
            .or_else(|| elapsed_db.map(|e| e.max(0) as u32))
            .unwrap_or(0);

        if elapsed_seconds > 0 {
            // Phase 414: mid-stream WaitingForGame STAFF-TRIGGERED end → EndedEarly + bill cumulative.
            // Per CONTEXT.md edge case 4: explicit termination uses EndEarly → EndedEarly via FSM-04
            // (Plan 01 added the WaitingForGame + EndEarly transition).
            // end_billing_session_public maps BillingSessionStatus::EndedEarly → BillingEvent::EndEarly
            // via its match table (billing_session_end.rs:54).
            let ended = billing::end_billing_session_public(
                &state,
                &id,
                rc_common::types::BillingSessionStatus::EndedEarly,
                Some("Phase 414: stop_billing mid-stream from waiting_for_game"),
            ).await;

            if ended {
                tracing::info!(
                    session_id = %id,
                    elapsed_seconds = elapsed_seconds,
                    "Phase 414: stop_billing mid-stream → EndedEarly + bill cumulative"
                );
                crate::activity_log::log_pod_activity(
                    &state, "server", "billing", "Session Ended (Mid-Stream)",
                    &format!("session_id={} — staff/customer end from waiting_for_game (elapsed_seconds={})", id, elapsed_seconds),
                    "staff", Some(&id),
                );
                crate::billing_replay::insert_audit_log(
                    &state.db, &id, "unknown", "stop", "waiting_for_game", "ended_early",
                    None, "staff", &state.config.venue.venue_id,
                ).await;
                state.billing_nonce_store.remove(&id).await;
                return Json(json!({ "ok": true, "end_status": "ended_early", "billed_cumulative": true, "elapsed_seconds": elapsed_seconds }));
            } else {
                tracing::warn!(
                    session_id = %id,
                    "Phase 414: stop_billing mid-stream → end_billing_session_public returned false (already finalized?)"
                );
                return Json(json!({ "ok": false, "error": "Session not found or already ended" }));
            }
        }
        // elapsed_seconds == 0 — fall through to first-wait CancelledNoPlayable path below (existing behaviour).
        tracing::info!(
            session_id = %id,
            "Phase 414: stop_billing first-wait (elapsed_seconds=0) → CancelledNoPlayable + full refund (existing behaviour)"
        );
    }

    let (end_status, log_action, audit_to) = match db_status.as_deref() {
        Some("waiting_for_game") | Some("pending") => {
            // Cancel with full refund — game never became playable
            // Refund wallet first (same pattern as LBILL auto-cancel)
            let refund_info: Option<(String, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT driver_id, wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
            )
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((driver_id, Some(debit), wallet_owner)) = &refund_info
                && *debit > 0 {
                    let refund_target = wallet_owner.as_deref().unwrap_or(driver_id.as_str());
                    match crate::wallet::credit(
                        &state,
                        refund_target,
                        *debit,
                        "refund_session",
                        Some(id.as_str()),
                        Some("Staff-cancelled: game never reached playable state"),
                        None,
                    ).await {
                        Ok(_) => tracing::info!(
                            "BILLING: Refunded {}p for staff-cancelled waiting_for_game session {} (driver={})",
                            debit, id, driver_id
                        ),
                        Err(e) => tracing::error!(
                            "BILLING: Failed to refund {}p for session {}: {}",
                            debit, id, e
                        ),
                    }
                }

            // Update DB directly (session is not in active_timers)
            let _ = sqlx::query(
                "UPDATE billing_sessions SET status = 'cancelled_no_playable', ended_at = datetime('now') \
                 WHERE id = ? AND ended_at IS NULL",
            )
            .bind(&id)
            .execute(&state.db)
            .await;

            // Remove from in-memory waiting_for_game map
            if let Some((_, _driver_id, _, _wallet_owner)) = refund_info.as_ref().map(|r| (&id, &r.0, r.1, &r.2)) {
                // Find pod_id from DB
                let pod_id_opt: Option<String> = sqlx::query_scalar(
                    "SELECT pod_id FROM billing_sessions WHERE id = ?",
                )
                .bind(&id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
                if let Some(ref pod_id) = pod_id_opt {
                    let normalized = pod_id.replace('-', "_");
                    state.billing.waiting_for_game.write().await.remove(&normalized);
                    tracing::info!("Cleared waiting_for_game entry for pod {} (session {} staff-cancelled)", pod_id, id);
                }
            }

            state.billing_nonce_store.remove(&id).await;
            crate::activity_log::log_pod_activity(
                &state, "server", "billing", "Session Cancelled (No Playable)",
                &format!("session_id={} — staff-cancelled from waiting_for_game", id),
                "staff", Some(&id),
            );
            crate::billing_replay::insert_audit_log(
                &state.db, &id, "unknown", "stop", "waiting_for_game", "cancelled_no_playable", None, "staff", &state.config.venue.venue_id,
            ).await;
            return Json(json!({ "ok": true, "end_status": "cancelled_no_playable", "refunded": true }));
        }
        _ => (
            rc_common::types::BillingSessionStatus::EndedEarly,
            "Session Ended",
            "ended_early",
        ),
    };

    let found = billing::end_billing_session_public(&state, &id, end_status, None).await;
    if found {
        // Phase 307 AUDIT-03: Log billing session end for hash chain coverage
        crate::activity_log::log_pod_activity(
            &state,
            "server",
            "billing",
            log_action,
            &format!("session_id={}", id),
            "staff",
            Some(&id),
        );
        // Phase 283: Audit log + nonce cleanup
        crate::billing_replay::insert_audit_log(
            &state.db, &id, "unknown", "stop", "active", audit_to, None, "staff", &state.config.venue.venue_id,
        ).await;
        state.billing_nonce_store.remove(&id).await;
        Json(json!({ "ok": true }))
    } else {
        Json(json!({ "ok": false, "error": "Session not found or already ended" }))
    }
}

/// DEPLOY-02: POST /billing/{id}/agent-shutdown
/// Called by rc-agent during graceful shutdown when a billing session is active.
/// No JWT required — agent authenticates via RCAGENT_SERVICE_KEY header (same key as remote_ops).
/// Ends the session with EndedEarly so the partial refund logic fires.
/// Idempotent — safe to call multiple times (e.g. from sentinel recovery on next restart).
#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct AgentShutdownBody {
    pod_id: String,
    shutdown_reason: String,
}

pub(crate) async fn agent_shutdown_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AgentShutdownBody>,
) -> impl axum::response::IntoResponse {
    // Validate service key against configured sentry_service_key.
    // Agent sends key as: Authorization: Bearer <service_key>
    let expected_key = state.config.pods.sentry_service_key.clone().unwrap_or_default();
    if !expected_key.is_empty() {
        let provided_key = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .unwrap_or("");
        if provided_key != expected_key.as_str() {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "invalid service key" })),
            ).into_response();
        }
    }
    let result = billing::handle_agent_shutdown(&state, &id, &body.pod_id, &body.shutdown_reason).await;
    Json(result).into_response()
}

/// DEPLOY-04: GET /billing/pod/{pod_id}/interrupted
/// Called by rc-agent on startup to check for and auto-end interrupted sessions.
/// This endpoint also accepts unauthenticated requests from pods (service key in header).
pub(crate) async fn interrupted_sessions_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let result = billing::handle_interrupted_sessions_check(&state, &pod_id).await;
    Json(result)
}

pub(crate) async fn pause_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Phase 283: Audit log for pause
    crate::billing_replay::insert_audit_log(
        &state.db, &id, "unknown", "pause", "active", "paused", None, "staff", &state.config.venue.venue_id,
    ).await;

    let cmd = rc_common::protocol::DashboardCommand::PauseBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

pub(crate) async fn resume_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Phase 283: Audit log for resume
    crate::billing_replay::insert_audit_log(
        &state.db, &id, "unknown", "resume", "paused", "active", None, "staff", &state.config.venue.venue_id,
    ).await;

    // Check if this is a disconnect-paused session (needs special handling)
    let is_disconnect_paused = {
        let timers = state.billing.active_timers.read().await;
        timers.values().any(|t| t.session_id == id && t.status == rc_common::types::BillingSessionStatus::PausedDisconnect)
    };

    if is_disconnect_paused {
        match billing::resume_billing_from_disconnect(&state, &id).await {
            Ok(()) => Json(json!({ "ok": true })),
            Err(e) => Json(json!({ "error": e })),
        }
    } else {
        let cmd = rc_common::protocol::DashboardCommand::ResumeBilling {
            billing_session_id: id,
        };
        billing::handle_dashboard_command(&state, cmd).await;
        Json(json!({ "ok": true }))
    }
}

pub(crate) async fn extend_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let additional_seconds = body
        .get("additional_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(600) as u32;

    // BILL-04: Validate additional_seconds is a multiple of 60 and within bounds (60..=3600)
    if !(60..=3600).contains(&additional_seconds) {
        return Json(json!({
            "ok": false,
            "error": format!("additional_seconds must be between 60 and 3600, got {}", additional_seconds)
        }));
    }
    if !additional_seconds.is_multiple_of(60) {
        return Json(json!({
            "ok": false,
            "error": format!("additional_seconds must be a multiple of 60, got {}", additional_seconds)
        }));
    }

    // FATM-07: Call extend_billing_session directly (not via DashboardCommand) to propagate errors
    match billing::extend_billing_session(&state, &id, additional_seconds).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}
