//! v34.0 Phase 285: Time-series metrics ring buffer -- SQLite TSDB for venue metrics.

use chrono::{Utc, Duration};
use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;

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
