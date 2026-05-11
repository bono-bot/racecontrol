//! Billing session lifecycle — dashboard command handler, status transitions, driving state.
//!
//! Extracted from billing.rs (Phase 385, v49.0 Architecture Completion).
//! Sub-modules handle start, end, extend/upgrade operations.
//! Re-exports all public items so existing `use crate::billing_session_lifecycle::*` continues to work.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{BillingSessionStatus, ConfigPushPayload, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::billing::{BillingTimer, PauseReason};
use crate::state::AppState;

// Re-export sub-modules so callers using `crate::billing_session_lifecycle::*` still work.
pub use crate::billing_session_start::*;
pub use crate::billing_session_end::*;
pub use crate::billing_session_extend::*;

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
            let pod_id_for_err = pod_id.clone();
            if let Err(e) = start_billing_session(
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
            .await
            {
                tracing::warn!("StartBilling failed for {}: {}", pod_id_for_err, e);
                let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
                    command: "start_billing".to_string(),
                    pod_id: pod_id_for_err,
                    error: e,
                });
            }
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
            if !end_billing_session(state, &billing_session_id, BillingSessionStatus::EndedEarly).await {
                tracing::warn!("EndBilling failed for session {}", billing_session_id);
                let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
                    command: "end_billing".to_string(),
                    pod_id: String::new(),
                    error: format!("Failed to end session {} — session may already be finalized", billing_session_id),
                });
            }
        }
        DashboardCommand::CancelBilling {
            billing_session_id,
        } => {
            if !end_billing_session(state, &billing_session_id, BillingSessionStatus::Cancelled).await {
                tracing::warn!("CancelBilling failed for session {}", billing_session_id);
                let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
                    command: "cancel_billing".to_string(),
                    pod_id: String::new(),
                    error: format!("Failed to cancel session {} — session may already be finalized", billing_session_id),
                });
            }
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

// ─── Set Billing Status (pause/resume) ──────────────────────────────────────

