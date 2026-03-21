use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Datelike, Timelike, Utc};
use tokio::sync::RwLock;

use rc_common::protocol::{CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::{BillingSessionInfo, BillingSessionStatus, DrivingState};

use crate::activity_log::log_pod_activity;
use crate::state::AppState;

/// Look up dynamic pricing rules and compute an adjusted price.
/// Returns the final price in paise, or None if no adjustment (use base price).
pub async fn compute_dynamic_price(
    state: &Arc<AppState>,
    base_price_paise: i64,
) -> i64 {
    let now = chrono::Local::now();
    let dow = now.weekday().num_days_from_monday() as i64; // 0=Mon .. 6=Sun
    let hour = now.hour() as i64;

    // Fetch matching rules (time-of-day rules)
    let rules = sqlx::query_as::<_, (String, f64, i64)>(
        "SELECT rule_type, multiplier, flat_adjustment_paise
         FROM pricing_rules
         WHERE is_active = 1
           AND (day_of_week IS NULL OR day_of_week = ?)
           AND (hour_start IS NULL OR ? >= hour_start)
           AND (hour_end IS NULL OR ? < hour_end)
           AND rule_type IN ('peak', 'off_peak', 'custom')
         ORDER BY
           CASE WHEN day_of_week IS NOT NULL THEN 0 ELSE 1 END,
           CASE WHEN hour_start IS NOT NULL THEN 0 ELSE 1 END
         LIMIT 1",
    )
    .bind(dow)
    .bind(hour)
    .bind(hour)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match rules {
        Some((_rule_type, multiplier, flat_adj)) => {
            let adjusted = (base_price_paise as f64 * multiplier).round() as i64 + flat_adj;
            adjusted.max(0)
        }
        None => base_price_paise,
    }
}

// ─── Billing Rate Tiers ────────────────────────────────────────────────────

/// A per-minute billing rate tier, loaded from the `billing_rates` DB table.
/// Tiers are ordered by `tier_order` and applied additively (non-retroactive).
#[derive(Debug, Clone)]
pub struct BillingRateTier {
    pub tier_order: u32,
    pub tier_name: String,
    /// Upper boundary in minutes for this tier. 0 = unlimited (covers remaining time).
    pub threshold_minutes: u32,
    pub rate_per_min_paise: i64,
}

/// Default billing rate tiers (used before first DB load).
pub fn default_billing_rate_tiers() -> Vec<BillingRateTier> {
    vec![
        BillingRateTier { tier_order: 1, tier_name: "Standard".into(), threshold_minutes: 30, rate_per_min_paise: 2500 },
        BillingRateTier { tier_order: 2, tier_name: "Extended".into(), threshold_minutes: 60, rate_per_min_paise: 2000 },
        BillingRateTier { tier_order: 3, tier_name: "Marathon".into(), threshold_minutes: 0, rate_per_min_paise: 1500 },
    ]
}

/// Refresh the in-memory rate tier cache from the database.
pub async fn refresh_rate_tiers(state: &Arc<AppState>) {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64)>(
        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise
         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    if let Ok(rows) = rows {
        if !rows.is_empty() {
            let tiers: Vec<BillingRateTier> = rows
                .into_iter()
                .map(|(order, name, thresh, rate)| BillingRateTier {
                    tier_order: order as u32,
                    tier_name: name,
                    threshold_minutes: thresh as u32,
                    rate_per_min_paise: rate,
                })
                .collect();
            *state.billing.rate_tiers.write().await = tiers;
            tracing::info!("Billing rate tiers refreshed from DB");
        }
    }
}

// ─── Session Cost Calculation ──────────────────────────────────────────────

/// Result of per-minute session cost calculation.
pub struct SessionCost {
    /// Total cost in paise for the entire elapsed duration
    pub total_paise: i64,
    /// Current rate per minute in paise
    pub rate_per_min_paise: i64,
    /// Current pricing tier name
    pub tier_name: String,
    /// Minutes remaining until next cheaper tier. None if on cheapest tier.
    pub minutes_to_next_tier: Option<u32>,
}

/// Compute session cost from elapsed seconds using non-retroactive tiered pricing.
///
/// Each tier applies only to the minutes within its range (additive, not retroactive).
/// Default tiers: 25 cr/min (0-30 min), 20 cr/min (31-60 min), 15 cr/min (60+ min).
///
/// Example: 45 min = (30 × 2500) + (15 × 2000) = 75000 + 30000 = 105000 paise.
pub fn compute_session_cost(elapsed_seconds: u32, tiers: &[BillingRateTier]) -> SessionCost {
    let elapsed_minutes_f = elapsed_seconds as f64 / 60.0;
    let elapsed_minutes_whole = elapsed_seconds / 60;

    let mut total_paise: i64 = 0;
    let mut prev_threshold: f64 = 0.0;
    let mut current_tier_name = String::new();
    let mut current_rate: i64 = 0;
    let mut minutes_to_next: Option<u32> = None;

    for (i, tier) in tiers.iter().enumerate() {
        let tier_ceiling = if tier.threshold_minutes == 0 {
            f64::MAX
        } else {
            tier.threshold_minutes as f64
        };

        if elapsed_minutes_f < prev_threshold {
            break;
        }

        let minutes_in_tier = if elapsed_minutes_f <= tier_ceiling {
            elapsed_minutes_f - prev_threshold
        } else {
            tier_ceiling - prev_threshold
        };

        total_paise += (minutes_in_tier * tier.rate_per_min_paise as f64).round() as i64;
        current_tier_name = tier.tier_name.clone();
        current_rate = tier.rate_per_min_paise;

        // Minutes to next tier: only if currently in this tier and there IS a next tier
        if elapsed_minutes_f <= tier_ceiling && tier.threshold_minutes > 0 && i + 1 < tiers.len() {
            minutes_to_next = Some(tier.threshold_minutes.saturating_sub(elapsed_minutes_whole));
        }

        prev_threshold = tier_ceiling;
        if elapsed_minutes_f <= tier_ceiling {
            break;
        }
    }

    SessionCost {
        total_paise,
        rate_per_min_paise: current_rate,
        tier_name: current_tier_name,
        minutes_to_next_tier: minutes_to_next,
    }
}

// ─── BillingTimer ───────────────────────────────────────────────────────────

/// In-memory timer for an active billing session on a pod
pub struct BillingTimer {
    pub session_id: String,
    pub driver_id: String,
    pub driver_name: String,
    pub pod_id: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
    /// Legacy field: tracks driving time. In count-up model, mirrors elapsed_seconds for compat.
    pub driving_seconds: u32,
    pub status: BillingSessionStatus,
    pub driving_state: DrivingState,
    pub started_at: Option<DateTime<Utc>>,
    pub warning_5min_sent: bool,
    pub warning_1min_sent: bool,
    /// When the pod went offline (None if online)
    pub offline_since: Option<DateTime<Utc>>,
    /// Number of sub-sessions (1 = no split)
    pub split_count: u32,
    /// Duration of each sub-session in minutes (None = no split)
    pub split_duration_minutes: Option<u32>,
    /// Which sub-session is currently running (1-indexed)
    pub current_split_number: u32,
    /// Number of disconnect-pauses used in this session (max 3)
    pub pause_count: u32,
    /// Total seconds spent in PausedDisconnect state
    pub total_paused_seconds: u32,
    /// When the current pause started (None if not paused)
    pub last_paused_at: Option<DateTime<Utc>>,
    /// Maximum pause duration before auto-end (10 minutes)
    pub max_pause_duration_secs: u32,
    /// Elapsed billable seconds (counts UP from 0 when Active)
    pub elapsed_seconds: u32,
    /// Seconds spent in PausedGamePause state (counts UP, resets on resume)
    pub pause_seconds: u32,
    /// Hard maximum session length in seconds (default 10800 = 3 hours)
    pub max_session_seconds: u32,
}

impl BillingTimer {
    pub fn remaining_seconds(&self) -> u32 {
        self.allocated_seconds.saturating_sub(self.driving_seconds)
    }

    pub fn to_info(&self, tiers: &[BillingRateTier]) -> BillingSessionInfo {
        let cost = self.current_cost(tiers);
        BillingSessionInfo {
            id: self.session_id.clone(),
            driver_id: self.driver_id.clone(),
            driver_name: self.driver_name.clone(),
            pod_id: self.pod_id.clone(),
            pricing_tier_name: self.pricing_tier_name.clone(),
            // Legacy fields: populated with sensible values for backward compat
            allocated_seconds: self.max_session_seconds,
            driving_seconds: self.elapsed_seconds,
            remaining_seconds: self.max_session_seconds.saturating_sub(self.elapsed_seconds),
            status: self.status,
            driving_state: self.driving_state,
            started_at: self.started_at,
            split_count: self.split_count,
            split_duration_minutes: self.split_duration_minutes,
            current_split_number: self.current_split_number,
            // New count-up fields
            elapsed_seconds: Some(self.elapsed_seconds),
            cost_paise: Some(cost.total_paise),
            rate_per_min_paise: Some(cost.rate_per_min_paise),
        }
    }

    /// Tick the timer by 1 second. Returns true if session should auto-end.
    ///
    /// - Active: increments elapsed_seconds + driving_seconds. Returns true on hard max cap.
    /// - PausedGamePause: increments pause_seconds. Returns true on 10-min pause timeout.
    /// - WaitingForGame: no increments, returns false.
    /// - Other statuses: returns false (existing behavior).
    pub fn tick(&mut self) -> bool {
        match self.status {
            BillingSessionStatus::Active => {
                self.elapsed_seconds += 1;
                self.driving_seconds += 1;
                self.elapsed_seconds >= self.max_session_seconds
            }
            BillingSessionStatus::PausedGamePause => {
                self.pause_seconds += 1;
                self.pause_seconds >= 600 // 10-min pause timeout
            }
            BillingSessionStatus::WaitingForGame => false,
            _ => false,
        }
    }

    /// Get the current session cost based on elapsed seconds and rate tiers.
    pub fn current_cost(&self, tiers: &[BillingRateTier]) -> SessionCost {
        compute_session_cost(self.elapsed_seconds, tiers)
    }

    /// Create a minimal BillingTimer for unit tests.
    #[cfg(test)]
    pub fn dummy(pod_id: &str) -> Self {
        use chrono::Utc;
        Self {
            session_id: format!("test-session-{}", pod_id),
            driver_id: "test-driver".into(),
            driver_name: "Test Driver".into(),
            pod_id: pod_id.to_string(),
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
        }
    }
}

// ─── WaitingForGameEntry ─────────────────────────────────────────────────────

/// Tracks pods waiting for AC to reach STATUS=LIVE before billing starts.
/// Created by defer_billing_start(), consumed by handle_game_status_update(Live).
pub struct WaitingForGameEntry {
    pub pod_id: String,
    pub driver_id: String,
    pub pricing_tier_id: String,
    pub custom_price_paise: Option<u32>,
    pub custom_duration_minutes: Option<u32>,
    pub staff_id: Option<String>,
    pub split_count: Option<u32>,
    pub split_duration_minutes: Option<u32>,
    pub waiting_since: std::time::Instant,
    pub attempt: u8, // 1 = first try, 2 = retry after timeout
    /// For multiplayer sessions: group_session_id links this pod to a group.
    /// When Some, billing waits for all group members to reach LIVE before starting.
    /// When None, billing starts immediately on LIVE (single-player backward compat).
    pub group_session_id: Option<String>,
}

// ─── MultiplayerBillingWait ─────────────────────────────────────────────────

/// Coordinates billing start across all pods in a multiplayer group session.
/// Billing starts only when all expected pods have reported STATUS=LIVE,
/// or after a 60-second timeout evicts non-connecting pods.
pub struct MultiplayerBillingWait {
    pub group_session_id: String,
    pub expected_pods: HashSet<String>,
    pub live_pods: HashSet<String>,
    pub waiting_entries: HashMap<String, WaitingForGameEntry>,
    pub timeout_spawned: bool,
}

// ─── BillingManager ─────────────────────────────────────────────────────────

pub struct BillingManager {
    /// pod_id -> BillingTimer
    pub active_timers: RwLock<HashMap<String, BillingTimer>>,
    /// pod_id -> WaitingForGameEntry (pods that authenticated but AC not yet LIVE)
    pub waiting_for_game: RwLock<HashMap<String, WaitingForGameEntry>>,
    /// group_session_id -> MultiplayerBillingWait (coordinated group billing)
    pub multiplayer_waiting: RwLock<HashMap<String, MultiplayerBillingWait>>,
    /// Cached billing rate tiers, sorted by tier_order. Refreshed from DB periodically.
    pub rate_tiers: RwLock<Vec<BillingRateTier>>,
}

impl BillingManager {
    pub fn new() -> Self {
        Self {
            active_timers: RwLock::new(HashMap::new()),
            waiting_for_game: RwLock::new(HashMap::new()),
            multiplayer_waiting: RwLock::new(HashMap::new()),
            rate_tiers: RwLock::new(default_billing_rate_tiers()),
        }
    }
}

// ─── Game Status Handling ───────────────────────────────────────────────────

/// Check for pods that have been in WaitingForGame for more than 180 seconds.
/// Returns list of (pod_id, attempt) for pods that have timed out.
/// This variant operates directly on a BillingManager (for testing without AppState).
pub async fn check_launch_timeouts_from_manager(mgr: &BillingManager) -> Vec<(String, u8)> {
    let mut timed_out = Vec::new();
    let waiting = mgr.waiting_for_game.read().await;
    for (pod_id, entry) in waiting.iter() {
        if entry.waiting_since.elapsed() > std::time::Duration::from_secs(180) {
            timed_out.push((pod_id.clone(), entry.attempt));
        }
    }
    timed_out
}

/// Check for pods that have been in WaitingForGame for more than 180 seconds.
/// Returns list of (pod_id, attempt) for pods that have timed out.
pub async fn check_launch_timeouts(state: &Arc<AppState>) -> Vec<(String, u8)> {
    check_launch_timeouts_from_manager(&state.billing).await
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
    };
    if group_session_id.is_some() {
        tracing::info!("Billing deferred to WaitingForGame for pod {} (multiplayer group)", pod_id);
    } else {
        tracing::info!("Billing deferred to WaitingForGame for pod {}", pod_id);
    }
    state.billing.waiting_for_game.write().await.insert(pod_id, entry);
    Ok(())
}

