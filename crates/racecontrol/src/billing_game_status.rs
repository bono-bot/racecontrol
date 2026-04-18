//! Billing game status handling — game launch <-> billing state transitions.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Contains handle_game_status_update (the main game->billing bridge).
//! Deferred billing and multiplayer coordination live in submodules.

#[path = "billing_game_status_defer.rs"]
mod defer;

#[path = "billing_game_status_mp.rs"]
mod mp;

pub use defer::*;

use std::sync::Arc;

use chrono::Utc;

use rc_common::pod_id::normalize_pod_id;
use rc_common::types::BillingSessionStatus;

use crate::billing::{
    PauseReason, WaitingForGameEntry,
    finalize_billing_start, start_billing_session,
    pause_multiplayer_group, resume_multiplayer_group,
};
use crate::state::AppState;

// ─── Game Status Handling ───────────────────────────────────────────────────

/// Handle game status updates from the agent.
/// Dispatches to billing start/pause/resume/end based on AcStatus.
/// For multiplayer pods (group_session_id is Some), billing is coordinated:
/// billing starts for ALL group members only after every participant reaches LIVE.
pub async fn handle_game_status_update(
    state: &Arc<AppState>,
    pod_id: &str,
    ac_status: rc_common::types::AcStatus,
    sim_type: Option<rc_common::types::SimType>,
    _cmd_tx: &tokio::sync::mpsc::Sender<rc_common::protocol::CoreMessage>,
) {
    // Normalize pod_id to canonical form (pod_N) at entry
    let pod_id_normalized = normalize_pod_id(pod_id).unwrap_or_else(|_| pod_id.to_string());
    let pod_id = pod_id_normalized.as_str();
    use rc_common::types::AcStatus;
    match ac_status {
        AcStatus::Live => {
            // Check if this pod is in waiting_for_game -- if so, start billing
            let entry = state.billing.waiting_for_game.write().await.remove(pod_id);
            if let Some(mut entry) = entry {
                // Update sim_type from the GameStatusUpdate message
                if sim_type.is_some() {
                    entry.sim_type = sim_type;
                }
                let entry = entry;
                if entry.group_session_id.is_some() {
                    // ── Multiplayer: coordinate billing across group ──────────
                    let group_id = entry.group_session_id.clone().unwrap();
                    mp::handle_multiplayer_live(state, pod_id, entry, group_id).await;
                } else if entry.pre_committed.is_some() {
                    // ── BILL-13: Kiosk staff path — session already committed in DB ──
                    // Take pre_committed out; pass remaining entry fields via separate args
                    let delta_ms = entry.waiting_since.elapsed().as_millis() as i64;
                    let sim_str = entry.sim_type.as_ref().map(|s| format!("{}", s));
                    let pre_data = entry.pre_committed.unwrap();
                    handle_precommitted_live(state, pod_id, delta_ms, sim_str, pre_data).await;
                } else {
                    // ── Single-player PIN auth path: start billing (existing behavior) ──
                    handle_single_player_live(state, pod_id, &entry).await;
                }
            } else {
                // No waiting entry -- check if timer exists and is PausedGamePause (resume)
                handle_live_resume(state, pod_id).await;
            }
        }
        AcStatus::Pause => {
            let mut timers = state.billing.active_timers.write().await;
            if let Some(timer) = timers.get_mut(pod_id) {
                match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::Pause) {
                    Ok(new_status) => {
                        timer.status = new_status;
                        timer.pause_seconds = 0;
                        timer.pause_count += 1;
                        // BILL-06: Manual ESC pause — not a crash recovery
                        timer.pause_reason = PauseReason::GamePause;
                        tracing::info!("Billing paused (game pause) for pod {}", pod_id);
                    }
                    Err(e) => {
                        tracing::warn!("BILLING: {}", e);
                    }
                }
            }
            // If no active timer, Pause is a no-op
        }
        AcStatus::Off => {
            handle_game_off(state, pod_id).await;
        }
        AcStatus::Replay => {
            // Replay mode -- treat same as Pause for billing purposes
            let mut timers = state.billing.active_timers.write().await;
            if let Some(timer) = timers.get_mut(pod_id) {
                match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::CrashPause) {
                    Ok(new_status) => {
                        timer.status = new_status;
                        timer.pause_seconds = 0;
                        timer.pause_count += 1;
                        tracing::info!("Billing paused (replay) for pod {}", pod_id);
                    }
                    Err(e) => {
                        tracing::warn!("BILLING: {}", e);
                    }
                }
            }
        }
        AcStatus::Error => {
            handle_game_error(state, pod_id).await;
        }
    }
}

// ─── AcStatus::Live Helpers ─────────────────────────────────────────────────

