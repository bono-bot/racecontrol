use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use rc_common::protocol::{CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{BillingSessionInfo, BillingSessionStatus, DrivingState};

use crate::state::AppState;

// ─── BillingTimer ───────────────────────────────────────────────────────────

/// In-memory timer for an active billing session on a pod
pub struct BillingTimer {
    pub session_id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pod_id: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
    pub driving_seconds: u32,
    pub status: BillingSessionStatus,
    pub driving_state: DrivingState,
    pub started_at: Option<DateTime<Utc>>,
    pub warning_5min_sent: bool,
    pub warning_1min_sent: bool,
}

impl BillingTimer {
    pub fn remaining_seconds(&self) -> u32 {
        self.allocated_seconds.saturating_sub(self.driving_seconds)
    }

    pub fn to_info(&self) -> BillingSessionInfo {
        BillingSessionInfo {
            id: self.session_id.clone(),
            driver_id: self.driver_id.clone(),
            driver_name: self.driver_name.clone(),
            pod_id: self.pod_id.clone(),
            pricing_tier_name: self.pricing_tier_name.clone(),
            allocated_seconds: self.allocated_seconds,
            driving_seconds: self.driving_seconds,
            remaining_seconds: self.remaining_seconds(),
            status: self.status,
            driving_state: self.driving_state,
            started_at: self.started_at,
        }
    }

    /// Tick the timer by 1 second. Returns true if time has expired.
    /// Timer always counts down for active sessions regardless of driving state.
    pub fn tick(&mut self) -> bool {
        if self.status != BillingSessionStatus::Active {
            return false;
        }

        self.driving_seconds += 1;
        self.remaining_seconds() == 0
    }
}

// ─── BillingManager ─────────────────────────────────────────────────────────

pub struct BillingManager {
    /// pod_id -> BillingTimer
    pub active_timers: RwLock<HashMap<String, BillingTimer>>,
}

impl BillingManager {
    pub fn new() -> Self {
        Self {
            active_timers: RwLock::new(HashMap::new()),
        }
    }
}

// ─── Tick Loop ──────────────────────────────────────────────────────────────

/// Called every 1 second to tick all active billing timers
pub async fn tick_all_timers(state: &Arc<AppState>) {
    let mut timers = state.billing.active_timers.write().await;
    let mut events_to_broadcast = Vec::new();
    let mut expired_sessions = Vec::new();
    let mut warnings = Vec::new();
    let mut agent_ticks: Vec<(String, u32, u32, String)> = Vec::new();

    for (pod_id, timer) in timers.iter_mut() {
        if timer.status != BillingSessionStatus::Active {
            continue;
        }

        let expired = timer.tick();
        let remaining = timer.remaining_seconds();

        // Check 5-minute warning
        if remaining <= 300 && remaining > 298 && !timer.warning_5min_sent {
            timer.warning_5min_sent = true;
            warnings.push((timer.session_id.clone(), pod_id.clone(), remaining));
        }

        // Check 1-minute warning
        if remaining <= 60 && remaining > 58 && !timer.warning_1min_sent {
            timer.warning_1min_sent = true;
            warnings.push((timer.session_id.clone(), pod_id.clone(), remaining));
        }

        // Broadcast tick to dashboards and agents
        events_to_broadcast.push(DashboardEvent::BillingTick(timer.to_info()));
        agent_ticks.push((pod_id.clone(), remaining, timer.allocated_seconds, timer.driver_name.clone()));

        if expired {
            timer.status = BillingSessionStatus::Completed;
            expired_sessions.push((
                pod_id.clone(),
                timer.session_id.clone(),
                timer.driving_seconds,
            ));
            events_to_broadcast.push(DashboardEvent::BillingSessionChanged(timer.to_info()));
        }
    }

    // Remove expired timers
    for (pod_id, _, _) in &expired_sessions {
        timers.remove(pod_id);
    }

    drop(timers); // Release write lock before DB/broadcast

    // Broadcast events to dashboards
    for event in events_to_broadcast {
        let _ = state.dashboard_tx.send(event);
    }

    // Send billing ticks to agents (for pod lock screen timer)
    if !agent_ticks.is_empty() {
        let agent_senders = state.agent_senders.read().await;
        for (pod_id, remaining, allocated, driver_name) in agent_ticks {
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender.send(CoreToAgentMessage::BillingTick {
                    remaining_seconds: remaining,
                    allocated_seconds: allocated,
                    driver_name,
                }).await;
            }
        }
    }

    // Broadcast warnings
    for (session_id, pod_id, remaining) in warnings {
        let _ = state.dashboard_tx.send(DashboardEvent::BillingWarning {
            billing_session_id: session_id.clone(),
            pod_id,
            remaining_seconds: remaining,
        });

        // Log warning event to DB
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
             VALUES (?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(if remaining <= 60 {
            "warning_1min"
        } else {
            "warning_5min"
        })
        .bind(0i64)
        .execute(&state.db)
        .await;
    }

    // Persist expired sessions to DB
    for (_, session_id, driving_seconds) in expired_sessions {
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'completed', driving_seconds = ?, ended_at = datetime('now')
             WHERE id = ?",
        )
        .bind(driving_seconds as i64)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
             VALUES (?, ?, 'time_expired', ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .execute(&state.db)
        .await;
    }
}

/// Called every 5 seconds to persist driving_seconds to database
pub async fn sync_timers_to_db(state: &Arc<AppState>) {
    let timers = state.billing.active_timers.read().await;
    for timer in timers.values() {
        if timer.status == BillingSessionStatus::Active
            || timer.status == BillingSessionStatus::PausedManual
        {
            let _ = sqlx::query(
                "UPDATE billing_sessions SET driving_seconds = ? WHERE id = ?",
            )
            .bind(timer.driving_seconds as i64)
            .bind(&timer.session_id)
            .execute(&state.db)
            .await;
        }
    }
}

// ─── Session Recovery ───────────────────────────────────────────────────────

/// On server startup, recover any active billing sessions from the database
pub async fn recover_active_sessions(state: &Arc<AppState>) -> anyhow::Result<()> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.status IN ('active', 'paused_manual')",
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let mut timers = state.billing.active_timers.write().await;
    for row in &rows {
        let status = match row.7.as_str() {
            "active" => BillingSessionStatus::Active,
            "paused_manual" => BillingSessionStatus::PausedManual,
            _ => continue,
        };

        let started_at = row
            .8
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let timer = BillingTimer {
            session_id: row.0.clone(),
            driver_id: row.1.clone(),
            driver_name: row.2.clone(),
            pod_id: row.3.clone(),
            pricing_tier_name: row.4.clone(),
            allocated_seconds: row.5 as u32,
            driving_seconds: row.6 as u32,
            status,
            driving_state: DrivingState::Idle, // Will be updated when agent reconnects
            started_at,
            warning_5min_sent: (row.5 as u32).saturating_sub(row.6 as u32) <= 300,
            warning_1min_sent: (row.5 as u32).saturating_sub(row.6 as u32) <= 60,
        };

        tracing::info!(
            "Recovered billing session {} for driver {} on pod {} ({}/{}s)",
            timer.session_id,
            timer.driver_name,
            timer.pod_id,
            timer.driving_seconds,
            timer.allocated_seconds
        );

        timers.insert(row.3.clone(), timer);
    }

    tracing::info!("Recovered {} active billing sessions", rows.len());
    Ok(())
}

