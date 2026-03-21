use std::sync::Arc;

use tokio::time::{interval, Duration};

use super::audit::{AuditEntry, AuditWriter};

/// Hourly retention check task.
/// In Phase 113: logs that purge ran (no embeddings to purge yet).
/// Phase 114 will add actual SQLite embedding purge.
pub async fn retention_purge_task(retention_days: u64, audit: Arc<AuditWriter>) {
    let mut tick = interval(Duration::from_secs(3600)); // hourly
    loop {
        tick.tick().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);

        tracing::info!(
            retention_days = retention_days,
            cutoff = %cutoff.to_rfc3339(),
            "retention purge check completed"
        );

        // Log purge run in audit trail
        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "retention_purge".to_string(),
            person_id: None,
            accessor: "system".to_string(),
            details: Some(format!("Purge check for entries older than {cutoff}")),
        };
        audit.log(entry);

        // Phase 114: purge embeddings from SQLite where created_at < cutoff
    }
}
