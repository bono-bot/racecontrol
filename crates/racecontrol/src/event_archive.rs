//! Phase 302-01: Structured Event Archive
//!
//! Provides fire-and-forget event writes to the `system_events` SQLite table,
//! hourly background tick that exports the previous day's events to a JSONL file,
//! 90-day purge of old SQLite rows, and nightly SCP transfer to Bono VPS.
//!
//! Standing rules compliance:
//! - No .unwrap() in production code — uses ?, .ok(), unwrap_or
//! - No lock held across .await — archive_tick clones config before async work
//! - StrictHostKeyChecking=no + BatchMode=yes + ConnectTimeout=10 on all ssh/scp
//! - No hardcoded IPs — uses config.event_archive.remote_host
//! - IST timestamps via chrono_tz::Asia::Kolkata
//! - Module is independent of backup_pipeline.rs (separate last_remote_transfer tracking)

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;

use crate::state::AppState;

#[path = "event_archive_export.rs"]
mod export;

#[path = "event_archive_transfer.rs"]
mod transfer;

const LOG_TARGET: &str = "event_archive";

// ─── Public API ──────────────────────────────────────────────────────────────

/// Fire-and-forget insert of a structured event into the `system_events` table.
///
/// UUID and IST timestamp are computed BEFORE the tokio::spawn so the caller's
/// timing is captured, not the async scheduling delay.
///
/// # Arguments
/// - `db`         — SQLite connection pool (cloned into the async task)
/// - `event_type` — dot-namespaced category, e.g. `"billing.session_started"`
/// - `source`     — subsystem that emitted the event, e.g. `"billing"`
/// - `pod`        — optional pod identifier (e.g. `"pod_4"`); None for server-level events
/// - `payload`    — arbitrary JSON data for this event type
/// - `venue_id`   — venue identifier for multi-venue partitioning
pub fn append_event(
    db: &SqlitePool,
    event_type: &str,
    source: &str,
    pod: Option<&str>,
    payload: serde_json::Value,
    venue_id: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    // IST timestamp — standing rule: never use plain Utc for stored timestamps
    let timestamp = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    // Clone all values before moving into the async block
    let db = db.clone();
    let event_type = event_type.to_string();
    let source = source.to_string();
    let pod = pod.map(|s| s.to_string());
    let payload_str = payload.to_string();
    let venue_id = venue_id.to_string();

    tokio::spawn(async move {
        let result = insert_event_direct(
            &db, &id, &event_type, &source, pod.as_deref(), &payload_str, &timestamp, &venue_id,
        )
        .await;
        if let Err(e) = result {
            tracing::warn!(target: LOG_TARGET, "append_event insert failed: {}", e);
        }
    });
}

/// Helper: direct INSERT into system_events (used by append_event + tests).
/// This is the synchronous inner function that tests call directly.
pub(crate) async fn insert_event_direct(
    db: &SqlitePool,
    id: &str,
    event_type: &str,
    source: &str,
    pod: Option<&str>,
    payload_str: &str,
    timestamp: &str,
    venue_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO system_events (id, event_type, source, pod, timestamp, payload, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(event_type)
    .bind(source)
    .bind(pod)
    .bind(timestamp)
    .bind(payload_str)
    .bind(venue_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Spawn the event archive background task.
///
/// Ticks hourly. Each tick:
///   1. Exports yesterday's events to a JSONL file (idempotent)
///   2. Purges events older than retention_days from SQLite
///   3. SCPs the JSONL to Bono VPS (IST 02:00-03:59 window, once per day)
pub fn spawn(state: Arc<AppState>) {
    if !state.config.event_archive.enabled {
        tracing::info!(target: LOG_TARGET, "event_archive disabled — skipping spawn");
        return;
    }

    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "event_archive task started");

        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        // Track the IST date of the last successful remote transfer.
        // Independent from backup_pipeline.rs — each module owns its own tracking.
        let mut last_remote_transfer: Option<chrono::NaiveDate> = None;

        loop {
            interval.tick().await;
            if let Err(e) = archive_tick(&state, &mut last_remote_transfer).await {
                tracing::error!(target: LOG_TARGET, "archive_tick error: {}", e);
            }
        }
    });
}

// ─── Private: archive tick ────────────────────────────────────────────────────

