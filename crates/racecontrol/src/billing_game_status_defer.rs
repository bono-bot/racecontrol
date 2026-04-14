//! Deferred billing start — WaitingForGame management and launch timeout checks.
//!
//! Extracted from billing_game_status.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use rc_common::pod_id::normalize_pod_id;

use crate::billing::{
    BillingManager, BillingStartData, WaitingForGameEntry,
};
use crate::state::AppState;

// ─── Launch Timeout Checking ────────────────────────────────────────────────

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

// ─── Deferred Billing Start ─────────────────────────────────────────────────

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
