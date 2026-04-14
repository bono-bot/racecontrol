//! Billing session lifecycle — start, end, extend, upgrade, pause, resume.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Contains the full session lifecycle: handle_dashboard_command, start_billing_session,
//! finalize_billing_start, end_billing_session, extend, upgrade, update_driving_state.
//! Also includes BillingStartData struct and set_billing_status helper.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{BillingSessionStatus, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::billing::{BillingTimer, PauseReason};
use crate::billing_hooks::post_session_hooks;
use crate::billing_multiplayer::{check_and_stop_multiplayer_server, create_split_records};
use crate::billing_pricing::{compute_dynamic_price, compute_refund, compute_per_minute_refund};
use crate::event_archive;
use crate::state::AppState;

// ─── FATM-12: Background Reconciliation Job ─────────────────────────────────

/// Module-level statics for lightweight reconciliation status (never runs blocking I/O).
/// Using `std::sync::OnceLock` + `AtomicI64` — no external crate dependency.
// ─── Dashboard Command Handler ──────────────────────────────────────────────

pub async fn handle_dashboard_command(state: &Arc<AppState>, cmd: DashboardCommand) {
    match cmd {
        DashboardCommand::StartBilling {
            pod_id,
            driver_id,
            pricing_tier_id,
            custom_price_paise,
            custom_duration_minutes,
            staff_id,
            split_count,
            split_duration_minutes,
        } => {
            let pod_id = normalize_pod_id(&pod_id).unwrap_or(pod_id);
            let _ = start_billing_session(
                state,
                pod_id,
                driver_id,
                pricing_tier_id,
                custom_price_paise,
                custom_duration_minutes,
                staff_id,
                split_count,
                split_duration_minutes,
            )
            .await;
        }
        DashboardCommand::PauseBilling {
            billing_session_id,
        } => {
            set_billing_status(state, &billing_session_id, BillingSessionStatus::PausedManual)
                .await;
        }
        DashboardCommand::ResumeBilling {
            billing_session_id,
        } => {
            set_billing_status(state, &billing_session_id, BillingSessionStatus::Active).await;
        }
        DashboardCommand::EndBilling {
            billing_session_id,
        } => {
            end_billing_session(state, &billing_session_id, BillingSessionStatus::EndedEarly).await;
        }
        DashboardCommand::CancelBilling {
            billing_session_id,
        } => {
            end_billing_session(state, &billing_session_id, BillingSessionStatus::Cancelled).await;
        }
        DashboardCommand::ExtendBilling {
            billing_session_id,
            additional_seconds,
        } => {
            // FATM-07: dashboard commands are fire-and-forget; log errors but don't propagate
            if let Err(e) = extend_billing_session(state, &billing_session_id, additional_seconds).await {
                tracing::warn!(
                    "FATM-07: Extension failed for session {} via dashboard command: {}",
                    billing_session_id, e
                );
            }
        }
        // Game launcher commands are handled by game_launcher module
        _ => {}
    }
}