async fn set_billing_status(
    state: &Arc<AppState>,
    session_id: &str,
    new_status: BillingSessionStatus,
) {
    // BUG-7: Pause/Resume FSM errors must surface to the kiosk so staff aren't
    // left with UI state that diverges from server state. Mirrors the
    // Start/End/Cancel CommandError pattern.
    let command_name = match new_status {
        BillingSessionStatus::PausedManual => "pause_billing",
        BillingSessionStatus::Active => "resume_billing",
        _ => "set_billing_status",
    };

    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;

    // Find the timer by session_id
    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    let Some(pod_id) = pod_id else {
        tracing::warn!("BILLING: {} for unknown session {}", command_name, session_id);
        let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
            command: command_name.to_string(),
            pod_id: String::new(),
            error: format!("No active billing timer for session {}", session_id),
        });
        return;
    };
    let Some(timer) = timers.get_mut(&pod_id) else {
        tracing::warn!("BILLING: timer disappeared for pod {} during {}", pod_id, command_name);
        let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
            command: command_name.to_string(),
            pod_id: pod_id.clone(),
            error: format!("Timer for session {} is no longer active", session_id),
        });
        return;
    };
    // FSM-01: gate every status mutation through validate_transition
    let event = match new_status {
        BillingSessionStatus::PausedManual => crate::billing_fsm::BillingEvent::PauseManual,
        BillingSessionStatus::Active => crate::billing_fsm::BillingEvent::Resume,
        other => {
            tracing::warn!("BILLING: set_billing_status called with unexpected status {:?} for session {}", other, session_id);
            let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
                command: command_name.to_string(),
                pod_id: pod_id.clone(),
                error: format!("Unsupported status change {:?}", other),
            });
            return;
        }
    };
    match crate::billing_fsm::validate_transition(timer.status, event) {
        Ok(new_status) => { timer.status = new_status; }
        Err(e) => {
            tracing::warn!("BILLING: {}", e);
            let _ = state.dashboard_tx.send(DashboardEvent::CommandError {
                command: command_name.to_string(),
                pod_id: pod_id.clone(),
                error: e.to_string(),
            });
            return;
        }
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

    // PACT-20260429-013 Phase 1: route billing_paused state to the agent via
    // Phase 177 config_push_queue substrate (V2-P1-CONFIG-SERVICE), replacing
    // the PR #49 BillingPaused/BillingResumed wire-protocol variants. Same
    // behavior on the agent side (`failure_monitor.billing_paused = bool`),
    // but with stronger delivery semantics: per-pod row + seq_num + ack
    // tracking + audit_log + graceful version skew (old agents log+ignore
    // unknowns instead of failing serde decode). On the agent side, the
    // ConfigPush handler at ws_handler.rs routes `billing_paused` field to
    // `failure_monitor_tx.send_modify` (see PACT-20260429-013 Phase 2).
    let billing_paused_value: Option<bool> = match new_status {
        BillingSessionStatus::PausedManual => Some(true),
        BillingSessionStatus::Active => Some(false),
        _ => None,
    };

    if let Some(paused) = billing_paused_value {
        let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
        fields.insert("billing_paused".to_string(), serde_json::json!(paused));

        let seq_num = state.config_push_seq.fetch_add(1, Ordering::SeqCst);
        let payload_json =
            serde_json::to_string(&fields).unwrap_or_else(|_| "{}".to_string());

        // Insert into config_push_queue (per-pod row, status='pending')
        let insert_result = sqlx::query(
            "INSERT INTO config_push_queue (pod_id, payload, seq_num, status) VALUES (?, ?, ?, 'pending')",
        )
        .bind(&pod_id)
        .bind(&payload_json)
        .bind(seq_num as i64)
        .execute(&state.db)
        .await;

        if let Err(e) = insert_result {
            tracing::error!(
                "PACT-013: failed to insert config_push_queue billing_paused={} for pod {}: {}",
                paused, pod_id, e
            );
        } else {
            // Deliver via WS if pod is connected
            let sender = {
                let agent_senders = state.agent_senders.read().await;
                agent_senders.get(&pod_id).cloned()
            };
            if let Some(tx) = sender {
                let push_payload = ConfigPushPayload {
                    fields: fields.clone(),
                    schema_version: 1,
                    sequence: seq_num,
                };
                match tx
                    .send(CoreMessage::wrap(CoreToAgentMessage::ConfigPush(
                        push_payload,
                    )))
                    .await
                {
                    Ok(_) => {
                        let _ = sqlx::query(
                            "UPDATE config_push_queue SET status = 'delivered' WHERE pod_id = ? AND seq_num = ?",
                        )
                        .bind(&pod_id)
                        .bind(seq_num as i64)
                        .execute(&state.db)
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "PACT-013: failed to send ConfigPush billing_paused to pod {} (seq={}): {}",
                            pod_id, seq_num, e
                        );
                    }
                }
            }
            // If pod is offline, status stays 'pending' (CP-02) — pod will pick up
            // on its next subscription/replay tick once it reconnects.

            // Audit log entry (mirrors push_config handler pattern)
            let _ = sqlx::query(
                "INSERT INTO config_audit_log \
                 (action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, seq_num) \
                 VALUES ('config_push', 'config', 'billing_paused', NULL, ?, 'set_billing_status', '[]', ?)",
            )
            .bind(serde_json::json!(paused).to_string())
            .bind(seq_num as i64)
            .execute(&state.db)
            .await;
        }
    }

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

// ─── Update Driving State ───────────────────────────────────────────────────

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

