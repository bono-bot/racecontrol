use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use rc_common::types::{BillingSessionInfo, BillingSessionStatus, DrivingState};

// Used by #[cfg(test)] modules via `use super::*`
#[cfg(test)]
use crate::state::AppState;

// Re-export extracted modules so callers using `crate::billing::*` still work.
pub use crate::billing_pricing::*;
pub use crate::billing_jobs::*;
pub use crate::billing_hooks::*;
pub use crate::billing_multiplayer::*;
pub use crate::billing_recovery::*;
pub use crate::billing_timer::*;
pub use crate::billing_game_status::*;
pub use crate::billing_orphan::*;
pub use crate::billing_session_lifecycle::*;

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
    /// Number of sub-sessions (1 = no split) — DEPRECATED (Act 2: one continuous timer)
    pub split_count: u32,
    /// Duration of each sub-session in minutes — DEPRECATED
    pub split_duration_minutes: Option<u32>,
    /// Which sub-session is currently running — DEPRECATED
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
    /// Game sim_type for per-game rate lookup. None = use universal rates.
    pub sim_type: Option<rc_common::types::SimType>,
    /// BILL-06: Seconds spent paused due to crash recovery (PausedGamePause + CrashRecovery origin).
    /// Excluded from billable time in cost computation. Tracked per-session, persisted to DB.
    pub recovery_pause_seconds: u32,
    /// BILL-06: Reason for the current pause (distinguishes crash recovery from manual ESC pause).
    pub pause_reason: PauseReason,
    /// Phase 283: Session nonce for replay protection. Rotated after each billing mutation.
    pub nonce: String,
    // ─── Act 2: Per-minute billing mode ────────────────────────────────────
    /// "package" (countdown from allocated_seconds) or "per_minute" (count-up, periodic debit)
    pub billing_mode: String,
    /// Per-minute rate in paise (e.g. 2500 = ₹25/min). Only used when billing_mode = "per_minute".
    pub rate_paise_per_minute: u32,
    /// Initial hold deducted at session start (pre-payment, not extra charge).
    pub hold_paise: u32,
    /// Total paise debited so far (hold + periodic debits). For reconciliation at session end.
    pub total_debited_paise: u32,
    /// Elapsed seconds since last per-minute debit. When this reaches 60, debit one minute.
    pub seconds_since_last_debit: u32,
    /// Wallet owner ID (parent for linked racers). Used for periodic debits.
    pub wallet_owner_id: String,
    /// Low balance warning threshold in paise. Alert staff when wallet approaches this.
    pub low_balance_warning_paise: u32,
    /// Whether low-balance warning has been sent for this session.
    pub low_balance_warned: bool,
    /// GLD-C-02: 1s-bucket telemetry coverage histogram. Set N = true if any telemetry
    /// packet was received during second N of the session (elapsed_second index). Flushed
    /// to billing_sessions.telemetry_coverage_pct on finalize. Lost on server crash
    /// (→ NULL coverage → UNVERIFIED per D-05). Updated non-blocking via try_write().
    /// Intentional default: empty set — pre-session state.
    pub telemetry_seconds_covered: std::collections::HashSet<u32>,
    /// GLD-C-04: UTC timestamp until which finalize is deferred (grace window for lap rejects).
    /// None = no grace window active. Some(t) = wait until Utc::now() >= t before finalizing.
    /// Persisted to billing_sessions.lap_reject_grace_until for restart-safety.
    /// Intentional default: None — no pending deferral.
    pub lap_reject_grace_until: Option<chrono::DateTime<chrono::Utc>>,
    /// GLD-C-04: End status deferred during grace window. Set at session-end trigger when
    /// grace window begins. Cleared after deferred finalize completes.
    /// Intentional default: None.
    pub pending_end_status: Option<BillingSessionStatus>,
}

/// BILL-06: Distinguishes why a billing session is paused.
/// Used to track crash-recovery pauses separately from manual (ESC) pauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    /// Not currently paused (default state when Active or Completed)
    None,
    /// Driver pressed ESC or manual pause from staff dashboard
    GamePause,
    /// Pod agent detected a crash and is recovering
    CrashRecovery,
    /// Pod WS connection dropped (reconnect pending)
    Disconnect,
}