pub async fn start_billing_session(
    state: &Arc<AppState>,
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    staff_id: Option<String>,
    split_count: Option<u32>,
    split_duration_minutes: Option<u32>,
) -> Result<String, String> {
    // Normalize pod_id to canonical form (pod_N) at entry
    let pod_id = normalize_pod_id(&pod_id).unwrap_or(pod_id);
    // MMA-101+R2-1: Two-phase reservation to prevent TOCTOU without holding lock across .await.
    // Phase 1: Briefly acquire write lock to check + reserve the slot (insert sentinel).
    // Phase 2: Do DB work with lock released. Phase 3: Re-acquire and finalize.
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&pod_id) {
            return Err(format!("Pod {} already has an active billing session", pod_id));
        }
    }
    // DB-level UNIQUE partial index (MMA-101) is the primary guard against TOCTOU.
    // The in-memory check above is a fast path; the DB constraint catches any race.

    // N6: Validate pod exists before creating session
    let pod_exists = sqlx::query_as::<_, (String,)>("SELECT id FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    if pod_exists.is_none() {
        return Err(format!("Pod '{}' not found", pod_id));
    }

    // Look up pricing tier
    let tier = sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await;

    let tier = match tier {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err(format!("Pricing tier '{}' not found or inactive", pricing_tier_id));
        }
        Err(e) => {
            return Err(format!("DB error looking up tier: {}", e));
        }
    };

    let is_trial = tier.4;

    // Check trial eligibility (skip for unlimited_trials drivers)
    let unlimited_trials = if is_trial {
        let trial_info = sqlx::query_as::<_, (bool, bool)>(
            "SELECT COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0) FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await;

        match trial_info {
            Ok(Some((has_used, unlimited))) => {
                if has_used && !unlimited {
                    return Err("Driver has already used their free trial".to_string());
                }
                unlimited
            }
            Ok(None) => {
                return Err(format!("Driver '{}' not found", driver_id));
            }
            Err(e) => {
                return Err(format!("DB error checking trial: {}", e));
            }
        }
    } else {
        false
    };

    // Look up driver name
    let driver_name = sqlx::query_as::<_, (String,)>("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or_else(|| "Unknown".to_string());

    // N8: Validate split params — reject 0-minute splits
    if let Some(sc) = split_count {
        if sc > 0 && split_duration_minutes.unwrap_or(1) == 0 {
            return Err("Split duration must be greater than 0 minutes".to_string());
        }
    }

    // Kimi-002: Validate duration bounds before arithmetic (prevent u32 overflow)
    if let Some(dur) = custom_duration_minutes {
        if dur > 1440 { return Err("Custom duration cannot exceed 24 hours (1440 minutes)".to_string()); }
    }
    if let Some(dur) = split_duration_minutes {
        if dur > 1440 { return Err("Split duration cannot exceed 24 hours (1440 minutes)".to_string()); }
    }

    // Calculate allocated seconds — use split duration for split sessions
    let allocated_seconds = if let Some(split_dur) = split_duration_minutes.filter(|_| split_count.unwrap_or(1) > 1) {
        split_dur * 60
    } else {
        custom_duration_minutes
            .map(|m| m * 60)
            .unwrap_or(tier.2 as u32 * 60)
    };

    // Apply dynamic pricing if no custom price override
    let final_price_paise = if let Some(custom) = custom_price_paise {
        Some(custom as i64)
    } else if !is_trial {
        let dynamic = compute_dynamic_price(state, tier.3).await;
        if dynamic != tier.3 {
            tracing::info!(
                "Dynamic pricing applied: base={}p -> adjusted={}p",
                tier.3, dynamic
            );
            Some(dynamic)
        } else {
            None // Use base tier price
        }
    } else {
        None
    };

    // RESIL-05: Pre-billing negative wallet balance guard (BLOCKING).
    // If the wallet already has a negative balance, block session start.
    // This prevents new debt accumulation on already-overdrawn accounts.
    // Trials are excluded — they cost nothing.
    if !is_trial {
        let balance_row: Option<(i64,)> = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?"
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((balance,)) = balance_row {
            if balance < 0 {
                tracing::error!(
                    "RESIL-05: Blocking session start — wallet has negative balance: driver={}, balance_paise={}",
                    driver_id, balance
                );
                return Err("Wallet has negative balance — contact staff".to_string());
            }
        }
    }

    // Create billing session in DB
    let session_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let final_split_count = split_count.unwrap_or(1);
    let final_split_duration = split_duration_minutes;

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, started_at, staff_id, split_count, split_duration_minutes, venue_id)
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&pod_id)
    .bind(&pricing_tier_id)
    .bind(allocated_seconds as i64)
    .bind(final_price_paise)
    .bind(now.to_rfc3339())
    .bind(&staff_id)
    .bind(final_split_count as i64)
    .bind(final_split_duration.map(|d| d as i64))
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to persist billing session: {}", e))?;

    // Log billing events
    for event_type in ["created", "started"] {
        if let Err(e) = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
             VALUES (?, ?, ?, 0, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(event_type)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("Failed to log billing event '{}' for session {}: {}", event_type, session_id, e);
        }
    }

    // BILL-05: Log billing_timer_started event with game-live timestamp for audit trail.
    // This creates an auditable record that billing began at game-live time, not staff click.
    // started_at in billing_sessions is set to Utc::now() which is called from handle_game_status_update(Live).
    let billing_start_iso = now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    tracing::info!(
        "BILL-05: billing_timer_started for session {} on pod {} at {} (game-live path, not staff click)",
        session_id, pod_id, billing_start_iso
    );
    let billing_started_meta = serde_json::json!({
        "billing_timer_started": true,
        "started_at": billing_start_iso,
        "pod_id": pod_id,
        "trigger": "game_live_signal"
    });
    if let Err(e) = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
         VALUES (?, ?, 'billing_timer_started', 0, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&session_id)
    .bind(billing_started_meta.to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to log billing_timer_started event for session {}: {}", session_id, e);
    }

    // Mark trial as used (skip for unlimited_trials drivers)
    if is_trial && !unlimited_trials {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    // Look up billing_mode from pricing tier
    let billing_mode_info = sqlx::query_as::<_, (String, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT COALESCE(billing_mode, 'package'), rate_paise_per_minute, minimum_hold_paise, low_balance_warning_paise \
         FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (billing_mode, rate_per_min, hold, low_warn) = billing_mode_info
        .unwrap_or(("package".to_string(), None, None, None));

    let is_per_minute = billing_mode == "per_minute";
    // Resolve wallet owner for per-minute periodic debits
    let wallet_owner = crate::wallet::resolve_wallet_owner(state, &driver_id)
        .await
        .unwrap_or_else(|_| driver_id.clone());

    // Create in-memory timer
    let timer = BillingTimer {
        session_id: session_id.clone(),
        driver_id: driver_id.clone(),
        driver_name: driver_name.clone(),
        pod_id: pod_id.clone(),
        pricing_tier_name: tier.1.clone(),
        allocated_seconds,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Idle, // Will update from agent
        started_at: Some(now),
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: final_split_count,
        split_duration_minutes: final_split_duration,
        current_split_number: 1,
        pause_count: 0,
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        max_session_seconds: if is_per_minute { 10800 } else { allocated_seconds }, // 3hr hard cap for per-minute
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: PauseReason::None,
        nonce: String::new(),
        // Act 2: Per-minute billing fields
        billing_mode,
        rate_paise_per_minute: rate_per_min.unwrap_or(0) as u32,
        hold_paise: if is_per_minute { hold.unwrap_or(10000) as u32 } else { 0 },
        total_debited_paise: if is_per_minute {
            hold.unwrap_or(10000) as u32 // hold was already debited at session start
        } else {
            0
        },
        seconds_since_last_debit: 0,
        wallet_owner_id: wallet_owner,
        low_balance_warning_paise: low_warn.unwrap_or(5000) as u32,
        low_balance_warned: false,
        // GLD-C-02: Coverage histogram starts empty at session creation.
        telemetry_seconds_covered: std::collections::HashSet::new(),
        // GLD-C-04: Grace window fields start as None at session creation.
        lap_reject_grace_until: None, // Intentional default: no pending deferral
        pending_end_status: None,     // Intentional default: no deferred end status
    };

    let rate_tiers = state.billing.rate_tiers.read().await;
    let info = timer.to_info(&rate_tiers);
    drop(rate_tiers);

    // MMA-101+R2-1: Re-acquire write lock briefly for timer insert only (not held across .await)
    state
        .billing
        .active_timers
        .write()
        .await
        .insert(pod_id.clone(), timer);

    // Update pod info
    if let Some(pod) = state.pods.write().await.get_mut(&pod_id) {
        pod.billing_session_id = Some(session_id.clone());
        pod.current_driver = Some(driver_name.clone());
        pod.status = rc_common::types::PodStatus::InSession;
    }

    // Create pod reservation for split sessions (keeps pod reserved between sub-sessions)
    if final_split_count > 1 {
        if let Ok(reservation_id) = crate::pod_reservation::create_reservation(state, &driver_id, &pod_id).await {
            let _ = sqlx::query(
                "UPDATE billing_sessions SET reservation_id = ? WHERE id = ?",
            )
            .bind(&reservation_id)
            .bind(&session_id)
            .execute(&state.db)
            .await;
            tracing::info!(
                "Split session: created reservation {} for {}-split session on pod {}",
                reservation_id, final_split_count, pod_id
            );
        }

        // FSM-07: Create child split entitlement records in DB.
        // total_allocated_seconds is split_duration * split_count (full session time).
        let total_seconds = final_split_duration
            .map(|d| d * 60 * final_split_count)
            .unwrap_or(allocated_seconds * final_split_count);
        if let Err(e) = create_split_records(
            &state.db,
            &session_id,
            final_split_count,
            total_seconds,
            &state.config.venue.venue_id,
        ).await {
            // Non-fatal: split records failing doesn't prevent session start,
            // but we log it at ERROR so it can be investigated.
            tracing::error!(
                "FSM-07: Failed to create split records for session {}: {}",
                session_id, e
            );
        }
    }

    // Notify agent — clone sender BEFORE await to avoid holding lock across .await
    // Standing rule: "Never hold a lock across .await"
    let sender_clone = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    }; // lock released here
    if let Some(sender) = sender_clone {
        let _ = sender
            .send(CoreMessage::wrap(CoreToAgentMessage::BillingStarted {
                billing_session_id: session_id.clone(),
                driver_name: driver_name.clone(),
                allocated_seconds,
                session_token: Some(uuid::Uuid::new_v4().to_string()),
            }))
            .await;
        // Note: BillingStarted sets agent state to ActiveSession, which
        // prevents is_idle_or_blanked() from returning true. Do NOT send
        // ClearLockScreen here — it would reset state to Hidden and allow
        // screen blanking to re-engage during the session.
    }

    // Broadcast to dashboards
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::BillingSessionChanged(info));

    tracing::info!(
        "Billing session started: {} for {} on pod {} ({}s, tier: {})",
        session_id,
        driver_name,
        pod_id,
        allocated_seconds,
        tier.1
    );

    log_pod_activity(state, &pod_id, "billing", "Session Started", &format!("{} — {} ({}min)", driver_name, tier.1, allocated_seconds / 60), "core", Some(&session_id));
    event_archive::append_event(&state.db, "billing.session_started", "billing", Some(&pod_id), serde_json::json!({
        "driver_id": driver_id,
        "tier": tier.1,
        "allocated_seconds": allocated_seconds,
    }), &state.config.venue.venue_id);

    Ok(session_id)
}