// ─── Dashboard Command Handler ──────────────────────────────────────────────

pub async fn handle_dashboard_command(state: &Arc<AppState>, cmd: DashboardCommand) {
    match cmd {
        DashboardCommand::StartBilling {
            pod_id,
            driver_id,
            pricing_tier_id,
            custom_price_paise,
            custom_duration_minutes,
        } => {
            let _ = start_billing_session(
                state,
                pod_id,
                driver_id,
                pricing_tier_id,
                custom_price_paise,
                custom_duration_minutes,
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
            extend_billing_session(state, &billing_session_id, additional_seconds).await;
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
) -> Option<String> {
    // Check no active session on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&pod_id) {
            tracing::warn!("Pod {} already has an active billing session", pod_id);
            return None;
        }
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
            tracing::warn!("Pricing tier {} not found or inactive", pricing_tier_id);
            return None;
        }
        Err(e) => {
            tracing::error!("DB error looking up tier: {}", e);
            return None;
        }
    };

    let is_trial = tier.4;

    // Check trial eligibility
    if is_trial {
        let has_used = sqlx::query_as::<_, (bool,)>(
            "SELECT has_used_trial FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await;

        match has_used {
            Ok(Some((true,))) => {
                tracing::warn!("Driver {} has already used their free trial", driver_id);
                return None;
            }
            Ok(None) => {
                tracing::warn!("Driver {} not found", driver_id);
                return None;
            }
            Err(e) => {
                tracing::error!("DB error checking trial: {}", e);
                return None;
            }
            _ => {} // OK, hasn't used trial
        }
    }

    // Look up driver name
    let driver_name = sqlx::query_as::<_, (String,)>("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or_else(|| "Unknown".to_string());

    // Calculate allocated seconds
    let allocated_seconds = custom_duration_minutes
        .map(|m| m * 60)
        .unwrap_or(tier.2 as u32 * 60);

    // Create billing session in DB
    let session_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let _ = sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, started_at)
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&pod_id)
    .bind(&pricing_tier_id)
    .bind(allocated_seconds as i64)
    .bind(custom_price_paise.map(|p| p as i64))
    .bind(now.to_rfc3339())
    .execute(&state.db)
    .await;

    // Log billing events
    for event_type in ["created", "started"] {
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
             VALUES (?, ?, ?, 0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(event_type)
        .execute(&state.db)
        .await;
    }

    // Mark trial as used
    if is_trial {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1 WHERE id = ?")
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

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
    };

    let info = timer.to_info();

    state
        .billing
        .active_timers
        .write()
        .await
        .insert(pod_id.clone(), timer);

    // Update pod info
    if let Some(pod) = state.pods.write().await.get_mut(&pod_id) {
        pod.billing_session_id = Some(session_id.clone());
    }

    // Notify agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender
            .send(CoreToAgentMessage::BillingStarted {
                billing_session_id: session_id.clone(),
                driver_name: driver_name.clone(),
                allocated_seconds,
            })
            .await;
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

    Some(session_id)
}

async fn set_billing_status(
    state: &Arc<AppState>,
    session_id: &str,
    new_status: BillingSessionStatus,
) {
    let mut timers = state.billing.active_timers.write().await;

    // Find the timer by session_id
    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id {
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.status = new_status;
            let info = timer.to_info();

            let event_type = match new_status {
                BillingSessionStatus::PausedManual => "paused_manual",
                BillingSessionStatus::Active => "resumed_manual",
                _ => "status_change",
            };

            drop(timers);

            // Log event
            let _ = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(info.driving_seconds as i64)
            .execute(&state.db)
            .await;

            // Update DB status
            let status_str = match new_status {
                BillingSessionStatus::Active => "active",
                BillingSessionStatus::PausedManual => "paused_manual",
                _ => "active",
            };
            let _ = sqlx::query("UPDATE billing_sessions SET status = ? WHERE id = ?")
                .bind(status_str)
                .bind(session_id)
                .execute(&state.db)
                .await;

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));
        }
    }
}

