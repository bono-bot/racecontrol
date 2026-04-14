//! Billing timer tick loop — the 1-second heartbeat that drives all billing state.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Contains tick_all_timers (1,166 lines), sync_timers_to_db, persist_timer_state.
//! Handles: time tracking, disconnect detection, pause timeouts, per-minute debits,
//! grace windows, stale session cleanup, launch timeouts, and agent tick broadcasts.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{BillingSessionStatus, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::billing::{PauseReason, check_launch_timeouts, end_billing_session};
use crate::billing_multiplayer::check_and_stop_multiplayer_server;
use crate::billing_pricing::{compute_refund, compute_per_minute_refund};
use crate::state::AppState;

/// GAP-3 fix: Monotonic billing tick sequence counter.
/// Kiosk/agent can ignore ticks with seq < last seen to prevent stale state after WS reconnect.
static BILLING_TICK_SEQ: AtomicU64 = AtomicU64::new(0);

// ─── Tick Loop ──────────────────────────────────────────────────────────────

/// Called every 1 second to tick all active billing timers
pub async fn tick_all_timers(state: &Arc<AppState>) {
    // FIX: Use try_write to prevent deadlock — if lock is contended, skip this tick.
    // The billing tick runs every 1s, so skipping one cycle is harmless.
    // Root cause: active_timers.write() can block for seconds when
    // handle_game_status_update holds it during DB operations.
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = match state.billing.active_timers.try_write() {
        Ok(t) => t,
        Err(_) => {
            drop(rate_tiers);
            // Lock contended — skip this tick cycle
            return;
        }
    };
    let mut events_to_broadcast = Vec::new();
    let mut expired_sessions = Vec::new();
    let mut warnings = Vec::new();
    let mut agent_ticks: Vec<(String, u32, u32, String, Option<u32>, Option<i64>, Option<i64>, Option<bool>, Option<u32>, Option<String>)> = Vec::new();
    let mut pause_timeout_end: Vec<(String, String, u32, String)> = Vec::new();
    // Act 2: Per-minute debits collected inside lock, processed after lock release
    let mut per_minute_debits: Vec<(String, String, String, u32)> = Vec::new(); // (session_id, pod_id, wallet_owner_id, rate_paise)
    let mut new_pauses: Vec<(String, String, u32)> = Vec::new(); // pod_id, session_id, pause_count
    let mut sessions_to_auto_end: Vec<(String, String, String)> = Vec::new(); // pod_id, session_id, reason
    // GLD-C-04: Grace window DB writes (session_id, grace_until RFC3339)
    let mut grace_window_sets: Vec<(String, String)> = Vec::new();
    // GLD-C-04: Expired grace windows to finalize (pod_id, session_id, end_status)
    // P0-2 fix: pod_id included so we can remove the timer from active_timers BEFORE
    // dropping the write lock, preventing the double-finalize race where the next tick
    // sees the timer with cleared grace fields and treats it as a normal active timer.
    let mut deferred_finalizes: Vec<(String, String, BillingSessionStatus)> = Vec::new();

    // Read pod statuses for offline detection
    let pods = state.pods.read().await;

    let now_for_grace = chrono::Utc::now();
    for (pod_id, timer) in timers.iter_mut() {
        // GLD-C-04: Check for expired grace windows first.
        // If a grace window is set and has elapsed, collect for deferred finalize.
        // The timer stays in active_timers until end_billing_session removes it.
        if let (Some(grace_until), Some(end_status)) = (timer.lap_reject_grace_until, timer.pending_end_status) {
            if now_for_grace >= grace_until {
                // P0-2 fix: include pod_id so timer can be removed from active_timers
                // BEFORE dropping the write lock (prevents double-finalize race).
                deferred_finalizes.push((pod_id.clone(), timer.session_id.clone(), end_status));
                timer.lap_reject_grace_until = None;
                timer.pending_end_status = None;
                // Skip normal tick processing for this timer — it's being finalized
                continue;
            }
            // Grace window still active — skip normal tick (don't increment time or expire)
            continue;
        }

        // ─── Handle PausedDisconnect state ────────────────────────────────
        if timer.status == BillingSessionStatus::PausedDisconnect {
            // Do NOT increment driving_seconds — billing is frozen
            timer.pause_seconds += 1;
            timer.total_paused_seconds += 1;

            // Check if THIS disconnect's pause timeout exceeded (10 min default).
            // Uses per-disconnect pause_seconds (reset on each disconnect entry),
            // NOT cumulative total_paused_seconds — so brief network blips don't
            // accumulate and kill the session prematurely.
            if timer.pause_seconds > timer.max_pause_duration_secs {
                tracing::info!(
                    "Disconnect pause timeout for session {} on pod {} ({}s this pause, {}s total paused) — auto-ending with refund",
                    timer.session_id, pod_id, timer.pause_seconds, timer.total_paused_seconds
                );
                pause_timeout_end.push((
                    pod_id.clone(),
                    timer.session_id.clone(),
                    timer.driving_seconds,
                    timer.driver_id.clone(),
                ));
            } else {
                // Broadcast paused tick to dashboards (so they see the session is paused)
                events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
            }
            continue;
        }

        // Handle PausedGamePause / PausedCrashRecovery — send paused tick to agent (overlay shows PAUSED badge)
        if matches!(timer.status, BillingSessionStatus::PausedGamePause | BillingSessionStatus::PausedCrashRecovery) {
            timer.pause_seconds += 1;
            timer.total_paused_seconds += 1;
            // PausedCrashRecovery always increments recovery_pause_seconds (not charged)
            if timer.status == BillingSessionStatus::PausedCrashRecovery
                || timer.pause_reason == PauseReason::CrashRecovery
            {
                timer.recovery_pause_seconds += 1;
            }

            // Check 10-min pause timeout
            if timer.pause_seconds > timer.max_pause_duration_secs {
                tracing::info!(
                    "Game-pause timeout for session {} on pod {} ({}s paused) — auto-ending",
                    timer.session_id, pod_id, timer.pause_seconds
                );
                pause_timeout_end.push((
                    pod_id.clone(),
                    timer.session_id.clone(),
                    timer.driving_seconds,
                    timer.driver_id.clone(),
                ));
            } else {
                let cost = timer.current_cost(&rate_tiers);
                events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
                agent_ticks.push((
                    pod_id.clone(), timer.remaining_seconds(), timer.allocated_seconds,
                    timer.driver_name.clone(),
                    Some(timer.elapsed_seconds), Some(cost.total_paise),
                    Some(cost.rate_per_min_paise), Some(true),
                    cost.minutes_to_next_tier, Some(cost.tier_name.clone()),
                ));
            }
            continue;
        }

        // Skip non-active timers (PausedManual, etc.)
        if timer.status != BillingSessionStatus::Active {
            continue;
        }

        // ─── Disconnect detection for Active sessions ─────────────────────
        let pod_is_offline = pods
            .get(pod_id.as_str())
            .map(|p| p.status == rc_common::types::PodStatus::Offline)
            .unwrap_or(true); // No pod info = treat as offline

        if pod_is_offline {
            if timer.offline_since.is_none() {
                timer.offline_since = Some(Utc::now());
            }

            // Immediately pause on disconnect (if pauses remaining)
            if timer.pause_count < 3 {
                match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::Disconnect) {
                    Ok(new_status) => {
                        timer.status = new_status;
                    }
                    Err(e) => {
                        tracing::warn!("BILLING: disconnect pause rejected: {}", e);
                    }
                }
                timer.pause_count += 1;
                timer.pause_seconds = 0; // Reset per-disconnect timer (each disconnect gets fresh 10-min window)
                timer.last_paused_at = Some(Utc::now());

                tracing::info!(
                    "Billing paused due to disconnect: session={} pod={} pause_count={}",
                    timer.session_id, pod_id, timer.pause_count
                );

                new_pauses.push((pod_id.clone(), timer.session_id.clone(), timer.pause_count));
                events_to_broadcast.push(DashboardEvent::BillingSessionChanged(timer.to_info(&rate_tiers)));
                continue; // Skip normal tick
            } else {
                // All 3 pauses used and pod still offline — auto-end after 5 min grace
                // to prevent charging customers for time they can't use (H11 audit fix)
                if let Some(offline_since) = timer.offline_since {
                    let offline_secs = (Utc::now() - offline_since).num_seconds();
                    if offline_secs > 300 {
                        tracing::warn!(
                            "Pod {} offline {}s with all pauses exhausted — auto-ending session {}",
                            pod_id, offline_secs, timer.session_id
                        );
                        sessions_to_auto_end.push((pod_id.clone(), timer.session_id.clone(),
                            format!("Pod offline {}s, all 3 disconnect-pauses exhausted", offline_secs)));
                        continue;
                    }
                }
                tracing::warn!(
                    "Pod {} offline but session {} has used all 3 pauses — billing continues (grace period)",
                    pod_id, timer.session_id
                );
            }
        } else {
            timer.offline_since = None; // Pod is back online
        }

        let expired = timer.tick();
        let remaining = timer.remaining_seconds();

        // Act 2: Per-minute debit check — collect for async processing after lock release
        if timer.needs_per_minute_debit() {
            per_minute_debits.push((
                timer.session_id.clone(),
                pod_id.clone(),
                timer.wallet_owner_id.clone(),
                timer.rate_paise_per_minute,
            ));
            timer.record_debit(timer.rate_paise_per_minute);
        }

        // Check 5-minute warning
        if remaining <= 300 && !timer.warning_5min_sent {
            timer.warning_5min_sent = true;
            warnings.push((timer.session_id.clone(), pod_id.clone(), remaining, timer.driving_seconds));
        }

        // Check 1-minute warning
        if remaining <= 60 && !timer.warning_1min_sent {
            timer.warning_1min_sent = true;
            warnings.push((timer.session_id.clone(), pod_id.clone(), remaining, timer.driving_seconds));
        }

        // Broadcast tick to dashboards and agents
        let cost = timer.current_cost(&rate_tiers);
        events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
        agent_ticks.push((
            pod_id.clone(), remaining, timer.allocated_seconds, timer.driver_name.clone(),
            Some(timer.elapsed_seconds), Some(cost.total_paise),
            Some(cost.rate_per_min_paise), Some(false),
            cost.minutes_to_next_tier, Some(cost.tier_name.clone()),
        ));

        if expired {
            // GLD-C-04: Enter 5s grace window for late lap-reject messages (D-10).
            // Instead of immediately finalizing, set the grace deadline and defer.
            // The billing tick will pick this up on the next pass once grace_until elapses.
            // Only applies if no grace window is already active (avoid re-setting on each tick).
            if timer.lap_reject_grace_until.is_none() {
                let grace_until = chrono::Utc::now() + chrono::Duration::seconds(5);
                timer.lap_reject_grace_until = Some(grace_until);
                timer.pending_end_status = Some(BillingSessionStatus::Completed);
                // Persist grace deadline to DB for restart-safety (fire-and-forget, errors logged in deferred step)
                let sid_grace = timer.session_id.clone();
                let grace_str = grace_until.to_rfc3339();
                // Collect for post-lock DB write (cannot .await while holding active_timers write lock)
                grace_window_sets.push((sid_grace, grace_str));
                tracing::info!(session_id = %timer.session_id, pod_id = %pod_id,
                    "GLD-C-04: session time expired, entering 5s grace window");
                // DO NOT add to expired_sessions yet — wait for grace window to elapse
            }
            // else: grace window already set from a previous tick — deferred finalize loop handles it
        }
    }

    // Remove expired timers
    for (pod_id, _, _, _) in &expired_sessions {
        timers.remove(pod_id);
    }

    // Remove pause-timeout-ended timers
    for (pod_id, _, _, _) in &pause_timeout_end {
        timers.remove(pod_id);
    }

    // P0-2 fix: Remove deferred-finalize timers BEFORE dropping the write lock.
    // This prevents the double-finalize race: without this, the next tick (1s cadence)
    // could see the timer with cleared grace fields and treat it as a normal active
    // timer, potentially spawning a new grace window and double-finalizing.
    // end_billing_session (called after lock drop) handles missing timers gracefully.
    for (pod_id, _, _) in &deferred_finalizes {
        timers.remove(pod_id);
    }

    drop(pods);   // Release pods read lock
    drop(timers); // Release write lock before DB/broadcast

    // GLD-C-04: Persist grace window deadlines to DB (fire-and-forget, lock already released)
    for (sid, grace_str) in &grace_window_sets {
        let _ = sqlx::query(
            "UPDATE billing_sessions SET lap_reject_grace_until = ? WHERE id = ?"
        )
        .bind(grace_str)
        .bind(sid)
        .execute(&state.db)
        .await;
    }

    // GLD-C-04: Execute deferred finalizes for timers whose grace windows have elapsed.
    // Lock is released above — end_billing_session acquires its own locks as needed.
    // Timer was already removed from active_timers above (P0-2 fix).
    for (_pod_id, sid, end_status) in deferred_finalizes {
        // Clear DB grace column (finalize will set terminal status)
        let _ = sqlx::query(
            "UPDATE billing_sessions SET lap_reject_grace_until = NULL WHERE id = ?"
        )
        .bind(&sid)
        .execute(&state.db)
        .await;
        tracing::info!(session_id = %sid, "GLD-C-04: grace window elapsed, running deferred finalize");
        if !end_billing_session(state, &sid, end_status).await {
            tracing::error!(session_id = %sid, "GLD-C-04: deferred finalize returned false");
        }
    }

    // Act 2: Process per-minute wallet debits (async DB operations, lock released)
    for (session_id, pod_id, wallet_owner_id, rate_paise) in &per_minute_debits {
        let debit_result = crate::wallet::debit_wallet(
            &state.db,
            wallet_owner_id,
            *rate_paise as i64,
            "per_minute_billing",
            Some(session_id),
            Some(&format!("Per-minute billing ({}p/min)", rate_paise)),
            &state.config.venue.venue_id,
        )
        .await;
        match debit_result {
            Ok(_) => {
                // Update DB total_debited_paise
                let _ = sqlx::query(
                    "UPDATE billing_sessions SET total_debited_paise = total_debited_paise + ? WHERE id = ?",
                )
                .bind(*rate_paise as i64)
                .bind(session_id)
                .execute(&state.db)
                .await;
            }
            Err(e) => {
                // Wallet empty — auto-end this session
                tracing::warn!(
                    "Per-minute debit failed for session {} (pod {}): {} — auto-ending session",
                    session_id, pod_id, e
                );
                // Re-acquire lock to mark session as ended
                let rate_tiers = state.billing.rate_tiers.read().await;
                let mut timers = state.billing.active_timers.write().await;
                if let Some(timer) = timers.get_mut(pod_id.as_str()) {
                    if let Ok(new_status) = crate::billing_fsm::validate_transition(
                        timer.status,
                        crate::billing_fsm::BillingEvent::End,
                    ) {
                        timer.status = new_status;
                        events_to_broadcast.push(DashboardEvent::BillingSessionChanged(timer.to_info(&rate_tiers)));
                    }
                    expired_sessions.push((
                        pod_id.clone(),
                        timer.session_id.clone(),
                        timer.driving_seconds,
                        timer.driver_name.clone(),
                    ));
                    timers.remove(pod_id.as_str());
                }
                drop(timers);
                drop(rate_tiers);
            }
        }

        // Check low balance warning
        if let Ok(Some((balance,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(wallet_owner_id)
        .fetch_optional(&state.db)
        .await
        {
            // Re-acquire lock briefly to check/set warning flag
            let mut timers = state.billing.active_timers.write().await;
            if let Some(timer) = timers.get_mut(pod_id.as_str()) {
                if balance <= timer.low_balance_warning_paise as i64 && !timer.low_balance_warned {
                    timer.low_balance_warned = true;
                    tracing::info!(
                        "Low balance warning: session {} (pod {}), wallet balance {}p",
                        session_id, pod_id, balance
                    );
                    // TODO: Send WS event to kiosk for audible alert
                }
            }
        }
    }

    // BILL-05: Broadcast WaitingForGame status each tick so kiosk shows "Loading..."
    // WaitingForGame entries are NOT in active_timers — they live in the waiting_for_game map.
    if let Ok(waiting) = state.billing.waiting_for_game.try_read() {
        for (pod_id, entry) in waiting.iter() {
            let info = rc_common::types::BillingSessionInfo {
                id: format!("deferred-{}", pod_id),
                driver_id: entry.driver_id.clone(),
                driver_name: String::new(),
                pod_id: pod_id.clone(),
                pricing_tier_name: entry.pricing_tier_id.clone(),
                allocated_seconds: entry.custom_duration_minutes.unwrap_or(30) * 60,
                driving_seconds: 0,
                remaining_seconds: entry.custom_duration_minutes.unwrap_or(30) * 60,
                status: BillingSessionStatus::WaitingForGame,
                driving_state: DrivingState::Idle,
                started_at: None,
                split_count: 1,
                split_duration_minutes: None,
                current_split_number: 1,
                elapsed_seconds: Some(entry.waiting_since.elapsed().as_secs() as u32),
                cost_paise: Some(0),
                rate_per_min_paise: Some(0),
                billing_mode: None, // Not yet known during waiting_for_game
                recovery_pause_seconds: None,
            };
            events_to_broadcast.push(DashboardEvent::BillingTick(info));
        }
    } // waiting_for_game try_read block — if lock contended, broadcast skipped this tick

    // Trigger any pending (deferred) rolling deploys for pods whose sessions just ended
    for (pod_id, _, _, _) in &expired_sessions {
        crate::deploy::check_and_trigger_pending_deploy(state, pod_id).await;
    }
    for (pod_id, _, _, _) in &pause_timeout_end {
        crate::deploy::check_and_trigger_pending_deploy(state, pod_id).await;
    }

    // Broadcast events to dashboards
    for event in events_to_broadcast {
        let _ = state.dashboard_tx.send(event);
    }

    // Send billing ticks to agents (for pod lock screen timer + overlay taxi meter)
    // Clone senders first, then drop lock before .await (standing rule: no lock across .await)
    if !agent_ticks.is_empty() {
        let seq = BILLING_TICK_SEQ.fetch_add(1, Ordering::Relaxed);
        let senders_snapshot: Vec<_> = {
            let agent_senders = state.agent_senders.read().await;
            agent_ticks.iter().filter_map(|(pod_id, ..)| {
                agent_senders.get(pod_id).map(|s| (pod_id.clone(), s.clone()))
            }).collect()
        }; // lock released
        for (pod_id, remaining, allocated, driver_name, elapsed, cost, rate, paused, min_to_tier, tier_nm) in agent_ticks {
            if let Some((_, sender)) = senders_snapshot.iter().find(|(p, _)| *p == pod_id) {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BillingTick {
                    remaining_seconds: remaining,
                    allocated_seconds: allocated,
                    driver_name,
                    tick_seq: seq,
                    elapsed_seconds: elapsed,
                    cost_paise: cost,
                    rate_per_min_paise: rate,
                    paused,
                    minutes_to_next_tier: min_to_tier,
                    tier_name: tier_nm,
                })).await;
            }
        }
    }

    // Bug #11 + LBILL: Auto-cancel DB billing sessions stuck in 'pending' or 'waiting_for_game' for > 5 minutes.
    // BILL-13 FIX: Also refund wallet for pre-committed sessions that were debited but never activated.
    // LBILL-01/02/03: Check GameTracker before cancelling waiting_for_game sessions — game-aware stale cancel.
    {
        let stale_sessions: Vec<(String, String, Option<i64>, String, String, String, Option<String>)> = match sqlx::query_as(
            "SELECT id, driver_id, wallet_debit_paise, pod_id, created_at, status, wallet_owner_id FROM billing_sessions \
             WHERE status IN ('pending', 'waiting_for_game') \
             AND created_at < datetime('now', '-5 minutes') \
             AND ended_at IS NULL",
        )
        .fetch_all(&state.db)
        .await {
            Ok(rows) => {
                if !rows.is_empty() {
                    tracing::info!("LBILL: found {} stale sessions to evaluate", rows.len());
                }
                rows
            }
            Err(e) => {
                tracing::error!("LBILL: DB query failed: {}", e);
                Vec::new()
            }
        };

        if !stale_sessions.is_empty() {
            // LBILL-01: Snapshot active_games — never hold lock across .await
            let game_snapshot: HashMap<String, rc_common::types::GameState> = {
                let games = state.game_launcher.active_games.read().await;
                games.iter().map(|(k, v)| (k.clone(), v.game_state)).collect()
            };

            // (session_id, driver_id, wallet_debit_paise, wallet_owner_id, pod_id)
            let mut sessions_to_cancel: Vec<(String, String, Option<i64>, Option<String>, String)> = Vec::new();

            for (session_id, driver_id, wallet_debit_paise, pod_id, created_at_str, status, wallet_owner_id) in &stale_sessions {
                // Parse created_at to compute age
                let created_at = chrono::NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
                let age_minutes = created_at
                    .map(|ca| (Utc::now() - ca).num_minutes())
                    .unwrap_or(99); // Treat unparseable as very old → cancel

                if status == "pending" {
                    // LBILL-03: Pending sessions always cancel — no game launched yet
                    tracing::info!(
                        "LBILL-03: Cancelling stale pending session {} — no game launched yet (pod {}, age {}min)",
                        session_id, pod_id, age_minutes
                    );
                    sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
                } else {
                    // status == "waiting_for_game"
                    let game_state = game_snapshot.get(pod_id.as_str()).copied();
                    let game_alive = matches!(game_state, Some(rc_common::types::GameState::Launching)
                        | Some(rc_common::types::GameState::Loading)
                        | Some(rc_common::types::GameState::Running));

                    if game_alive && age_minutes < 10 {
                        // LBILL-02: Game is alive and under 10 min — extend, don't cancel
                        tracing::info!(
                            "LBILL-02: Extending stale session {} — game {:?} on pod {} (age {}min, created {})",
                            session_id, game_state, pod_id, age_minutes, created_at_str
                        );
                        // Skip — do not add to sessions_to_cancel
                    } else if game_alive && age_minutes >= 10 {
                        // LBILL-02: Absolute timeout — cancel despite game being alive
                        tracing::warn!(
                            "LBILL-02: Absolute timeout — cancelling session {} despite game alive on pod {} ({}min)",
                            session_id, pod_id, age_minutes
                        );
                        sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
                    } else {
                        // LBILL-03: Game is dead — cancel with refund
                        tracing::info!(
                            "LBILL-03: Cancelling stale session {} — no active game on pod {} (game_state={:?}, age {}min)",
                            session_id, pod_id, game_state, age_minutes
                        );
                        sessions_to_cancel.push((session_id.clone(), driver_id.clone(), *wallet_debit_paise, wallet_owner_id.clone(), pod_id.clone()));
                    }
                }
            }

            // Refund wallet for sessions being cancelled (BILL-13 kiosk path)
            for (session_id, driver_id, wallet_debit_paise, wallet_owner, _pod_id) in &sessions_to_cancel {
                if let Some(debit) = wallet_debit_paise {
                    if *debit > 0 {
                        let refund_target = wallet_owner.as_deref().unwrap_or(driver_id.as_str());
                        match crate::wallet::credit(
                            state,
                            refund_target,
                            *debit,
                            "refund_session",
                            Some(session_id.as_str()),
                            Some("Auto-refund: session cancelled (game never reached playable state)"),
                            None,
                        ).await {
                            Ok(_) => tracing::info!(
                                "Bug #11: Refunded {}p for stale cancelled session {} (driver={})",
                                debit, session_id, driver_id
                            ),
                            Err(e) => tracing::error!(
                                "Bug #11: Failed to refund {}p for stale session {}: {}",
                                debit, session_id, e
                            ),
                        }
                    }
                }
            }

            // Cancel only the sessions that were not extended
            for (session_id, _, _, _, cancel_pod_id) in &sessions_to_cancel {
                if let Err(e) = sqlx::query(
                    "UPDATE billing_sessions SET status = 'cancelled', ended_at = datetime('now') \
                     WHERE id = ? AND ended_at IS NULL",
                )
                .bind(session_id)
                .execute(&state.db)
                .await
                {
                    tracing::warn!("Failed to auto-cancel stale billing session {}: {}", session_id, e);
                }

                // CRITICAL FIX: Remove entry from in-memory waiting_for_game map
                // Without this, the per-pod billing lock blocks all future billing/start on this pod
                let normalized = cancel_pod_id.replace('-', "_");
                if state.billing.waiting_for_game.write().await.remove(&normalized).is_some() {
                    tracing::info!(
                        "Cleared waiting_for_game entry for pod {} (session {} auto-cancelled)",
                        cancel_pod_id, session_id
                    );
                }
            }
        }
    }

    // Send StopGame + SessionEnded/SubSessionEnded to agents for expired sessions
    if !expired_sessions.is_empty() {
        // Log activity for expired sessions
        for (pod_id, _, driving_seconds, driver_name) in &expired_sessions {
            log_pod_activity(state, pod_id, "billing", "Session Expired", &format!("{} — {}s driven", driver_name, driving_seconds), "core", None);
        }

        // Snapshot senders to avoid holding lock across .await (standing rule)
        let sender_snapshot: Vec<_> = {
            let agent_senders = state.agent_senders.read().await;
            expired_sessions.iter().filter_map(|(pod_id, _, _, _)| {
                agent_senders.get(pod_id).map(|s| (pod_id.clone(), s.clone()))
            }).collect()
        }; // lock dropped here
        for (pod_id, session_id, driving_seconds, driver_name) in &expired_sessions {
            // Check if pod has active reservation (multi-sub-session support)
            let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, pod_id)
                .await
                .is_some();

            if let Some((_, sender)) = sender_snapshot.iter().find(|(id, _)| id == pod_id) {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;

                if has_reservation {
                    // Sub-session ended — pod stays reserved, customer picks next race
                    let driver_id_for_wallet = sqlx::query_as::<_, (String,)>(
                        "SELECT driver_id FROM billing_sessions WHERE id = ?",
                    )
                    .bind(session_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
                    .map(|r| r.0)
                    .unwrap_or_default();

                    let wallet_balance = crate::wallet::get_balance(state, &driver_id_for_wallet)
                        .await
                        .unwrap_or(0);

                    // Look up split info to determine current/total
                    let split_info = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
                        "SELECT split_count, reservation_id FROM billing_sessions WHERE id = ?",
                    )
                    .bind(session_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten();

                    let (current_split, total_splits) = if let Some((Some(sc), Some(res_id))) = &split_info {
                        let completed = sqlx::query_as::<_, (i64,)>(
                            "SELECT COUNT(*) FROM billing_sessions WHERE reservation_id = ? AND status IN ('completed', 'ended_early')",
                        )
                        .bind(res_id)
                        .fetch_one(&state.db)
                        .await
                        .map(|r| r.0)
                        .unwrap_or(1);
                        (completed as u32, *sc as u32)
                    } else {
                        (1, 1)
                    };

                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SubSessionEnded {
                            billing_session_id: session_id.clone(),
                            driver_name: driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds: *driving_seconds,
                            wallet_balance_paise: wallet_balance,
                            current_split_number: current_split,
                            total_splits,
                        }))
                        .await;

                    // If this was the last split, end the reservation
                    if current_split >= total_splits {
                        if let Some((_, Some(res_id))) = &split_info {
                            let _ = crate::pod_reservation::end_reservation(state, res_id).await;
                            tracing::info!("Last split completed — reservation {} ended", res_id);
                        }
                    }
                } else {
                    // Full session ended — pod returns to idle
                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                            billing_session_id: session_id.clone(),
                            driver_name: driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds: *driving_seconds,
                        }))
                        .await;

                    // BlankScreen is handled by rc-agent after showing session summary
                }
            }

            // Clear pod billing reference
            {
                let mut pods = state.pods.write().await;
                if let Some(pod) = pods.get_mut(pod_id) {
                    pod.billing_session_id = None;
                    if has_reservation {
                        // Pod stays reserved for next sub-session — keep driver name visible
                    } else {
                        pod.current_driver = None;
                        pod.status = rc_common::types::PodStatus::Idle;
                    }
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }
        }
    }

    // MULTI-02: Check if any expired pod was part of a multiplayer group
    for (pod_id, _, _, _) in &expired_sessions {
        check_and_stop_multiplayer_server(state, pod_id).await;
    }

    // Broadcast warnings — BILL-02: also send BillingCountdownWarning to the specific pod's agent
    for (session_id, pod_id, remaining, driving_seconds) in warnings {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id.clone(),
            pod_id: pod_id.clone(),
            remaining_seconds: remaining,
        });

        // BILL-02: Send countdown warning to agent for persistent overlay on customer screen
        let level = if remaining <= 60 { "red" } else { "yellow" };
        tracing::info!("BILL-02: Sending {} countdown warning to pod {} ({}s remaining)", level, pod_id, remaining);
        {
            let sender_clone = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender_clone {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BillingCountdownWarning {
                    remaining_secs: remaining,
                    level: level.to_string(),
                })).await;
            }
        } // agent_senders lock dropped

        // Log warning event to DB
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(if remaining <= 60 {
            "warning_1min"
        } else {
            "warning_5min"
        })
        .bind(driving_seconds as i64)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
    }

    // Persist expired sessions to DB
    for (_, session_id, driving_seconds, _) in expired_sessions {
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'completed', driving_seconds = ?, ended_at = datetime('now')
             WHERE id = ?",
        )
        .bind(driving_seconds as i64)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
             VALUES (?, ?, 'time_expired', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
    }

    // Persist new disconnect pauses to DB
    for (pod_id, session_id, pause_count) in &new_pauses {
        log_pod_activity(state, pod_id, "billing", "Session Paused (Disconnect)",
            &format!("Pod offline — pause {}/3", pause_count), "race_engineer", Some(session_id));
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'paused_disconnect', pause_count = ?, last_paused_at = datetime('now')
             WHERE id = ?",
        )
        .bind(*pause_count as i64)
        .bind(session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'paused_disconnect', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(0i64) // driving_seconds not incremented during pause
        .bind(format!("{{\"pause_count\":{},\"reason\":\"disconnect\"}}", pause_count))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        // Broadcast SessionPaused to dashboards
        let _ = state.dashboard_tx.send(DashboardEvent::SessionPaused {
            pod_id: pod_id.clone(),
            session_id: session_id.clone(),
            reason: "disconnect".to_string(),
            pause_count: *pause_count,
        });

        // Send ShowPauseOverlay to agent — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ShowPauseOverlay {
                session_id: session_id.clone(),
                remaining_seconds: 600, // max pause duration
                pause_count: *pause_count,
            })).await;
        }
    }

    // Handle pause timeout auto-end with partial refund
    for (pod_id, session_id, driving_seconds, driver_id) in pause_timeout_end {
        log_pod_activity(state, &pod_id, "billing", "Session Auto-Ended",
            "Disconnect pause timeout (10min) — auto-ended with partial refund", "race_engineer", Some(&session_id));

        // Calculate partial refund
        let session_info = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
            "SELECT allocated_seconds, wallet_debit_paise, wallet_owner_id, \
             COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let mut refund_paise: i64 = 0;
        if let Some((allocated, Some(debit), wallet_owner, billing_mode, total_debited, rate_per_min)) = session_info {
            refund_paise = if billing_mode == "per_minute" {
                compute_per_minute_refund(debit, total_debited.unwrap_or(0), rate_per_min.unwrap_or(2500), driving_seconds as i64)
            } else {
                compute_refund(allocated, driving_seconds as i64, debit)
            };
            if refund_paise > 0 {
                let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                // L2-01 fix: handle refund failure explicitly (not let _ =)
                match crate::wallet::refund(
                    state,
                    refund_target,
                    refund_paise,
                    Some(&session_id),
                    Some("Auto-refund: disconnect pause timeout"),
                )
                .await
                {
                    Ok(_) => tracing::info!("BILLING: disconnect timeout refund {}p for session {}", refund_paise, session_id),
                    Err(e) => tracing::error!("CRITICAL: disconnect timeout refund FAILED for session {} ({}p): {}", session_id, refund_paise, e),
                }
            }
        }

        // FATM-04: CAS guard — only update if session is still active/paused_disconnect
        // Prevents double-refund if end_billing_session also races to close this session
        let cas_result = sqlx::query(
            "UPDATE billing_sessions SET status = 'ended_early', driving_seconds = ?, ended_at = datetime('now'),
             refund_paise = ?, notes = 'Auto-ended: disconnect pause timeout (10min)'
             WHERE id = ? AND status IN ('active', 'paused_disconnect')",
        )
        .bind(driving_seconds as i64)
        .bind(refund_paise)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        match cas_result {
            Ok(result) if result.rows_affected() == 0 => {
                tracing::warn!("BILLING: CAS rejected disconnect-timeout end for session {} — already finalized (double-end prevented)", session_id);
            }
            Err(e) => {
                tracing::error!("Failed to update billing session {} on disconnect timeout: {}", session_id, e);
            }
            _ => {}
        }

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
             VALUES (?, ?, 'pause_timeout_ended', ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .bind(format!("{{\"refund_paise\":{}}}", refund_paise))
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        // Clear pod billing reference and restore idle state
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&pod_id) {
                pod.billing_session_id = None;
                pod.current_driver = None;
                pod.status = rc_common::types::PodStatus::Idle;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }

        // Notify agent: session ended — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(&pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::HidePauseOverlay {
                session_id: session_id.clone(),
            })).await;
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                    billing_session_id: session_id.clone(),
                    driver_name: "".to_string(), // Name not needed for timeout end
                    total_laps: 0,
                    best_lap_ms: None,
                    driving_seconds,
                }))
                .await;
        }

        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id,
            pod_id,
            remaining_seconds: 0,
        });
    }

    // ─── H11: Auto-end sessions where pod is offline with all pauses exhausted ────
    for (pod_id, session_id, reason) in sessions_to_auto_end {
        tracing::warn!("Auto-ending session {} on pod {} — {}", session_id, pod_id, reason);
        log_pod_activity(state, &pod_id, "billing", "Session Auto-Ended (Offline)",
            &reason, "race_engineer", Some(&session_id));

        // H11-REFUND: Calculate partial refund (same as pause_timeout path)
        let session_info = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
            "SELECT allocated_seconds, driving_seconds, wallet_debit_paise, wallet_owner_id, \
             COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let mut refund_paise: i64 = 0;
        if let Some((allocated, driving_seconds, Some(debit), wallet_owner, billing_mode, total_debited, rate_per_min)) = session_info {
            refund_paise = if billing_mode == "per_minute" {
                compute_per_minute_refund(debit, total_debited.unwrap_or(0), rate_per_min.unwrap_or(2500), driving_seconds)
            } else {
                compute_refund(allocated, driving_seconds, debit)
            };
            if refund_paise > 0 {
                let driver_id_row = sqlx::query_as::<_, (String,)>(
                    "SELECT driver_id FROM billing_sessions WHERE id = ?",
                )
                .bind(&session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                let driver_id = driver_id_row.map(|(d,)| d).unwrap_or_default();
                let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                match crate::wallet::refund(
                    state,
                    refund_target,
                    refund_paise,
                    Some(&session_id),
                    Some("Auto-refund: offline auto-end (H11)"),
                )
                .await
                {
                    Ok(_) => tracing::info!("BILLING: H11 offline refund {}p for session {}", refund_paise, session_id),
                    Err(e) => tracing::error!("CRITICAL: H11 offline refund FAILED for session {} ({}p): {}", session_id, refund_paise, e),
                }
            }
        }

        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'ended_early', ended_at = datetime('now'),
             refund_paise = ?, notes = ? WHERE id = ? AND status IN ('active', 'paused_disconnect')",
        )
        .bind(refund_paise)
        .bind(&reason)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'offline_auto_ended', 0, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(format!("{{\"reason\":\"{}\",\"refund_paise\":{}}}", reason.replace('"', "\\\""), refund_paise))
        .execute(&state.db)
        .await;

        // Remove the timer
        {
            let mut timers = state.billing.active_timers.write().await;
            timers.remove(&pod_id);
        }

        // Reset pod state
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&pod_id) {
                pod.billing_session_id = None;
                pod.current_driver = None;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }

        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id,
            pod_id,
            remaining_seconds: 0,
        });
    }

    // ─── Launch timeout handling ─────────────────────────────────────────
    // Check for pods stuck in WaitingForGame for >180 seconds
    let timed_out = check_launch_timeouts(state).await;
    for (pod_id, attempt) in timed_out {
        if attempt == 1 {
            // First timeout: reset to attempt 2 and allow another 3 minutes.
            // CRITICAL: acquire write lock in a tight block, snapshot retry data, then drop.
            // Previous code held the write lock alive when acquiring a read lock on the same
            // RwLock — tokio::sync::RwLock is not re-entrant, causing a deadlock that froze
            // the entire billing tick loop.
            let (retry_sim, retry_args) = {
                let mut waiting = state.billing.waiting_for_game.write().await;
                if let Some(entry) = waiting.get_mut(&pod_id) {
                    tracing::warn!(
                        "Launch timeout (attempt 1) for pod {} — allowing retry (attempt 2)",
                        pod_id
                    );
                    entry.attempt = 2;
                    entry.waiting_since = std::time::Instant::now();
                    // Snapshot retry data while we have the lock
                    (
                        entry.sim_type.unwrap_or(rc_common::types::SimType::AssettoCorsa),
                        entry.launch_args.clone(),
                    )
                } else {
                    (rc_common::types::SimType::AssettoCorsa, None)
                }
                // write lock dropped here
            };
            log_pod_activity(state, &pod_id, "billing", "Launch Timeout",
                "AC failed to reach LIVE in 3 min — retry allowed", "race_engineer", None);
            // The agent-side LaunchState machine handles the actual retry
            // Send LaunchGame again with the ORIGINAL sim_type and args (not hardcoded AC)
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                    sim_type: retry_sim,
                    launch_args: retry_args,
                    force_clean: false,
                    duration_minutes: None,
                    launch_id: None,
                })).await;
            }
        } else {
            // Second timeout: cancel with no charge.
            // CRITICAL: Remove entry and drop write lock immediately — never hold across .await.
            // Previous code held waiting_for_game.write() across multiple DB queries, wallet
            // credit, and WS sends (~90 lines of async work), blocking ALL billing operations.
            let entry = {
                let mut waiting = state.billing.waiting_for_game.write().await;
                waiting.remove(&pod_id)
                // write lock dropped here
            };
            tracing::error!(
                "Launch timeout (attempt 2) for pod {} — cancelling session (no charge)",
                pod_id
            );
            log_pod_activity(state, &pod_id, "billing", "Launch Failed",
                "AC failed to reach LIVE after 2 attempts (6 min total) — session cancelled, no charge", "race_engineer", None);

            // BILL-06: Cancel session — handle both pre-committed (BILL-13) and PIN-auth paths
            if let Some(ref timed_out_entry) = entry {
                if let Some(ref pre_data) = timed_out_entry.pre_committed {
                    // BILL-13: Pre-committed session already in DB — UPDATE existing record + refund
                    let _ = sqlx::query(
                        "UPDATE billing_sessions SET status = 'cancelled_no_playable', ended_at = datetime('now'), driving_seconds = 0 WHERE id = ?",
                    )
                    .bind(&pre_data.session_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| tracing::error!("Failed to update cancelled_no_playable for session {}: {}", pre_data.session_id, e));
                    // Refund wallet debit
                    let debit_row: Option<(i64, Option<String>)> = sqlx::query_as(
                        "SELECT wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
                    )
                    .bind(&pre_data.session_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten();
                    if let Some((debit_paise, wallet_owner)) = debit_row {
                        if debit_paise > 0 {
                            let refund_target = wallet_owner.as_deref().unwrap_or(&timed_out_entry.driver_id);
                            match crate::wallet::credit(
                                state, refund_target, debit_paise, "refund_session",
                                Some(&pre_data.session_id),
                                Some("Auto-refund: launch timeout (game never reached playable state)"),
                                None,
                            ).await {
                                Ok(_) => tracing::info!(
                                    "Launch timeout refund: {}p for session {} (pod={}, driver={})",
                                    debit_paise, pre_data.session_id, timed_out_entry.pod_id, timed_out_entry.driver_id
                                ),
                                Err(e) => tracing::error!(
                                    "Launch timeout refund FAILED: {}p for session {}: {}",
                                    debit_paise, pre_data.session_id, e
                                ),
                            }
                        }
                    }
                } else {
                    // PIN auth path — no DB record exists yet, create cancelled_no_playable record
                    let session_id = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        "INSERT INTO billing_sessions (id, pod_id, driver_id, pricing_tier_id, allocated_seconds, status, created_at, ended_at, driving_seconds, total_paused_seconds, venue_id)
                         VALUES (?, ?, ?, ?, 0, 'cancelled_no_playable', datetime('now'), datetime('now'), 0, 0, ?)",
                    )
                    .bind(&session_id)
                    .bind(&timed_out_entry.pod_id)
                    .bind(&timed_out_entry.driver_id)
                    .bind(&timed_out_entry.pricing_tier_id)
                    .bind(&state.config.venue.venue_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| tracing::error!("Failed to insert cancelled_no_playable record (launch timeout): {}", e));
                }
                tracing::warn!(
                    "Session cancelled_no_playable: pod={} driver={} (launch timeout attempt 2)",
                    timed_out_entry.pod_id, timed_out_entry.driver_id
                );
            }

            // Send BillingStopped to agent so it shows session cancelled
            // Snapshot sender — don't hold agent_senders lock across .await
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender {
                let billing_session_id = entry
                    .map(|e| format!("deferred-{}", e.pod_id))
                    .unwrap_or_default();
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::BillingStopped {
                    billing_session_id,
                })).await;
            }

            // Clear pod state back to idle
            {
                let mut pods = state.pods.write().await;
                if let Some(pod) = pods.get_mut(&pod_id) {
                    pod.billing_session_id = None;
                    pod.current_driver = None;
                    pod.status = rc_common::types::PodStatus::Idle;
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }
        }
    }
}