// ─── BillingStartData + finalize (FATM-01) ──────────────────────────────────

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
        // Per-minute: 24hr safety cap (was 3hr, raised for iRacing endurance). Package: allocated time.
        max_session_seconds: if is_per_minute { 86400 } else { data.allocated_seconds },
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
        // Phase 414: Idle counter starts at 0 for all new sessions.
        between_games_idle_seconds: 0,
        idle_warning_sent: false,
        idle_auto_end_queued: false,
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
    if data.split_count > 1
        && let Ok(reservation_id) = crate::pod_reservation::create_reservation(state, &data.driver_id, &data.pod_id).await {
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

// ─── Resume from Disconnect ─────────────────────────────────────────────────

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

// ─── PACT-20260429-013 Phase 1+2 tests ──────────────────────────────────────
//
// Replacement for `test_billing_paused_resumed_roundtrip` (rc-common protocol.rs)
// which was removed when the `CoreToAgentMessage::BillingPaused`/`BillingResumed`
// wire variants retired in this PR. Tests live here (not billing_tests.rs)
// because `set_billing_status` is private to this module — direct access via
// `super::*` keeps the function's surface narrow and avoids a `pub(crate)`
// widening just for tests.

#[cfg(test)]
mod set_billing_status_config_push_tests {
    use super::*;
    use crate::billing::BillingTimer;
    use chrono::Utc;
    use rc_common::types::DrivingState;

    /// Bootstrap an in-memory SQLite pool with the three tables `set_billing_status`
    /// writes to under the PACT-013 path. Fire-and-forget writes (billing_events,
    /// billing_sessions, pod_activity_log) are also created so the function's
    /// secondary INSERTs don't surface as `tracing::error!` and pollute test logs.
    async fn setup_test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        // PACT-013 primary write surfaces (assertions read these)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS config_push_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pod_id TEXT NOT NULL,
                payload TEXT NOT NULL,
                seq_num INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT DEFAULT (datetime('now')),
                acked_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create config_push_queue");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS config_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT DEFAULT (datetime('now')),
                action TEXT NOT NULL,
                entity_type TEXT,
                entity_name TEXT,
                old_value TEXT,
                new_value TEXT,
                pushed_by TEXT,
                pods_acked TEXT,
                seq_num INTEGER
            )",
        )
        .execute(&pool)
        .await
        .expect("create config_audit_log");

        // Fire-and-forget secondary surfaces (the function logs errors but proceeds)
        for ddl in &[
            "CREATE TABLE IF NOT EXISTS billing_events (
                id TEXT PRIMARY KEY,
                billing_session_id TEXT,
                event_type TEXT,
                driving_seconds_at_event INTEGER,
                venue_id TEXT
            )",
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                status TEXT,
                last_paused_at TEXT
            )",
            "CREATE TABLE IF NOT EXISTS pod_activity_log (
                id TEXT PRIMARY KEY,
                pod_id TEXT,
                category TEXT,
                action TEXT,
                details TEXT,
                source TEXT,
                billing_session_id TEXT,
                created_at TEXT,
                previous_hash TEXT,
                entry_hash TEXT
            )",
        ] {
            sqlx::query(ddl).execute(&pool).await.expect("create aux table");
        }
        pool
    }

    fn make_active_timer(session_id: &str, pod_id: &str) -> BillingTimer {
        BillingTimer {
            session_id: session_id.into(),
            driver_id: "test-driver".into(),
            driver_name: "Test Driver".into(),
            pod_id: pod_id.into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 0,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        }
    }

    /// PACT-20260429-013 Phase 1+2 — `set_billing_status(PausedManual)` writes a
    /// `billing_paused=true` row to `config_push_queue` (per-pod + seq_num) + an
    /// audit row to `config_audit_log` instead of firing the retired
    /// `CoreToAgentMessage::BillingPaused` wire variant. `set_billing_status(Active)`
    /// writes the inverse row with monotonically-incremented seq_num. This test
    /// closes §6 NOT TESTED #3 of the PR #54 §S-146 RCA + MMA Step 1 amendment #1.
    #[tokio::test]
    async fn test_billing_paused_via_config_push_roundtrip() {
        let pool = setup_test_pool().await;
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new(config, pool, field_cipher));

        let session_id = "test-pact-013-session";
        let pod_id = "test-pod-1";
        {
            let mut timers = state.billing.active_timers.write().await;
            timers.insert(pod_id.to_string(), make_active_timer(session_id, pod_id));
        }

        // ── PAUSE: Active → PausedManual ──
        set_billing_status(&state, session_id, BillingSessionStatus::PausedManual).await;

        // Assert config_push_queue row exists with billing_paused=true
        let pause_row: (String, String, i64, String) = sqlx::query_as(
            "SELECT pod_id, payload, seq_num, status FROM config_push_queue ORDER BY seq_num ASC LIMIT 1",
        )
        .fetch_one(&state.db)
        .await
        .expect("config_push_queue row must exist after pause");

        assert_eq!(pause_row.0, pod_id, "pod_id must match");
        assert!(
            pause_row.1.contains("\"billing_paused\":true"),
            "payload must contain billing_paused:true — got: {}",
            pause_row.1
        );
        assert_eq!(
            pause_row.2, 1,
            "seq_num must be 1 (AtomicU64::new(1).fetch_add(1) returns 1)"
        );
        assert_eq!(
            pause_row.3, "pending",
            "status must remain 'pending' when no agent_sender registered (offline-pod path)"
        );

        // Assert config_audit_log row exists with entity_name='billing_paused' and new_value=true
        let pause_audit: (String, String, String) = sqlx::query_as(
            "SELECT entity_name, new_value, pushed_by FROM config_audit_log \
             WHERE entity_name='billing_paused' ORDER BY id ASC LIMIT 1",
        )
        .fetch_one(&state.db)
        .await
        .expect("config_audit_log row must exist after pause");

        assert_eq!(pause_audit.0, "billing_paused");
        assert!(
            pause_audit.1.contains("true"),
            "audit_log new_value must record true — got: {}",
            pause_audit.1
        );
        assert_eq!(pause_audit.2, "set_billing_status");

        // ── RESUME: PausedManual → Active ──
        set_billing_status(&state, session_id, BillingSessionStatus::Active).await;

        // Assert second config_push_queue row with billing_paused=false + seq_num=2
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT payload, seq_num FROM config_push_queue ORDER BY seq_num ASC",
        )
        .fetch_all(&state.db)
        .await
        .expect("query config_push_queue rows");

        assert_eq!(
            rows.len(),
            2,
            "must have 2 rows in config_push_queue after pause+resume"
        );
        assert!(
            rows[0].0.contains("\"billing_paused\":true"),
            "row 1 (pause) must carry billing_paused=true"
        );
        assert_eq!(rows[0].1, 1, "row 1 seq_num must be 1");
        assert!(
            rows[1].0.contains("\"billing_paused\":false"),
            "row 2 (resume) must carry billing_paused=false"
        );
        assert_eq!(
            rows[1].1, 2,
            "row 2 seq_num must be 2 (monotonic +1 from row 1)"
        );

        // Assert second config_audit_log row exists with new_value=false
        let audit_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT entity_name, new_value FROM config_audit_log \
             WHERE entity_name='billing_paused' ORDER BY id ASC",
        )
        .fetch_all(&state.db)
        .await
        .expect("query config_audit_log rows");

        assert_eq!(audit_rows.len(), 2, "must have 2 audit rows after pause+resume");
        assert!(audit_rows[0].1.contains("true"), "audit row 1 (pause) records true");
        assert!(audit_rows[1].1.contains("false"), "audit row 2 (resume) records false");
    }

    /// PACT-20260429-013 invariant: when there is no agent_sender for the pod
    /// (offline pod), the config_push_queue row is created with status='pending'
    /// (not 'delivered'). On agent reconnect, CP-02 replay flips status to
    /// 'delivered'. This test asserts the offline-pod state machine starts
    /// correctly; the reconnect-replay leg is owned by config_push_replay tests.
    #[tokio::test]
    async fn test_billing_paused_offline_pod_leaves_pending() {
        let pool = setup_test_pool().await;
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        let state = Arc::new(AppState::new(config, pool, field_cipher));

        let session_id = "offline-pod-session";
        let pod_id = "offline-pod-7";
        {
            let mut timers = state.billing.active_timers.write().await;
            timers.insert(pod_id.to_string(), make_active_timer(session_id, pod_id));
        }
        // Deliberately do NOT register an agent_sender → exercises offline path.

        set_billing_status(&state, session_id, BillingSessionStatus::PausedManual).await;

        let status: String = sqlx::query_scalar(
            "SELECT status FROM config_push_queue WHERE pod_id = ? ORDER BY seq_num DESC LIMIT 1",
        )
        .bind(pod_id)
        .fetch_one(&state.db)
        .await
        .expect("config_push_queue row must exist");

        assert_eq!(
            status, "pending",
            "offline pod must leave row as 'pending' (CP-02 reconnect-replay will flip to 'delivered')"
        );
    }
}