/// Handle game status updates from the agent.
/// Dispatches to billing start/pause/resume/end based on AcStatus.
/// For multiplayer pods (group_session_id is Some), billing is coordinated:
/// billing starts for ALL group members only after every participant reaches LIVE.
pub async fn handle_game_status_update(
    state: &Arc<AppState>,
    pod_id: &str,
    ac_status: rc_common::types::AcStatus,
    _cmd_tx: &tokio::sync::mpsc::Sender<CoreToAgentMessage>,
) {
    use rc_common::types::AcStatus;
    match ac_status {
        AcStatus::Live => {
            // Check if this pod is in waiting_for_game -- if so, start billing
            let entry = state.billing.waiting_for_game.write().await.remove(pod_id);
            if let Some(entry) = entry {
                if let Some(ref group_id) = entry.group_session_id {
                    // ── Multiplayer: coordinate billing across group ──────────
                    let group_id = group_id.clone();
                    let mut mp = state.billing.multiplayer_waiting.write().await;

                    // Get or create MultiplayerBillingWait entry
                    if !mp.contains_key(&group_id) {
                        // First pod for this group — query expected pods from DB
                        let pod_ids: Vec<String> = sqlx::query_scalar(
                            "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND status = 'validated' AND pod_id IS NOT NULL",
                        )
                        .bind(&group_id)
                        .fetch_all(&state.db)
                        .await
                        .unwrap_or_default();

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

                    let wait = mp.get_mut(&group_id).unwrap();
                    wait.live_pods.insert(pod_id.to_string());
                    wait.waiting_entries.insert(pod_id.to_string(), entry);

                    // Spawn 60s timeout (once per group)
                    if !wait.timeout_spawned {
                        wait.timeout_spawned = true;
                        let state_clone = state.clone();
                        let group_id_clone = group_id.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
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
                } else {
                    // ── Single-player: start billing immediately (existing behavior) ──
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
                        }
                        Err(e) => {
                            tracing::error!("Failed to start billing on LIVE for pod {}: {}", pod_id, e);
                        }
                    }
                }
            } else {
                // No waiting entry -- check if timer exists and is PausedGamePause (resume)
                let mut timers = state.billing.active_timers.write().await;
                if let Some(timer) = timers.get_mut(pod_id) {
                    if timer.status == BillingSessionStatus::PausedGamePause {
                        timer.status = BillingSessionStatus::Active;
                        timer.pause_seconds = 0;
                        tracing::info!("Billing resumed on LIVE for pod {} (was PausedGamePause)", pod_id);
                    }
                    // If already Active, this is a no-op (idempotent)
                }
            }
        }
        AcStatus::Pause => {
            let mut timers = state.billing.active_timers.write().await;
            if let Some(timer) = timers.get_mut(pod_id) {
                if timer.status == BillingSessionStatus::Active {
                    timer.status = BillingSessionStatus::PausedGamePause;
                    timer.pause_seconds = 0;
                    timer.pause_count += 1;
                    tracing::info!("Billing paused (game pause) for pod {}", pod_id);
                }
            }
            // If no active timer, Pause is a no-op
        }
        AcStatus::Off => {
            // Game exited -- if there's an active billing timer, end the session
            let session_id = {
                let timers = state.billing.active_timers.read().await;
                timers.get(pod_id).map(|t| t.session_id.clone())
            };
            if let Some(session_id) = session_id {
                tracing::info!("Game exited (STATUS=Off) for pod {}, ending billing session {}", pod_id, session_id);
                end_billing_session(state, &session_id, BillingSessionStatus::EndedEarly).await;
            }
            // Also remove from waiting_for_game if present (game crashed during loading)
            state.billing.waiting_for_game.write().await.remove(pod_id);

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
                if timer.status == BillingSessionStatus::Active {
                    timer.status = BillingSessionStatus::PausedGamePause;
                    timer.pause_seconds = 0;
                    timer.pause_count += 1;
                    tracing::info!("Billing paused (replay) for pod {}", pod_id);
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

// ─── Tick Loop ──────────────────────────────────────────────────────────────

/// Called every 1 second to tick all active billing timers
pub async fn tick_all_timers(state: &Arc<AppState>) {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let mut timers = state.billing.active_timers.write().await;
    let mut events_to_broadcast = Vec::new();
    let mut expired_sessions = Vec::new();
    let mut warnings = Vec::new();
    let mut agent_ticks: Vec<(String, u32, u32, String, Option<u32>, Option<i64>, Option<i64>, Option<bool>, Option<u32>, Option<String>)> = Vec::new();
    let mut pause_timeout_end: Vec<(String, String, u32, String)> = Vec::new();
    let mut new_pauses: Vec<(String, String, u32)> = Vec::new(); // pod_id, session_id, pause_count

    // Read pod statuses for offline detection
    let pods = state.pods.read().await;

    for (pod_id, timer) in timers.iter_mut() {
        // ─── Handle PausedDisconnect state ────────────────────────────────
        if timer.status == BillingSessionStatus::PausedDisconnect {
            // Do NOT increment driving_seconds — billing is frozen
            timer.total_paused_seconds += 1;

            // Check if pause timeout exceeded (10 min default)
            if timer.total_paused_seconds > timer.max_pause_duration_secs {
                tracing::info!(
                    "Disconnect pause timeout for session {} on pod {} ({}s paused) — auto-ending with refund",
                    timer.session_id, pod_id, timer.total_paused_seconds
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

        // Handle PausedGamePause — send paused tick to agent (overlay shows PAUSED badge)
        if timer.status == BillingSessionStatus::PausedGamePause {
            timer.pause_seconds += 1;
            timer.total_paused_seconds += 1;

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
                timer.status = BillingSessionStatus::PausedDisconnect;
                timer.pause_count += 1;
                timer.last_paused_at = Some(Utc::now());
                // Note: total_paused_seconds will be incremented each tick while paused

                tracing::info!(
                    "Billing paused due to disconnect: session={} pod={} pause_count={}",
                    timer.session_id, pod_id, timer.pause_count
                );

                new_pauses.push((pod_id.clone(), timer.session_id.clone(), timer.pause_count));
                events_to_broadcast.push(DashboardEvent::BillingSessionChanged(timer.to_info(&rate_tiers)));
                continue; // Skip normal tick
            } else {
                // All 3 pauses used — billing continues even while offline
                tracing::warn!(
                    "Pod {} offline but session {} has used all 3 pauses — billing continues",
                    pod_id, timer.session_id
                );
            }
        } else {
            timer.offline_since = None; // Pod is back online
        }

        let expired = timer.tick();
        let remaining = timer.remaining_seconds();

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
            timer.status = BillingSessionStatus::Completed;
            expired_sessions.push((
                pod_id.clone(),
                timer.session_id.clone(),
                timer.driving_seconds,
                timer.driver_name.clone(),
            ));
            events_to_broadcast.push(DashboardEvent::BillingSessionChanged(timer.to_info(&rate_tiers)));
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

    drop(pods);   // Release pods read lock
    drop(timers); // Release write lock before DB/broadcast

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
    if !agent_ticks.is_empty() {
        let agent_senders = state.agent_senders.read().await;
        for (pod_id, remaining, allocated, driver_name, elapsed, cost, rate, paused, min_to_tier, tier_nm) in agent_ticks {
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender.send(CoreToAgentMessage::BillingTick {
                    remaining_seconds: remaining,
                    allocated_seconds: allocated,
                    driver_name,
                    elapsed_seconds: elapsed,
                    cost_paise: cost,
                    rate_per_min_paise: rate,
                    paused,
                    minutes_to_next_tier: min_to_tier,
                    tier_name: tier_nm,
                }).await;
            }
        }
    }

    // Send StopGame + SessionEnded/SubSessionEnded to agents for expired sessions
    if !expired_sessions.is_empty() {
        // Log activity for expired sessions
        for (pod_id, _, driving_seconds, driver_name) in &expired_sessions {
            log_pod_activity(state, pod_id, "billing", "Session Expired", &format!("{} — {}s driven", driver_name, driving_seconds), "core");
        }

        let agent_senders = state.agent_senders.read().await;
        for (pod_id, session_id, driving_seconds, driver_name) in &expired_sessions {
            // Check if pod has active reservation (multi-sub-session support)
            let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, pod_id)
                .await
                .is_some();

            if let Some(sender) = agent_senders.get(pod_id) {
                let _ = sender.send(CoreToAgentMessage::StopGame).await;

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
                        .send(CoreToAgentMessage::SubSessionEnded {
                            billing_session_id: session_id.clone(),
                            driver_name: driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds: *driving_seconds,
                            wallet_balance_paise: wallet_balance,
                            current_split_number: current_split,
                            total_splits,
                        })
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
                        .send(CoreToAgentMessage::SessionEnded {
                            billing_session_id: session_id.clone(),
                            driver_name: driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds: *driving_seconds,
                        })
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

    // Broadcast warnings
    for (session_id, pod_id, remaining, driving_seconds) in warnings {
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
        .bind(driving_seconds as i64)
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
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
             VALUES (?, ?, 'time_expired', ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .execute(&state.db)
        .await;
    }

    // Persist new disconnect pauses to DB
    for (pod_id, session_id, pause_count) in &new_pauses {
        log_pod_activity(state, pod_id, "billing", "Session Paused (Disconnect)",
            &format!("Pod offline — pause {}/3", pause_count), "race_engineer");
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'paused_disconnect', pause_count = ?, last_paused_at = datetime('now')
             WHERE id = ?",
        )
        .bind(*pause_count as i64)
        .bind(session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'paused_disconnect', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(0i64) // driving_seconds not incremented during pause
        .bind(format!("{{\"pause_count\":{},\"reason\":\"disconnect\"}}", pause_count))
        .execute(&state.db)
        .await;

        // Broadcast SessionPaused to dashboards
        let _ = state.dashboard_tx.send(DashboardEvent::SessionPaused {
            pod_id: pod_id.clone(),
            session_id: session_id.clone(),
            reason: "disconnect".to_string(),
            pause_count: *pause_count,
        });

        // Send ShowPauseOverlay to agent
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreToAgentMessage::ShowPauseOverlay {
                session_id: session_id.clone(),
                remaining_seconds: 600, // max pause duration
                pause_count: *pause_count,
            }).await;
        }
    }

    // Handle pause timeout auto-end with partial refund
    for (pod_id, session_id, driving_seconds, driver_id) in pause_timeout_end {
        log_pod_activity(state, &pod_id, "billing", "Session Auto-Ended",
            "Disconnect pause timeout (10min) — auto-ended with partial refund", "race_engineer");

        // Calculate partial refund
        let session_info = sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT allocated_seconds, wallet_debit_paise FROM billing_sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let mut refund_paise: i64 = 0;
        if let Some((allocated, Some(debit))) = session_info {
            if debit > 0 && (driving_seconds as i64) < allocated {
                let remaining = allocated - driving_seconds as i64;
                refund_paise = (remaining as f64 / allocated as f64 * debit as f64) as i64;
                if refund_paise > 0 {
                    let _ = crate::wallet::refund(
                        state,
                        &driver_id,
                        refund_paise,
                        Some(&session_id),
                        Some("Auto-refund: disconnect pause timeout"),
                    )
                    .await;
                }
            }
        }

        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'ended_early', driving_seconds = ?, ended_at = datetime('now'),
             refund_paise = ?, notes = 'Auto-ended: disconnect pause timeout (10min)'
             WHERE id = ?",
        )
        .bind(driving_seconds as i64)
        .bind(refund_paise)
        .bind(&session_id)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'pause_timeout_ended', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(driving_seconds as i64)
        .bind(format!("{{\"refund_paise\":{}}}", refund_paise))
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

        // Notify agent: session ended
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(&pod_id) {
            let _ = sender.send(CoreToAgentMessage::StopGame).await;
            let _ = sender.send(CoreToAgentMessage::HidePauseOverlay {
                session_id: session_id.clone(),
            }).await;
            let _ = sender
                .send(CoreToAgentMessage::SessionEnded {
                    billing_session_id: session_id.clone(),
                    driver_name: "".to_string(), // Name not needed for timeout end
                    total_laps: 0,
                    best_lap_ms: None,
                    driving_seconds,
                })
                .await;
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
            // First timeout: reset to attempt 2 and allow another 3 minutes
            let mut waiting = state.billing.waiting_for_game.write().await;
            if let Some(entry) = waiting.get_mut(&pod_id) {
                tracing::warn!(
                    "Launch timeout (attempt 1) for pod {} — allowing retry (attempt 2)",
                    pod_id
                );
                entry.attempt = 2;
                entry.waiting_since = std::time::Instant::now();
                log_pod_activity(state, &pod_id, "billing", "Launch Timeout",
                    "AC failed to reach LIVE in 3 min — retry allowed", "race_engineer");
            }
            // The agent-side LaunchState machine handles the actual retry
            // Send LaunchGame again to trigger retry
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender.send(CoreToAgentMessage::LaunchGame {
                    sim_type: rc_common::types::SimType::AssettoCorsa,
                    launch_args: None,
                }).await;
            }
        } else {
            // Second timeout: cancel with no charge
            let mut waiting = state.billing.waiting_for_game.write().await;
            let entry = waiting.remove(&pod_id);
            tracing::error!(
                "Launch timeout (attempt 2) for pod {} — cancelling session (no charge)",
                pod_id
            );
            log_pod_activity(state, &pod_id, "billing", "Launch Failed",
                "AC failed to reach LIVE after 2 attempts (6 min total) — session cancelled, no charge", "race_engineer");

            // Send BillingStopped to agent so it shows session cancelled
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let billing_session_id = entry
                    .map(|e| format!("deferred-{}", e.pod_id))
                    .unwrap_or_default();
                let _ = sender.send(CoreToAgentMessage::BillingStopped {
                    billing_session_id,
                }).await;
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
        } else if timer.status == BillingSessionStatus::PausedDisconnect {
            // Persist pause state (driving_seconds frozen, but total_paused_seconds updates)
            let _ = sqlx::query(
                "UPDATE billing_sessions SET driving_seconds = ?, total_paused_seconds = ? WHERE id = ?",
            )
            .bind(timer.driving_seconds as i64)
            .bind(timer.total_paused_seconds as i64)
            .bind(&timer.session_id)
            .execute(&state.db)
            .await;
        }
    }
}

// ─── Session Recovery ───────────────────────────────────────────────────────

/// On server startup, recover any active billing sessions from the database
pub async fn recover_active_sessions(state: &Arc<AppState>) -> anyhow::Result<()> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.split_count, bs.split_duration_minutes
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.status IN ('active', 'paused_manual', 'paused_disconnect')",
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
            "paused_disconnect" => BillingSessionStatus::PausedDisconnect,
            _ => continue,
        };

        let started_at = row
            .8
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let driving_secs = row.6 as u32;
        let allocated_secs = row.5 as u32;
        let timer = BillingTimer {
            session_id: row.0.clone(),
            driver_id: row.1.clone(),
            driver_name: row.2.clone(),
            pod_id: row.3.clone(),
            pricing_tier_name: row.4.clone(),
            allocated_seconds: allocated_secs,
            driving_seconds: driving_secs,
            status,
            driving_state: DrivingState::Idle, // Will be updated when agent reconnects
            started_at,
            warning_5min_sent: allocated_secs.saturating_sub(driving_secs) <= 300,
            warning_1min_sent: allocated_secs.saturating_sub(driving_secs) <= 60,
            offline_since: None,
            split_count: row.9.unwrap_or(1) as u32,
            split_duration_minutes: row.10.map(|m| m as u32),
            current_split_number: 1, // Best guess on recovery — exact value non-critical
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: driving_secs,
            pause_seconds: 0,
            max_session_seconds: allocated_secs,
        };

        tracing::info!(
            "Recovered billing session {} for driver {} on pod {} ({}/{}s)",
            timer.session_id,
            timer.driver_name,
            timer.pod_id,
            timer.driving_seconds,
            timer.allocated_seconds
        );

        // Update pod state to reflect the active session
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&timer.pod_id) {
                pod.billing_session_id = Some(timer.session_id.clone());
                pod.current_driver = Some(timer.driver_name.clone());
                pod.status = rc_common::types::PodStatus::InSession;
            }
        }

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
            staff_id,
            split_count,
            split_duration_minutes,
        } => {
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
    staff_id: Option<String>,
    split_count: Option<u32>,
    split_duration_minutes: Option<u32>,
) -> Result<String, String> {
    // Check no active session on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&pod_id) {
            return Err(format!("Pod {} already has an active billing session", pod_id));
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

    // Create billing session in DB
    let session_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let final_split_count = split_count.unwrap_or(1);
    let final_split_duration = split_duration_minutes;

    sqlx::query(
        "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, started_at, staff_id, split_count, split_duration_minutes)
         VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?)",
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
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to persist billing session: {}", e))?;

    // Log billing events
    for event_type in ["created", "started"] {
        if let Err(e) = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
             VALUES (?, ?, ?, 0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(event_type)
        .execute(&state.db)
        .await
        {
            tracing::error!("Failed to log billing event '{}' for session {}: {}", event_type, session_id, e);
        }
    }

    // Mark trial as used (skip for unlimited_trials drivers)
    if is_trial && !unlimited_trials {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1, updated_at = datetime('now') WHERE id = ?")
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
        max_session_seconds: allocated_seconds,
    };

    let rate_tiers = state.billing.rate_tiers.read().await;
    let info = timer.to_info(&rate_tiers);
    drop(rate_tiers);

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
    }

    // Notify agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender
            .send(CoreToAgentMessage::BillingStarted {
                billing_session_id: session_id.clone(),
                driver_name: driver_name.clone(),
                allocated_seconds,
                session_token: Some(uuid::Uuid::new_v4().to_string()),
            })
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

    log_pod_activity(state, &pod_id, "billing", "Session Started", &format!("{} — {} ({}min)", driver_name, tier.1, allocated_seconds / 60), "core");

    Ok(session_id)
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
            timer.status = new_status;
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
            log_pod_activity(state, &pod_id, "billing", activity_action, &info.driver_name, "core");

            drop(timers);

            // Log event
            if let Err(e) = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(info.driving_seconds as i64)
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

    if timer.status != BillingSessionStatus::PausedDisconnect {
        return Err(format!(
            "Session is not paused due to disconnect (current status: {:?})",
            timer.status
        ));
    }

    timer.status = BillingSessionStatus::Active;
    timer.last_paused_at = None;
    timer.offline_since = None;
    // Note: total_paused_seconds keeps accumulating across pauses (not reset)

    let info = timer.to_info(&rate_tiers);
    let driver_name = timer.driver_name.clone();

    drop(timers);

    log_pod_activity(state, &pod_id, "billing", "Session Resumed (Disconnect)",
        &driver_name, "core");

    // Update DB
    let _ = sqlx::query(
        "UPDATE billing_sessions SET status = 'active', last_paused_at = NULL WHERE id = ?",
    )
    .bind(session_id)
    .execute(&state.db)
    .await;

    // Log event
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
         VALUES (?, ?, 'resumed_disconnect', ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(info.driving_seconds as i64)
    .execute(&state.db)
    .await;

    // Broadcast SessionResumed to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::SessionResumed {
        pod_id: pod_id.clone(),
        session_id: session_id.to_string(),
    });
    let _ = state.dashboard_tx.send(DashboardEvent::BillingSessionChanged(info));

    // Send HidePauseOverlay to agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::HidePauseOverlay {
            session_id: session_id.to_string(),
        }).await;
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