/// Called every 5 seconds to persist driving_seconds to database
pub async fn sync_timers_to_db(state: &Arc<AppState>) {
    // MMA-P2: Snapshot timer data under lock, then release lock before DB writes.
    // This prevents the read lock from blocking tick_all_timers during DB contention.
    let snapshots: Vec<(String, BillingSessionStatus, u32, u32)> = {
        let timers = state.billing.active_timers.read().await;
        timers.values()
            .filter(|t| matches!(t.status,
                BillingSessionStatus::Active
                | BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
                | BillingSessionStatus::PausedCrashRecovery
            ))
            .map(|t| (t.session_id.clone(), t.status, t.driving_seconds, t.total_paused_seconds))
            .collect()
    }; // lock released here

    for (session_id, status, driving_seconds, total_paused_seconds) in &snapshots {
        let result = if *status == BillingSessionStatus::Active
            || *status == BillingSessionStatus::PausedManual
        {
            sqlx::query("UPDATE billing_sessions SET driving_seconds = ? WHERE id = ?")
                .bind(*driving_seconds as i64)
                .bind(session_id)
                .execute(&state.db)
                .await
        } else {
            // PausedDisconnect or PausedGamePause: also persist pause seconds
            sqlx::query("UPDATE billing_sessions SET driving_seconds = ?, total_paused_seconds = ? WHERE id = ?")
                .bind(*driving_seconds as i64)
                .bind(*total_paused_seconds as i64)
                .bind(session_id)
                .execute(&state.db)
                .await
        };
        // MMA-P2: Log SQLITE_BUSY errors instead of silently dropping them
        if let Err(e) = result {
            tracing::warn!("billing sync_to_db failed for session {}: {} — will retry next cycle", session_id, e);
        }
    }
}