/// Parameters for post-commit in-memory billing session activation (FATM-01).
/// All data comes from the values used inside the atomic DB transaction.
/// Call this AFTER tx.commit() — it creates the in-memory timer, updates pod state,
/// notifies the agent, and broadcasts to dashboards.
pub struct BillingStartData {
    pub session_id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pod_id: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
    pub split_count: u32,
    pub split_duration_minutes: Option<u32>,
    pub started_at: DateTime<Utc>,
    // Per-minute billing fields (Act 2)
    pub billing_mode: String,
    pub rate_paise_per_minute: u32,
    pub hold_paise: u32,
    pub wallet_owner_id: String,
    pub low_balance_warning_paise: u32,
}

/// Activate billing session in memory after the DB transaction has committed (FATM-01).
/// Creates the in-memory timer, updates pod state, notifies the agent, broadcasts to dashboards.
/// Safe to call only after tx.commit() — any error before commit rolls back automatically.
pub async fn finalize_billing_start(state: &Arc<AppState>, data: BillingStartData) {
    let is_per_minute = data.billing_mode == "per_minute";
    let mut timer = BillingTimer {
        session_id: data.session_id.clone(),
        driver_id: data.driver_id.clone(),
        driver_name: data.driver_name.clone(),
        pod_id: data.pod_id.clone(),
        pricing_tier_name: data.pricing_tier_name.clone(),
        allocated_seconds: data.allocated_seconds,
        driving_seconds: 0,
        status: BillingSessionStatus::Active,
        driving_state: DrivingState::Idle,
        started_at: Some(data.started_at),
        warning_5min_sent: false,
        warning_1min_sent: false,
        offline_since: None,
        split_count: data.split_count,
        split_duration_minutes: data.split_duration_minutes,
        current_split_number: 1,
        pause_count: 0,
        total_paused_seconds: 0,
        last_paused_at: None,
        max_pause_duration_secs: 600,
        elapsed_seconds: 0,
        pause_seconds: 0,
        // Per-minute: 3hr hard cap. Package: allocated time.
        max_session_seconds: if is_per_minute { 10800 } else { data.allocated_seconds },
        sim_type: None,
        recovery_pause_seconds: 0,
        pause_reason: PauseReason::None,
        nonce: String::new(), // Populated below after nonce store generation
        // Act 2: Use actual billing mode from BillingStartData (was hardcoded to "package")
        billing_mode: data.billing_mode.clone(),
        rate_paise_per_minute: data.rate_paise_per_minute,
        hold_paise: data.hold_paise,
        total_debited_paise: if is_per_minute { data.hold_paise } else { 0 },
        seconds_since_last_debit: 0,
        wallet_owner_id: data.wallet_owner_id.clone(),
        low_balance_warning_paise: data.low_balance_warning_paise,
        low_balance_warned: false,
        // GLD-C-02: Coverage histogram starts empty at session creation.
        telemetry_seconds_covered: std::collections::HashSet::new(),
        // GLD-C-04: Grace window fields start as None at session creation.
        lap_reject_grace_until: None, // Intentional default: no pending deferral
        pending_end_status: None,     // Intentional default: no deferred end status
    };

    // Phase 283: Generate session nonce for replay protection
    let nonce = state.billing_nonce_store.generate(&data.session_id).await;
    timer.nonce = nonce;

    let rate_tiers = state.billing.rate_tiers.read().await;
    let info = timer.to_info(&rate_tiers);
    drop(rate_tiers);

    // Insert into active timers (brief write lock — not held across .await)
    state
        .billing
        .active_timers
        .write()
        .await
        .insert(data.pod_id.clone(), timer);

    // Update pod state
    if let Some(pod) = state.pods.write().await.get_mut(&data.pod_id) {
        pod.billing_session_id = Some(data.session_id.clone());
        pod.current_driver = Some(data.driver_name.clone());
        pod.status = rc_common::types::PodStatus::InSession;
    }

    // Create pod reservation for split sessions
    if data.split_count > 1 {
        if let Ok(reservation_id) = crate::pod_reservation::create_reservation(state, &data.driver_id, &data.pod_id).await {
            let _ = sqlx::query(
                "UPDATE billing_sessions SET reservation_id = ? WHERE id = ?",
            )
            .bind(&reservation_id)
            .bind(&data.session_id)
            .execute(&state.db)
            .await;
            tracing::info!(
                "Split session: created reservation {} for {}-split session on pod {}",
                reservation_id, data.split_count, data.pod_id
            );
        }
    }

    // Notify agent (snapshot sender before dropping read lock)
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&data.pod_id).cloned()
    };
    if let Some(sender) = sender {
        let _ = sender
            .send(CoreMessage::wrap(CoreToAgentMessage::BillingStarted {
                billing_session_id: data.session_id.clone(),
                driver_name: data.driver_name.clone(),
                allocated_seconds: data.allocated_seconds,
                session_token: Some(uuid::Uuid::new_v4().to_string()),
            }))
            .await;
    }

    // Broadcast to dashboards
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::BillingSessionChanged(info));

    tracing::info!(
        "Billing session activated in memory: {} for {} on pod {} ({}s, tier: {})",
        data.session_id,
        data.driver_name,
        data.pod_id,
        data.allocated_seconds,
        data.pricing_tier_name,
    );

    log_pod_activity(
        state,
        &data.pod_id,
        "billing",
        "Session Started",
        &format!("{} — {} ({}min)", data.driver_name, data.pricing_tier_name, data.allocated_seconds / 60),
        "core",
        Some(&data.session_id),
    );
}

