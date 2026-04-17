//! Billing session start — start_billing_session.
//!
//! Extracted from billing_session_lifecycle.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use chrono::Utc;

use rc_common::pod_id::normalize_pod_id;
use rc_common::types::{BillingSessionStatus, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::billing::{BillingTimer, PauseReason};
use crate::billing_multiplayer::create_split_records;
use crate::billing_pricing::compute_dynamic_price;
use crate::event_archive;
use crate::state::AppState;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

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
    if let Some(sc) = split_count
        && sc > 0 && split_duration_minutes.unwrap_or(1) == 0 {
            return Err("Split duration must be greater than 0 minutes".to_string());
        }

    // Kimi-002: Validate duration bounds before arithmetic (prevent u32 overflow)
    if let Some(dur) = custom_duration_minutes
        && dur > 1440 { return Err("Custom duration cannot exceed 24 hours (1440 minutes)".to_string()); }
    if let Some(dur) = split_duration_minutes
        && dur > 1440 { return Err("Split duration cannot exceed 24 hours (1440 minutes)".to_string()); }

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
        if let Some((balance,)) = balance_row
            && balance < 0 {
                tracing::error!(
                    "RESIL-05: Blocking session start — wallet has negative balance: driver={}, balance_paise={}",
                    driver_id, balance
                );
                return Err("Wallet has negative balance — contact staff".to_string());
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

    // BUG-5: fallback changed from "per_minute" to "package". Rationale:
    // - "per_minute" fallback gave max_session_seconds=86400 (24h). A trial session
    //   that hit this fallback would stay "active" for 24h, blocking the pod and
    //   preventing post-session hooks from firing.
    // - "package" fallback uses allocated_seconds (what the customer actually paid
    //   for: 300s trial, 1800s 30-min package, etc). Errs toward shorter session
    //   if we can't determine the mode. The allocated_seconds==0 floor at line 299
    //   still protects legitimate per-minute sessions that fall through.
    // Also log at WARN so we can observe how often this defensive path fires.
    let (billing_mode, rate_per_min, hold, low_warn) = billing_mode_info
        .unwrap_or_else(|| {
            tracing::warn!(
                "BILLING: pricing_tier {} not found or query failed — falling back to 'package' mode with default rate. Session: {}",
                pricing_tier_id, session_id
            );
            ("package".to_string(), Some(2500), Some(10000), Some(5000))
        });

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
        max_session_seconds: if is_per_minute || allocated_seconds == 0 { 86400 } else { allocated_seconds }, // 24hr safety cap for per-minute (was 3hr, raised for iRacing endurance). BUG-5 floor: allocated_seconds==0 also caps at 24h so a per-minute session that fell through to the package fallback doesn't end instantly.
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
