//! Phase 2 (v29.0): Maintenance event & task persistence (SQLite).
//!
//! CRUD operations for `maintenance_events` and `maintenance_tasks` tables,
//! stored in the main `racecontrol.db` alongside billing/session data.
//!
//! Business metrics, HR, attendance, payroll, KPIs, and auto-assignment have been
//! extracted to `business_store` and `hr_store` modules. Re-exports below preserve
//! backward compatibility for callers using `maintenance_store::*`.

#[path = "maintenance_store_rows.rs"]
mod rows;
use rows::{row_to_event, row_to_task, EventRow, TaskRow};

use crate::maintenance_models::{
    MaintenanceEvent, MaintenanceEventType, MaintenanceSummary, MaintenanceTask, ResolutionMethod,
    Severity, TaskStatus,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Re-exports for backward compatibility
// ---------------------------------------------------------------------------

pub use crate::business_store::{
    auto_assign_task, calculate_kpis, get_ebitda_summary, init_business_tables,
    query_business_metrics, upsert_daily_metrics, MaintenanceKPIs,
};
pub use crate::hr_store::{
    calculate_monthly_payroll, get_employee, init_hr_tables, insert_employee, list_employees,
    query_attendance, record_attendance, update_employee,
};

// ---------------------------------------------------------------------------
// Table creation
// ---------------------------------------------------------------------------

/// Create the maintenance tables if they don't already exist.
pub async fn init_maintenance_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS maintenance_events (
            id TEXT PRIMARY KEY,
            pod_id INTEGER,
            event_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            component TEXT NOT NULL,
            description TEXT NOT NULL,
            detected_at TEXT NOT NULL,
            resolved_at TEXT,
            resolution_method TEXT,
            source TEXT NOT NULL,
            correlation_id TEXT,
            revenue_impact_paise INTEGER,
            customers_affected INTEGER,
            downtime_minutes INTEGER,
            cost_estimate_paise INTEGER,
            assigned_staff_id TEXT,
            metadata TEXT NOT NULL DEFAULT '{}'
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_maint_events_pod ON maintenance_events(pod_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_maint_events_detected ON maintenance_events(detected_at)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS maintenance_tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            pod_id INTEGER,
            component TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 3,
            status TEXT NOT NULL DEFAULT 'Open',
            created_at TEXT NOT NULL,
            due_by TEXT,
            assigned_to TEXT,
            source_event_id TEXT,
            before_metrics TEXT,
            after_metrics TEXT,
            cost_estimate_paise INTEGER,
            actual_cost_paise INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_maint_tasks_status ON maintenance_tasks(status)",
    )
    .execute(pool)
    .await?;

    tracing::info!("Maintenance tables initialized");
    Ok(())
}

// ---------------------------------------------------------------------------
// Events — insert / query / summary
// ---------------------------------------------------------------------------

/// Insert a new maintenance event.
pub async fn insert_event(pool: &SqlitePool, event: &MaintenanceEvent) -> anyhow::Result<()> {
    let event_type_str = serde_json::to_string(&event.event_type)?;
    let severity_str = serde_json::to_string(&event.severity)?;
    let component_str = serde_json::to_string(&event.component)?;
    let resolution_str = event
        .resolution_method
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let metadata_str = serde_json::to_string(&event.metadata)?;

    sqlx::query(
        "INSERT INTO maintenance_events
            (id, pod_id, event_type, severity, component, description,
             detected_at, resolved_at, resolution_method, source,
             correlation_id, revenue_impact_paise, customers_affected,
             downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
    )
    .bind(event.id.to_string())
    // MMA-R1: Use i64::from for infallible widening instead of `as`
    .bind(event.pod_id.map(i64::from))
    .bind(&event_type_str)
    .bind(&severity_str)
    .bind(&component_str)
    .bind(&event.description)
    .bind(event.detected_at.to_rfc3339())
    .bind(event.resolved_at.map(|t| t.to_rfc3339()))
    .bind(&resolution_str)
    .bind(&event.source)
    .bind(event.correlation_id.map(|u| u.to_string()))
    .bind(event.revenue_impact_paise)
    .bind(event.customers_affected.map(i64::from))
    .bind(event.downtime_minutes.map(i64::from))
    .bind(event.cost_estimate_paise)
    .bind(&event.assigned_staff_id)
    .bind(&metadata_str)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query maintenance events with optional filters.
pub async fn query_events(
    pool: &SqlitePool,
    pod_id: Option<u8>,
    since: Option<DateTime<Utc>>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceEvent>> {
    // Build a simple dynamic query — SQLite doesn't have great dynamic support
    // in sqlx, so we use a broad query and filter in Rust for simplicity.
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events
         ORDER BY detected_at DESC
         LIMIT ?1",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let evt = row_to_event(row)?;
        // Apply optional filters
        if let Some(pid) = pod_id
            && evt.pod_id != Some(pid) {
                continue;
            }
        if let Some(ref s) = since
            && evt.detected_at < *s {
                continue;
            }
        events.push(evt);
    }
    Ok(events)
}

/// Get a summary of maintenance events (last 24h by default).
pub async fn get_summary(pool: &SqlitePool) -> anyhow::Result<MaintenanceSummary> {
    let since = Utc::now() - chrono::Duration::hours(24);

    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, pod_id, event_type, severity, component, description,
                detected_at, resolved_at, resolution_method, source,
                correlation_id, revenue_impact_paise, customers_affected,
                downtime_minutes, cost_estimate_paise, assigned_staff_id, metadata
         FROM maintenance_events
         WHERE detected_at >= ?1
         ORDER BY detected_at DESC
         LIMIT 5000",
    )
    .bind(since.to_rfc3339())
    .fetch_all(pool)
    .await?;

    let total_events = rows.len() as u32;
    let mut by_severity = std::collections::HashMap::<String, u32>::new();
    let mut by_type = std::collections::HashMap::<String, u32>::new();
    let mut resolved_count = 0u32;
    let mut total_ttrs = 0f64;
    let mut self_heal_count = 0u32;

    for row in &rows {
        // severity
        let sev: Severity = serde_json::from_str(&row.severity).unwrap_or(Severity::Medium);
        let sev_label = serde_json::to_string(&sev).unwrap_or_default().replace('"', "");
        *by_severity.entry(sev_label).or_default() += 1;

        // type
        let etype: MaintenanceEventType = serde_json::from_str(&row.event_type)
            .unwrap_or(MaintenanceEventType::SelfHealAttempted);
        let type_label = serde_json::to_string(&etype).unwrap_or_default().replace('"', "");
        *by_type.entry(type_label).or_default() += 1;

        // MTTR
        if let (Some(det), Some(res)) = (&row.detected_at_str, &row.resolved_at_str)
            && let (Ok(d), Ok(r)) = (
                DateTime::parse_from_rfc3339(det),
                DateTime::parse_from_rfc3339(res),
            ) {
                let mins = (r - d).num_minutes() as f64;
                if mins >= 0.0 {
                    total_ttrs += mins;
                    resolved_count += 1;
                }
            }

        // Self-heal
        if let Some(ref rm) = row.resolution_method
            && let Ok(ResolutionMethod::AutoHealed(_)) = serde_json::from_str(rm) {
                self_heal_count += 1;
            }
    }

    let mttr_minutes = if resolved_count > 0 {
        total_ttrs / resolved_count as f64
    } else {
        0.0
    };
    let self_heal_rate = if total_events > 0 {
        self_heal_count as f64 / total_events as f64
    } else {
        0.0
    };

    // Open tasks
    let open_row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_tasks WHERE status IN ('Open','Assigned','InProgress')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    Ok(MaintenanceSummary {
        total_events,
        by_severity,
        by_type,
        mttr_minutes,
        self_heal_rate,
        // MMA-R1: Use try_from instead of `as` for safe narrowing
        open_tasks: u32::try_from(open_row.0.max(0)).unwrap_or(0),
    })
}

// ---------------------------------------------------------------------------
// Tasks — insert / query / update
// ---------------------------------------------------------------------------

/// Insert a new maintenance task.
pub async fn insert_task(pool: &SqlitePool, task: &MaintenanceTask) -> anyhow::Result<()> {
    let component_str = serde_json::to_string(&task.component)?;
    let status_str = serde_json::to_string(&task.status)?.replace('"', "");
    let before_str = task
        .before_metrics
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let after_str = task
        .after_metrics
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    sqlx::query(
        "INSERT INTO maintenance_tasks
            (id, title, description, pod_id, component, priority, status,
             created_at, due_by, assigned_to, source_event_id,
             before_metrics, after_metrics, cost_estimate_paise, actual_cost_paise)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
    )
    .bind(task.id.to_string())
    .bind(&task.title)
    .bind(&task.description)
    // MMA-R1: Use i64::from for infallible widening instead of `as`
    .bind(task.pod_id.map(i64::from))
    .bind(&component_str)
    .bind(i64::from(task.priority))
    .bind(&status_str)
    .bind(task.created_at.to_rfc3339())
    .bind(task.due_by.map(|t| t.to_rfc3339()))
    .bind(&task.assigned_to)
    .bind(task.source_event_id.map(|u| u.to_string()))
    .bind(&before_str)
    .bind(&after_str)
    .bind(task.cost_estimate_paise)
    .bind(task.actual_cost_paise)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query maintenance tasks with optional status filter.
pub async fn query_tasks(
    pool: &SqlitePool,
    status_filter: Option<&str>,
    limit: u32,
) -> anyhow::Result<Vec<MaintenanceTask>> {
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT id, title, description, pod_id, component, priority, status,
                created_at, due_by, assigned_to, source_event_id,
                before_metrics, after_metrics, cost_estimate_paise, actual_cost_paise
         FROM maintenance_tasks
         ORDER BY priority ASC, created_at DESC
         LIMIT ?1",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    let mut tasks = Vec::with_capacity(rows.len());
    for row in rows {
        let task = row_to_task(row)?;
        if let Some(filter) = status_filter {
            let status_str = serde_json::to_string(&task.status)?.replace('"', "");
            if status_str != filter {
                continue;
            }
        }
        tasks.push(task);
    }
    Ok(tasks)
}

/// Update the status of a maintenance task.
pub async fn update_task_status(
    pool: &SqlitePool,
    task_id: Uuid,
    new_status: &TaskStatus,
) -> anyhow::Result<bool> {
    let status_str = serde_json::to_string(new_status)?.replace('"', "");
    let result = sqlx::query(
        "UPDATE maintenance_tasks SET status = ?1 WHERE id = ?2",
    )
    .bind(&status_str)
    .bind(task_id.to_string())
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