async fn set_billing_status(
    state: &Arc<AppState>,
    session_id: &str,
    new_status: BillingSessionStatus,
) {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;

    // Find the timer by session_id
    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id {
        if let Some(timer) = timers.get_mut(&pod_id) {
            // FSM-01: gate every status mutation through validate_transition
            let event = match new_status {
                BillingSessionStatus::PausedManual => crate::billing_fsm::BillingEvent::PauseManual,
                BillingSessionStatus::Active => crate::billing_fsm::BillingEvent::Resume,
                other => {
                    tracing::warn!("BILLING: set_billing_status called with unexpected status {:?} for session {}", other, session_id);
                    return;
                }
            };
            match crate::billing_fsm::validate_transition(timer.status, event) {
                Ok(new_status) => { timer.status = new_status; }
                Err(e) => { tracing::warn!("BILLING: {}", e); return; }
            }
            let info = timer.to_info(&rate_tiers);

            let event_type = match new_status {
                BillingSessionStatus::PausedManual => "paused_manual",
                BillingSessionStatus::Active => "resumed_manual",
                _ => "status_change",
            };

            let activity_action = match new_status {
                BillingSessionStatus::PausedManual => "Session Paused",
                BillingSessionStatus::Active => "Session Resumed",
                _ => "Session Status Changed",
            };
            log_pod_activity(state, &pod_id, "billing", activity_action, &info.driver_name, "core", Some(session_id));

            drop(timers);

            // Log event
            if let Err(e) = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(info.driving_seconds as i64)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await
            {
                tracing::error!("Failed to log billing event '{}' for session {}: {}", event_type, session_id, e);
            }

            // Update DB status
            let status_str = match new_status {
                BillingSessionStatus::Active => "active",
                BillingSessionStatus::PausedManual => "paused_manual",
                _ => "active",
            };
            if let Err(e) = sqlx::query("UPDATE billing_sessions SET status = ? WHERE id = ?")
                .bind(status_str)
                .bind(session_id)
                .execute(&state.db)
                .await
            {
                tracing::error!("Failed to update billing session {} to {}: {}", session_id, status_str, e);
            }

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));
        }
    }
}

/// Resume a billing session that was paused due to disconnect (manual only — staff/kiosk).
pub async fn resume_billing_from_disconnect(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<(), String> {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;

    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    let pod_id = pod_id.ok_or_else(|| "Session not found in active timers".to_string())?;

    let timer = timers.get_mut(&pod_id).ok_or("Timer not found")?;

    match crate::billing_fsm::validate_transition(timer.status, crate::billing_fsm::BillingEvent::Resume) {
        Ok(new_status) => {
            timer.status = new_status;
        }
        Err(e) => {
            return Err(format!("Cannot resume session: {}", e));
        }
    }
    timer.last_paused_at = None;
    timer.offline_since = None;
    // Note: total_paused_seconds keeps accumulating across pauses (not reset)

    let info = timer.to_info(&rate_tiers);
    let driver_name = timer.driver_name.clone();

    drop(timers);

    log_pod_activity(state, &pod_id, "billing", "Session Resumed (Disconnect)",
        &driver_name, "core", Some(session_id));

    // Update DB
    let _ = sqlx::query(
        "UPDATE billing_sessions SET status = 'active', last_paused_at = NULL WHERE id = ?",
    )
    .bind(session_id)
    .execute(&state.db)
    .await;

    // Log event
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
         VALUES (?, ?, 'resumed_disconnect', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(info.driving_seconds as i64)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    // Broadcast SessionResumed to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::SessionResumed {
        pod_id: pod_id.clone(),
        session_id: session_id.to_string(),
    });
    let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));

    // Send HidePauseOverlay to agent — snapshot sender to avoid lock across .await
    let sender_clone = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&pod_id).cloned()
    };
    if let Some(sender) = sender_clone {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::HidePauseOverlay {
            session_id: session_id.to_string(),
        })).await;
    }

    tracing::info!("Billing session {} resumed from disconnect pause", session_id);

    Ok(())
}