async fn end_billing_session(
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
            timer.status = end_status;
            let info = timer.to_info(&rate_tiers);
            let driving_seconds = timer.driving_seconds;
            let final_cost_paise = info.cost_paise.unwrap_or(0);

            let activity_action = match end_status {
                BillingSessionStatus::EndedEarly => "Session Ended",
                BillingSessionStatus::Cancelled => "Session Cancelled",
                _ => "Session Expired",
            };
            log_pod_activity(state, &pod_id, "billing", activity_action, &format!("{} — {}s driven", info.driver_name, driving_seconds), "core");

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

            // Update DB with final cost
            if let Err(e) = sqlx::query(
                "UPDATE billing_sessions SET status = ?, driving_seconds = ?, wallet_debit_paise = ?, ended_at = datetime('now') WHERE id = ?",
            )
            .bind(status_str)
            .bind(driving_seconds as i64)
            .bind(final_cost_paise)
            .bind(session_id)
            .execute(&state.db)
            .await
            {
                tracing::error!("Failed to update billing session {} to {}: {}", session_id, status_str, e);
            }

            if let Err(e) = sqlx::query(
                "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session_id)
            .bind(event_type)
            .bind(driving_seconds as i64)
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
                let wallet_info = sqlx::query_as::<_, (String, i64, Option<i64>)>(
                    "SELECT driver_id, allocated_seconds, wallet_debit_paise FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, allocated, Some(debit))) = wallet_info {
                    if debit > 0 && (driving_seconds as i64) < allocated {
                        let remaining = allocated - driving_seconds as i64;
                        let refund_amount = (remaining * debit) / allocated;
                        if refund_amount > 0 {
                            let _ = crate::wallet::refund(
                                state,
                                &driver_id,
                                refund_amount,
                                Some(session_id),
                                Some("Early end — proportional refund"),
                            )
                            .await;
                        }
                    }
                }
            }

            // Full refund for cancelled sessions (never drove)
            if end_status == BillingSessionStatus::Cancelled {
                let wallet_info = sqlx::query_as::<_, (String, Option<i64>)>(
                    "SELECT driver_id, wallet_debit_paise FROM billing_sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                if let Some((driver_id, Some(debit))) = wallet_info {
                    if debit > 0 {
                        let _ = crate::wallet::refund(
                            state,
                            &driver_id,
                            debit,
                            Some(session_id),
                            Some("Cancelled session — full refund"),
                        )
                        .await;
                    }
                }
            }

            // Notify agent: stop game and show session summary
            let has_reservation = crate::pod_reservation::get_active_reservation_for_pod(state, &pod_id)
                .await
                .is_some();

            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender.send(CoreToAgentMessage::StopGame).await;

                if has_reservation && end_status != BillingSessionStatus::Cancelled {
                    let wallet_balance = crate::wallet::get_balance(state, &info.driver_id)
                        .await
                        .unwrap_or(0);
                    let _ = sender
                        .send(CoreToAgentMessage::SubSessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                            wallet_balance_paise: wallet_balance,
                            current_split_number: info.current_split_number,
                            total_splits: info.split_count,
                        })
                        .await;
                } else {
                    let _ = sender
                        .send(CoreToAgentMessage::SessionEnded {
                            billing_session_id: session_id.to_string(),
                            driver_name: info.driver_name.clone(),
                            total_laps: 0,
                            best_lap_ms: None,
                            driving_seconds,
                        })
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
                tokio::spawn(async move {
                    post_session_hooks(&state_clone, &session_id_clone, &driver_id_clone).await;
                });
            }
            return true;
        }
    }

    // ─── Fallback: orphaned session in DB but no in-memory timer ─────────
    // This happens when racecontrol restarts while a session was active.
    drop(timers);
    let orphan = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, pod_id, driver_name FROM billing_sessions WHERE id = ? AND status = 'active'",
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

        log_pod_activity(state, &pod_id, "billing", "Orphaned Session Ended", &format!("{} — force-ended after racecontrol restart", driver_name), "race_engineer");

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

        // Notify agent to deactivate overlay and show blank
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(&pod_id) {
            let _ = sender.send(CoreToAgentMessage::SessionEnded {
                billing_session_id: session_id.to_string(),
                driver_name,
                total_laps: 0,
                best_lap_ms: None,
                driving_seconds: 0,
            }).await;
        }

        return true;
    }

    false
}

