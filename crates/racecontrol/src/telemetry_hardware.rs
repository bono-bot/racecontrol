//! v29.0: Hardware telemetry — aggregation, retention, maintenance scheduler,
//! trend analysis, and extended telemetry storage.
//! Extracted from telemetry_store.rs for module size compliance.

use chrono::{Timelike, Utc};
use rc_common::protocol::AgentMessage;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

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
    /// Confidence of the trend (0.0-1.0), based on R²
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
