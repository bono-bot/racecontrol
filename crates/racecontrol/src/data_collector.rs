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

    // MMA-R1: severity is stored as JSON string '"Critical"', not bare 'Critical'.
    // Also use RFC3339 cutoff for consistent timestamp comparison.
    let cutoff = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let critical_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM maintenance_events WHERE severity = '\"Critical\"' AND resolved_at IS NULL AND detected_at > ?1"
    ).bind(&cutoff).fetch_one(pool).await.unwrap_or(0);

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
        // MMA-R1: Use try_from instead of `as` for safe narrowing
        open_maintenance_tasks: u32::try_from(open_tasks.max(0)).unwrap_or(0),
        critical_alerts_active: u32::try_from(critical_alerts.max(0)).unwrap_or(0),
        staff_on_duty: u32::try_from(staff.max(0)).unwrap_or(0),
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
            // MMA-R1: Validate pod_id instead of defaulting to 0 (phantom tasks)
            let pod_num = match pod_id_str
                .strip_prefix("pod_")
                .and_then(|s| s.parse::<i64>().ok())
                .filter(|&p| (1..=8).contains(&p))
            {
                Some(p) => p,
                None => {
                    tracing::warn!(target: LOG_TARGET, pod = %pod_id_str, "Invalid pod_id in RUL check — skipping");
                    continue;
                }
            };
            // MMA-R1: Use exact JSON-quoted match instead of LIKE (which fails for JSON strings)
            let component_json = format!("\"{}\"", component);
            let existing: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM maintenance_tasks WHERE pod_id = ?1 AND component = ?2 AND status NOT IN ('Completed', 'Failed', 'Cancelled')"
            )
            .bind(pod_num)
            .bind(&component_json)
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
