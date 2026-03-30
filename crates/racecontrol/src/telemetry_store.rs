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

use chrono::{DateTime, Timelike, Utc};
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

// ─── v29.0 Phase 3: Historical Data Warehouse + Retention ───────────────────

/// Known numeric metric columns in hardware_telemetry for aggregation.
const METRIC_COLUMNS: &[&str] = &[
    "gpu_temp_celsius",
    "cpu_usage_pct",
    "gpu_usage_pct",
    "memory_usage_pct",
    "disk_usage_pct",
    "network_latency_ms",
    "process_handle_count",
];

/// Result of trend analysis on a metric.
#[derive(Debug, serde::Serialize)]
pub struct MetricTrend {
    pub current_value: f64,
    /// "rising", "stable", or "declining"
    pub trend: String,
    /// Slope: change per day
    pub rate_per_day: f64,
    /// ISO 8601 date, only set for declining health metrics
    pub predicted_failure_date: Option<String>,
    /// Confidence of the trend (0.0–1.0), based on R²
    pub confidence: f64,
    /// Number of data points used
    pub data_points: u32,
}

/// Run hourly aggregation: compute min/max/avg/std_dev per pod per metric
/// for the last hour from raw `hardware_telemetry`.
pub async fn run_hourly_aggregation(pool: &SqlitePool) {
    let now = Utc::now();
    let period_start = now - chrono::Duration::hours(1);
    let period_start_str = period_start.to_rfc3339();
    let now_str = now.to_rfc3339();

    for metric in METRIC_COLUMNS {
        // SQLite doesn't support parameterized column names, so we build the query string.
        // These are compile-time constants, not user input — safe from injection.
        let query = format!(
            "INSERT OR REPLACE INTO telemetry_aggregates
                (pod_id, metric_name, period_start, period_hours, min_val, max_val, avg_val, std_dev, sample_count)
             SELECT
                pod_id,
                '{metric}' AS metric_name,
                ?1 AS period_start,
                1 AS period_hours,
                MIN(CAST({metric} AS REAL)),
                MAX(CAST({metric} AS REAL)),
                AVG(CAST({metric} AS REAL)),
                -- population std dev via sqrt(avg(x²) - avg(x)²)
                CASE WHEN COUNT({metric}) > 1
                    THEN SQRT(MAX(0.0, AVG(CAST({metric} AS REAL) * CAST({metric} AS REAL)) - AVG(CAST({metric} AS REAL)) * AVG(CAST({metric} AS REAL))))
                    ELSE 0.0
                END,
                COUNT({metric})
             FROM hardware_telemetry
             WHERE collected_at >= ?1 AND collected_at < ?2 AND {metric} IS NOT NULL
             GROUP BY pod_id",
            metric = metric,
        );

        if let Err(e) = sqlx::query(&query)
            .bind(&period_start_str)
            .bind(&now_str)
            .execute(pool)
            .await
        {
            tracing::warn!("v29.0: hourly aggregation failed for {}: {}", metric, e);
        }
    }

    tracing::info!("v29.0: hourly aggregation completed at {}", now.to_rfc3339());
}

/// Run daily aggregation: compute min/max/avg/std_dev per pod per metric
/// for the last 24 hours from raw `hardware_telemetry`.
pub async fn run_daily_aggregation(pool: &SqlitePool) {
    let now = Utc::now();
    let period_start = now - chrono::Duration::hours(24);
    let period_start_str = period_start.to_rfc3339();
    let now_str = now.to_rfc3339();

    for metric in METRIC_COLUMNS {
        let query = format!(
            "INSERT OR REPLACE INTO telemetry_aggregates
                (pod_id, metric_name, period_start, period_hours, min_val, max_val, avg_val, std_dev, sample_count)
             SELECT
                pod_id,
                '{metric}' AS metric_name,
                ?1 AS period_start,
                24 AS period_hours,
                MIN(CAST({metric} AS REAL)),
                MAX(CAST({metric} AS REAL)),
                AVG(CAST({metric} AS REAL)),
                CASE WHEN COUNT({metric}) > 1
                    THEN SQRT(MAX(0.0, AVG(CAST({metric} AS REAL) * CAST({metric} AS REAL)) - AVG(CAST({metric} AS REAL)) * AVG(CAST({metric} AS REAL))))
                    ELSE 0.0
                END,
                COUNT({metric})
             FROM hardware_telemetry
             WHERE collected_at >= ?1 AND collected_at < ?2 AND {metric} IS NOT NULL
             GROUP BY pod_id",
            metric = metric,
        );

        if let Err(e) = sqlx::query(&query)
            .bind(&period_start_str)
            .bind(&now_str)
            .execute(pool)
            .await
        {
            tracing::warn!("v29.0: daily aggregation failed for {}: {}", metric, e);
        }
    }

    tracing::info!("v29.0: daily aggregation completed at {}", now.to_rfc3339());
}

