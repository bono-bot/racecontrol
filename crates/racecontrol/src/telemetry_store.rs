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

use rc_common::types::TelemetryFrame;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use tokio::sync::mpsc;

// ─── Submodules (flat files in src/) ────────────────────────────────────────

#[path = "telemetry_writer.rs"]
mod writer;

#[path = "telemetry_hardware.rs"]
mod hardware;

// Re-export public API from submodules
pub use hardware::{
    get_metric_trend,
    run_daily_aggregation,
    run_hourly_aggregation,
    run_retention_cleanup,
    spawn_maintenance_scheduler,
    store_extended_telemetry,
    MetricTrend,
};

// ─── Constants (shared with writer submodule) ───────────────────────────────

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

// ─── Database Initialization ────────────────────────────────────────────────

/// Initialize the telemetry database pool and create tables + indexes.
pub async fn init_telemetry_db(db_path: &str) -> anyhow::Result<SqlitePool> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = format!("sqlite:{}?mode=rwc", db_path);
    // RESIL-02: Pool sized for 8 pods x 10Hz writes + dashboard telemetry reads.
    // Previous pool=3 caused insert stalls when dashboard queried playback data.
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
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

    // v29.0: Extended hardware telemetry table for preventive maintenance
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS hardware_telemetry (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pod_id TEXT NOT NULL,
            collected_at TEXT NOT NULL,
            gpu_temp_celsius REAL,
            cpu_temp_celsius REAL,
            gpu_power_watts REAL,
            vram_usage_mb INTEGER,
            disk_smart_health_pct INTEGER,
            disk_power_on_hours INTEGER,
            game_crashes_last_hour INTEGER,
            windows_critical_errors TEXT,
            process_handle_count INTEGER,
            system_uptime_secs INTEGER,
            cpu_usage_pct REAL,
            gpu_usage_pct REAL,
            memory_usage_pct REAL,
            disk_usage_pct REAL,
            network_latency_ms INTEGER,
            usb_device_count INTEGER,
            fan_speeds_rpm TEXT
        )"
    ).execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hw_telem_pod_time ON hardware_telemetry(pod_id, collected_at)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hw_telem_time ON hardware_telemetry(collected_at)")
        .execute(&pool).await?;

    // v29.0 Phase 3: Aggregated telemetry for trend analysis
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telemetry_aggregates (
            pod_id TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            period_start TEXT NOT NULL,
            period_hours INTEGER NOT NULL,
            min_val REAL,
            max_val REAL,
            avg_val REAL,
            std_dev REAL,
            sample_count INTEGER,
            had_active_session INTEGER DEFAULT 0,
            was_peak_hours INTEGER DEFAULT 0,
            PRIMARY KEY (pod_id, metric_name, period_start, period_hours)
        )"
    ).execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_agg_pod_metric ON telemetry_aggregates(pod_id, metric_name)")
        .execute(&pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_agg_period ON telemetry_aggregates(period_start)")
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
        writer::writer_loop(pool, rx, retention).await;
        tracing::warn!("TelemetryWriter exited");
    });

    tx
}