impl Default for BillingTimer {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            driver_id: String::new(),
            driver_name: String::new(),
            pod_id: String::new(),
            pricing_tier_name: String::new(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
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
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            billing_mode: "per_minute".to_string(),
            rate_paise_per_minute: 2500,
            hold_paise: 10000,
            total_debited_paise: 0,
            seconds_since_last_debit: 0,
            wallet_owner_id: String::new(),
            low_balance_warning_paise: 5000,
            low_balance_warned: false,
            telemetry_seconds_covered: std::collections::HashSet::new(),
            lap_reject_grace_until: None, // Intentional default: no pending deferral
            pending_end_status: None,     // Intentional default: no deferred end status
        }
    }
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
            // Act 2: Billing mode for frontend display (countdown vs count-up)
            billing_mode: Some(self.billing_mode.clone()),
            // BILL-06: Recovery pause time excluded from billing
            recovery_pause_seconds: if self.recovery_pause_seconds > 0 {
                Some(self.recovery_pause_seconds)
            } else {
                None
            },
        }
    }

    /// Whether this session needs a per-minute wallet debit on the next tick cycle.
    /// The caller checks this after tick() and performs the async DB debit.
    pub fn needs_per_minute_debit(&self) -> bool {
        self.billing_mode == "per_minute" && self.seconds_since_last_debit >= 60
    }

    /// Compute debit (or credit-back) for the next per-minute tick using snap pricing.
    pub fn snap_debit_amount(&self) -> i32 {
        let billable_seconds = self.elapsed_seconds.saturating_sub(self.recovery_pause_seconds);
        let new_minutes = billable_seconds / 60;
        let target_total = crate::billing_pricing::snap_cost_for_minutes(new_minutes, 2500, 70000, 90000);
        (target_total - self.total_debited_paise as i64) as i32
    }

    /// Record that a per-minute debit was performed.
    pub fn record_debit(&mut self, amount_paise: u32) {
        self.seconds_since_last_debit = 0;
        self.total_debited_paise += amount_paise;
    }

    /// Record a snap debit (may be negative = credit-back at boundaries).
    pub fn record_snap_debit(&mut self, amount: i32) {
        self.seconds_since_last_debit = 0;
        if amount >= 0 {
            self.total_debited_paise += amount as u32;
        } else {
            self.total_debited_paise = self.total_debited_paise.saturating_sub((-amount) as u32);
        }
    }

    /// Tick the timer by 1 second. Returns true if session should auto-end.
    ///
    /// - Active: increments elapsed_seconds + driving_seconds. Returns true on hard max cap.
    ///   Per-minute mode: also increments seconds_since_last_debit (caller handles async debit).
    /// - PausedGamePause: increments pause_seconds. Returns true on 10-min pause timeout.
    ///   If pause_reason == CrashRecovery, also increments recovery_pause_seconds (BILL-06).
    /// - WaitingForGame: no increments, returns false.
    /// - Other statuses: returns false (existing behavior).
    pub fn tick(&mut self) -> bool {
        match self.status {
            BillingSessionStatus::Active => {
                self.elapsed_seconds += 1;
                self.driving_seconds += 1;
                self.seconds_since_last_debit += 1;
                self.elapsed_seconds >= self.max_session_seconds
            }
            BillingSessionStatus::PausedGamePause => {
                self.pause_seconds += 1;
                // BILL-06: Track crash-recovery time separately for billing exclusion
                if self.pause_reason == PauseReason::CrashRecovery {
                    self.recovery_pause_seconds += 1;
                }
                self.pause_seconds >= 600 // 10-min pause timeout
            }
            BillingSessionStatus::PausedCrashRecovery => {
                self.pause_seconds += 1;
                self.recovery_pause_seconds += 1;
                self.pause_seconds >= 600 // 10-min pause timeout
            }
            BillingSessionStatus::WaitingForGame => false,
            _ => false,
        }
    }

    /// Get the current session cost based on elapsed seconds and rate tiers.
    /// BILL-06: Subtracts recovery_pause_seconds from billable time so crash-recovery
    /// pauses are not charged to the customer.
    pub fn current_cost(&self, tiers: &[BillingRateTier]) -> SessionCost {
        let filtered: Vec<BillingRateTier> = get_tiers_for_game(tiers, self.sim_type)
            .into_iter()
            .cloned()
            .collect();
        // BILL-06: Exclude recovery pause time from billable seconds
        let billable_seconds = self.elapsed_seconds.saturating_sub(self.recovery_pause_seconds);
        compute_session_cost(billable_seconds, &filtered)
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
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            ..Default::default()
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
    /// Game sim_type for per-game rate lookup. Set when AcStatus::Live received.
    pub sim_type: Option<rc_common::types::SimType>,
    /// Launch args for retry on timeout (track, car, AI config, etc.)
    pub launch_args: Option<String>,
    /// BILL-13: Pre-committed session data from kiosk staff path (FATM-01).
    /// When Some, the DB record + wallet debit already committed. On Live, just activate
    /// the in-memory timer via finalize_billing_start() — do NOT call start_billing_session().
    /// When None (PIN auth path), start_billing_session() creates the DB record on Live.
    pub pre_committed: Option<BillingStartData>,
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
    /// Per-pod lock to serialize start_billing calls — prevents TOCTOU race (BATOM-01).
    /// Key = normalized pod_id. Lock held for entire start_billing duration.
    /// Uses std::sync::Mutex wrapping a HashMap of Arc<tokio::sync::Mutex<()>> so the
    /// outer lock is only held briefly (to get/insert the inner Arc), and the inner
    /// tokio::Mutex is held across async work without blocking other pods.
    pub billing_start_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// CONC-01: Per-driver lock to prevent same driver from starting sessions on multiple pods.
    /// Same pattern as billing_start_locks but keyed on driver_id.
    pub driver_billing_locks: std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl BillingManager {
    pub fn new() -> Self {
        Self {
            active_timers: RwLock::new(HashMap::new()),
            waiting_for_game: RwLock::new(HashMap::new()),
            multiplayer_waiting: RwLock::new(HashMap::new()),
            rate_tiers: RwLock::new(default_billing_rate_tiers()),
            billing_start_locks: std::sync::Mutex::new(HashMap::new()),
            driver_billing_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a per-pod lock for serializing start_billing.
    /// The outer std::sync::Mutex is held only briefly (HashMap lookup/insert).
    /// The returned Arc<tokio::sync::Mutex<()>> can be .lock().await'd across async work.
    pub fn get_billing_start_lock(&self, pod_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.billing_start_locks.lock().unwrap_or_else(|e| e.into_inner());
        locks.entry(pod_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// CONC-01: Get or create a per-driver lock for serializing billing starts.
    /// Prevents the same driver from starting sessions on 2 pods simultaneously.
    pub fn get_driver_billing_lock(&self, driver_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.driver_billing_locks.lock().unwrap_or_else(|e| e.into_inner());
        locks.entry(driver_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}


#[cfg(test)]
#[path = "billing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "billing_grace_tests.rs"]
mod billing_grace;