/// Retention cleanup:
/// - Raw hardware_telemetry older than 7 days
/// - Hourly aggregates older than 30 days
/// - Daily aggregates older than 90 days
/// Then incremental vacuum.
pub async fn run_retention_cleanup(pool: &SqlitePool) {
    let now = Utc::now();

    // 1. Raw hardware_telemetry > 7 days
    let raw_cutoff = (now - chrono::Duration::days(7)).to_rfc3339();
    match sqlx::query("DELETE FROM hardware_telemetry WHERE collected_at < ?1")
        .bind(&raw_cutoff)
        .execute(pool)
        .await
    {
        Ok(res) => {
            if res.rows_affected() > 0 {
                tracing::info!("v29.0: retention cleanup deleted {} raw hw_telemetry rows", res.rows_affected());
            }
        }
        Err(e) => tracing::warn!("v29.0: retention cleanup (raw) failed: {}", e),
    }

    // 2. Hourly aggregates > 30 days
    let hourly_cutoff = (now - chrono::Duration::days(30)).to_rfc3339();
    match sqlx::query("DELETE FROM telemetry_aggregates WHERE period_hours = 1 AND period_start < ?1")
        .bind(&hourly_cutoff)
        .execute(pool)
        .await
    {
        Ok(res) => {
            if res.rows_affected() > 0 {
                tracing::info!("v29.0: retention cleanup deleted {} hourly aggregate rows", res.rows_affected());
            }
        }
        Err(e) => tracing::warn!("v29.0: retention cleanup (hourly) failed: {}", e),
    }

    // 3. Daily aggregates > 90 days
    let daily_cutoff = (now - chrono::Duration::days(90)).to_rfc3339();
    match sqlx::query("DELETE FROM telemetry_aggregates WHERE period_hours = 24 AND period_start < ?1")
        .bind(&daily_cutoff)
        .execute(pool)
        .await
    {
        Ok(res) => {
            if res.rows_affected() > 0 {
                tracing::info!("v29.0: retention cleanup deleted {} daily aggregate rows", res.rows_affected());
            }
        }
        Err(e) => tracing::warn!("v29.0: retention cleanup (daily) failed: {}", e),
    }

    // Reclaim disk space
    if let Err(e) = sqlx::query("PRAGMA incremental_vacuum").execute(pool).await {
        tracing::warn!("v29.0: retention incremental_vacuum failed: {}", e);
    }

    tracing::info!("v29.0: retention cleanup completed at {}", now.to_rfc3339());
}

