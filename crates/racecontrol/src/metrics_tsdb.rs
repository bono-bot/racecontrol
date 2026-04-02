//! v34.0 Phase 285: Time-series metrics ring buffer -- SQLite TSDB for venue metrics.

use chrono::{Utc, Duration};
use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

const LOG_TARGET: &str = "metrics-tsdb";

/// Known metric names (TSDB-05).
pub const METRIC_CPU_USAGE: &str = "cpu_usage";
pub const METRIC_GPU_TEMP: &str = "gpu_temp";
pub const METRIC_FPS: &str = "fps";
pub const METRIC_BILLING_REVENUE: &str = "billing_revenue";
pub const METRIC_WS_CONNECTIONS: &str = "ws_connections";
pub const METRIC_POD_HEALTH_SCORE: &str = "pod_health_score";
pub const METRIC_GAME_SESSION_COUNT: &str = "game_session_count";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub metric_name: String,
    pub pod_id: Option<String>,
    pub value: f64,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRollup {
    pub resolution: String,
    pub metric_name: String,
    pub pod_id: Option<String>,
    pub min_value: f64,
    pub max_value: f64,
    pub avg_value: f64,
    pub sample_count: i64,
    pub period_start: String,
}

/// Insert a single metric sample (TSDB-01).
pub async fn record_sample(pool: &SqlitePool, sample: &MetricSample) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at) VALUES (?1, ?2, ?3, ?4)"
    )
    .bind(&sample.metric_name)
    .bind(&sample.pod_id)
    .bind(sample.value)
    .bind(&sample.recorded_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert a batch of metric samples (for async batched writes -- TSDB-06).
pub async fn record_samples_batch(pool: &SqlitePool, samples: &[MetricSample]) -> Result<usize, sqlx::Error> {
    let mut count = 0usize;
    for sample in samples {
        sqlx::query(
            "INSERT INTO metrics_samples (metric_name, pod_id, value, recorded_at) VALUES (?1, ?2, ?3, ?4)"
        )
        .bind(&sample.metric_name)
        .bind(&sample.pod_id)
        .bind(sample.value)
        .bind(&sample.recorded_at)
        .execute(pool)
        .await?;
        count += 1;
    }
    Ok(count)
}

/// Compute hourly rollups from raw samples (TSDB-03).
/// Aggregates samples from the previous full hour into metrics_rollups.
pub async fn compute_hourly_rollups(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    let now = Utc::now();
    let hour_start = now.format("%Y-%m-%dT%H:00:00").to_string();
    let prev_hour = (now - Duration::hours(1)).format("%Y-%m-%dT%H:00:00").to_string();

    let result = sqlx::query(
        "INSERT OR IGNORE INTO metrics_rollups (resolution, metric_name, pod_id, min_value, max_value, avg_value, sample_count, period_start)
         SELECT 'hourly', metric_name, pod_id, MIN(value), MAX(value), AVG(value), COUNT(*), ?1
         FROM metrics_samples
         WHERE recorded_at >= ?1 AND recorded_at < ?2
         GROUP BY metric_name, pod_id"
    )
    .bind(&prev_hour)
    .bind(&hour_start)
    .execute(pool)
    .await?;

    let rows = result.rows_affected() as usize;
    if rows > 0 {
        tracing::info!(target: LOG_TARGET, rows, "Computed hourly rollups for {}", prev_hour);
    }
    Ok(rows)
}

/// Compute daily rollups from hourly rollups (TSDB-04).
/// Aggregates hourly rollups from the previous full day into daily rollups.
pub async fn compute_daily_rollups(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    let now = Utc::now();
    let today = now.format("%Y-%m-%dT00:00:00").to_string();
    let yesterday = (now - Duration::days(1)).format("%Y-%m-%dT00:00:00").to_string();

    let result = sqlx::query(
        "INSERT OR IGNORE INTO metrics_rollups (resolution, metric_name, pod_id, min_value, max_value, avg_value, sample_count, period_start)
         SELECT 'daily', metric_name, pod_id,
                MIN(min_value), MAX(max_value),
                SUM(avg_value * sample_count) / SUM(sample_count),
                SUM(sample_count), ?1
         FROM metrics_rollups
         WHERE resolution = 'hourly' AND period_start >= ?1 AND period_start < ?2
         GROUP BY metric_name, pod_id"
    )
    .bind(&yesterday)
    .bind(&today)
    .execute(pool)
    .await?;

    let rows = result.rows_affected() as usize;
    if rows > 0 {
        tracing::info!(target: LOG_TARGET, rows, "Computed daily rollups for {}", yesterday);
    }
    Ok(rows)
}

/// Channel sender for non-blocking metric ingestion (TSDB-06).
pub type MetricsSender = mpsc::Sender<MetricSample>;

/// Spawn the async ingestion task. Returns a Sender callers use to submit samples.
/// Batches up to 64 samples or flushes every 5 seconds (whichever comes first).
pub fn spawn_metrics_ingestion(pool: SqlitePool) -> MetricsSender {
    let (tx, mut rx) = mpsc::channel::<MetricSample>(512);
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "metrics-ingestion task started (batch=64, flush=5s)");
        let mut batch: Vec<MetricSample> = Vec::with_capacity(64);
        let mut flush_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(sample) = rx.recv() => {
                    batch.push(sample);
                    if batch.len() >= 64 {
                        if let Err(e) = record_samples_batch(&pool, &batch).await {
                            tracing::warn!(target: LOG_TARGET, error = %e, "Failed to write metrics batch");
                        }
                        batch.clear();
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        if let Err(e) = record_samples_batch(&pool, &batch).await {
                            tracing::warn!(target: LOG_TARGET, error = %e, "Failed to flush metrics batch");
                        }
                        batch.clear();
                    }
                }
                else => {
                    // Channel closed -- flush remaining
                    if !batch.is_empty() {
                        let _ = record_samples_batch(&pool, &batch).await;
                    }
                    tracing::info!(target: LOG_TARGET, "metrics-ingestion task exiting");
                    break;
                }
            }
        }
    });
    tx
}

