//! Phase 251: Telemetry sample persistence to a SEPARATE telemetry.db file.
//!
//! Incoming `TelemetryFrame` messages are sent through an mpsc channel to a
//! background `TelemetryWriter` task that buffers and batch-inserts them.
//!
//! Flush triggers: every 1 second OR when buffer hits 50 samples.
//! 10 Hz sampling cap: frames <100 ms apart from the same pod are discarded.
//! Frames without `lap_id` (pre-lap data) are silently discarded.
//!
//! Nightly cleanup: batched DELETE of samples older than retention_days.

use chrono::{DateTime, Utc};
use rc_common::types::TelemetryFrame;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc;

/// Default retention for telemetry samples (days).
const DEFAULT_RETENTION_DAYS: u32 = 30;

/// Maximum buffer size before forced flush.
const BUFFER_FLUSH_SIZE: usize = 50;

/// Flush interval in milliseconds.
const FLUSH_INTERVAL_MS: u64 = 1000;

/// Minimum interval between samples from the same pod (10 Hz cap).
const MIN_SAMPLE_INTERVAL_MS: i64 = 100;

/// Rows deleted per batch during nightly cleanup.
const CLEANUP_BATCH_SIZE: i64 = 1000;

/// Sleep between cleanup batches (ms).
const CLEANUP_BATCH_SLEEP_MS: u64 = 100;

/// Initialize the telemetry database pool and create tables + indexes.
pub async fn init_telemetry_db(db_path: &str) -> anyhow::Result<SqlitePool> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path);
    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .max_lifetime(std::time::Duration::from_secs(300))
        .connect(&url)
        .await?;

    // WAL mode for concurrent read/write
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await?;
    // Enable incremental vacuum so nightly cleanup can reclaim space
    sqlx::query("PRAGMA auto_vacuum=INCREMENTAL").execute(&pool).await?;

    // Create the telemetry_samples table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telemetry_samples (
            lap_id TEXT NOT NULL,
            offset_ms INTEGER NOT NULL,
            speed REAL,
            throttle REAL,
            brake REAL,
            steering REAL,
            gear INTEGER,
            rpm INTEGER,
            pos_x REAL,
            pos_y REAL,
            pos_z REAL,
            pod_id TEXT NOT NULL,
            sampled_at TEXT NOT NULL
        )"
    ).execute(&pool).await?;

    // Indexes for common queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap ON telemetry_samples(lap_id)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_sampled_at ON telemetry_samples(sampled_at)")
        .execute(&pool).await?;

    tracing::info!("Telemetry DB initialized at {}", db_path);
    Ok(pool)
}

/// Derive telemetry.db path from the main DB path.
/// e.g. `./data/racecontrol.db` -> `./data/telemetry.db`
pub fn telemetry_db_path(main_db_path: &str) -> String {
    let main_path = Path::new(main_db_path);
    let parent = main_path.parent().unwrap_or(Path::new("."));
    parent.join("telemetry.db").to_string_lossy().to_string()
}

/// Spawn the telemetry writer background task.
/// Returns the sender half of the channel for ws/mod.rs to send frames.
pub fn spawn_writer(
    pool: SqlitePool,
    retention_days: Option<u32>,
) -> mpsc::Sender<TelemetryFrame> {
    let (tx, rx) = mpsc::channel::<TelemetryFrame>(512);
    let retention = retention_days.unwrap_or(DEFAULT_RETENTION_DAYS);

    tokio::spawn(async move {
        tracing::info!("TelemetryWriter started (retention={}d, flush_interval={}ms, buffer_size={})",
            retention, FLUSH_INTERVAL_MS, BUFFER_FLUSH_SIZE);
        writer_loop(pool, rx, retention).await;
        tracing::warn!("TelemetryWriter exited");
    });

    tx
}

/// Internal writer loop — consumes frames, buffers, and batch-flushes.
async fn writer_loop(
    pool: SqlitePool,
    mut rx: mpsc::Receiver<TelemetryFrame>,
    retention_days: u32,
) {
    let mut buffer: Vec<TelemetryFrame> = Vec::with_capacity(BUFFER_FLUSH_SIZE);
    let mut last_sample_ts: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut flush_interval = tokio::time::interval(
        std::time::Duration::from_millis(FLUSH_INTERVAL_MS)
    );
    let mut cleanup_interval = tokio::time::interval(
        std::time::Duration::from_secs(24 * 3600) // daily
    );
    // Skip first tick (fires immediately)
    flush_interval.tick().await;
    cleanup_interval.tick().await;

    loop {
        tokio::select! {
            frame = rx.recv() => {
                match frame {
                    Some(frame) => {
                        // Discard frames without lap_id (pre-lap data)
                        let Some(ref _lap_id) = frame.lap_id else {
                            continue;
                        };

                        // 10 Hz sampling cap: discard if <100ms since last sample from same pod
                        if let Some(last_ts) = last_sample_ts.get(&frame.pod_id) {
                            let diff_ms = (frame.timestamp - *last_ts).num_milliseconds();
                            if diff_ms < MIN_SAMPLE_INTERVAL_MS {
                                continue;
                            }
                        }
                        last_sample_ts.insert(frame.pod_id.clone(), frame.timestamp);

                        buffer.push(frame);

                        // Flush if buffer is full
                        if buffer.len() >= BUFFER_FLUSH_SIZE {
                            flush_buffer(&pool, &mut buffer).await;
                        }
                    }
                    None => {
                        // Channel closed — flush remaining and exit
                        if !buffer.is_empty() {
                            flush_buffer(&pool, &mut buffer).await;
                        }
                        break;
                    }
                }
            }
            _ = flush_interval.tick() => {
                if !buffer.is_empty() {
                    flush_buffer(&pool, &mut buffer).await;
                }
            }
            _ = cleanup_interval.tick() => {
                run_nightly_cleanup(&pool, retention_days).await;
            }
        }
    }
}