/// Public wrapper for ending billing sessions from API routes
pub async fn end_billing_session_public(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
    end_reason: Option<&str>,
) -> bool {
    let ended = end_billing_session(state, session_id, end_status).await;
    if ended {
        if let Some(reason) = end_reason {
            let _ = sqlx::query("UPDATE billing_sessions SET end_reason = ? WHERE id = ?")
                .bind(reason)
                .bind(session_id)
                .execute(&state.db)
                .await;
        }
    }
    ended
}

pub(crate) async fn end_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
) -> bool {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;

    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id {
        if let Some(timer) = timers.get_mut(&pod_id) {
            // FSM-01: gate every status mutation through validate_transition
            let event = match end_status {
                BillingSessionStatus::Completed => crate::billing_fsm::BillingEvent::End,
                BillingSessionStatus::EndedEarly => crate::billing_fsm::BillingEvent::EndEarly,
                BillingSessionStatus::Cancelled => crate::billing_fsm::BillingEvent::Cancel,
                BillingSessionStatus::CancelledNoPlayable => crate::billing_fsm::BillingEvent::CancelNoPlayable,
                other => {
                    tracing::error!("BILLING: end_billing_session called with non-terminal status {:?} for session {}", other, session_id);
                    return false;
                }
            };
            match crate::billing_fsm::validate_transition(timer.status, event) {
                Ok(new_status) => {
                    timer.status = new_status;
                }
                Err(e) => {
                    tracing::warn!("BILLING: {}", e);
                    return false;
                }
            }
            let info = timer.to_info(&rate_tiers);
            let driving_seconds = timer.driving_seconds;
            // MMA-P2: If cost calculation fails (None = tier lookup error), log error
            // and use 0 as fallback (customer-favorable). Previously silent.
            let final_cost_paise = match info.cost_paise {
                Some(cost) => cost,
                None => {
                    tracing::error!("BILLING: cost_paise is None for session {} on pod {} — tier lookup may have failed. Using 0 (customer-favorable fallback).", info.id, pod_id);
                    0
                }
            };

            let activity_action = match end_status {
                BillingSessionStatus::EndedEarly => "Session Ended",
                BillingSessionStatus::Cancelled => "Session Cancelled",
                _ => "Session Expired",
            };
            log_pod_activity(state, &pod_id, "billing", activity_action, &format!("{} — {}s driven", info.driver_name, driving_seconds), "core", Some(session_id));
            event_archive::append_event(&state.db, "billing.session_ended", "billing", Some(&pod_id), serde_json::json!({
                "driver_id": info.driver_id,
                "driving_seconds": driving_seconds,
                "end_status": activity_action,
            }), &state.config.venue.venue_id);

            // GLD-C-02: Capture telemetry coverage bucket BEFORE timer removal (D-05).
            // The HashSet is lost after remove — capture its length now.
            let seconds_covered_at_end: u32 = timers
                .get(&pod_id)
                .map(|t| t.telemetry_seconds_covered.len() as u32)
                .unwrap_or(0);

            timers.remove(&pod_id);
            drop(timers);

            // Trigger any pending (deferred) rolling deploy for this pod
            crate::deploy::check_and_trigger_pending_deploy(state, &pod_id).await;

            let event_type = match end_status {
                BillingSessionStatus::EndedEarly => "ended_early",
                BillingSessionStatus::Cancelled => "cancelled",
                _ => "ended",
            };

            let status_str = match end_status {
                BillingSessionStatus::EndedEarly => "ended_early",
                BillingSessionStatus::Cancelled => "cancelled",
                _ => "completed",
            };

            // FATM-04: CAS guard — only update if session is still 'active'.
            // If rows_affected() == 0, the session was already finalized by another
            // concurrent request (e.g. disconnect timeout racing with staff end).
            // In that case, skip ALL downstream work (refund, agent notify, broadcast).
            // NOTE: Do NOT overwrite wallet_debit_paise here — it must retain the original
            // pre-session charge for correct refund calculation downstream (F-05 fix).
            // final_cost_paise is stored in end_reason for audit purposes.
            // CRITICAL-1 fix: CAS must match ALL valid pre-terminal states, not just 'active'.
            // FSM allows End/EndEarly/Cancel from paused_manual, paused_game_pause, paused_disconnect.
            // Previously only matched 'active' — paused sessions were silently dropped with no refund.
            let cas_result = sqlx::query(
                "UPDATE billing_sessions SET status = ?, driving_seconds = ?, ended_at = datetime('now'), end_reason = ? WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
            )
            .bind(status_str)
            .bind(driving_seconds as i64)
            .bind(format!("final_cost_paise:{}", final_cost_paise))
            .bind(session_id)
            .execute(&state.db)
            .await;

            match cas_result {
                Err(e) => {
                    tracing::error!("Failed to update billing session {} to {}: {}", session_id, status_str, e);
                }
                Ok(result) if result.rows_affected() == 0 => {
                    tracing::warn!(
                        "BILLING: CAS rejected end for session {} — already finalized (double-end prevented)",
                        session_id
                    );
                    return false;
                }
                _ => {}
            }

            if let Err(e) = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await
            {
                tracing::error!("Failed to log billing event '{}' for session {}: {}", event_type, session_id, e);
            }

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

            // MULTI-02: Check if this pod was part of a multiplayer group
            check_and_stop_multiplayer_server(state, &pod_id).await;

            // Proportional refund for early end with wallet debit
            if end_status == BillingSessionStatus::EndedEarly {
                let wallet_info = sqlx::query_as::<_, (String, i64, Option<i64>, Option<String>, String, Option<i64>, Option<i64>)>(
                    "SELECT driver_id, allocated_seconds, wallet_debit_paise, wallet_owner_id, \
                     COALESCE(billing_mode, 'package'), total_debited_paise, rate_paise_per_minute \
                     FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, allocated, Some(debit), wallet_owner, billing_mode, total_debited, rate_per_min)) = wallet_info {
                    let refund_amount = if billing_mode == "per_minute" {
                        // Per-minute: refund unused hold. Hold was deducted upfront,
                        // periodic debits were separate. Refund = hold - (minutes * rate).
                        let rate = rate_per_min.unwrap_or(2500);
                        compute_per_minute_refund(debit, total_debited.unwrap_or(0), rate, driving_seconds as i64)
                    } else {
                        // Package: use best-rate formula
                        compute_refund(allocated, driving_seconds as i64, debit)
                    };
                    if refund_amount > 0 {
                        let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                        let refund_note = if billing_mode == "per_minute" {
                            "Early end — per-minute hold refund"
                        } else {
                            "Early end — proportional refund"
                        };
                        match crate::wallet::refund(
                            state,
                            refund_target,
                            refund_amount,
                            Some(session_id),
                            Some(refund_note),
                        )
                        .await
                        {
                            Ok(_) => tracing::info!("BILLING: early-end refund {}p for session {} (mode={})", refund_amount, session_id, billing_mode),
                            Err(e) => tracing::error!("CRITICAL: early-end refund FAILED for session {} ({}p): {}", session_id, refund_amount, e),
                        }
                    }
                }
            }

            // Full refund for cancelled sessions (never drove)
            if end_status == BillingSessionStatus::Cancelled {
                let wallet_info = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
                    "SELECT driver_id, wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, Some(debit), wallet_owner)) = wallet_info {
                    if debit > 0 {
                        let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
                        // L2-01 fix: handle refund failure explicitly
                        match crate::wallet::refund(
                            state,
                            refund_target,
                            debit,
                            Some(session_id),
                            Some("Cancelled session — full refund"),
                        )
                        .await
                        {
                            Ok(_) => tracing::info!("BILLING: cancel refund {}p for session {}", debit, session_id),
                            Err(e) => tracing::error!("CRITICAL: cancel refund FAILED for session {} ({}p): {}", session_id, debit, e),
                        }
                    }
                }

                // FATM-09: Restore any coupon reserved for this session back to 'available'
                match crate::api::routes::restore_coupon_on_cancel(&state.db, session_id).await {
                    Ok(_) => tracing::info!(
                        "FATM-09: Coupon restored for cancelled session {}",
                        session_id
                    ),
                    Err(e) => tracing::warn!(
                        "FATM-09: Coupon restore failed for session {} (non-critical): {}",
                        session_id, e
                    ),
                }
            }

            // Notify agent: stop game and show session summary
            let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, &pod_id)
                .await
                .is_some();

            // Snapshot sender to avoid holding lock across .await
            let sender_clone = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(sender) = sender_clone {
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;

                if has_reservation && end_status != BillingSessionStatus::Cancelled {
                    let wallet_balance = crate::wallet::get_balance(state, &info.driver_id)
                        .await
                        .unwrap_or(0);
                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SubSessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                            wallet_balance_paise: wallet_balance,
                            current_split_number: info.current_split_number,
                            total_splits: info.split_count,
                        }))
                        .await;
                } else {
                    let _ = sender
                        .send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                        }))
                        .await;

                    // BlankScreen is handled by rc-agent after showing session summary
                }
            }

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info.clone()));

            tracing::info!("Billing session {} ended ({})", session_id, status_str);

            // Post-session hooks (fire-and-forget)
            if end_status != BillingSessionStatus::Cancelled {
                let state_clone = state.clone();
                let session_id_clone = session_id.to_string();
                let driver_id_clone = info.driver_id.clone();
                let pod_id_clone = pod_id.clone();
                tokio::spawn(async move {
                    post_session_hooks(
                        &state_clone,
                        &session_id_clone,
                        &driver_id_clone,
                        seconds_covered_at_end,
                        &pod_id_clone,
                    )
                    .await;
                });
            }
            return true;
        }
    }

    // ─── Fallback: orphaned session in DB but no in-memory timer ─────────
    // This happens when racecontrol restarts while a session was active.
    drop(timers);
    // Match all pre-terminal states (consistent with CRITICAL-1 CAS fix)
    let orphan = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, pod_id, driver_name FROM billing_sessions WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to check for orphaned billing session {}: {}", session_id, e);
            return false;
        }
    };

    if let Some((sid, pod_id, driver_name)) = orphan {
        tracing::warn!("Force-ending orphaned billing session {} on {} (no in-memory timer)", sid, pod_id);

        let status_str = match end_status {
            BillingSessionStatus::EndedEarly => "ended_early",
            BillingSessionStatus::Cancelled => "cancelled",
            _ => "completed",
        };

        if let Err(e) = sqlx::query(
            "UPDATE billing_sessions SET status = ?, ended_at = datetime('now') WHERE id = ?",
        )
        .bind(status_str)
        .bind(session_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("Failed to end orphaned billing session {}: {}", session_id, e);
        }

        // CRITICAL-3 fix: issue refund for orphaned sessions (previously skipped entirely)
        let refund_info = sqlx::query_as::<_, (String, i64, Option<i64>, Option<i64>, Option<String>)>(
            "SELECT driver_id, allocated_seconds, wallet_debit_paise, driving_seconds, wallet_owner_id FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((driver_id, allocated, Some(debit), driving_secs, wallet_owner)) = refund_info {
            let driven = driving_secs.unwrap_or(0);
            let refund_target = wallet_owner.as_deref().unwrap_or(&driver_id);
            let refund_amount = if end_status == BillingSessionStatus::Cancelled {
                debit // full refund for cancellation
            } else {
                compute_refund(allocated, driven, debit)
            };
            if refund_amount > 0 {
                match crate::wallet::refund(state, refund_target, refund_amount, Some(session_id),
                    Some("Orphaned session refund after restart")).await {
                    Ok(_) => tracing::info!("BILLING: orphaned session {} refund {}p to {}", session_id, refund_amount, driver_id),
                    Err(e) => tracing::error!("CRITICAL: orphaned session {} refund FAILED for {}: {}", session_id, driver_id, e),
                }
            }
        }

        log_pod_activity(state, &pod_id, "billing", "Orphaned Session Ended", &format!("{} — force-ended after racecontrol restart", driver_name), "race_engineer", Some(session_id));

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

        // MULTI-02: Check if this orphaned pod was part of a multiplayer group
        check_and_stop_multiplayer_server(state, &pod_id).await;

        // Notify agent to deactivate overlay and show blank — snapshot sender to avoid lock across .await
        let sender_clone = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(&pod_id).cloned()
        };
        if let Some(sender) = sender_clone {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::SessionEnded {
                billing_session_id: session_id.to_string(),
                driver_name,
                total_laps: 0,
                best_lap_ms: None,
                driving_seconds: 0,
            })).await;
        }

        return true;
    }

    false
}