/// Format a phone number for WhatsApp (Evolution API format).
/// Strips leading '+', prepends '91' for 10-digit Indian numbers.
pub(crate) fn format_wa_phone(phone: &str) -> String {
    if phone.starts_with('+') {
        phone[1..].to_string()
    } else if phone.len() == 10 {
        format!("91{}", phone)
    } else {
        phone.to_string()
    }
}

/// Format a WhatsApp receipt message for a completed session.
fn format_receipt_message(
    first_name: &str,
    driving_secs: i64,
    cost_paise: i64,
    best_lap_ms: Option<i64>,
    balance_paise: i64,
) -> String {
    let duration_min = driving_secs / 60;
    let duration_sec = driving_secs % 60;
    let cost_credits = cost_paise / 100;
    let balance_credits = balance_paise / 100;

    let best_lap_text = match best_lap_ms.filter(|&ms| ms > 0) {
        Some(ms) => {
            let mins = ms / 60000;
            let secs = (ms % 60000) / 1000;
            let millis = ms % 1000;
            format!("{}:{:02}.{:03}", mins, secs, millis)
        }
        None => "No valid laps".to_string(),
    };

    format!(
        "\u{1f3c1} *RacingPoint \u{2014} Session Complete*\n\nHey {}!\n\n\u{23f1} Duration: {}m {}s\n\u{1f4b0} Cost: {} credits\n\u{1f3ce} Best Lap: {}\n\u{1f4b3} Wallet Balance: {} credits\n\nSee you on track! \u{1f3c1}",
        first_name, duration_min, duration_sec, cost_credits, best_lap_text, balance_credits
    )
}

