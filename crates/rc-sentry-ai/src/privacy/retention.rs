use std::sync::Arc;

use tokio::time::{interval, Duration};

use super::audit::{AuditEntry, AuditWriter};

/// Hourly retention check task.
/// Purges expired face embeddings and old attendance records from SQLite.
pub async fn retention_purge_task(retention_days: u64, db_path: String, audit: Arc<AuditWriter>) {
    let mut tick = interval(Duration::from_secs(3600)); // hourly
    loop {
        tick.tick().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        // CRITICAL: Use SQLite-compatible format (space separator, no timezone suffix).
        // SQLite datetime('now') produces "2026-03-24 12:00:00" — RFC3339's "T" separator
        // causes string comparison to match ALL rows (space 0x20 < T 0x54 = delete everything).
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

        let db = db_path.clone();
        let cutoff_clone = cutoff_str.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(usize, usize), String> {
            let conn = rusqlite::Connection::open(&db).map_err(|e| e.to_string())?;
            conn.execute("PRAGMA foreign_keys = ON", []).map_err(|e| e.to_string())?;

            // Purge expired embeddings (expires_at < now)
            let embeddings_purged = conn
                .execute(
                    "DELETE FROM face_embeddings WHERE expires_at < datetime('now')",
                    [],
                )
                .map_err(|e| e.to_string())?;

            // Purge old attendance logs beyond retention window
            let attendance_purged = conn
                .execute(
                    "DELETE FROM attendance_log WHERE logged_at < ?1",
                    [&cutoff_clone],
                )
                .map_err(|e| e.to_string())?;

            Ok((embeddings_purged, attendance_purged))
        })
        .await;

        match result {
            Ok(Ok((embeddings, attendance))) => {
                tracing::info!(
                    retention_days = retention_days,
                    cutoff = %cutoff_str,
                    embeddings_purged = embeddings,
                    attendance_purged = attendance,
                    "retention purge completed"
                );

                let entry = AuditEntry {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    action: "retention_purge".to_string(),
                    person_id: None,
                    accessor: "system".to_string(),
                    details: Some(format!(
                        "Purged {embeddings} expired embeddings, {attendance} old attendance records (cutoff: {cutoff_str})"
                    )),
                };
                audit.log(entry);
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "retention purge DB error");
            }
            Err(e) => {
                tracing::error!(error = %e, "retention purge task panicked");
            }
        }
    }
}
