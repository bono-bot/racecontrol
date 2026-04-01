//! Unified pod activity logger with SHA-256 append-only hash chain (Phase 307).
//!
//! Single entry point for all pod events: inserts to `pod_activity_log` (fire-and-forget)
//! and broadcasts `DashboardEvent::PodActivity` for real-time WebSocket delivery.
//!
//! ## Hash Chain Protocol
//! Every new entry computes:
//!   `entry_hash = SHA-256(id|timestamp|category|action|details|source|previous_hash)`
//! using `|` as field delimiter. The first entry after migration uses `previous_hash = "GENESIS"`.
//! Pre-migration rows have NULL hashes and are outside the chain.
//!
//! The `AppState.audit_last_hash` mutex serializes hash computation so the chain is consistent
//! even under concurrent callers. The mutex is held only for hash computation (no async inside);
//! the DB insert runs after the mutex is released.

use std::sync::Arc;

use rc_common::protocol::DashboardEvent;
use rc_common::types::PodActivityEntry;

use crate::state::AppState;

/// Compute the SHA-256 chain hash for one activity entry.
///
/// Formula: `SHA-256(id|timestamp|category|action|details|source|previous_hash)`
pub fn compute_activity_hash(
    id: &str,
    timestamp: &str,
    category: &str,
    action: &str,
    details: &str,
    source: &str,
    previous_hash: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update(b"|");
    hasher.update(timestamp.as_bytes());
    hasher.update(b"|");
    hasher.update(category.as_bytes());
    hasher.update(b"|");
    hasher.update(action.as_bytes());
    hasher.update(b"|");
    hasher.update(details.as_bytes());
    hasher.update(b"|");
    hasher.update(source.as_bytes());
    hasher.update(b"|");
    hasher.update(previous_hash.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Log a pod activity event. Non-blocking: spawns a task for the DB insert.
///
/// Hash chain is maintained by briefly locking `state.audit_last_hash` to:
/// 1. Read the previous hash
/// 2. Compute the new entry_hash
/// 3. Update the in-memory last hash
/// Then release the mutex before the async DB insert.
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

    // Compute hash chain - lock held only for hash computation, released before .await
    // Standing rule: never hold a lock across .await
    let (prev_hash, entry_hash) = {
        let mut last = state
            .audit_last_hash
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev = last.clone();
        let hash = compute_activity_hash(
            &id, &timestamp, category, action, details, source, &prev,
        );
        *last = hash.clone();
        (prev, hash)
        // mutex released here - no .await will be called while holding this lock
    };

    // Fire-and-forget DB insert
    let db = state.db.clone();
    let pod_id = pod_id.to_string();
    let category = category.to_string();
    let action = action.to_string();
    let details = details.to_string();
    let source = source.to_string();

    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT INTO pod_activity_log
             (id, pod_id, pod_number, timestamp, category, action, details, source, entry_hash, previous_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&pod_id)
        .bind(pod_number as i64)
        .bind(&timestamp)
        .bind(&category)
        .bind(&action)
        .bind(&details)
        .bind(&source)
        .bind(&entry_hash)
        .bind(&prev_hash)
        .execute(&db)
        .await;

        if let Err(e) = result {
            tracing::warn!(
                "Failed to insert activity log entry {}: {}",
                id, e
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_is_deterministic() {
        let h1 = compute_activity_hash(
            "id1", "2026-04-01T10:00:00Z", "system", "Test", "details", "staff", "GENESIS",
        );
        let h2 = compute_activity_hash(
            "id1", "2026-04-01T10:00:00Z", "system", "Test", "details", "staff", "GENESIS",
        );
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // hex SHA-256 is 64 chars
    }

    #[test]
    fn test_hash_chain_different_entries() {
        let h1 = compute_activity_hash(
            "id1", "2026-04-01T10:00:00Z", "system", "Action A", "", "core", "GENESIS",
        );
        let h2 = compute_activity_hash(
            "id2", "2026-04-01T10:00:01Z", "system", "Action B", "", "core", &h1,
        );
        // Different entries produce different hashes
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_tamper_detection() {
        let h1 = compute_activity_hash(
            "id1", "2026-04-01T10:00:00Z", "system", "Original", "", "core", "GENESIS",
        );
        // Tampered: different action
        let h_tampered = compute_activity_hash(
            "id1", "2026-04-01T10:00:00Z", "system", "Tampered!", "", "core", "GENESIS",
        );
        assert_ne!(h1, h_tampered);
    }

    #[test]
    fn test_genesis_chain_start() {
        // Verify GENESIS produces a valid 64-char hex hash
        let h = compute_activity_hash(
            "some-uuid", "2026-04-01T00:00:00Z", "config", "Test", "", "staff", "GENESIS",
        );
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