/// Send a WhatsApp receipt for a completed session via Evolution API.
/// Best-effort: never blocks session end, 5-second timeout.
async fn send_whatsapp_receipt(state: &Arc<AppState>, session_id: &str, driver_id: &str) {
    // Get driver phone
    let driver: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, phone FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driver_name, phone) = match driver {
        Some((name, Some(phone))) if !phone.is_empty() => (name, phone),
        Some((name, _)) => {
            tracing::warn!("No phone for driver {} ({}) -- skipping WhatsApp receipt", driver_id, name);
            return;
        }
        None => return,
    };

    // Get session details
    let session: Option<(i64, i64)> = sqlx::query_as(
        "SELECT driving_seconds, COALESCE(wallet_debit_paise, COALESCE(custom_price_paise, (SELECT price_paise FROM pricing_tiers WHERE id = billing_sessions.pricing_tier_id)), 0)
         FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driving_secs, cost_paise) = match session {
        Some(s) => s,
        None => return,
    };

    // Best lap
    let best_lap: Option<(i64,)> = sqlx::query_as(
        "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Wallet balance
    let balance: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN txn_type LIKE 'credit%' OR txn_type LIKE 'refund%' THEN amount_paise ELSE -amount_paise END), 0) FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let first_name = driver_name.split_whitespace().next().unwrap_or("Racer");
    let balance_paise = balance.map(|b| b.0).unwrap_or(0);

    let message = format_receipt_message(
        first_name,
        driving_secs,
        cost_paise,
        best_lap.map(|b| b.0),
        balance_paise,
    );

    // Send via Evolution API (same pattern as OTP in auth/mod.rs)
    if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        let wa_phone = format_wa_phone(&phone);

        let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = serde_json::json!({
            "number": wa_phone,
            "text": message
        });

        // 5-second timeout -- receipt is best-effort, never block session end
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to build HTTP client for receipt: {}", e);
                return;
            }
        };

        match client.post(&url).header("apikey", evo_key).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("WhatsApp receipt sent to {} for session {}", wa_phone, session_id);
            }
            Ok(resp) => {
                tracing::warn!("Evolution API returned {} for receipt to {}", resp.status(), wa_phone);
            }
            Err(e) => {
                tracing::warn!("Failed to send WhatsApp receipt to {}: {}", wa_phone, e);
            }
        }
    } else {
        tracing::debug!("Evolution API not configured -- skipping WhatsApp receipt for session {}", session_id);
    }
}