/// BILL-13: Kiosk staff path — session already committed in DB.
/// Wallet debit + DB INSERT already done in atomic tx (FATM-01).
/// Just activate the in-memory timer and update DB started_at to NOW.
async fn handle_precommitted_live(
    state: &Arc<AppState>,
    pod_id: &str,
    delta_ms: i64,
    sim_str: Option<String>,
    pre_data: crate::billing::BillingStartData,
) {
    let session_id = pre_data.session_id.clone();
    let now = Utc::now();

    // Update DB started_at to game-live time (not staff-click time)
    let _ = sqlx::query(
        "UPDATE billing_sessions SET started_at = ?, status = 'active' WHERE id = ?",
    )
    .bind(now.to_rfc3339())
    .bind(&session_id)
    .execute(&state.db)
    .await
    .map_err(|e| tracing::error!("BILL-13: Failed to update started_at for session {}: {}", session_id, e));

    // Log billing_timer_started event
    let billing_start_iso = now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id) VALUES (?, ?, 'billing_timer_started', 0, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&session_id)
    .bind(serde_json::json!({
        "billing_timer_started": true,
        "started_at": billing_start_iso,
        "pod_id": pod_id,
        "trigger": "game_live_signal",
        "deferred_from_kiosk": true,
        "wait_ms": delta_ms,
    }).to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    // Activate in-memory timer with started_at = NOW (game-live time)
    let mut activated_data = pre_data;
    activated_data.started_at = now;
    finalize_billing_start(state, activated_data).await;

    tracing::info!(
        "BILL-13: Pre-committed billing activated on LIVE for pod {} (session {}, waited {}ms)",
        pod_id, session_id, delta_ms
    );

    // Record billing accuracy event (METRICS-03)
    let ba_event = crate::metrics::BillingAccuracyEvent {
        id: uuid::Uuid::new_v4().to_string(),
        session_id,
        pod_id: pod_id.to_string(),
        sim_type: sim_str,
        event_type: "start".to_string(),
        launch_command_at: None,
        playable_signal_at: Some(billing_start_iso.clone()),
        billing_start_at: Some(billing_start_iso),
        delta_ms: Some(delta_ms),
        details: Some("kiosk_deferred".to_string()),
    };
    crate::metrics::record_billing_accuracy_event(&state.db, &ba_event, &state.config.venue.venue_id).await;
}

/// Single-player PIN auth path: start billing session on LIVE.
async fn handle_single_player_live(
    state: &Arc<AppState>,
    pod_id: &str,
    entry: &WaitingForGameEntry,
) {
    let delta_ms = entry.waiting_since.elapsed().as_millis() as i64;
    let sim_str = entry.sim_type.as_ref().map(|s| format!("{}", s));
    match start_billing_session(
        state,
        entry.pod_id.clone(),
        entry.driver_id.clone(),
        entry.pricing_tier_id.clone(),
        entry.custom_price_paise,
        entry.custom_duration_minutes,
        entry.staff_id.clone(),
        entry.split_count,
        entry.split_duration_minutes,
    ).await {
        Ok(session_id) => {
            tracing::info!("Billing started on LIVE for pod {} (session {})", pod_id, session_id);
            // Record billing accuracy event (METRICS-03)
            // BILL-09: Single Utc::now() call for both playable_signal_at and billing_start_at
            let now = Utc::now();
            let billing_start_at = now
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let ba_event = crate::metrics::BillingAccuracyEvent {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.clone(),
                pod_id: pod_id.to_string(),
                sim_type: sim_str,
                event_type: "start".to_string(),
                launch_command_at: None,
                playable_signal_at: Some(billing_start_at.clone()),
                billing_start_at: Some(billing_start_at),
                delta_ms: Some(delta_ms),
                details: None,
            };
            crate::metrics::record_billing_accuracy_event(&state.db, &ba_event, &state.config.venue.venue_id).await;
        }
        Err(e) => {
            tracing::error!("Failed to start billing on LIVE for pod {}: {}", pod_id, e);
        }
    }
}

