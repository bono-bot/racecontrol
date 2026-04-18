//! Billing timer tick loop — the 1-second heartbeat that drives all billing state.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Core tick loop lives here; post-lock deferred work delegated to:
//!   - billing_timer_stale.rs  — stale session cleanup (Bug #11 / LBILL)
//!   - billing_timer_expiry.rs — expired sessions, warnings, pause/offline/launch timeouts

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::Utc;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{BillingSessionStatus, DrivingState};

use crate::billing::{PauseReason, check_launch_timeouts, end_billing_session};
use crate::billing_timer_expiry;
use crate::billing_timer_stale;
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
    let mut per_minute_debits: Vec<(String, String, String, u32)> = Vec::new(); // (session_id, pod_id, wallet_owner_id, debit_paise)
    let mut credit_backs: Vec<(String, String, String, u32)> = Vec::new(); // snap pricing credit-backs
    let mut new_pauses: Vec<(String, String, u32)> = Vec::new(); // pod_id, session_id, pause_count
    let mut sessions_to_auto_end: Vec<(String, String, String)> = Vec::new(); // pod_id, session_id, reason (H11 offline auto-end → ended_early)
    // Phase 414 Plan 04 Task 2a: separate vec for the 15-min idle auto-end path.
    // CRITICAL (B4 / D-IDLE-AUTOEND): this path uses BillingEvent::End → Completed (NOT EndedEarly).
    // Must NOT be merged with sessions_to_auto_end above — H11 path writes status='ended_early'
    // and computes a partial refund; Phase 414 auto-end is a "natural completion" via end_billing_session.
    let mut phase414_idle_auto_ends: Vec<(String, String, String)> = Vec::new(); // pod_id, session_id, reason
    // GLD-C-04: Grace window DB writes (session_id, grace_until RFC3339)
    let mut grace_window_sets: Vec<(String, String)> = Vec::new();
    // GLD-C-04: Expired grace windows to finalize (pod_id, session_id, end_status)
    // P0-2 fix: pod_id included so we can remove the timer from active_timers BEFORE
    // dropping the write lock, preventing the double-finalize race where the next tick
    // sees the timer with cleared grace fields and treats it as a normal active timer.
    let mut deferred_finalizes: Vec<(String, String, BillingSessionStatus)> = Vec::new();
    // Phase 414: Collect (pod_id, session_id, wallet_owner_id, rate_paise_per_minute) candidates inside the lock.
    // Plan 03 will use this vec to query wallet balance + emit DashboardEvent::IdleWarning AFTER lock drop.
    // For Plan 02, we just log the candidates so we have evidence the threshold was hit.
    let mut idle_warnings_to_emit: Vec<(String, String, String, u32)> = Vec::new();

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

        // Phase 414: Mid-stream WaitingForGame — idle counter has been ticked by timer.tick() above.
        // Check if the 10-min warning threshold was just crossed; if so, set the one-shot flag and
        // queue the candidate for post-lock IdleWarning broadcast.
        if timer.status == BillingSessionStatus::WaitingForGame && timer.elapsed_seconds > 0 {
            if timer.between_games_idle_seconds == 600 && !timer.idle_warning_sent {
                timer.idle_warning_sent = true;
                idle_warnings_to_emit.push((
                    pod_id.clone(),
                    timer.session_id.clone(),
                    timer.wallet_owner_id.clone(),
                    timer.rate_paise_per_minute,
                ));
            }
            // Phase 414 Plan 04 Task 2a: 15-min idle auto-end.
            // Push to dedicated post-lock vec (phase414_idle_auto_ends) so end_billing_session_public
            // runs AFTER active_timers write lock drops.
            // CRITICAL (B4 / D-IDLE-AUTOEND): uses BillingEvent::End → Completed.
            // Staff-triggered stop uses BillingEvent::EndEarly → EndedEarly (Task 2b stop_billing handler).
            // DO NOT use sessions_to_auto_end (H11 vec) — that one routes through handle_offline_auto_end
            // which writes status='ended_early'. The two paths are distinct (B4 lock).
            if timer.between_games_idle_seconds >= 900 && !timer.idle_auto_end_queued {
                // MMA P1-A fix: one-shot guard. Without this, every tick from second 900+
                // re-pushed to phase414_idle_auto_ends until the async end_billing_session_public
                // CAS removed the timer, producing duplicate "Session Auto-Ended" activity log
                // entries. Mirrors the idle_warning_sent pattern at line 161.
                timer.idle_auto_end_queued = true;
                phase414_idle_auto_ends.push((
                    pod_id.clone(),
                    timer.session_id.clone(),
                    "Phase 414: 15-min idle auto-end (between games)".to_string(),
                ));
            }

            // Phase 414 Plan 03 (per plan-checker B2): emit BillingTick on every WaitingForGame
            // mid-stream tick so the kiosk paused-meter UI can render the live
            // between_games_idle_seconds countdown. Without this, the kiosk has the
            // initial state from the last Active tick but no live updates during the wait.
            // to_info() populates between_games_idle_seconds (Some(N)) per the billing.rs impl.
            events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info(&rate_tiers)));
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

        // Act 2: Per-minute debit — snap-to-package pricing with credit-back at boundaries
        if timer.needs_per_minute_debit() {
            let snap_amount = timer.snap_debit_amount();
            if snap_amount > 0 {
                per_minute_debits.push((
                    timer.session_id.clone(), pod_id.clone(),
                    timer.wallet_owner_id.clone(), snap_amount as u32,
                ));
            } else if snap_amount < 0 {
                credit_backs.push((
                    timer.session_id.clone(), pod_id.clone(),
                    timer.wallet_owner_id.clone(), (-snap_amount) as u32,
                ));
            }
            timer.record_snap_debit(snap_amount);
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

    // Phase 414 Plan 03: Broadcast DashboardEvent::IdleWarning for each candidate.
    // Wallet balance lookup is async and MUST happen AFTER the active_timers lock drops (CLAUDE.md mandate).
    for (pod_id, session_id, wallet_owner_id, rate_paise_per_minute) in &idle_warnings_to_emit {
        // Same query pattern as the existing per-minute debit (billing_timer.rs ~line 415)
        let balance: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?",
        )
        .bind(wallet_owner_id.as_str())
        .fetch_one(&state.db)
        .await
        .unwrap_or(0); // CLAUDE.md: no .unwrap() — graceful degradation; treat missing wallet as 0 balance

        let balance_paise: u64 = balance.max(0) as u64;
        let can_continue: bool = (balance_paise as u32) >= *rate_paise_per_minute;

        let event = DashboardEvent::IdleWarning {
            pod_id: pod_id.clone(),
            session_id: session_id.clone(),
            balance_paise,
            seconds_remaining: 300, // 600s idle elapsed → 5 min until 15-min auto-end (Plan 04)
            can_continue,
        };
        let _ = state.dashboard_tx.send(event);

        tracing::info!(
            pod_id = %pod_id,
            session_id = %session_id,
            balance_paise,
            can_continue,
            "Phase 414: Broadcast DashboardEvent::IdleWarning (10-min mark, 5min until auto-end)"
        );

        // Plan 04 also adds: log to pod_activity table with kind="between_games_idle_warning"
    }

    // ─── Phase 414 Plan 04 Task 2a: 15-min idle AUTO-END (between-games) ────────
    // Process auto-end candidates after lock drop. end_billing_session_public is async + does its own DB work.
    // PER CONTEXT.md D-IDLE-AUTOEND: use BillingEvent::End → Completed (NOT EndEarly → EndedEarly).
    // Rationale: 15-min auto-end is a "natural completion" (customer walked away after the warning),
    // not an explicit early termination. Staff-triggered stop on mid-stream uses EndEarly (Task 2b).
    // We pass BillingSessionStatus::Completed which end_billing_session maps to BillingEvent::End
    // via its match table (billing_session_end.rs:53).
    for (pod_id, session_id, reason) in &phase414_idle_auto_ends {
        let ended = crate::billing_session_end::end_billing_session_public(
            &state,
            session_id.as_str(),
            rc_common::types::BillingSessionStatus::Completed, // BillingEvent::End via end_billing_session match
            Some(reason.as_str()),
        ).await;

        if ended {
            tracing::info!(
                pod_id = %pod_id,
                session_id = %session_id,
                "Phase 414: Auto-ended session at 15-min idle (Completed via BillingEvent::End)"
            );
            crate::activity_log::log_pod_activity(
                &state, pod_id, "billing", "Session Auto-Ended (Idle)",
                reason, "core", Some(session_id),
            );
        } else {
            tracing::warn!(
                pod_id = %pod_id,
                session_id = %session_id,
                "Phase 414: Auto-end at 15-min idle returned false (already finalized or CAS-rejected); will not retry"
            );
        }
    }

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
            "debit_session",
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
            if let Some(timer) = timers.get_mut(pod_id.as_str())
                && balance <= timer.low_balance_warning_paise as i64 && !timer.low_balance_warned {
                    timer.low_balance_warned = true;
                    tracing::info!(
                        "Low balance warning: session {} (pod {}), wallet balance {}p",
                        session_id, pod_id, balance
                    );
                    // TODO: Send WS event to kiosk for audible alert
                }
        }
    }

    // Snap pricing: credit-backs at tier boundaries
    for (session_id, pod_id, wallet_owner_id, refund_paise) in &credit_backs {
        match crate::wallet::refund(state, wallet_owner_id, *refund_paise as i64, Some(session_id), Some("Snap pricing boundary credit-back")).await {
            Ok(_) => {
                let _ = sqlx::query("UPDATE billing_sessions SET total_debited_paise = total_debited_paise - ? WHERE id = ?")
                    .bind(*refund_paise as i64).bind(session_id).execute(&state.db).await;
                tracing::info!("Snap credit-back: {}p for session {} (pod {})", refund_paise, session_id, pod_id);
            }
            Err(e) => tracing::error!("Snap credit-back FAILED: session {} pod {} — {}", session_id, pod_id, e),
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
                between_games_idle_seconds: None,
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

    // Bug #11 + LBILL: Auto-cancel stale 'pending'/'waiting_for_game' sessions (>5 min)
    billing_timer_stale::cleanup_stale_sessions(state).await;

    // Send StopGame + SessionEnded/SubSessionEnded to agents for expired sessions
    billing_timer_expiry::handle_expired_sessions(state, &expired_sessions).await;

    // Broadcast warnings — BILL-02: also send BillingCountdownWarning to the specific pod's agent
    billing_timer_expiry::broadcast_warnings(state, warnings).await;

    // Persist expired sessions to DB
    billing_timer_expiry::persist_expired_sessions(state, expired_sessions).await;

    // Persist new disconnect pauses to DB
    billing_timer_expiry::persist_new_pauses(state, &new_pauses).await;

    // Handle pause timeout auto-end with partial refund
    billing_timer_expiry::handle_pause_timeouts(state, pause_timeout_end).await;

    // H11: Auto-end sessions where pod is offline with all pauses exhausted
    billing_timer_expiry::handle_offline_auto_end(state, sessions_to_auto_end).await;

    // Launch timeout handling — check for pods stuck in WaitingForGame for >180 seconds
    let timed_out = check_launch_timeouts(state).await;
    billing_timer_expiry::handle_launch_timeouts(state, timed_out).await;
}

// Re-export persistence functions (moved to billing_timer_persist.rs)
pub use crate::billing_timer_persist::{sync_timers_to_db, persist_timer_state};