async fn end_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    end_status: BillingSessionStatus,
) {
    let mut timers = state.billing.active_timers.write().await;

    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id {
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.status = end_status;
            let info = timer.to_info();
            let driving_seconds = timer.driving_seconds;

            timers.remove(&pod_id);
            drop(timers);

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

            // Update DB
            let _ = sqlx::query(
                "UPDATE billing_sessions SET status = ?, driving_seconds = ?, ended_at = datetime('now') WHERE id = ?",
            )
            .bind(status_str)
            .bind(driving_seconds as i64)
            .bind(session_id)
            .execute(&state.db)
            .await;

            let _ = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
            .execute(&state.db)
            .await;

            // Clear pod billing reference
            if let Some(pod) = state.pods.write().await.get_mut(&pod_id) {
                pod.billing_session_id = None;
            }

            // Notify agent
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender
                    .send(CoreToAgentMessage::BillingStopped {
                        billing_session_id: session_id.to_string(),
                    })
                    .await;
            }

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));

            tracing::info!("Billing session {} ended ({})", session_id, status_str);
        }
    }
}

async fn extend_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    additional_seconds: u32,
) {
    let mut timers = state.billing.active_timers.write().await;

    let pod_id = timers
        .iter()
        .find(|(_, t)| t.session_id == session_id)
        .map(|(k, _)| k.clone());

    if let Some(pod_id) = pod_id {
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.allocated_seconds += additional_seconds;
            // Reset warnings if we extended past thresholds
            if timer.remaining_seconds() > 300 {
                timer.warning_5min_sent = false;
            }
            if timer.remaining_seconds() > 60 {
                timer.warning_1min_sent = false;
            }
            let info = timer.to_info();

            drop(timers);

            // Update DB
            let _ = sqlx::query(
                "UPDATE billing_sessions SET allocated_seconds = allocated_seconds + ? WHERE id = ?",
            )
            .bind(additional_seconds as i64)
            .bind(session_id)
            .execute(&state.db)
            .await;

            let metadata = serde_json::json!({ "extended_by_seconds": additional_seconds });
            let _ = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
                 VALUES (?, ?, 'extended', ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(info.driving_seconds as i64)
            .bind(metadata.to_string())
            .execute(&state.db)
            .await;

            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));

            tracing::info!(
                "Billing session {} extended by {} seconds",
                session_id,
                additional_seconds
            );
        }
    }
}

/// Update the driving state for a pod's billing timer
pub async fn update_driving_state(
    state: &Arc<AppState>,
    pod_id: &str,
    new_state: DrivingState,
) {
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
            let info = timer.to_info();

            drop(timers);

            // Log state transition
            let _ = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
            .execute(&state.db)
            .await;

            // Broadcast updated state
            let _ = state
                .dashboard_tx
                .send(DashboardEvent::BillingSessionChanged(info));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_only_counts_when_driving() {
        let mut timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
        };

        // Should count when driving
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 1);

        // Should NOT count when idle
        timer.driving_state = DrivingState::Idle;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 1); // Still 1

        // Should NOT count when paused
        timer.driving_state = DrivingState::Active;
        timer.status = BillingSessionStatus::PausedManual;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 1); // Still 1
    }

    #[test]
    fn timer_expires_correctly() {
        let mut timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Trial".into(),
            allocated_seconds: 3,
            driving_seconds: 2,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
        };

        // One more tick should expire
        assert!(timer.tick());
        assert_eq!(timer.driving_seconds, 3);
        assert_eq!(timer.remaining_seconds(), 0);
    }

    #[test]
    fn remaining_seconds_calculation() {
        let timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "1 Hour".into(),
            allocated_seconds: 3600,
            driving_seconds: 1000,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
        };

        assert_eq!(timer.remaining_seconds(), 2600);
    }
}