/// Persist billing timer elapsed_seconds to DB for a specific pod index.
/// Called by the staggered timer persistence loop — each pod writes at a different
/// second offset within the minute: Pod N writes at second (N * 7) % 60.
/// This prevents all 8 pods from hitting SQLite simultaneously. (RESIL-02)
pub async fn persist_timer_state(state: &Arc<AppState>, target_pod_number: Option<u32>) {
    let now_str = chrono::Utc::now().to_rfc3339();

    // Snapshot timers under lock, then release before any async work (no RwLock across .await)
    let snapshots: Vec<(String, u32, u32, u32, String, u32)> = {
        let timers = state.billing.active_timers.read().await;
        timers.values()
            .filter(|t| matches!(t.status,
                BillingSessionStatus::Active
                | BillingSessionStatus::PausedManual
                | BillingSessionStatus::PausedDisconnect
                | BillingSessionStatus::PausedGamePause
                | BillingSessionStatus::PausedCrashRecovery
            ))
            .filter(|t| {
                // If target_pod_number specified, only persist that pod's timer
                match target_pod_number {
                    Some(n) => {
                        // Extract pod number from pod_id (e.g., "pod_3" -> 3)
                        t.pod_id.trim_start_matches("pod_").parse::<u32>().unwrap_or(0) == n
                    }
                    None => true, // persist all (used for shutdown/emergency)
                }
            })
            .map(|t| (t.session_id.clone(), t.elapsed_seconds, t.driving_seconds, t.total_paused_seconds, t.pod_id.clone(), t.recovery_pause_seconds))
            .collect()
    }; // lock released here

    for (session_id, elapsed, driving, paused, pod_id, recovery_pause) in &snapshots {
        let result = sqlx::query(
            "UPDATE billing_sessions SET elapsed_seconds = ?, driving_seconds = ?, total_paused_seconds = ?, recovery_pause_seconds = ?, last_timer_sync_at = ? WHERE id = ?"
        )
        .bind(*elapsed as i64)
        .bind(*driving as i64)
        .bind(*paused as i64)
        .bind(*recovery_pause as i64)
        .bind(&now_str)
        .bind(session_id)
        .execute(&state.db)
        .await;

        match result {
            Ok(_) => tracing::debug!("Timer persisted for session {} on {}: elapsed={}s", session_id, pod_id, elapsed),
            Err(e) => tracing::warn!("Timer persist failed for session {} on {}: {} — will retry next cycle", session_id, pod_id, e),
        }
    }
}