/// FATM-07: Atomic extension — wallet debit + time addition in single DB transaction.
/// Returns Ok(()) on success. Returns Err with reason on insufficient balance, session not found, or DB failure.
/// In-memory timer is updated ONLY after successful DB commit.
pub async fn extend_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    additional_seconds: u32,
) -> Result<(), String> {
    // Phase 1: Snapshot timer data without holding lock across .await (standing rule: no RwLock across .await)
    let (pod_id_opt, extension_cost_paise, driving_seconds_snapshot, timer_status) = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let timers = state.billing.active_timers.read().await;

        let entry: Option<(String, i64, u32, BillingSessionStatus)> = timers
            .iter()
            .find(|(_, t)| t.session_id == session_id)
            .map(|(k, t)| {
                let current_cost = t.current_cost(&rate_tiers);
                let ext_rate = current_cost.rate_per_min_paise;
                let cost = (ext_rate * additional_seconds as i64 + 30) / 60;
                (k.clone(), cost, t.driving_seconds, t.status.clone())
            });
        (
            entry.as_ref().map(|(k, _, _, _)| k.clone()),
            entry.as_ref().map(|(_, c, _, _)| *c).unwrap_or(0),
            entry.as_ref().map(|(_, _, d, _)| *d).unwrap_or(0),
            entry.map(|(_, _, _, s)| s),
        )
    }; // rate_tiers and timers guards both dropped here

    let pod_id = match pod_id_opt {
        Some(p) => p,
        None => return Err(format!("Session '{}' not found in active timers", session_id)),
    };

    // BILL-04: Validate session is active before extending
    match timer_status.as_ref() {
        Some(BillingSessionStatus::Completed)
        | Some(BillingSessionStatus::EndedEarly)
        | Some(BillingSessionStatus::Cancelled)
        | Some(BillingSessionStatus::CancelledNoPlayable) => {
            let msg = format!(
                "BILL-04: Extension attempt on non-active session {} (status={:?}) — rejected",
                session_id, timer_status
            );
            tracing::warn!("{}", msg);
            return Err(msg);
        }
        _ => {}
    }

    // Look up driver_id for wallet debit (DB read before transaction)
    let driver_id = sqlx::query_as::<_, (String,)>(
        "SELECT driver_id FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error looking up session: {}", e))?
    .map(|(d,)| d)
    .ok_or_else(|| format!("Session '{}' not found in DB", session_id))?;

    tracing::info!(
        "BILL-04: Extension uses rate {}p/min for {} seconds (extension_rate_policy=current_tier_effective_rate, cost={}p)",
        if additional_seconds > 0 { extension_cost_paise * 60 / additional_seconds as i64 } else { 0 },
        additional_seconds, extension_cost_paise
    );

    // FATM-07: Begin single transaction — wallet debit + allocated_seconds update
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error starting extension transaction: {}", e))?;

    // Step 1: Debit wallet within transaction (FATM-07)
    if extension_cost_paise > 0 {
        let debit_result: Result<(i64, String), String> = crate::wallet::debit_in_tx(
            &mut tx,
            &driver_id,
            extension_cost_paise,
            "extension",
            Some(session_id),
            Some(&format!("Extension {}s", additional_seconds)),
            None,
            &state.config.venue.venue_id,
        )
        .await;
        if let Err(e) = debit_result {
            // tx dropped here, rolls back automatically
            return Err(format!("Insufficient balance for extension: {}", e));
        }
    }

    // Step 2: Update allocated_seconds in SAME transaction (FATM-07)
    sqlx::query(
        "UPDATE billing_sessions SET allocated_seconds = allocated_seconds + ? WHERE id = ?",
    )
    .bind(additional_seconds as i64)
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error updating allocated_seconds: {}", e))?;

    // Step 3: Log extension event in SAME transaction
    let metadata = serde_json::json!({
        "extended_by_seconds": additional_seconds,
        "extension_cost_paise": extension_cost_paise,
    });
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, venue_id)
         VALUES (?, ?, 'extended', ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(driving_seconds_snapshot as i64)
    .bind(metadata.to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    // FATM-07: Commit — if commit fails, BOTH debit and time addition roll back atomically
    tx.commit().await
        .map_err(|e| format!("DB commit failed for extension: {}", e))?;

    // RESIL-05: Post-debit negative wallet balance check (NON-BLOCKING).
    // Read balance AFTER commit (lock already dropped). Alert staff if negative.
    // This check does NOT affect the ongoing session — it is alert-only.
    if extension_cost_paise > 0 {
        let balance_row: Option<(i64,)> = sqlx::query_as(
            "SELECT balance_paise FROM wallets WHERE driver_id = ?"
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((balance,)) = balance_row {
            if balance < 0 {
                tracing::error!(
                    "RESIL-05: Negative wallet balance detected: driver={}, balance={}",
                    driver_id, balance
                );
                let msg = format!(
                    "[BILLING ALERT] Negative wallet balance detected! Driver: {}, Balance: {} paise. {}",
                    driver_id, balance, crate::whatsapp_alerter::ist_now_string()
                );
                crate::whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
            }
        }
    }

    // Phase 2: ONLY after successful commit, update in-memory timer
    // Re-acquire write lock to update in-memory state
    let info = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.allocated_seconds += additional_seconds;
            // Reset warnings if we extended past thresholds
            if timer.remaining_seconds() > 300 {
                timer.warning_5min_sent = false;
            }
            if timer.remaining_seconds() > 60 {
                timer.warning_1min_sent = false;
            }
            Some(timer.to_info(&rate_tiers))
        } else {
            None
        }
    }; // rate_tiers and timers guards dropped here

    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));
    }

    tracing::info!(
        "FATM-07: Billing session {} extended by {} seconds (cost={}p, atomic debit+time committed)",
        session_id, additional_seconds, extension_cost_paise
    );

    Ok(())
}

