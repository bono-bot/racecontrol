//! Billing game status handling — game launch ↔ billing state transitions.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Contains handle_game_status_update (the main game→billing bridge), launch timeout
//! checks, deferred billing start, and multiplayer billing timeout.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;

use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::CoreMessage;
use rc_common::types::BillingSessionStatus;

use crate::billing::{
    BillingManager, BillingStartData, MultiplayerBillingWait,
    PauseReason, WaitingForGameEntry,
    finalize_billing_start, start_billing_session,
    pause_multiplayer_group, resume_multiplayer_group,
};
use crate::billing_session_lifecycle::end_billing_session;
use crate::state::AppState;

// ─── Game Status Handling ───────────────────────────────────────────────────

/// Check for pods that have been in WaitingForGame for more than `timeout_secs` seconds.
/// Returns list of (pod_id, attempt) for pods that have timed out.
/// This variant operates directly on a BillingManager (for testing without AppState).
/// Pass timeout_secs explicitly to allow test overrides (default 180s in production).
pub async fn check_launch_timeouts_from_manager(mgr: &BillingManager, timeout_secs: u64) -> Vec<(String, u8)> {
    let mut timed_out = Vec::new();
    let waiting = mgr.waiting_for_game.read().await;
    for (pod_id, entry) in waiting.iter() {
        if entry.waiting_since.elapsed() > std::time::Duration::from_secs(timeout_secs) {
            timed_out.push((pod_id.clone(), entry.attempt));
        }
    }
    timed_out
}

/// Check for pods that have been in WaitingForGame beyond the configured launch timeout.
/// Uses BillingConfig.launch_timeout_per_attempt_secs from AppState config (BILL-12).
pub async fn check_launch_timeouts(state: &Arc<AppState>) -> Vec<(String, u8)> {
    check_launch_timeouts_from_manager(&state.billing, state.config.billing.launch_timeout_per_attempt_secs).await
}

/// Defer billing start until AC reaches STATUS=LIVE.
/// Called from auth instead of start_billing_session.
/// For multiplayer pods, pass `group_session_id: Some(id)` to coordinate billing
/// across all group members. Single-player pods pass `None`.
pub async fn defer_billing_start(
    state: &Arc<AppState>,
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    staff_id: Option<String>,
    split_count: Option<u32>,
    split_duration_minutes: Option<u32>,
    group_session_id: Option<String>,
) -> Result<(), String> {
    // Normalize pod_id to canonical form (pod_N) at entry
    let pod_id = normalize_pod_id(&pod_id).unwrap_or(pod_id);
    let entry = WaitingForGameEntry {
        pod_id: pod_id.clone(),
        driver_id,
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        staff_id,
        split_count,
        split_duration_minutes,
        waiting_since: std::time::Instant::now(),
        attempt: 1,
        group_session_id: group_session_id.clone(),
        sim_type: None,
        launch_args: None,
        pre_committed: None,
    };
    if group_session_id.is_some() {
        tracing::info!("Billing deferred to WaitingForGame for pod {} (multiplayer group)", pod_id);
    } else {
        tracing::info!("Billing deferred to WaitingForGame for pod {}", pod_id);
    }
    state.billing.waiting_for_game.write().await.insert(pod_id, entry);
    Ok(())
}

/// BILL-13: Defer billing timer activation for kiosk staff path.
/// The DB record + wallet debit are ALREADY committed (FATM-01 atomic tx).
/// This puts the session into waiting_for_game with the pre-committed data.
/// When AcStatus::Live arrives, finalize_billing_start() activates the timer
/// without creating a duplicate DB record.
pub async fn defer_billing_with_precommitted_session(
    state: &Arc<AppState>,
    pod_id: String,
    data: BillingStartData,
) {
    let pod_id_normalized = normalize_pod_id(&pod_id).unwrap_or(pod_id);
    let entry = WaitingForGameEntry {
        pod_id: pod_id_normalized.clone(),
        driver_id: data.driver_id.clone(),
        pricing_tier_id: String::new(), // already committed in DB
        custom_price_paise: None,
        custom_duration_minutes: None,
        staff_id: None,
        split_count: Some(data.split_count),
        split_duration_minutes: data.split_duration_minutes,
        waiting_since: std::time::Instant::now(),
        attempt: 1,
        group_session_id: None,
        sim_type: None,
        launch_args: None,
        pre_committed: Some(data),
    };
    tracing::info!(
        "BILL-13: Billing deferred to WaitingForGame for pod {} (kiosk staff path, session pre-committed)",
        pod_id_normalized
    );
    state.billing.waiting_for_game.write().await.insert(pod_id_normalized, entry);
}

