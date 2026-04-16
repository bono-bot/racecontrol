use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::GameState;

use crate::billing;
use crate::state::AppState;

/// Cleanup when a WebSocket connection drops (ungraceful disconnect).
/// Only acts if this connection is still the active one for the pod.
pub(crate) async fn cleanup_on_disconnect(
    state: &Arc<AppState>,
    registered_pod_id: &Option<String>,
    conn_id: u64,
) {
    let Some(pod_id) = registered_pod_id else { return };

    let current_conn_id = state.agent_conn_ids.read().await.get(pod_id).copied();
    let is_stale = current_conn_id.is_some() && current_conn_id != Some(conn_id);

    if is_stale {
        tracing::info!(
            "Stale WebSocket cleanup for pod {} (conn_id={}, current={}). Skipping.",
            pod_id, conn_id, current_conn_id.unwrap()
        );
        return;
    }

    state.agent_senders.write().await.remove(pod_id);
    state.agent_conn_ids.write().await.remove(pod_id);

    // Clear fleet health version/uptime on ungraceful disconnect.
    {
        let mut fleet = state.pod_fleet_health.write().await;
        if let Some(store) = fleet.get_mut(pod_id.as_str()) {
            crate::fleet_health::clear_on_disconnect(store);
        }
    }

    // Sweep pending WS command entries for this pod (they use "pod_X:" prefix)
    {
        let prefix = format!("{}:", pod_id);
        let mut pending = state.pending_ws_execs.write().await;
        let stale_keys: Vec<String> = pending.keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        for key in &stale_keys {
            pending.remove(key);
        }
        if !stale_keys.is_empty() {
            tracing::info!("Cleaned {} pending WS command(s) for disconnected {}", stale_keys.len(), pod_id);
        }
    }

    // Clean up pending self-tests for disconnected pod
    {
        let mut pending = state.pending_self_tests.write().await;
        let before = pending.len();
        pending.retain(|_req_id, (pid, _tx)| pid != pod_id);
        let removed = before - pending.len();
        if removed > 0 {
            tracing::info!("Cleaned {} pending self-test(s) for disconnected {}", removed, pod_id);
        }
    }

    let has_active_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(pod_id.as_str());

    // Mark pod offline on ungraceful disconnect (WebSocket dropped without Disconnect message)
    if let Some(pod) = state.pods.write().await.get_mut(pod_id.as_str())
        && pod.status != rc_common::types::PodStatus::Offline
            && pod.status != rc_common::types::PodStatus::Disabled
        {
            tracing::warn!("Pod {} WebSocket dropped without Disconnect (conn_id={}) — marking Offline", pod_id, conn_id);
            crate::activity_log::log_pod_activity(state, pod_id, "system", "Pod Disconnected", &format!("WebSocket dropped unexpectedly (conn_id={})", conn_id), "core", None);
            pod.status = rc_common::types::PodStatus::Offline;
            pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
            // Preserve game_state if billing is active — agent will resync on reconnect
            if !has_active_billing {
                pod.game_state = Some(GameState::Idle);
                pod.current_game = None;
            }
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }

    // MMA-P1-FIX: Sync offline status to DB — preserves disabled/maintenance
    if let Err(e) = sqlx::query(
        "UPDATE pods SET status = 'offline', last_seen = datetime('now')
         WHERE id = ? AND status NOT IN ('disabled', 'maintenance')"
    )
    .bind(pod_id)
    .execute(&state.db)
    .await {
        tracing::warn!("Failed to sync pod {} disconnect to DB: {}", pod_id, e);
    }

    billing::update_driving_state(state, pod_id, rc_common::types::DrivingState::NoDevice)
        .await;
}