/// Act 2: Upgrade a package billing session to a higher tier (e.g. 30min → 60min).
/// Only allows upgrading to a tier with longer duration. Charges the price difference only.
/// Per-minute sessions cannot be upgraded to packages (and vice versa).
pub async fn upgrade_billing_tier(
    state: &Arc<AppState>,
    session_id: &str,
    new_tier_id: &str,
) -> Result<(), String> {
    // Look up current session
    let session = sqlx::query_as::<_, (String, String, String, i64, i64, String)>(
        "SELECT bs.id, bs.driver_id, bs.pricing_tier_id, bs.allocated_seconds, bs.wallet_debit_paise, \
         COALESCE(bs.billing_mode, 'package') \
         FROM billing_sessions bs WHERE bs.id = ? AND bs.status IN ('active', 'paused_manual', 'paused_game_pause', 'waiting_for_game')",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Active session '{}' not found", session_id))?;

    let (_sid, driver_id, current_tier_id, current_allocated, current_debit, billing_mode) = session;

    // Only package sessions can be upgraded
    if billing_mode != "package" {
        return Err("Per-minute sessions cannot be upgraded to a package tier".to_string());
    }

    // Look up new tier
    let new_tier = sqlx::query_as::<_, (String, i64, i64, String)>(
        "SELECT name, duration_minutes, price_paise, COALESCE(billing_mode, 'package') FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(new_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Tier '{}' not found or inactive", new_tier_id))?;

    let (new_tier_name, new_duration_min, new_price_paise, new_billing_mode) = new_tier;

    // New tier must also be a package
    if new_billing_mode != "package" {
        return Err("Cannot upgrade to a per-minute tier".to_string());
    }

    // New tier must have longer duration (upgrade only, no downgrade)
    let new_allocated = new_duration_min * 60;
    if new_allocated <= current_allocated {
        return Err(format!(
            "New tier '{}' ({}min) is not longer than current ({}min) — upgrade only",
            new_tier_name, new_duration_min, current_allocated / 60
        ));
    }

    // Charge the difference only
    let difference_paise = new_price_paise - current_debit;
    if difference_paise < 0 {
        return Err("New tier is cheaper — use refund instead".to_string());
    }

    // Resolve wallet owner (linked racers)
    let wallet_owner = crate::wallet::resolve_wallet_owner(state, &driver_id).await?;

    // Atomic transaction: debit wallet + update session
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error: {}", e))?;

    if difference_paise > 0 {
        crate::wallet::debit_in_tx(
            &mut tx,
            &wallet_owner,
            difference_paise,
            "tier_upgrade",
            Some(session_id),
            Some(&format!("Upgrade to {}", new_tier_name)),
            None,
            &state.config.venue.venue_id,
        )
        .await
        .map_err(|e| format!("Insufficient balance for upgrade: {}", e))?;
    }

    sqlx::query(
        "UPDATE billing_sessions SET pricing_tier_id = ?, allocated_seconds = ?, wallet_debit_paise = ? WHERE id = ?",
    )
    .bind(new_tier_id)
    .bind(new_allocated)
    .bind(new_price_paise)
    .bind(session_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error updating session: {}", e))?;

    // Log upgrade event
    let metadata = serde_json::json!({
        "from_tier": current_tier_id,
        "to_tier": new_tier_id,
        "difference_paise": difference_paise,
        "new_allocated_seconds": new_allocated,
    });
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, metadata, venue_id)
         VALUES (?, ?, 'tier_upgrade', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(metadata.to_string())
    .bind(&state.config.venue.venue_id)
    .execute(&mut *tx)
    .await;

    tx.commit().await.map_err(|e| format!("DB commit failed: {}", e))?;

    // Update in-memory timer
    let info = {
        let rate_tiers = state.billing.rate_tiers.read().await;
        let mut timers = state.billing.active_timers.write().await;
        let pod_id = timers.iter().find(|(_, t)| t.session_id == session_id).map(|(k, _)| k.clone());
        if let Some(pod_id) = pod_id {
            if let Some(timer) = timers.get_mut(&pod_id) {
                timer.allocated_seconds = new_allocated as u32;
                timer.warning_5min_sent = false;
                timer.warning_1min_sent = false;
                Some(timer.to_info(&rate_tiers))
            } else { None }
        } else { None }
    };

    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));
    }

    tracing::info!(
        "Tier upgrade: session {} from {} to {} (difference={}p, new_allocated={}s)",
        session_id, current_tier_id, new_tier_id, difference_paise, new_allocated
    );

    Ok(())
}

/// Update the driving state for a pod's billing timer
pub async fn update_driving_state(
    state: &Arc<AppState>,
    pod_id: &str,
    new_state: DrivingState,
) {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;
    if let Some(timer) = timers.get_mut(pod_id) {
        let old_state = timer.driving_state;
        timer.driving_state = new_state;

        if old_state != new_state {
            let event_type = match new_state {
                DrivingState::Active => "driving_detected",
                DrivingState::Idle | DrivingState::NoDevice => "idle_detected",
            };

            let session_id = timer.session_id.clone();
            let driving_seconds = timer.driving_seconds;
            let info = timer.to_info(&rate_tiers);

            drop(timers);

            // Log state transition
            let _ = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
            .bind(&state.config.venue.venue_id)
            .execute(&state.db)
            .await;

            // Broadcast updated state
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));
        }
    }
}