/// Spawn the background maintenance scheduler:
/// - Hourly aggregation every hour
/// - Daily aggregation at 03:00 IST (21:30 UTC)
/// - Retention cleanup at 03:30 IST (22:00 UTC)
pub fn spawn_maintenance_scheduler(pool: SqlitePool) {
    tokio::spawn(async move {
        tracing::info!("v29.0: maintenance scheduler started");

        let mut hourly_interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        // Skip first tick (fires immediately)
        hourly_interval.tick().await;

        // Track last daily run date (UTC) to avoid duplicate runs
        let mut last_daily_date: Option<chrono::NaiveDate> = None;
        let mut last_cleanup_date: Option<chrono::NaiveDate> = None;

        loop {
            // Wait for next hourly tick
            hourly_interval.tick().await;

            // Always run hourly aggregation
            run_hourly_aggregation(&pool).await;

            // Check if it's time for daily tasks
            // IST = UTC + 5:30. 03:00 IST = 21:30 UTC (previous day).
            // 03:30 IST = 22:00 UTC (previous day).
            let now_utc = Utc::now();
            let ist_hour = {
                let ist = now_utc + chrono::Duration::minutes(330); // UTC+5:30
                ist.hour()
            };
            let ist_date = {
                let ist = now_utc + chrono::Duration::minutes(330);
                ist.date_naive()
            };

            // Daily aggregation at 03:xx IST (we check the hour since we tick hourly)
            if ist_hour == 3 && last_daily_date != Some(ist_date) {
                last_daily_date = Some(ist_date);
                run_daily_aggregation(&pool).await;
            }

            // Retention cleanup also at 03:xx IST (runs after daily aggregation in the same hour)
            if ist_hour == 3 && last_cleanup_date != Some(ist_date) {
                last_cleanup_date = Some(ist_date);
                // Small delay so daily aggregation finishes first
                tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                run_retention_cleanup(&pool).await;
            }
        }
    });
}

