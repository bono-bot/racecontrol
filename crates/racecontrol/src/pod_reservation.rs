use std::sync::Arc;

use uuid::Uuid;

use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::PodReservationInfo;

/// Find a pod that is idle and has no active reservation.
pub async fn find_idle_pod(state: &Arc<AppState>) -> Option<String> {
    // Get pods that are idle (from in-memory state)
    let pods = state.pods.read().await;
    let idle_pods: Vec<String> = pods
        .values()
        .filter(|p| {
            p.status == rc_common::types::PodStatus::Idle && p.billing_session_id.is_none()
        })
        .map(|p| p.id.clone())
        .collect();
    drop(pods);

    if idle_pods.is_empty() {
        return None;
    }

    // Filter out pods that have active reservations
    for pod_id in idle_pods {
        let has_reservation = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM pod_reservations WHERE pod_id = ? AND status = 'active'",
        )
        .bind(&pod_id)
        .fetch_one(&state.db)
        .await
        .map(|r| r.0 > 0)
        .unwrap_or(true); // If query fails, assume reserved

        if !has_reservation {
            return Some(pod_id);
        }
    }

    None
}

/// Create a new pod reservation. Returns the reservation ID.
pub async fn create_reservation(
    state: &Arc<AppState>,
    driver_id: &str,
    pod_id: &str,
) -> Result<String, String> {
    let reservation_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO pod_reservations (id, driver_id, pod_id, status, created_at, last_activity_at)
         VALUES (?, ?, ?, 'active', datetime('now'), datetime('now'))",
    )
    .bind(&reservation_id)
    .bind(driver_id)
    .bind(pod_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating reservation: {}", e))?;

    // Broadcast reservation event
    let _ = state.dashboard_tx.send(DashboardEvent::PodReservationChanged {
        reservation_id: reservation_id.clone(),
        driver_id: driver_id.to_string(),
        pod_id: pod_id.to_string(),
        status: "active".to_string(),
    });

    tracing::info!(
        "Pod reservation created: {} for driver {} on pod {}",
        reservation_id,
        driver_id,
        pod_id
    );

    Ok(reservation_id)
}

/// End (complete) a pod reservation.
pub async fn end_reservation(state: &Arc<AppState>, reservation_id: &str) -> Result<(), String> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT driver_id, pod_id FROM pod_reservations WHERE id = ? AND status = 'active'",
    )
    .bind(reservation_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Reservation not found or not active".to_string())?;

    sqlx::query(
        "UPDATE pod_reservations SET status = 'completed', ended_at = datetime('now') WHERE id = ?",
    )
    .bind(reservation_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error ending reservation: {}", e))?;

    let _ = state.dashboard_tx.send(DashboardEvent::PodReservationChanged {
        reservation_id: reservation_id.to_string(),
        driver_id: row.0,
        pod_id: row.1,
        status: "completed".to_string(),
    });

    tracing::info!("Pod reservation {} completed", reservation_id);
    Ok(())
}

/// Update last_activity_at for a reservation (called when billing starts on the pod).
pub async fn touch_reservation(state: &Arc<AppState>, reservation_id: &str) {
    let _ = sqlx::query(
        "UPDATE pod_reservations SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(reservation_id)
    .execute(&state.db)
    .await;
}

/// Get active reservation for a driver.
pub async fn get_active_reservation_for_driver(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Option<PodReservationInfo> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, driver_id, pod_id, status, created_at, ended_at, last_activity_at
         FROM pod_reservations WHERE driver_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()?;

    Some(PodReservationInfo {
        id: row.0,
        driver_id: row.1,
        pod_id: row.2,
        status: row.3,
        created_at: row.4,
        ended_at: row.5,
        last_activity_at: row.6,
    })
}

/// Get active reservation for a pod.
pub async fn get_active_reservation_for_pod(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Option<PodReservationInfo> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, driver_id, pod_id, status, created_at, ended_at, last_activity_at
         FROM pod_reservations WHERE pod_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()?;

    Some(PodReservationInfo {
        id: row.0,
        driver_id: row.1,
        pod_id: row.2,
        status: row.3,
        created_at: row.4,
        ended_at: row.5,
        last_activity_at: row.6,
    })
}

/// Expire idle reservations — called periodically. Returns number expired.
/// Expires reservations where no billing activity has occurred for 5 minutes.
pub async fn expire_idle_reservations(state: &Arc<AppState>) -> u32 {
    let expired = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, driver_id, pod_id FROM pod_reservations
         WHERE status = 'active'
           AND last_activity_at < datetime('now', '-5 minutes')
           AND NOT EXISTS (
               SELECT 1 FROM billing_sessions
               WHERE billing_sessions.reservation_id = pod_reservations.id
                 AND billing_sessions.status IN ('active', 'paused_manual', 'paused_disconnect', 'pending')
           )",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if expired.is_empty() {
        return 0;
    }

    let count = expired.len() as u32;

    for (res_id, driver_id, pod_id) in &expired {
        let _ = sqlx::query(
            "UPDATE pod_reservations SET status = 'expired', ended_at = datetime('now') WHERE id = ?",
        )
        .bind(res_id)
        .execute(&state.db)
        .await;

        let _ = state.dashboard_tx.send(DashboardEvent::PodReservationChanged {
            reservation_id: res_id.clone(),
            driver_id: driver_id.clone(),
            pod_id: pod_id.clone(),
            status: "expired".to_string(),
        });

        // Send full SessionEnded to agent so pod returns to idle
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender
                .send(rc_common::protocol::CoreToAgentMessage::SessionEnded {
                    billing_session_id: String::new(),
                    driver_name: String::new(),
                    total_laps: 0,
                    best_lap_ms: None,
                    driving_seconds: 0,
                })
                .await;
        }
    }

    tracing::info!("Expired {} idle pod reservations", count);
    count
}
