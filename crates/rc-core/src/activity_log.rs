//! Unified pod activity logger.
//!
//! Single entry point for all pod events: inserts to `pod_activity_log` (fire-and-forget)
//! and broadcasts `DashboardEvent::PodActivity` for real-time WebSocket delivery.

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::PodActivityEntry;

use crate::state::AppState;

/// Log a pod activity event. Non-blocking: spawns a task for the DB insert.
pub fn log_pod_activity(
    state: &Arc<AppState>,
    pod_id: &str,
    category: &str,
    action: &str,
    details: &str,
    source: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Look up pod_number from in-memory state (try_read to avoid blocking)
    let pod_number = state
        .pods
        .try_read()
        .ok()
        .and_then(|pods| pods.get(pod_id).map(|p| p.number))
        .unwrap_or(0);

    let entry = PodActivityEntry {
        id: id.clone(),
        pod_id: pod_id.to_string(),
        pod_number,
        timestamp: timestamp.clone(),
        category: category.to_string(),
        action: action.to_string(),
        details: details.to_string(),
        source: source.to_string(),
    };

    // Broadcast immediately for real-time dashboard delivery
    let _ = state.dashboard_tx.send(DashboardEvent::PodActivity(entry));

    // Fire-and-forget DB insert
    let db = state.db.clone();
    let pod_id = pod_id.to_string();
    let category = category.to_string();
    let action = action.to_string();
    let details = details.to_string();
    let source = source.to_string();

    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO pod_activity_log (id, pod_id, pod_number, timestamp, category, action, details, source)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&pod_id)
        .bind(pod_number as i64)
        .bind(&timestamp)
        .bind(&category)
        .bind(&action)
        .bind(&details)
        .bind(&source)
        .execute(&db)
        .await;
    });
}