/// Batch-insert buffered frames into telemetry.db.
async fn flush_buffer(pool: &SqlitePool, buffer: &mut Vec<TelemetryFrame>) {
    if buffer.is_empty() {
        return;
    }

    let count = buffer.len();

    // Build a single transaction for the batch
    let result: Result<(), sqlx::Error> = async {
        let mut tx = pool.begin().await?;

        for frame in buffer.iter() {
            let lap_id = match &frame.lap_id {
                Some(id) => id.as_str(),
                None => continue, // should not happen — filtered above
            };
            let (pos_x, pos_y, pos_z) = match &frame.position {
                Some(p) => (Some(p.x as f64), Some(p.y as f64), Some(p.z as f64)),
                None => (None, None, None),
            };
            let sampled_at = frame.timestamp.to_rfc3339();

            sqlx::query(
                "INSERT INTO telemetry_samples (lap_id, offset_ms, speed, throttle, brake, steering, gear, rpm, pos_x, pos_y, pos_z, pod_id, sampled_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(lap_id)
            .bind(frame.session_time_ms as i64)
            .bind(frame.speed_kmh as f64)
            .bind(frame.throttle as f64)
            .bind(frame.brake as f64)
            .bind(frame.steering as f64)
            .bind(frame.gear as i32)
            .bind(frame.rpm as i64)
            .bind(pos_x)
            .bind(pos_y)
            .bind(pos_z)
            .bind(&frame.pod_id)
            .bind(&sampled_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }.await;

    match result {
        Ok(()) => {
            tracing::debug!("TelemetryWriter flushed {} samples", count);
            buffer.clear();
        }
        Err(e) => {
            tracing::error!("TelemetryWriter flush failed ({} samples): {}", count, e);
            // Keep buffer intact so samples can be retried on next flush.
            // If buffer grows too large (>500), drop oldest to prevent OOM.
            if buffer.len() > 500 {
                let excess = buffer.len() - 500;
                buffer.drain(..excess);
                tracing::warn!("TelemetryWriter dropped {} old samples due to repeated flush failure", excess);
            }
        }
    }
}

/// Nightly cleanup: batched DELETE of old samples, then incremental vacuum.
async fn run_nightly_cleanup(pool: &SqlitePool, retention_days: u32) {
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    tracing::info!("TelemetryWriter nightly cleanup: deleting samples before {}", cutoff_str);

    let mut total_deleted: u64 = 0;
    loop {
        let result = sqlx::query(
            "DELETE FROM telemetry_samples WHERE rowid IN (
                SELECT rowid FROM telemetry_samples WHERE sampled_at < ? LIMIT ?
            )"
        )
        .bind(&cutoff_str)
        .bind(CLEANUP_BATCH_SIZE)
        .execute(pool)
        .await;

        match result {
            Ok(res) => {
                let rows = res.rows_affected();
                total_deleted += rows;
                if rows < CLEANUP_BATCH_SIZE as u64 {
                    break; // no more rows to delete
                }
                tokio::time::sleep(std::time::Duration::from_millis(CLEANUP_BATCH_SLEEP_MS)).await;
            }
            Err(e) => {
                tracing::error!("TelemetryWriter cleanup batch failed: {}", e);
                break;
            }
        }
    }

    if total_deleted > 0 {
        tracing::info!("TelemetryWriter cleanup: deleted {} old samples", total_deleted);
        // Reclaim disk space
        if let Err(e) = sqlx::query("PRAGMA incremental_vacuum").execute(pool).await {
            tracing::warn!("TelemetryWriter incremental_vacuum failed: {}", e);
        }
    }

    // MMA-#7 + ITER5-MMA: Alert if table is growing unbounded AND log cleanup heartbeat
    // The heartbeat proves cleanup actually ran — absence of heartbeat = job dead
    tracing::info!("TelemetryWriter cleanup heartbeat: job completed at {}", Utc::now().to_rfc3339());
    match sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM telemetry_samples")
        .fetch_one(pool)
        .await
    {
        Ok((count,)) => {
            tracing::info!("TelemetryWriter row count after cleanup: {}", count);
            if count > 10_000_000 {
                tracing::error!(
                    "TELEMETRY ALERT: telemetry_samples has {} rows (>10M). \
                     Cleanup may be insufficient. Check disk space and retention_days config.",
                    count
                );
            }
        }
        Err(e) => {
            tracing::error!("TelemetryWriter failed to count rows after cleanup: {}", e);
        }
    }
}