/// Handle AcStatus::Live when no waiting entry exists — resume paused billing.
async fn handle_live_resume(state: &Arc<AppState>, pod_id: &str) {
    let (was_crash_recovery, had_timer) = {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id) {
            let was_crash = timer.pause_reason == PauseReason::CrashRecovery;
            match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::Resume) {
                Ok(new_status) => {
                    timer.status = new_status;
                    timer.pause_seconds = 0;
                    // BILL-06: Clear pause reason on resume
                    timer.pause_reason = PauseReason::None;
                    // Phase 414: Reset between-games idle counter on resume to Active.
                    // Covers the WaitingForGame → Active path (GameLive event fires on next game launch).
                    // between_games_idle_seconds and idle_warning_sent are reset so the next
                    // game-stop starts a fresh 15-min window (D-IDLE-AUTOEND).
                    timer.between_games_idle_seconds = 0;
                    timer.idle_warning_sent = false;
                    tracing::info!(
                        pod_id = %pod_id,
                        session_id = %timer.session_id,
                        "Phase 414: Billing resumed on LIVE, idle counter reset (WaitingForGame→Active or PausedGamePause→Active)"
                    );
                    (was_crash, true)
                }
                Err(e) => {
                    // No-op if already Active (idempotent) or other invalid state
                    tracing::debug!("BILLING: resume on LIVE no-op for pod {}: {}", pod_id, e);
                    (false, true)
                }
            }
        } else {
            (false, false)
        }
    }; // timers lock dropped

    // BILL-07: If this was a crash-recovery pause and the pod is in a multiplayer
    // group, resume billing for ALL group members (not just this pod).
    if had_timer && was_crash_recovery {
        let group_session_id: Option<String> = sqlx::query_scalar(
            "SELECT gs.id
             FROM group_session_members gsm
             JOIN group_sessions gs ON gs.id = gsm.group_session_id
             WHERE gsm.pod_id = ? AND gs.status IN ('active', 'forming')
             ORDER BY gs.created_at DESC LIMIT 1",
        )
        .bind(pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some(ref gid) = group_session_id {
            tracing::info!(
                "BILL-07: Pod {} recovered in multiplayer group {} — resuming all group members",
                pod_id, gid
            );
            resume_multiplayer_group(state, gid).await;
        }
    }
}

// ─── AcStatus::Off Handler ──────────────────────────────────────────────────

/// Handle game exited (AcStatus::Off).
/// For multiplayer pods, pauses the whole group. For single-player, ends the session.
/// Also cleans up waiting_for_game and multiplayer_waiting state.
async fn handle_game_off(state: &Arc<AppState>, pod_id: &str) {
    // Game exited -- check if this pod is in an active multiplayer group first.
    // BILL-07: If the pod is part of a multiplayer group, pause the WHOLE group
    // (crash recovery) rather than ending this pod's session immediately.
    // The group resumes when the crashed pod's game recovers (AcStatus::Live).
    let group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT gs.id
         FROM group_session_members gsm
         JOIN group_sessions gs ON gs.id = gsm.group_session_id
         WHERE gsm.pod_id = ? AND gs.status IN ('active', 'forming')
         ORDER BY gs.created_at DESC LIMIT 1",
    )
    .bind(pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(ref gid) = group_session_id {
        // BILL-07: Multiplayer crash — pause entire group, not just this pod
        tracing::warn!(
            "BILL-07: Pod {} crashed in multiplayer group {} — pausing all group members",
            pod_id, gid
        );
        pause_multiplayer_group(state, gid, "crash_recovery").await;
    } else {
        // Single-player path: Phase 414 — fire GameStopped FSM event to pause meter.
        // Game stop no longer ends billing; meter moves to WaitingForGame (between-games state).
        // The 15-min idle counter starts in tick_all_timers (Plan 04 Task 2a).
        // D-FSM-01: Active + GameStopped → WaitingForGame (Plan 01 added this transition).
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(pod_id) {
            match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::GameStopped) {
                Ok(new_status) => {
                    timer.status = new_status; // → WaitingForGame
                    timer.between_games_idle_seconds = 0;
                    timer.idle_warning_sent = false;
                    tracing::info!(
                        pod_id = %pod_id,
                        session_id = %timer.session_id,
                        "Phase 414: Game stopped on single-player pod, billing paused (mid-stream WaitingForGame)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        pod_id = %pod_id,
                        session_id = %timer.session_id,
                        err = %e,
                        status = ?timer.status,
                        "Phase 414: GameStopped FSM transition rejected (status not Active); skipping pause"
                    );
                }
            }
        }
        drop(timers);
    }
    // Also remove from waiting_for_game if present (game crashed during loading)
    // BILL-06: Insert cancelled_no_playable record — customer charged nothing
    let crashed_entry = state.billing.waiting_for_game.write().await.remove(pod_id);
    if let Some(crashed_entry) = crashed_entry {
        handle_crashed_waiting_entry(state, pod_id, &crashed_entry).await;
    }

    // Clean up from multiplayer_waiting if pod was still waiting
    {
        let mut mp = state.billing.multiplayer_waiting.write().await;
        let mut groups_to_remove = Vec::new();
        for (gid, wait) in mp.iter_mut() {
            if wait.waiting_entries.remove(pod_id).is_some() {
                wait.live_pods.remove(pod_id);
                wait.expected_pods.remove(pod_id);
                tracing::info!("Pod {} disconnected from multiplayer group {} during wait", pod_id, gid);
                // If no more expected pods, clean up
                if wait.expected_pods.is_empty() {
                    groups_to_remove.push(gid.clone());
                }
            }
        }
        for gid in groups_to_remove {
            mp.remove(&gid);
        }
    }
}