/// One archive tick: export → purge → optional SCP transfer.
/// Order matters: export MUST run before purge so yesterday's rows exist during export.
async fn archive_tick(
    state: &Arc<AppState>,
    last_remote_transfer: &mut Option<chrono::NaiveDate>,
) -> anyhow::Result<()> {
    // Clone config values before any async work — no lock held across .await
    let archive_dir = state.config.event_archive.archive_dir.clone();
    let remote_enabled = state.config.event_archive.remote_enabled;
    let retention_days = state.config.event_archive.retention_days;

    // Step 1: Export yesterday's events to JSONL (idempotent via file-exists check)
    let filename = export::export_daily_jsonl(&state.db, &archive_dir).await?;

    // Step 2: Purge events older than retention_days (AFTER export — order is critical)
    export::purge_old_events(&state.db, retention_days).await?;

    // Step 3: SCP the JSONL file to Bono VPS (only during IST 02:00-03:59 window, once per day)
    if remote_enabled {
        let filepath = format!("{}/{}", archive_dir, filename);
        if let Err(e) = transfer::transfer_jsonl_to_remote(
            state,
            &filepath,
            &filename,
            last_remote_transfer,
        )
        .await
        {
            tracing::error!(target: LOG_TARGET, "JSONL remote transfer failed: {}", e);
        }
    }

    Ok(())
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory SQLite pool with the system_events table.
    async fn make_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                source TEXT NOT NULL,
                pod TEXT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                payload TEXT NOT NULL DEFAULT '{}',
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_system_events_type ON system_events(event_type)")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_append_event_inserts_row() {
        let db = make_test_db().await;

        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = "2026-04-01T02:30:00".to_string();
        insert_event_direct(
            &db,
            &id,
            "billing.session_started",
            "billing",
            Some("pod_1"),
            r#"{"driver_id":"d1","tier":"standard"}"#,
            &timestamp,
            "racingpoint-hyd-001",
        )
        .await
        .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM system_events")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "Should have exactly 1 row after insert");

        let row: (String, String, String, Option<String>, String, String) =
            sqlx::query_as("SELECT id, event_type, source, pod, timestamp, payload FROM system_events")
                .fetch_one(&db)
                .await
                .unwrap();

        assert_eq!(row.0, id);
        assert_eq!(row.1, "billing.session_started");
        assert_eq!(row.2, "billing");
        assert_eq!(row.3, Some("pod_1".to_string()));
        assert_eq!(row.4, timestamp);
        assert_eq!(row.5, r#"{"driver_id":"d1","tier":"standard"}"#);
    }

    #[tokio::test]
    async fn test_export_creates_jsonl() {
        let db = make_test_db().await;
        let tmp = tempfile::tempdir().expect("Failed to create temp dir");
        let dir = tmp.path().to_str().unwrap().to_string();

        // Insert 3 events for yesterday's date
        let yesterday = (chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata)
            - chrono::Duration::days(1))
        .date_naive();
        let date_str = yesterday.format("%Y-%m-%d").to_string();

        for i in 0..3 {
            let id = uuid::Uuid::new_v4().to_string();
            let ts = format!("{}T0{}:00:00", date_str, i);
            insert_event_direct(
                &db,
                &id,
                "test.event",
                "test",
                None,
                "{}",
                &ts,
                "racingpoint-hyd-001",
            )
            .await
            .unwrap();
        }

        let filename = export::export_daily_jsonl(&db, &dir).await.unwrap();

        let filepath = format!("{}/{}", dir, filename);
        assert!(
            std::path::Path::new(&filepath).exists(),
            "JSONL file should exist"
        );

        let content = std::fs::read_to_string(&filepath).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "Should have 3 lines (one per event)");

        // Each line should be valid JSON
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("Each line should be valid JSON");
        }
    }

    #[tokio::test]
    async fn test_export_is_idempotent() {
        let db = make_test_db().await;
        let tmp = tempfile::tempdir().expect("Failed to create temp dir");
        let dir = tmp.path().to_str().unwrap().to_string();

        let yesterday = (chrono::Utc::now().with_timezone(&chrono_tz::Asia::Kolkata)
            - chrono::Duration::days(1))
        .date_naive();
        let date_str = yesterday.format("%Y-%m-%d").to_string();

        let id = uuid::Uuid::new_v4().to_string();
        let ts = format!("{}T10:00:00", date_str);
        insert_event_direct(&db, &id, "test.event", "test", None, "{}", &ts, "racingpoint-hyd-001")
            .await
            .unwrap();

        // First export
        let filename = export::export_daily_jsonl(&db, &dir).await.unwrap();
        let filepath = format!("{}/{}", dir, filename);
        let content_first = std::fs::read_to_string(&filepath).unwrap();

        // Insert another event — second export should NOT include it (idempotent)
        let id2 = uuid::Uuid::new_v4().to_string();
        insert_event_direct(&db, &id2, "test.event2", "test", None, "{}", &ts, "racingpoint-hyd-001")
            .await
            .unwrap();

        // Second export — should return same filename and NOT rewrite
        let filename2 = export::export_daily_jsonl(&db, &dir).await.unwrap();
        assert_eq!(filename, filename2, "Filename should be the same");

        let content_second = std::fs::read_to_string(&filepath).unwrap();
        assert_eq!(
            content_first, content_second,
            "File content should be unchanged on second export (idempotent)"
        );
    }

    #[tokio::test]
    async fn test_purge_deletes_old_keeps_recent() {
        let db = make_test_db().await;

        // Insert one event 100 days ago and one 10 days ago
        let old_id = uuid::Uuid::new_v4().to_string();
        let recent_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO system_events (id, event_type, source, pod, timestamp, payload)
             VALUES (?, 'test.old', 'test', NULL, datetime('now', '-100 days'), '{}')",
        )
        .bind(&old_id)
        .execute(&db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO system_events (id, event_type, source, pod, timestamp, payload)
             VALUES (?, 'test.recent', 'test', NULL, datetime('now', '-10 days'), '{}')",
        )
        .bind(&recent_id)
        .execute(&db)
        .await
        .unwrap();

        let deleted = export::purge_old_events(&db, 90).await.unwrap();
        assert_eq!(deleted, 1, "Should delete exactly 1 old event");

        let remaining: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM system_events")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(remaining.0, 1, "Should have 1 event remaining");

        let remaining_type: (String,) =
            sqlx::query_as("SELECT event_type FROM system_events")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            remaining_type.0, "test.recent",
            "Remaining event should be the recent one"
        );
    }

    #[test]
    fn test_event_archive_config_defaults() {
        use crate::config::EventArchiveConfig;
        let cfg = EventArchiveConfig::default();
        assert!(cfg.enabled, "enabled should default to true");
        assert_eq!(cfg.retention_days, 90, "retention_days should default to 90");
        assert_eq!(
            cfg.archive_dir, "./data/event-archive",
            "archive_dir should default to ./data/event-archive"
        );
        assert!(cfg.remote_enabled, "remote_enabled should default to true");
        assert_eq!(
            cfg.remote_host, "root@100.70.177.44",
            "remote_host should default to Bono VPS"
        );
    }
}
