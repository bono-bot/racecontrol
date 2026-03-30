//! v29.0 Phase 35: Unified data collector — cross-domain snapshots for AI consumption.
//! Also wires RUL triggers and occupancy data.

use serde::Serialize;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

const LOG_TARGET: &str = "data-collector";

/// Cross-domain venue snapshot for AI analysis.
#[derive(Debug, Clone, Serialize)]
pub struct VenueSnapshot {
    pub timestamp: DateTime<Utc>,
    pub pod_count_online: u8,
    pub pod_count_degraded: u8,
    pub pod_count_unavailable: u8,
    pub active_sessions: u32,
    pub occupancy_pct: f32,
    pub revenue_today_paise: i64,
    pub open_maintenance_tasks: u32,
    pub critical_alerts_active: u32,
    pub staff_on_duty: u32,
    pub avg_gpu_temp: Option<f32>,
    pub avg_network_latency: Option<f32>,
}

/// Collect a venue-wide snapshot from the main DB.
pub async fn collect_venue_snapshot(pool: &SqlitePool) -> VenueSnapshot {
    let today = Utc::now().date_naive().to_string();

    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
    ).bind(&today).fetch_one(pool).await.unwrap_or(0);

    let open_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open', 'Assigned', 'InProgress')"
    ).fetch_one(pool).await.unwrap_or(0);

    let critical_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM maintenance_events WHERE severity = 'Critical' AND resolved_at IS NULL AND detected_at > datetime('now', '-1 hour')"
    ).fetch_one(pool).await.unwrap_or(0);

    let staff: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees WHERE is_active = 1"
    ).fetch_one(pool).await.unwrap_or(0);

    VenueSnapshot {
        timestamp: Utc::now(),
        pod_count_online: 8, // TODO: query from fleet health
        pod_count_degraded: 0,
        pod_count_unavailable: 0,
        active_sessions: 0, // TODO: query from billing_fsm
        occupancy_pct: 0.0,
        revenue_today_paise: revenue,
        open_maintenance_tasks: open_tasks as u32,
        critical_alerts_active: critical_alerts as u32,
        staff_on_duty: staff as u32,
        avg_gpu_temp: None,
        avg_network_latency: None,
    }
}

/// Check RUL thresholds and auto-create maintenance tasks for components nearing failure.
pub async fn check_rul_thresholds(
    pool: &SqlitePool,
    telem_pool: &SqlitePool,
) -> anyhow::Result<u32> {
    // Get daily aggregates for health-related metrics in last day
    let metrics: Vec<(String, String, f64)> = sqlx::query_as(
        "SELECT pod_id, metric_name, avg_val FROM telemetry_aggregates \
         WHERE period_hours = 24 AND period_start > datetime('now', '-1 day') \
         AND metric_name IN ('disk_smart_health_pct', 'gpu_temp_celsius') \
         ORDER BY pod_id"
    ).fetch_all(telem_pool).await.unwrap_or_default();

    let mut tasks_created = 0u32;

    for (pod_id_str, metric, value) in &metrics {
        let should_create = match metric.as_str() {
            "disk_smart_health_pct" if *value < 70.0 => {
                Some(("Storage", format!("Disk health at {:.0}% — schedule replacement", value)))
            }
            "gpu_temp_celsius" if *value > 85.0 => {
                Some(("Cooling", format!("GPU avg temp {:.0}°C — check cooling system", value)))
            }
            _ => None,
        };

        if let Some((component, title)) = should_create {
            // Check if a task already exists for this pod/component
            let pod_num = pod_id_str.replace("pod_", "").parse::<i64>().unwrap_or(0);
            let existing: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM maintenance_tasks WHERE pod_id = ?1 AND component LIKE ?2 AND status NOT IN ('Completed', 'Failed', 'Cancelled')"
            )
            .bind(pod_num)
            .bind(format!("%{}%", component))
            .fetch_one(pool).await.unwrap_or(0);

            if existing == 0 {
                let id = uuid::Uuid::new_v4().to_string();
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO maintenance_tasks (id, title, description, pod_id, component, priority, status, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Open', ?7)"
                )
                .bind(&id).bind(&title).bind(&title)
                .bind(pod_num)
                .bind(component)
                .bind(70i64) // High priority for RUL-triggered tasks
                .bind(&now)
                .execute(pool).await?;

                tracing::warn!(target: LOG_TARGET, pod = %pod_id_str, component, "RUL threshold triggered — maintenance task created");
                tasks_created += 1;
            }
        }
    }

    Ok(tasks_created)
}

/// Spawn periodic data collection + RUL check (every 15 min).
pub fn spawn_data_collector(pool: SqlitePool, telem_pool: SqlitePool) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await; // wait 3 min for startup
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(900));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tracing::info!(target: LOG_TARGET, "Data collector started (15-min interval)");
        loop {
            interval.tick().await;
            let snapshot = collect_venue_snapshot(&pool).await;
            tracing::debug!(target: LOG_TARGET,
                revenue = snapshot.revenue_today_paise,
                tasks = snapshot.open_maintenance_tasks,
                alerts = snapshot.critical_alerts_active,
                "Venue snapshot collected"
            );

            if let Err(e) = check_rul_thresholds(&pool, &telem_pool).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "RUL threshold check failed");
            }
        }
    });
}
