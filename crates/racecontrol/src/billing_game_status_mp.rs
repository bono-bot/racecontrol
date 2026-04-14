//! Multiplayer billing coordination — group billing start and timeout eviction.
//!
//! Extracted from billing_game_status.rs (Phase 385, v49.0 Architecture Completion).
//! Handles the coordinated billing start for multiplayer groups:
//! billing starts for ALL group members only after every participant reaches LIVE.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;

use crate::billing::{
    MultiplayerBillingWait, WaitingForGameEntry,
    start_billing_session,
};
use crate::state::AppState;

// ─── Multiplayer Live Coordination ──────────────────────────────────────────

/// Handle a multiplayer pod reaching AcStatus::Live.
/// Coordinates billing across all group members — billing starts only after
/// every participant reaches LIVE (or the timeout fires).
///
/// Returns `true` if the entry was consumed (caller should not process it further).
pub(crate) async fn handle_multiplayer_live(
    state: &Arc<AppState>,
    pod_id: &str,
    entry: WaitingForGameEntry,
    group_id: String,
) {
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
        start_billing_for_entries(state, entries, "multiplayer").await;
    } else {
        let remaining = wait.expected_pods.len() - wait.live_pods.len();
        tracing::info!(
            "Waiting for {} more player(s) in group {} ({}/{} live)",
            remaining, group_id, wait.live_pods.len(), wait.expected_pods.len()
        );
    }
}

// ─── Multiplayer Billing Timeout ─────────────────────────────────────────────

/// Called after the configured timeout to evict non-connecting pods from a multiplayer group.
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

    // Some pods failed to connect within timeout
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
    start_billing_for_entries(state, entries, "multiplayer_timeout").await;
}

// ─── Shared Billing Start Helper ─────────────────────────────────────────────

/// Start billing sessions for a batch of WaitingForGameEntry items.
/// Records billing accuracy events (METRICS-03) for each.
/// `detail_tag` is used in the billing accuracy event details field.
async fn start_billing_for_entries(
    state: &Arc<AppState>,
    entries: Vec<WaitingForGameEntry>,
    detail_tag: &str,
) {
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
                tracing::info!("Multiplayer billing started for pod {} (session {})", ep_id, session_id);
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
                    details: Some(detail_tag.to_string()),
                };
                crate::metrics::record_billing_accuracy_event(&state.db, &ba_event, &state.config.venue.venue_id).await;
            }
            Err(err) => {
                tracing::error!("Failed to start multiplayer billing for pod {}: {}", ep_id, err);
            }
        }
    }
}