/// Handle game status updates from the agent.
/// Dispatches to billing start/pause/resume/end based on AcStatus.
/// For multiplayer pods (group_session_id is Some), billing is coordinated:
/// billing starts for ALL group members only after every participant reaches LIVE.
pub async fn handle_game_status_update(
    state: &Arc<AppState>,
    pod_id: &str,
    ac_status: rc_common::types::AcStatus,
    sim_type: Option<rc_common::types::SimType>,
    _cmd_tx: &tokio::sync::mpsc::Sender<CoreMessage>,
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
                if let Some(ref group_id) = entry.group_session_id {
                    // ── Multiplayer: coordinate billing across group ──────────
                    let group_id = group_id.clone();

                    // Check if group exists (read lock, cheap)
                    let needs_init = !state.billing.multiplayer_waiting.read().await.contains_key(&group_id);

                    // If first pod for this group, query DB WITHOUT holding the lock
                    let expected_pods_from_db: Option<Vec<String>> = if needs_init {
                        // BILL-10: Reject billing on DB failure (no silent unwrap_or_default)
                        match sqlx::query_scalar(
                            "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND status = 'validated' AND pod_id IS NOT NULL",
                        )
                        .bind(&group_id)
                        .fetch_all(&state.db)
                        .await
                        {
                            Ok(ids) => Some(ids),
                            Err(e) => {
                                tracing::error!(
                                    "group_session_members query failed for group {} — billing REJECTED: {}",
                                    group_id, e
                                );
                                state.billing.waiting_for_game.write().await.insert(pod_id.to_string(), entry);
                                return;
                            }
                        }
                    } else {
                        None
                    };

                    // Now acquire write lock (DB query already done)
                    let mut mp = state.billing.multiplayer_waiting.write().await;

                    if !mp.contains_key(&group_id) {
                        let pod_ids = expected_pods_from_db.unwrap_or_default();

                        let expected: HashSet<String> = if pod_ids.is_empty() {
                            // Fallback: if no DB results, just expect this pod
                            let mut s = HashSet::new();
                            s.insert(pod_id.to_string());
                            s
                        } else {
                            pod_ids.into_iter().collect()
                        };

                        mp.insert(group_id.clone(), MultiplayerBillingWait {
                            group_session_id: group_id.clone(),
                            expected_pods: expected,
                            live_pods: HashSet::new(),
                            waiting_entries: HashMap::new(),
                            timeout_spawned: false,
                        });
                    }

                    let Some(wait) = mp.get_mut(&group_id) else {
                        tracing::error!("multiplayer group_id {} missing from map after insert", group_id);
                        return;
                    };
                    wait.live_pods.insert(pod_id.to_string());
                    wait.waiting_entries.insert(pod_id.to_string(), entry);

                    // Spawn configurable timeout (once per group) — BILL-11
                    if !wait.timeout_spawned {
                        wait.timeout_spawned = true;
                        let state_clone = state.clone();
                        let group_id_clone = group_id.clone();
                        let mp_timeout = state.config.billing.multiplayer_wait_timeout_secs;
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(mp_timeout)).await;
                            multiplayer_billing_timeout(&state_clone, &group_id_clone).await;
                        });
                    }

                    if wait.live_pods.len() >= wait.expected_pods.len() {
                        // All pods are live — start billing for all
                        let entries: Vec<WaitingForGameEntry> = wait.waiting_entries.drain().map(|(_, e)| e).collect();
                        let gid = group_id.clone();
                        mp.remove(&group_id);
                        drop(mp); // Release lock before async DB calls

                        tracing::info!("All {} pods live in group {} — starting billing for all", entries.len(), gid);
                        for e in entries {
                            let delta_ms = e.waiting_since.elapsed().as_millis() as i64;
                            let sim_str = e.sim_type.as_ref().map(|s| format!("{}", s));
                            let ep_id = e.pod_id.clone();
                            match start_billing_session(
                                state,
                                e.pod_id.clone(),
                                e.driver_id,
                                e.pricing_tier_id,
                                e.custom_price_paise,
                                e.custom_duration_minutes,
                                e.staff_id,
                                e.split_count,
                                e.split_duration_minutes,
                            ).await {
                                Ok(session_id) => {
                                    tracing::info!("Multiplayer billing started for pod {} (session {})", e.pod_id, session_id);
                                    // Record billing accuracy event (METRICS-03)
                                    // BILL-09: Single Utc::now() call for both playable_signal_at and billing_start_at
                                    let now = Utc::now();
                                    let billing_start_at = now
                                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                        .to_string();
                                    let ba_event = crate::metrics::BillingAccuracyEvent {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        session_id: session_id.clone(),
                                        pod_id: ep_id.clone(),
                                        sim_type: sim_str,
                                        event_type: "start".to_string(),
                                        launch_command_at: None,
                                        playable_signal_at: Some(billing_start_at.clone()),
                                        billing_start_at: Some(billing_start_at),
                                        delta_ms: Some(delta_ms),
                                        details: Some("multiplayer".to_string()),
                                    };
                                    crate::metrics::record_billing_accuracy_event(&state.db, &ba_event, &state.config.venue.venue_id).await;
                                }
                                Err(err) => {
                                    tracing::error!("Failed to start multiplayer billing for pod {}: {}", e.pod_id, err);
                                }
                            }
                        }
                    } else {
                        let remaining = wait.expected_pods.len() - wait.live_pods.len();
                        tracing::info!(
                            "Waiting for {} more player(s) in group {} ({}/{} live)",
                            remaining, group_id, wait.live_pods.len(), wait.expected_pods.len()
                        );
                    }
                } else if let Some(pre_data) = entry.pre_committed {
                    // ── BILL-13: Kiosk staff path — session already committed in DB ──
                    // Wallet debit + DB INSERT already done in atomic tx (FATM-01).
                    // Just activate the in-memory timer and update DB started_at to NOW.
                    let delta_ms = entry.waiting_since.elapsed().as_millis() as i64;
                    let sim_str = entry.sim_type.as_ref().map(|s| format!("{}", s));
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
                } else {
                    // ── Single-player PIN auth path: start billing (existing behavior) ──
                    let delta_ms = entry.waiting_since.elapsed().as_millis() as i64;
                    let sim_str = entry.sim_type.as_ref().map(|s| format!("{}", s));
                    match start_billing_session(
                        state,
                        entry.pod_id,
                        entry.driver_id,
                        entry.pricing_tier_id,
                        entry.custom_price_paise,
                        entry.custom_duration_minutes,
                        entry.staff_id,
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
            } else {
                // No waiting entry -- check if timer exists and is PausedGamePause (resume)
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
                                tracing::info!("Billing resumed on LIVE for pod {} (was PausedGamePause)", pod_id);
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
                // Single-player path: end billing session normally
                let session_id = {
                    let timers = state.billing.active_timers.read().await;
                    timers.get(pod_id).map(|t| t.session_id.clone())
                };
                if let Some(session_id) = session_id {
                    tracing::info!("Game exited (STATUS=Off) for pod {}, ending billing session {}", pod_id, session_id);
                    end_billing_session(state, &session_id, BillingSessionStatus::EndedEarly).await;
                }
            }
            // Also remove from waiting_for_game if present (game crashed during loading)
            // BILL-06: Insert cancelled_no_playable record — customer charged nothing
            let crashed_entry = state.billing.waiting_for_game.write().await.remove(pod_id);
            if let Some(crashed_entry) = crashed_entry {
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
                    if let Some((debit_paise, wallet_owner)) = debit_row {
                        if debit_paise > 0 {
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
            // Launch failed (timeout or process died) — clean up waiting state, no charge
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
    }
}

// ─── Multiplayer Billing Timeout ─────────────────────────────────────────────

/// Called after 60 seconds to evict non-connecting pods from a multiplayer group.
/// If some pods have connected (LIVE), billing starts for those.
/// Pods that never reached LIVE do not get billing started.
async fn multiplayer_billing_timeout(state: &Arc<AppState>, group_session_id: &str) {
    let mut mp = state.billing.multiplayer_waiting.write().await;

    let wait = match mp.get_mut(group_session_id) {
        Some(w) => w,
        None => {
            // Entry already consumed (all pods connected in time) -- no-op
            return;
        }
    };

    if wait.live_pods.len() >= wait.expected_pods.len() {
        // All connected in time -- entry should have been consumed already
        // but clean up just in case
        mp.remove(group_session_id);
        return;
    }

    // Some pods failed to connect within 60s
    let non_connected: Vec<String> = wait
        .expected_pods
        .iter()
        .filter(|p| !wait.live_pods.contains(*p))
        .cloned()
        .collect();

    tracing::warn!(
        "Multiplayer billing timeout: {} pod(s) failed to connect for group {}: {:?}",
        non_connected.len(),
        group_session_id,
        non_connected
    );

    if wait.live_pods.is_empty() {
        // No pods connected at all -- just clean up
        tracing::warn!("No pods connected in group {} -- cleaning up", group_session_id);
        mp.remove(group_session_id);
        return;
    }

    // Collect entries for live pods and start billing
    let entries: Vec<WaitingForGameEntry> = wait
        .waiting_entries
        .drain()
        .filter(|(pod_id, _)| wait.live_pods.contains(pod_id))
        .map(|(_, e)| e)
        .collect();

    let gid = group_session_id.to_string();
    mp.remove(group_session_id);
    drop(mp); // Release lock before async DB calls

    tracing::info!(
        "Starting billing for {} live pod(s) in group {} after timeout eviction",
        entries.len(),
        gid
    );
    for e in entries {
        match start_billing_session(
            state,
            e.pod_id.clone(),
            e.driver_id,
            e.pricing_tier_id,
            e.custom_price_paise,
            e.custom_duration_minutes,
            e.staff_id,
            e.split_count,
            e.split_duration_minutes,
        )
        .await
        {
            Ok(session_id) => {
                tracing::info!(
                    "Multiplayer billing started for pod {} after timeout (session {})",
                    e.pod_id,
                    session_id
                );
            }
            Err(err) => {
                tracing::error!(
                    "Failed to start multiplayer billing for pod {} after timeout: {}",
                    e.pod_id,
                    err
                );
            }
        }
    }
}


