//! Event archive JSONL export and purge operations.
//!
//! Split from event_archive.rs — contains the daily JSONL file export
//! and the retention-based SQLite purge.

use sqlx::SqlitePool;

use super::LOG_TARGET;

// ─── JSONL export ────────────────────────────────────────────────────────────

/// Export the previous day's events (IST) to a JSONL file.
///
/// Idempotent: if the file already exists, returns immediately without re-querying.
/// One JSON object per line. Payload is re-parsed from string to avoid double-encoding.
///
/// Returns the filename (not full path) for use in the SCP call.
pub(super) async fn export_daily_jsonl(db: &SqlitePool, archive_dir: &str) -> anyhow::Result<String> {
    use chrono::Datelike;

    let yesterday = (chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata)
        - chrono::Duration::days(1))
    .date_naive();
    let date_str = yesterday.format("%Y-%m-%d").to_string();
    let filename = format!("events-{}.jsonl", date_str);
    let filepath = format!("{}/{}", archive_dir, filename);

    // Idempotent: skip if file already exists (handles server restarts mid-window)
    if std::path::Path::new(&filepath).exists() {
        tracing::debug!(target: LOG_TARGET, "JSONL already exists, skipping: {}", filename);
        return Ok(filename);
    }

    // Fetch all events for yesterday ordered by time ascending
    let rows: Vec<(String, String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT id, event_type, source, pod, timestamp, payload
         FROM system_events
         WHERE date(timestamp) = ?
         ORDER BY timestamp ASC",
    )
    .bind(&date_str)
    .fetch_all(db)
    .await?;

    // Create the archive directory if it does not exist
    std::fs::create_dir_all(archive_dir)?;

    let mut lines = String::new();
    for (id, event_type, source, pod, timestamp, payload) in &rows {
        let payload_value = serde_json::from_str::<serde_json::Value>(payload)
            .unwrap_or_else(|_| serde_json::Value::String(payload.clone()));

        let obj = serde_json::json!({
            "id": id,
            "event_type": event_type,
            "source": source,
            "pod": pod,
            "timestamp": timestamp,
            "payload": payload_value,
        });
        lines.push_str(&obj.to_string());
        lines.push('\n');
    }

    std::fs::write(&filepath, lines)?;
    tracing::info!(
        target: LOG_TARGET,
        "JSONL export complete: {} ({} events)",
        filename,
        rows.len()
    );
    Ok(filename)
}

// ─── Purge ───────────────────────────────────────────────────────────────────

/// Delete events older than `retention_days` from the system_events table.
///
/// Uses SQLite datetime modifier in the SQL string — the days parameter is
/// formatted inline since SQLite datetime() does not accept bind parameters for modifiers.
pub(super) async fn purge_old_events(db: &SqlitePool, retention_days: u32) -> anyhow::Result<u64> {
    let sql = format!(
        "DELETE FROM system_events WHERE timestamp < datetime('now', '-{} days')",
        retention_days
    );
    let result = sqlx::query(&sql).execute(db).await?;
    let deleted = result.rows_affected();
    if deleted > 0 {
        tracing::info!(
            target: LOG_TARGET,
            "Purged {} events older than {} days",
            deleted,
            retention_days
        );
    }
    Ok(deleted)
}