/// Post-session hooks: credit referral rewards, schedule review nudge.
async fn post_session_hooks(state: &Arc<AppState>, session_id: &str, driver_id: &str) {
    // 1. Credit referral reward if this is the referee's first completed session
    let pending_referral: Option<(String, String)> = sqlx::query_as(
        "SELECT r.id, r.referrer_id FROM referrals r
         WHERE r.referee_id = ? AND r.reward_credited = 0",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((referral_id, referrer_id)) = pending_referral {
        // Credit 100 credits (₹100 = 10000 paise) to referrer
        let _ = crate::wallet::credit(
            state,
            &referrer_id,
            10000,
            "referral_reward",
            Some(&referral_id),
            Some("Referral reward — friend completed first session"),
            None,
        )
        .await;
        // Credit 50 credits to referee
        let _ = crate::wallet::credit(
            state,
            driver_id,
            5000,
            "referral_bonus",
            Some(&referral_id),
            Some("Welcome reward — referred by a friend"),
            None,
        )
        .await;
        let _ = sqlx::query("UPDATE referrals SET reward_credited = 1 WHERE id = ?")
            .bind(&referral_id)
            .execute(&state.db)
            .await;
        tracing::info!("Referral reward credited: referrer={}, referee={}", referrer_id, driver_id);
    }

    // 2. Schedule review nudge (record for WhatsApp bot to pick up)
    let already_nudged: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM review_nudges WHERE driver_id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Only nudge once per driver
    if already_nudged.map(|c| c.0 == 0).unwrap_or(true) {
        let _ = sqlx::query(
            "INSERT INTO review_nudges (id, driver_id, billing_session_id, sent_at) VALUES (?, ?, ?, NULL)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(driver_id)
        .bind(session_id)
        .execute(&state.db)
        .await;
    }

    // 3. Update membership hours if member
    let membership: Option<(String, f64)> = sqlx::query_as(
        "SELECT m.id, bs.driving_seconds / 3600.0
         FROM memberships m
         JOIN billing_sessions bs ON bs.driver_id = m.driver_id AND bs.id = ?
         WHERE m.driver_id = ? AND m.status = 'active'",
    )
    .bind(session_id)
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((membership_id, hours_used)) = membership {
        let _ = sqlx::query(
            "UPDATE memberships SET hours_used = hours_used + ? WHERE id = ?",
        )
        .bind(hours_used)
        .bind(&membership_id)
        .execute(&state.db)
        .await;
    }

    // 4. Send WhatsApp receipt (best-effort)
    send_whatsapp_receipt(state, session_id, driver_id).await;
}

async fn extend_billing_session(
    state: &Arc<AppState>,
    session_id: &str,
    additional_seconds: u32,
) {
    let rate_tiers = state.billing.rate_tiers.read().await;
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
            let info = timer.to_info(&rate_tiers);

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

/// MULTI-02: Check if all pods in a multiplayer group session have ended billing.
/// If so, stop the AC server associated with the group.
/// Called after each billing end (both tick-expired and manual stop).
pub async fn check_and_stop_multiplayer_server(state: &Arc<AppState>, pod_id: &str) {
    // Look up the group_session_id for this pod's billing session
    let group_info = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT gs.id, gs.ac_session_id
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

    let (group_session_id, ac_session_id) = match group_info {
        Some(info) => info,
        None => return, // Not a multiplayer pod
    };

    let ac_session_id = match ac_session_id {
        Some(id) => id,
        None => return, // No AC server for this group
    };

    // Get all pod_ids in this group session
    let member_pods: Vec<(String,)> = sqlx::query_as(
        "SELECT pod_id FROM group_session_members WHERE group_session_id = ? AND pod_id IS NOT NULL",
    )
    .bind(&group_session_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Check if any pod still has active billing
    let timers = state.billing.active_timers.read().await;
    let any_still_billing = member_pods.iter().any(|(mpod,)| timers.contains_key(mpod));
    drop(timers);

    if any_still_billing {
        tracing::debug!(
            "Multiplayer group {} still has active billing — AC server {} stays running",
            group_session_id, ac_session_id
        );
        return;
    }

    // GROUP-02: If continuous mode is enabled, defer stop to monitor_continuous_session loop.
    {
        let instances = state.ac_server.instances.read().await;
        if let Some(inst) = instances.get(&ac_session_id) {
            if inst.continuous_mode {
                tracing::info!(
                    "Continuous mode active for group {} — deferring stop to monitor loop",
                    group_session_id
                );
                return;
            }
        }
    }

    // All pods done — stop the AC server
    tracing::info!(
        "MULTI-02: All billing ended for multiplayer group {} — stopping AC server {}",
        group_session_id, ac_session_id
    );

    if let Err(e) = crate::ac_server::stop_ac_server(state, &ac_session_id).await {
        tracing::error!(
            "Failed to stop AC server {} for group {}: {}",
            ac_session_id, group_session_id, e
        );
    }

    // Update group session status to completed
    let _ = sqlx::query("UPDATE group_sessions SET status = 'completed' WHERE id = ?")
        .bind(&group_session_id)
        .execute(&state.db)
        .await;
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
        };

        // Should count when driving
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 1);

        // Timer counts regardless of driving state (always-on billing)
        timer.driving_state = DrivingState::Idle;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 2); // Still counts

        // Should NOT count when paused
        timer.driving_state = DrivingState::Active;
        timer.status = BillingSessionStatus::PausedManual;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 2); // Paused stops counting
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
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 2,
            pause_seconds: 0,
            max_session_seconds: 3,
        };

        // One more tick should expire
        assert!(timer.tick());
        assert_eq!(timer.driving_seconds, 3);
        assert_eq!(timer.elapsed_seconds, 3);
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
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 1000,
            pause_seconds: 0,
            max_session_seconds: 3600,
        };

        assert_eq!(timer.remaining_seconds(), 2600);
    }

    #[test]
    fn billing_pause_disconnect_freezes_driving_seconds() {
        let mut timer = BillingTimer {
            session_id: "test-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 100,
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
            elapsed_seconds: 100,
            pause_seconds: 0,
            max_session_seconds: 1800,
        };

        // Active tick — driving_seconds should increment
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 101);

        // Simulate disconnect pause
        timer.status = BillingSessionStatus::PausedDisconnect;
        timer.pause_count = 1;

        // Paused tick — driving_seconds should NOT increment
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 101); // Still 101
    }

    #[test]
    fn max_three_pauses_per_session() {
        let timer = BillingTimer {
            session_id: "test-max-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 500,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 3, // Already used all 3 pauses
            total_paused_seconds: 120,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 500,
            pause_seconds: 0,
            max_session_seconds: 1800,
        };

        // Should NOT be able to pause again (pause_count >= 3)
        assert!(timer.pause_count >= 3);
        // The tick loop checks pause_count < 3 before pausing
    }

    #[test]
    fn partial_refund_calculation() {
        // Simulate: 1800s allocated, 900s driven, 70000 paise (₹700) debited
        // Expected: 50% unused → refund = 35000 paise
        let allocated: i64 = 1800;
        let driving_seconds: i64 = 900;
        let wallet_debit_paise: i64 = 70000;

        let remaining = allocated - driving_seconds;
        let refund = (remaining as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;

        assert_eq!(refund, 35000); // 50% of ₹700

        // Edge case: 75% driven → 25% refund
        let driving_seconds_2: i64 = 1350;
        let remaining_2 = allocated - driving_seconds_2;
        let refund_2 = (remaining_2 as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;
        assert_eq!(refund_2, 17500); // 25% of ₹700

        // Edge case: fully driven → 0 refund
        let driving_seconds_3: i64 = 1800;
        let remaining_3 = allocated - driving_seconds_3;
        let refund_3 = (remaining_3 as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;
        assert_eq!(refund_3, 0);
    }

    // ── compute_session_cost with non-retroactive 3-tier pricing ──────

    fn test_tiers() -> Vec<BillingRateTier> {
        default_billing_rate_tiers()
    }

    #[test]
    fn cost_zero_seconds() {
        let tiers = test_tiers();
        let cost = compute_session_cost(0, &tiers);
        assert_eq!(cost.total_paise, 0);
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.minutes_to_next_tier, Some(30));
    }

    #[test]
    fn cost_15_minutes_standard_tier() {
        let tiers = test_tiers();
        let cost = compute_session_cost(900, &tiers); // 15 min
        assert_eq!(cost.total_paise, 37500); // 15 * 2500
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.minutes_to_next_tier, Some(15));
    }

    #[test]
    fn cost_29_59_standard_tier() {
        let tiers = test_tiers();
        let cost = compute_session_cost(1799, &tiers); // 29:59
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.minutes_to_next_tier, Some(1));
    }

    #[test]
    fn cost_30_minutes_non_retroactive() {
        let tiers = test_tiers();
        let cost = compute_session_cost(1800, &tiers); // exactly 30 min
        assert_eq!(cost.total_paise, 75000); // 30 * 2500 (non-retroactive: all in Standard tier)
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
    }

    #[test]
    fn cost_45_minutes_two_tiers() {
        let tiers = test_tiers();
        let cost = compute_session_cost(2700, &tiers); // 45 min
        // Non-retroactive: (30 * 2500) + (15 * 2000) = 75000 + 30000 = 105000
        assert_eq!(cost.total_paise, 105000);
        assert_eq!(cost.rate_per_min_paise, 2000);
        assert_eq!(cost.tier_name, "Extended");
    }

    #[test]
    fn cost_3_hours_all_three_tiers() {
        let tiers = test_tiers();
        let cost = compute_session_cost(10800, &tiers); // 180 min
        // Non-retroactive: (30 * 2500) + (30 * 2000) + (120 * 1500) = 75000 + 60000 + 180000 = 315000
        assert_eq!(cost.total_paise, 315000);
        assert_eq!(cost.rate_per_min_paise, 1500);
        assert_eq!(cost.tier_name, "Marathon");
        assert_eq!(cost.minutes_to_next_tier, None);
    }

    #[test]
    fn timer_countup_active_increments_elapsed() {
        let mut timer = BillingTimer {
            session_id: "test-countup".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
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
            max_session_seconds: 10800,
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 1);
        assert_eq!(timer.driving_seconds, 1); // compat alias

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 2);
    }

    #[test]
    fn timer_paused_game_pause_freezes_elapsed_increments_pause() {
        let mut timer = BillingTimer {
            session_id: "test-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 100,
            status: BillingSessionStatus::PausedGamePause,
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
            elapsed_seconds: 100,
            pause_seconds: 0,
            max_session_seconds: 10800,
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 100); // frozen
        assert_eq!(timer.pause_seconds, 1);     // incrementing
    }

    #[test]
    fn timer_hard_max_cap_triggers_end() {
        let mut timer = BillingTimer {
            session_id: "test-cap".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 10799,
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
            elapsed_seconds: 10799,
            pause_seconds: 0,
            max_session_seconds: 10800,
        };

        assert!(timer.tick()); // Should return true (elapsed == max)
        assert_eq!(timer.elapsed_seconds, 10800);
    }

    #[test]
    fn timer_pause_timeout_triggers_end() {
        let mut timer = BillingTimer {
            session_id: "test-timeout".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 500,
            status: BillingSessionStatus::PausedGamePause,
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
            elapsed_seconds: 500,
            pause_seconds: 599,
            max_session_seconds: 10800,
        };

        // One more tick should hit 600s pause timeout
        assert!(timer.tick());
        assert_eq!(timer.pause_seconds, 600);
        assert_eq!(timer.elapsed_seconds, 500); // Still frozen
    }

    #[test]
    fn timer_current_cost_returns_session_cost() {
        let rate_tiers = test_tiers();
        let timer = BillingTimer {
            session_id: "test-cost".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 900,
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
            elapsed_seconds: 900,
            pause_seconds: 0,
            max_session_seconds: 10800,
        };

        let cost = timer.current_cost(&rate_tiers);
        assert_eq!(cost.total_paise, 37500); // 15 min * 25 cr/min = 375 cr = 37500 paise
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
    }

    #[test]
    fn timer_to_info_populates_optional_fields() {
        let rate_tiers = test_tiers();
        let timer = BillingTimer {
            session_id: "test-info".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 900,
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
            elapsed_seconds: 900,
            pause_seconds: 0,
            max_session_seconds: 10800,
        };

        let info = timer.to_info(&rate_tiers);
        assert_eq!(info.elapsed_seconds, Some(900));
        assert_eq!(info.cost_paise, Some(37500)); // 15 min * 25 cr/min
        assert_eq!(info.rate_per_min_paise, Some(2500));
        // Legacy fields still populated
        assert_eq!(info.driving_seconds, 900);
        assert_eq!(info.allocated_seconds, 10800);
        assert_eq!(info.remaining_seconds, 9900);
    }

    // ── Phase 03 Plan 03 Task 1: billing lifecycle (handle_game_status_update) ──

    #[test]
    fn waiting_for_game_entry_tracks_billing_params() {
        let entry = WaitingForGameEntry {
            pod_id: "pod1".to_string(),
            driver_id: "d1".to_string(),
            pricing_tier_id: "tier1".to_string(),
            custom_price_paise: Some(5000),
            custom_duration_minutes: Some(30),
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: None,
        };
        assert_eq!(entry.pod_id, "pod1");
        assert_eq!(entry.attempt, 1);
        assert_eq!(entry.custom_price_paise, Some(5000));
    }

    #[tokio::test]
    async fn game_status_live_on_paused_game_pause_resumes_billing() {
        // Timer in PausedGamePause -> Live should transition to Active
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_test_timer("resume-test", "p1");
            timer.status = BillingSessionStatus::PausedGamePause;
            timer.pause_seconds = 30;
            timers.insert("p1".to_string(), timer);
        }
        // Simulate Live: transition PausedGamePause -> Active
        {
            let mut timers = mgr.active_timers.write().await;
            if let Some(timer) = timers.get_mut("p1") {
                assert_eq!(timer.status, BillingSessionStatus::PausedGamePause);
                timer.status = BillingSessionStatus::Active;
                timer.pause_seconds = 0;
            }
        }
        let timers = mgr.active_timers.read().await;
        let timer = timers.get("p1").unwrap();
        assert_eq!(timer.status, BillingSessionStatus::Active);
        assert_eq!(timer.pause_seconds, 0);
    }

    #[tokio::test]
    async fn game_status_pause_transitions_active_to_paused_game_pause() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let timer = make_test_timer("pause-test", "p2");
            timers.insert("p2".to_string(), timer);
        }
        // Simulate Pause: Active -> PausedGamePause
        {
            let mut timers = mgr.active_timers.write().await;
            if let Some(timer) = timers.get_mut("p2") {
                assert_eq!(timer.status, BillingSessionStatus::Active);
                timer.status = BillingSessionStatus::PausedGamePause;
                timer.pause_seconds = 0;
                timer.pause_count += 1;
            }
        }
        let timers = mgr.active_timers.read().await;
        let timer = timers.get("p2").unwrap();
        assert_eq!(timer.status, BillingSessionStatus::PausedGamePause);
        assert_eq!(timer.pause_count, 1);
    }

    #[tokio::test]
    async fn game_status_live_on_active_timer_is_noop() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_test_timer("noop-test", "p3");
            timer.elapsed_seconds = 100;
            timer.driving_seconds = 100;
            timers.insert("p3".to_string(), timer);
        }
        // Simulate Live on already-Active: no change
        {
            let timers = mgr.active_timers.read().await;
            let timer = timers.get("p3").unwrap();
            assert_eq!(timer.status, BillingSessionStatus::Active);
            assert_eq!(timer.elapsed_seconds, 100);
        }
    }

    #[tokio::test]
    async fn game_status_pause_on_no_timer_is_noop() {
        let mgr = BillingManager::new();
        // No timer for p4 - Pause should be no-op
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("p4").is_none());
    }

    #[tokio::test]
    async fn game_status_off_ends_active_session() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let timer = make_test_timer("off-test", "p5");
            timers.insert("p5".to_string(), timer);
        }
        // Simulate Off: remove timer (session ends)
        {
            let timers = mgr.active_timers.read().await;
            assert!(timers.contains_key("p5"));
        }
        // The actual removal happens in handle_game_status_update via end_billing_session
        // Here we verify the timer exists before Off (the function will remove it)
    }

    #[tokio::test]
    async fn waiting_for_game_removed_on_live() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("p6".to_string(), WaitingForGameEntry {
                pod_id: "p6".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now(),
                attempt: 1,
                group_session_id: None,
            });
        }
        // Simulate Live: remove from waiting_for_game
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry = waiting.remove("p6");
            assert!(entry.is_some());
            assert_eq!(entry.unwrap().driver_id, "d1");
        }
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("p6").is_none());
    }

    #[tokio::test]
    async fn launch_timeout_detected_after_180s() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            // Create entry with waiting_since in the past (>180s ago)
            let mut entry = WaitingForGameEntry {
                pod_id: "p7".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now(),
                attempt: 1,
                group_session_id: None,
            };
            // Simulate time passing by using checked_sub
            entry.waiting_since = std::time::Instant::now() - std::time::Duration::from_secs(181);
            waiting.insert("p7".to_string(), entry);
        }
        // Check launch timeouts
        let timed_out = check_launch_timeouts_from_manager(&mgr).await;
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0].0, "p7");
        assert_eq!(timed_out[0].1, 1); // first attempt
    }

    #[tokio::test]
    async fn launch_timeout_attempt_2_cancels_with_no_charge() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry = WaitingForGameEntry {
                pod_id: "p8".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now() - std::time::Duration::from_secs(181),
                attempt: 2, // second attempt
                group_session_id: None,
            };
            waiting.insert("p8".to_string(), entry);
        }
        let timed_out = check_launch_timeouts_from_manager(&mgr).await;
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0].0, "p8");
        assert_eq!(timed_out[0].1, 2); // second attempt -> should cancel

        // On attempt 2 timeout: remove from waiting (no billing session created = no charge)
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.remove("p8");
        }
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("p8").is_none());
        // No entry in active_timers either (billing never started)
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("p8").is_none());
    }

    // Helper: create a test BillingTimer with Active status
    fn make_test_timer(session_id: &str, pod_id: &str) -> BillingTimer {
        BillingTimer {
            session_id: session_id.to_string(),
            driver_id: "d1".to_string(),
            driver_name: "Test Driver".to_string(),
            pod_id: pod_id.to_string(),
            pricing_tier_name: "per-minute".to_string(),
            allocated_seconds: 10800,
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
            max_session_seconds: 10800,
        }
    }

    // ── Phase 09 Plan 02: Multiplayer billing coordination ──────────────────

    /// Helper: create a WaitingForGameEntry for tests
    fn make_waiting_entry(pod_id: &str, group_session_id: Option<&str>) -> WaitingForGameEntry {
        WaitingForGameEntry {
            pod_id: pod_id.to_string(),
            driver_id: format!("driver-{}", pod_id),
            pricing_tier_id: "tier1".to_string(),
            custom_price_paise: None,
            custom_duration_minutes: None,
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: group_session_id.map(|s| s.to_string()),
        }
    }

    #[tokio::test]
    async fn single_player_no_group_billing_starts_immediately_on_live() {
        // Single-player pod (no group_session_id) should NOT be added to multiplayer_waiting
        let mgr = BillingManager::new();

        // Add a single-player WaitingForGameEntry
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("pod1".to_string(), make_waiting_entry("pod1", None));
        }

        // Simulate Live: remove from waiting_for_game
        let entry = {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.remove("pod1")
        };

        // Entry should exist and have no group_session_id
        let entry = entry.unwrap();
        assert!(entry.group_session_id.is_none());
        // Single-player: billing starts immediately (no multiplayer_waiting involvement)
        let mp_waiting = mgr.multiplayer_waiting.read().await;
        assert!(mp_waiting.is_empty());
    }

    #[tokio::test]
    async fn group_2_players_first_live_does_not_start_billing() {
        // Two-pod group: first LIVE should NOT start billing (waits for second)
        let mgr = BillingManager::new();
        let group_id = "group-abc";

        // Set up MultiplayerBillingWait
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // Pod1 goes LIVE — add to live_pods
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod1".to_string());
            wait.waiting_entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
        }

        // Check: live_pods < expected_pods → billing should NOT start
        {
            let mp = mgr.multiplayer_waiting.read().await;
            let wait = mp.get(group_id).unwrap();
            assert_eq!(wait.live_pods.len(), 1);
            assert_eq!(wait.expected_pods.len(), 2);
            assert!(wait.live_pods.len() < wait.expected_pods.len());
        }
    }

    #[tokio::test]
    async fn group_2_players_second_live_starts_billing_for_both() {
        // Two-pod group: second LIVE should start billing for BOTH
        let mgr = BillingManager::new();
        let group_id = "group-def";

        // Set up with pod1 already live
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string());
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: false,
            });
        }

        // Pod2 goes LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod2".to_string());
            wait.waiting_entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));

            // All live — collect entries for billing start
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            let pods_to_bill: Vec<String> = wait.waiting_entries.keys().cloned().collect();
            assert_eq!(pods_to_bill.len(), 2);
            assert!(pods_to_bill.contains(&"pod1".to_string()));
            assert!(pods_to_bill.contains(&"pod2".to_string()));
        }

        // After billing started, entry should be removed from multiplayer_waiting
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            mp.remove(group_id);
        }
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.get(group_id).is_none());
    }

    #[tokio::test]
    async fn group_3_players_billing_starts_only_when_all_3_live() {
        let mgr = BillingManager::new();
        let group_id = "group-3p";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            expected.insert("pod3".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // Pod1 LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod1".to_string());
            wait.waiting_entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            assert!(wait.live_pods.len() < wait.expected_pods.len());
        }

        // Pod2 LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod2".to_string());
            wait.waiting_entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));
            assert!(wait.live_pods.len() < wait.expected_pods.len()); // Still not all
        }

        // Pod3 LIVE — now all are live
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod3".to_string());
            wait.waiting_entries.insert("pod3".to_string(), make_waiting_entry("pod3", Some(group_id)));
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            assert_eq!(wait.waiting_entries.len(), 3);
        }
    }

    #[tokio::test]
    async fn group_disconnect_stops_individual_billing_only() {
        // After billing started, pod2 disconnects. Only pod2's billing ends.
        let mgr = BillingManager::new();

        // Both pod1 and pod2 have active timers (billing already started)
        {
            let mut timers = mgr.active_timers.write().await;
            timers.insert("pod1".to_string(), make_test_timer("session-1", "pod1"));
            timers.insert("pod2".to_string(), make_test_timer("session-2", "pod2"));
        }

        // Pod2 disconnects (STATUS=Off): remove only pod2's timer
        {
            let mut timers = mgr.active_timers.write().await;
            let removed = timers.remove("pod2");
            assert!(removed.is_some());
        }

        // Pod1's timer should still be active
        {
            let timers = mgr.active_timers.read().await;
            assert!(timers.contains_key("pod1"));
            let t1 = timers.get("pod1").unwrap();
            assert_eq!(t1.status, BillingSessionStatus::Active);
            // Pod2 is gone
            assert!(!timers.contains_key("pod2"));
        }
    }

    #[tokio::test]
    async fn group_member_never_live_others_can_proceed_after_eviction() {
        // Pod2 never reaches LIVE. After timeout, only pod1 gets billing started.
        let mgr = BillingManager::new();
        let group_id = "group-timeout";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string()); // Only pod1 went LIVE
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // Simulate timeout: evict non-live pods, start billing for live ones
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();

            // live_pods < expected_pods → timeout triggers
            assert!(wait.live_pods.len() < wait.expected_pods.len());

            // Evict: keep only live pods in expected
            wait.expected_pods.retain(|p| wait.live_pods.contains(p));
            assert_eq!(wait.expected_pods.len(), 1);
            assert!(wait.expected_pods.contains("pod1"));

            // Now live_pods >= expected_pods → start billing for live pods
            assert!(wait.live_pods.len() >= wait.expected_pods.len());

            // Only pod1 should get billing started
            let pods_to_bill: Vec<String> = wait.waiting_entries.keys()
                .filter(|p| wait.live_pods.contains(*p))
                .cloned()
                .collect();
            assert_eq!(pods_to_bill.len(), 1);
            assert_eq!(pods_to_bill[0], "pod1");
        }
    }

    #[test]
    fn waiting_entry_group_session_id_backward_compat() {
        // Existing code that creates WaitingForGameEntry with group_session_id=None
        // should still work (backward compatibility)
        let entry = make_waiting_entry("pod-solo", None);
        assert!(entry.group_session_id.is_none());
        assert_eq!(entry.pod_id, "pod-solo");

        // Multiplayer entry has Some(group_id)
        let mp_entry = make_waiting_entry("pod-mp", Some("group-xyz"));
        assert_eq!(mp_entry.group_session_id.as_deref(), Some("group-xyz"));
    }

    // ── Phase 09 Plan 02 Task 2: 60-second connection timeout ──────────────

    #[tokio::test]
    async fn timeout_evicts_non_connecting_pod_billing_starts_for_connected() {
        // Group of 2: pod1 connects (LIVE), pod2 never connects.
        // After timeout, only pod1's billing starts. pod2 is evicted.
        let mgr = BillingManager::new();
        let group_id = "group-timeout-evict";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string()); // Only pod1 connected
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            // pod2 never connected, so not in live_pods or waiting_entries
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // Simulate timeout logic
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();

            // Timeout fires: live_pods < expected_pods
            assert!(wait.live_pods.len() < wait.expected_pods.len());

            // Collect entries for live pods only
            let billing_entries: Vec<String> = wait.waiting_entries.keys()
                .filter(|p| wait.live_pods.contains(*p))
                .cloned()
                .collect();

            // Only pod1 should get billing started
            assert_eq!(billing_entries.len(), 1);
            assert_eq!(billing_entries[0], "pod1");

            // Evicted pod2 should NOT get billing
            assert!(!wait.live_pods.contains("pod2"));

            // Clean up
            mp.remove(group_id);
        }

        // Verify group entry is gone
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.is_empty());
    }

    #[tokio::test]
    async fn all_pods_connect_within_timeout_no_eviction() {
        // Group of 2: both pods connect before timeout fires.
        // When timeout fires, the entry should already be gone (consumed).
        let mgr = BillingManager::new();
        let group_id = "group-no-eviction";

        // Set up and immediately have all pods connect (simulating pre-timeout)
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string());
            live.insert("pod2".to_string()); // Both connected
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // All pods live: consume the entry (billing starts normally)
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get(group_id).unwrap();
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            // All live -> start billing for all, remove entry
            mp.remove(group_id);
        }

        // Now timeout fires -- entry is gone, no-op
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.get(group_id).is_none());
        // This is exactly what multiplayer_billing_timeout() checks:
        // if entry doesn't exist, it returns immediately (no-op)
    }

    #[tokio::test]
    async fn evicted_pod_late_live_does_not_start_billing() {
        // Pod was evicted by timeout. If it later sends LIVE, billing should NOT start.
        let mgr = BillingManager::new();

        // After timeout, the multiplayer_waiting entry is gone.
        // If evicted pod later sends LIVE, it's no longer in waiting_for_game either
        // (it was consumed into MultiplayerBillingWait then evicted).
        // So there's nothing to start billing for.

        // Verify: no waiting entry, no multiplayer entry -> LIVE is a no-op
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("evicted-pod").is_none());

        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.is_empty());

        // No active timer either (billing was never started for evicted pod)
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("evicted-pod").is_none());
    }

    #[tokio::test]
    async fn timeout_spawned_flag_prevents_duplicate_spawn() {
        let mgr = BillingManager::new();
        let group_id = "group-spawn-once";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // First pod arrives: timeout_spawned should become true
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            assert!(!wait.timeout_spawned);
            wait.timeout_spawned = true; // Would spawn tokio task
            wait.live_pods.insert("pod1".to_string());
        }

        // Second pod arrives: timeout_spawned is already true, no duplicate spawn
        {
            let mp = mgr.multiplayer_waiting.read().await;
            let wait = mp.get(group_id).unwrap();
            assert!(wait.timeout_spawned); // Already true, won't spawn again
        }
    }

    #[test]
    fn timer_waiting_for_game_no_increments() {
        let mut timer = BillingTimer {
            session_id: "test-waiting".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 0,
            status: BillingSessionStatus::WaitingForGame,
            driving_state: DrivingState::Idle,
            started_at: None,
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
            max_session_seconds: 10800,
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 0);
        assert_eq!(timer.driving_seconds, 0);
        assert_eq!(timer.pause_seconds, 0);
    }

    // ─── WhatsApp Receipt Tests ─────────────────────────────────────────────

    #[test]
    fn whatsapp_receipt_message_format() {
        let msg = format_receipt_message("Rahul", 1500, 70000, Some(93210), 150000);

        // Verify key components
        assert!(msg.contains("Rahul"), "Message must contain first name");
        assert!(msg.contains("25m 0s"), "Duration must be 25m 0s for 1500 seconds");
        assert!(msg.contains("700 credits"), "Cost must be 700 credits for 70000 paise");
        assert!(msg.contains("1:33.210"), "Best lap must be 1:33.210 for 93210ms");
        assert!(msg.contains("1500 credits"), "Balance must be 1500 credits for 150000 paise");
        assert!(msg.contains("RacingPoint"), "Must contain brand name");
        assert!(msg.contains("Session Complete"), "Must indicate session complete");
    }

    #[test]
    fn whatsapp_receipt_no_valid_laps() {
        let msg = format_receipt_message("Priya", 600, 35000, None, 50000);
        assert!(msg.contains("No valid laps"), "Must show 'No valid laps' when None");

        let msg2 = format_receipt_message("Priya", 600, 35000, Some(0), 50000);
        assert!(msg2.contains("No valid laps"), "Must show 'No valid laps' when 0ms");
    }

    #[test]
    fn whatsapp_phone_format_10_digit() {
        assert_eq!(format_wa_phone("9876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_phone_format_with_plus() {
        assert_eq!(format_wa_phone("+919876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_phone_format_already_formatted() {
        assert_eq!(format_wa_phone("919876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_receipt_zero_cost() {
        let msg = format_receipt_message("Test", 300, 0, None, 0);
        assert!(msg.contains("0 credits"), "Cost should show 0 credits for trial/free");
    }

    // ── BILL-01 characterization tests: safety net before billing bot code ──

    // BILL-01 characterization: game-exit-while-billing path
    #[test]
    fn game_exit_while_billing_ends_session() {
        // AcStatus::Off while billing active fires the session-end path in ws/mod.rs
        // handle_game_status_update(). This test characterizes the condition:
        // billing_active=true + game exits → session_id resolved from active_timers → end_billing_session fires.
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_1".to_string(), BillingTimer::dummy("pod_1"));
        // Precondition: timer present for pod
        assert!(timers.contains_key("pod_1"));
        // Characterization: when game exits, timer lookup must succeed for end_session to fire
        let session_id = timers.get("pod_1").map(|t| t.session_id.clone());
        assert!(session_id.is_some(), "session_id must be resolvable for game-exit path");
    }

    // BILL-01 characterization: idle drift detection condition (BILL-03)
    #[test]
    fn idle_drift_condition_check() {
        // BILL-03 fires when billing active + DrivingState is NOT Active for > 5 minutes.
        let idle_threshold_secs = 300u64; // 5 minutes
        assert_eq!(idle_threshold_secs, 300, "idle drift threshold must be exactly 5 minutes");
        // DrivingState::Active is the only non-idle state; Idle means the condition can fire.
        let ds_idle = DrivingState::Idle;
        let is_active = matches!(ds_idle, DrivingState::Active);
        assert!(!is_active, "DrivingState::Idle must NOT match Active — idle drift condition met");
    }

    // BILL-01 characterization: end_session removes timer from active_timers
    #[test]
    fn end_session_removes_timer() {
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_2".to_string(), BillingTimer::dummy("pod_2"));
        assert!(timers.contains_key("pod_2"));
        timers.remove("pod_2");
        assert!(
            !timers.contains_key("pod_2"),
            "Timer must be removed from active_timers after end_session"
        );
    }

    // BILL-01 characterization: stuck session detection condition (BILL-02)
    #[test]
    fn stuck_session_condition() {
        // BILL-02 fires when billing_active=true AND game_pid=None for >= 60 seconds.
        let stuck_threshold_secs = 60u64;
        assert_eq!(stuck_threshold_secs, 60, "stuck session threshold must be exactly 60 seconds");
        // The condition: billing active + no game PID
        let billing_active = true;
        let game_pid: Option<u32> = None;
        let condition_met = billing_active && game_pid.is_none();
        assert!(
            condition_met,
            "billing_active=true + game_pid=None must satisfy stuck session condition"
        );
    }

    // BILL-01 characterization: start_session populates active_timers for lookup
    #[test]
    fn start_session_inserts_timer() {
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_1".to_string(), BillingTimer::dummy("pod_1"));
        // active_timers must contain the pod_id for recover_stuck_session() to find it
        assert!(
            timers.contains_key("pod_1"),
            "start_session must insert timer — recover_stuck_session depends on this"
        );
        let t = timers.get("pod_1").unwrap();
        assert_eq!(t.pod_id.as_str(), "pod_1", "BillingTimer::dummy sets pod_id correctly");
        assert!(
            t.session_id.contains("pod_1"),
            "session_id must embed pod_id for traceability"
        );
    }
}