/// Handle a crashed waiting entry — create cancelled_no_playable record and refund if needed.
async fn handle_crashed_waiting_entry(
    state: &Arc<AppState>,
    pod_id: &str,
    crashed_entry: &WaitingForGameEntry,
) {
    if let Some(pre_data) = &crashed_entry.pre_committed {
        // BILL-13: Kiosk path — DB record already exists, UPDATE it + refund wallet
        let pre_session_id = pre_data.session_id.clone();
        let pre_driver_id = pre_data.driver_id.clone();
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'cancelled_no_playable', ended_at = datetime('now'), driving_seconds = 0, total_paused_seconds = 0 WHERE id = ?",
        )
        .bind(&pre_session_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::error!("BILL-13: Failed to cancel pre-committed session {}: {}", pre_session_id, e));

        // Refund the wallet debit — game never reached playable
        let debit_row: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
        )
        .bind(&pre_session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((debit_paise, wallet_owner)) = debit_row
            && debit_paise > 0 {
                let refund_target = wallet_owner.as_deref().unwrap_or(&pre_driver_id);
                match crate::wallet::credit(
                    state,
                    refund_target,
                    debit_paise,
                    "refund_session",
                    Some(&pre_session_id),
                    Some("Auto-refund: game never reached playable state"),
                    None, // staff_id — system-initiated refund
                ).await {
                    Ok(_) => tracing::info!(
                        "BILL-13: Refunded {}p for cancelled_no_playable session {} (pod={}, driver={})",
                        debit_paise, pre_session_id, pod_id, pre_driver_id
                    ),
                    Err(e) => tracing::error!(
                        "BILL-13: Failed to refund {}p for session {}: {}",
                        debit_paise, pre_session_id, e
                    ),
                }
            }
        tracing::warn!(
            "BILL-13: Pre-committed session cancelled_no_playable: pod={} session={} (game died before PlayableSignal)",
            pod_id, pre_session_id
        );
    } else {
        // PIN auth path — no DB record exists yet, create cancelled_no_playable record
        let session_id = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO billing_sessions (id, pod_id, driver_id, pricing_tier_id, allocated_seconds, status, created_at, ended_at, driving_seconds, total_paused_seconds, venue_id)
             VALUES (?, ?, ?, ?, 0, 'cancelled_no_playable', datetime('now'), datetime('now'), 0, 0, ?)",
        )
        .bind(&session_id)
        .bind(pod_id)
        .bind(&crashed_entry.driver_id)
        .bind(&crashed_entry.pricing_tier_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| tracing::error!("Failed to insert cancelled_no_playable record (game crash): {}", e));
        tracing::warn!(
            "Session cancelled_no_playable: pod={} driver={} (game died before PlayableSignal)",
            pod_id, crashed_entry.driver_id
        );
    }
}

// ─── AcStatus::Error Handler ────────────────────────────────────────────────

/// Handle launch failure (AcStatus::Error) — clean up waiting state, no charge.
async fn handle_game_error(state: &Arc<AppState>, pod_id: &str) {
    tracing::warn!("Pod {} launch FAILED (AcStatus::Error) — cleaning up, no charge", pod_id);
    // Remove from waiting_for_game if still pending
    let removed = state.billing.waiting_for_game.write().await.remove(pod_id);
    if let Some(entry) = removed {
        tracing::info!("Cleaned up waiting_for_game for pod {} (was waiting {}ms)",
            pod_id, entry.waiting_since.elapsed().as_millis());
        // If pre-committed (kiosk staff path), refund the wallet debit
        if let Some(ref pre_data) = entry.pre_committed {
            tracing::warn!("Pod {} had pre-committed session {} — needs refund", pod_id, pre_data.session_id);
            // Mark session as cancelled in DB
            let _ = sqlx::query(
                "UPDATE billing_sessions SET status = 'cancelled', ended_at = datetime('now') WHERE id = ? AND status = 'pending'",
            )
            .bind(&pre_data.session_id)
            .execute(&state.db)
            .await
            .map_err(|e| tracing::error!("Failed to cancel pre-committed session {}: {}", pre_data.session_id, e));
        }
    }
    // Remove GameTracker so the pod isn't stuck in "Launching"
    {
        let mut games = state.game_launcher.active_games.write().await;
        if games.remove(pod_id).is_some() {
            tracing::info!("GameTracker removed for pod {} after launch error", pod_id);
        }
    }
}