/// Purge raw samples older than 7 days (TSDB-02, TSDB-07).
pub async fn purge_old_samples(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let threshold = (Utc::now() - Duration::days(7)).format("%Y-%m-%dT%H:%M:%S").to_string();
    let result = sqlx::query("DELETE FROM metrics_samples WHERE recorded_at < ?1")
        .bind(&threshold)
        .execute(pool)
        .await?;
    let deleted = result.rows_affected();
    if deleted > 0 {
        tracing::info!(target: LOG_TARGET, deleted, "Purged raw samples older than 7 days");
    }
    Ok(deleted)
}

/// Purge rollups older than 90 days (TSDB-07).
pub async fn purge_old_rollups(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let threshold = (Utc::now() - Duration::days(90)).format("%Y-%m-%dT%H:%M:%S").to_string();
    let result = sqlx::query("DELETE FROM metrics_rollups WHERE period_start < ?1")
        .bind(&threshold)
        .execute(pool)
        .await?;
    let deleted = result.rows_affected();
    if deleted > 0 {
        tracing::info!(target: LOG_TARGET, deleted, "Purged rollups older than 90 days");
    }
    Ok(deleted)
}

/// Spawn background task for hourly rollups, daily rollups, and purge.
/// Runs every 60 minutes. On each tick: hourly rollup, daily rollup, purge.
pub fn spawn_rollup_and_purge(pool: SqlitePool) {
    tokio::spawn(async move {
        // Wait 2 min for system to stabilize
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        tracing::info!(target: LOG_TARGET, "rollup-and-purge task started (60min interval)");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // Hourly rollup
            if let Err(e) = compute_hourly_rollups(&pool).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "Hourly rollup failed");
            }

            // Daily rollup (run every tick -- INSERT OR IGNORE makes it idempotent)
            if let Err(e) = compute_daily_rollups(&pool).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "Daily rollup failed");
            }

            // Purge old data
            if let Err(e) = purge_old_samples(&pool).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "Sample purge failed");
            }
            if let Err(e) = purge_old_rollups(&pool).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "Rollup purge failed");
            }
        }
    });
}