/// Compute trend analysis for a metric over a time window using linear regression
/// on daily aggregates.
pub async fn get_metric_trend(
    pool: &SqlitePool,
    pod_id: &str,
    metric_name: &str,
    window_days: u32,
) -> anyhow::Result<MetricTrend> {
    let cutoff = (Utc::now() - chrono::Duration::days(window_days as i64)).to_rfc3339();

    // Fetch daily aggregates sorted by period_start
    let rows = sqlx::query_as::<_, (String, f64)>(
        "SELECT period_start, avg_val FROM telemetry_aggregates
         WHERE pod_id = ?1 AND metric_name = ?2 AND period_hours = 24
           AND period_start >= ?3
         ORDER BY period_start ASC"
    )
    .bind(pod_id)
    .bind(metric_name)
    .bind(&cutoff)
    .fetch_all(pool)
    .await?;

    let n = rows.len() as f64;
    if rows.is_empty() {
        return Ok(MetricTrend {
            current_value: 0.0,
            trend: "stable".to_string(),
            rate_per_day: 0.0,
            predicted_failure_date: None,
            confidence: 0.0,
            data_points: 0,
        });
    }

    // Use sequential indices (0, 1, 2, ...) as x values (each = 1 day)
    let ys: Vec<f64> = rows.iter().map(|(_, v)| *v).collect();
    let current_value = *ys.last().unwrap_or(&0.0);

    // Simple linear regression: y = slope * x + intercept
    let sum_x: f64 = (0..rows.len()).map(|i| i as f64).sum();
    let sum_y: f64 = ys.iter().sum();
    let sum_xy: f64 = ys.iter().enumerate().map(|(i, y)| i as f64 * y).sum();
    let sum_x2: f64 = (0..rows.len()).map(|i| (i as f64) * (i as f64)).sum();

    let denom = n * sum_x2 - sum_x * sum_x;
    let (slope, _intercept) = if denom.abs() < 1e-12 {
        (0.0, current_value)
    } else {
        let s = (n * sum_xy - sum_x * sum_y) / denom;
        let i = (sum_y - s * sum_x) / n;
        (s, i)
    };

    // R² (coefficient of determination)
    let mean_y = sum_y / n;
    let ss_tot: f64 = ys.iter().map(|y| (y - mean_y).powi(2)).sum();
    let ss_res: f64 = ys.iter().enumerate().map(|(i, y)| {
        let predicted = slope * i as f64 + (sum_y - slope * sum_x) / n;
        (y - predicted).powi(2)
    }).sum();
    let r_squared = if ss_tot.abs() < 1e-12 { 1.0 } else { 1.0 - ss_res / ss_tot };
    let confidence = r_squared.max(0.0).min(1.0);

    // Classify trend
    let trend = if slope.abs() < 0.5 {
        "stable".to_string()
    } else if slope > 0.0 {
        "rising".to_string()
    } else {
        "declining".to_string()
    };

    // Predicted failure date: only for declining health metrics
    // (e.g., disk_smart_health_pct declining toward 0)
    let predicted_failure_date = if trend == "declining" && confidence > 0.5 {
        // Days until value reaches 0 from current position
        let days_to_zero = if slope.abs() > 1e-6 {
            (-current_value / slope).max(0.0)
        } else {
            f64::MAX
        };
        if days_to_zero < 365.0 && days_to_zero > 0.0 {
            let failure_date = Utc::now() + chrono::Duration::days(days_to_zero as i64);
            Some(failure_date.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        } else {
            None
        }
    } else {
        None
    };

    Ok(MetricTrend {
        current_value,
        trend,
        rate_per_day: slope,
        predicted_failure_date,
        confidence,
        data_points: rows.len() as u32,
    })
}

// ─── v29.0: Extended Hardware Telemetry Storage ─────────────────────────────

use rc_common::protocol::AgentMessage;
use std::sync::Arc;

/// Store an ExtendedTelemetry message in the hardware_telemetry table.
/// Called from ws/mod.rs when an ExtendedTelemetry message arrives.
/// Non-blocking: spawns a background task for the DB write.
pub fn store_extended_telemetry(
    state: &Arc<crate::state::AppState>,
    pod_id: &str,
    msg: &AgentMessage,
) {
    let AgentMessage::ExtendedTelemetry {
        gpu_temp_celsius,
        cpu_temp_celsius,
        gpu_power_watts,
        vram_usage_mb,
        disk_smart_health_pct,
        disk_power_on_hours,
        game_crashes_last_hour,
        windows_critical_errors,
        process_handle_count,
        system_uptime_secs,
        cpu_usage_pct,
        gpu_usage_pct,
        memory_usage_pct,
        disk_usage_pct,
        network_latency_ms,
        usb_device_count,
        fan_speeds_rpm,
        collected_at,
        ..
    } = msg
    else {
        return;
    };

    let pool = match &state.telemetry_db {
        Some(p) => p.clone(),
        None => return, // telemetry DB not initialized
    };

    let pod_id = pod_id.to_string();
    let collected_at = collected_at.clone();
    let win_errors = serde_json::to_string(windows_critical_errors).unwrap_or_default();
    let fan_speeds = fan_speeds_rpm
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    // Clone Option values for the async block
    let gpu_temp = *gpu_temp_celsius;
    let cpu_temp = *cpu_temp_celsius;
    let gpu_power = *gpu_power_watts;
    let vram = vram_usage_mb.map(|v| v as i64);
    let smart = disk_smart_health_pct.map(|v| v as i64);
    let disk_hours = disk_power_on_hours.map(|v| v as i64);
    let crashes = game_crashes_last_hour.map(|v| v as i64);
    let handles = process_handle_count.map(|v| v as i64);
    let uptime = system_uptime_secs.map(|v| v as i64);
    let cpu = *cpu_usage_pct;
    let gpu = *gpu_usage_pct;
    let mem = *memory_usage_pct;
    let disk = *disk_usage_pct;
    let latency = network_latency_ms.map(|v| v as i64);
    let usb = usb_device_count.map(|v| v as i64);

    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT INTO hardware_telemetry (
                pod_id, collected_at,
                gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts, vram_usage_mb,
                disk_smart_health_pct, disk_power_on_hours, game_crashes_last_hour,
                windows_critical_errors, process_handle_count, system_uptime_secs,
                cpu_usage_pct, gpu_usage_pct, memory_usage_pct, disk_usage_pct,
                network_latency_ms, usb_device_count, fan_speeds_rpm
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
            )"
        )
        .bind(&pod_id)
        .bind(&collected_at)
        .bind(gpu_temp)
        .bind(cpu_temp)
        .bind(gpu_power)
        .bind(vram)
        .bind(smart)
        .bind(disk_hours)
        .bind(crashes)
        .bind(&win_errors)
        .bind(handles)
        .bind(uptime)
        .bind(cpu)
        .bind(gpu)
        .bind(mem)
        .bind(disk)
        .bind(latency)
        .bind(usb)
        .bind(&fan_speeds)
        .execute(&pool)
        .await;

        if let Err(e) = result {
            tracing::warn!("v29.0: Failed to store hardware telemetry for {}: {}", pod_id, e);
        }
    });
}
