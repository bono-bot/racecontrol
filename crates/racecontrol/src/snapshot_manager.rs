//! v29.0 Phase 25: System configuration snapshots before maintenance/patches.

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub id: String,
    pub pod_id: u8,
    pub timestamp: DateTime<Utc>,
    pub snapshot_type: String, // "pre_maintenance", "pre_deploy", "scheduled"
    pub config_hash: String,
    pub build_id: String,
    pub metadata: serde_json::Value,
}

/// Create a snapshot record (metadata stored in DB, actual configs on pod filesystem)
pub fn create_snapshot_record(
    pod_id: u8,
    snapshot_type: &str,
    config_hash: &str,
    build_id: &str,
) -> SystemSnapshot {
    SystemSnapshot {
        id: uuid::Uuid::new_v4().to_string(),
        pod_id,
        timestamp: Utc::now(),
        snapshot_type: snapshot_type.to_string(),
        config_hash: config_hash.to_string(),
        build_id: build_id.to_string(),
        metadata: serde_json::json!({}),
    }
}
