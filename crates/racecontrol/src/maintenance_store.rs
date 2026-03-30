//! Phase 2 (v29.0): Maintenance event & task persistence (SQLite).
//!
//! CRUD operations for `maintenance_events` and `maintenance_tasks` tables,
//! stored in the main `racecontrol.db` alongside billing/session data.

use crate::maintenance_models::{
    AttendanceRecord, ComponentType, DailyBusinessMetrics, EbitdaSummary, Employee,
    EmployeePayroll, MaintenanceEvent, MaintenanceEventType, MaintenanceSummary, MaintenanceTask,
    PayrollSummary, ResolutionMethod, Severity, StaffRole, TaskStatus,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

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
        .map(|r| serde_json::to_string(r))
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
    .bind(event.pod_id.map(|p| p as i64))
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
    .bind(event.customers_affected.map(|c| c as i64))
    .bind(event.downtime_minutes.map(|d| d as i64))
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
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let evt = row_to_event(row)?;
        // Apply optional filters
        if let Some(pid) = pod_id {
            if evt.pod_id != Some(pid) {
                continue;
            }
        }
        if let Some(ref s) = since {
            if evt.detected_at < *s {
                continue;
            }
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
        if let (Some(det), Some(res)) = (&row.detected_at_str, &row.resolved_at_str) {
            if let (Ok(d), Ok(r)) = (
                DateTime::parse_from_rfc3339(det),
                DateTime::parse_from_rfc3339(res),
            ) {
                let mins = (r - d).num_minutes() as f64;
                if mins >= 0.0 {
                    total_ttrs += mins;
                    resolved_count += 1;
                }
            }
        }

        // Self-heal
        if let Some(ref rm) = row.resolution_method {
            if let Ok(ResolutionMethod::AutoHealed(_)) = serde_json::from_str(rm) {
                self_heal_count += 1;
            }
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
        open_tasks: open_row.0 as u32,
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
        .map(|v| serde_json::to_string(v))
        .transpose()?;
    let after_str = task
        .after_metrics
        .as_ref()
        .map(|v| serde_json::to_string(v))
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
    .bind(task.pod_id.map(|p| p as i64))
    .bind(&component_str)
    .bind(task.priority as i64)
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
    .bind(limit as i64)
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

// ---------------------------------------------------------------------------
// Internal row types (sqlx::FromRow)
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EventRow {
    id: String,
    pod_id: Option<i64>,
    event_type: String,
    severity: String,
    component: String,
    description: String,
    #[sqlx(rename = "detected_at")]
    detected_at_str: Option<String>,
    #[sqlx(rename = "resolved_at")]
    resolved_at_str: Option<String>,
    resolution_method: Option<String>,
    source: String,
    correlation_id: Option<String>,
    revenue_impact_paise: Option<i64>,
    customers_affected: Option<i64>,
    downtime_minutes: Option<i64>,
    cost_estimate_paise: Option<i64>,
    assigned_staff_id: Option<String>,
    metadata: String,
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    title: String,
    description: String,
    pod_id: Option<i64>,
    component: String,
    priority: i64,
    status: String,
    #[sqlx(rename = "created_at")]
    created_at_str: Option<String>,
    #[sqlx(rename = "due_by")]
    due_by_str: Option<String>,
    assigned_to: Option<String>,
    source_event_id: Option<String>,
    before_metrics: Option<String>,
    after_metrics: Option<String>,
    cost_estimate_paise: Option<i64>,
    actual_cost_paise: Option<i64>,
}

// ---------------------------------------------------------------------------
// Row → model conversions
// ---------------------------------------------------------------------------

fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    // P1-2: Log warning on date parse fallback instead of silent Utc::now()
    let detected_at = match row.detected_at_str.as_deref() {
        Some(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(d) => d.with_timezone(&Utc),
            Err(e) => {
                tracing::warn!(
                    "maintenance_store: detected_at parse failed for event {}: '{}' — {}. Using Utc::now() fallback.",
                    row.id, s, e
                );
                Utc::now()
            }
        },
        None => {
            tracing::warn!(
                "maintenance_store: detected_at is NULL for event {}. Using Utc::now() fallback.",
                row.id
            );
            Utc::now()
        }
    };

    let resolved_at = row
        .resolved_at_str
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));

    Ok(MaintenanceEvent {
        id: Uuid::parse_str(&row.id)?,
        // P1-1: Safe narrowing cast — clamp pod_id to 1-8 range
        pod_id: row.pod_id.and_then(|p| {
            u8::try_from(p.clamp(0, 255)).ok().map(|v| v.clamp(1, 8))
        }),
        event_type: serde_json::from_str(&row.event_type)?,
        severity: serde_json::from_str(&row.severity)?,
        component: serde_json::from_str(&row.component)?,
        description: row.description,
        detected_at,
        resolved_at,
        resolution_method: row
            .resolution_method
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        source: row.source,
        correlation_id: row
            .correlation_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()?,
        revenue_impact_paise: row.revenue_impact_paise,
        // P1-1: Safe narrowing cast for customers_affected
        customers_affected: row.customers_affected.map(|c| {
            u32::try_from(c.max(0)).unwrap_or(u32::MAX)
        }),
        // P1-1: Safe narrowing cast for downtime_minutes
        downtime_minutes: row.downtime_minutes.map(|d| {
            u32::try_from(d.max(0)).unwrap_or(u32::MAX)
        }),
        cost_estimate_paise: row.cost_estimate_paise,
        assigned_staff_id: row.assigned_staff_id,
        metadata: serde_json::from_str(&row.metadata)?,
    })
}

fn row_to_task(row: TaskRow) -> anyhow::Result<MaintenanceTask> {
    // P1-2: Log warning on date parse fallback instead of silent Utc::now()
    let created_at = match row.created_at_str.as_deref() {
        Some(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(d) => d.with_timezone(&Utc),
            Err(e) => {
                tracing::warn!(
                    "maintenance_store: created_at parse failed for task {}: '{}' — {}. Using Utc::now() fallback.",
                    row.id, s, e
                );
                Utc::now()
            }
        },
        None => {
            tracing::warn!(
                "maintenance_store: created_at is NULL for task {}. Using Utc::now() fallback.",
                row.id
            );
            Utc::now()
        }
    };

    let due_by = row
        .due_by_str
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));

    // Wrap status string in quotes for JSON deserialization of enum variant
    let status_json = format!("\"{}\"", row.status);

    Ok(MaintenanceTask {
        id: Uuid::parse_str(&row.id)?,
        title: row.title,
        description: row.description,
        // P1-1: Safe narrowing cast — clamp pod_id to 1-8 range
        pod_id: row.pod_id.and_then(|p| {
            u8::try_from(p.clamp(0, 255)).ok().map(|v| v.clamp(1, 8))
        }),
        component: serde_json::from_str(&row.component)?,
        // P1-1: Safe narrowing cast — clamp priority to 0-100
        priority: u8::try_from(row.priority.clamp(0, 100)).unwrap_or(50),
        status: serde_json::from_str(&status_json)?,
        created_at,
        due_by,
        assigned_to: row.assigned_to,
        source_event_id: row
            .source_event_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()?,
        before_metrics: row
            .before_metrics
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        after_metrics: row
            .after_metrics
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        cost_estimate_paise: row.cost_estimate_paise,
        actual_cost_paise: row.actual_cost_paise,
    })
}

// ===========================================================================
// Phase 11 (v29.0): Business metrics — tables, upsert, query, EBITDA
// ===========================================================================

/// Create the daily_business_metrics table.
pub async fn init_business_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS daily_business_metrics (
            date TEXT PRIMARY KEY,
            revenue_gaming_paise INTEGER DEFAULT 0,
            revenue_cafe_paise INTEGER DEFAULT 0,
            revenue_other_paise INTEGER DEFAULT 0,
            expense_rent_paise INTEGER DEFAULT 0,
            expense_utilities_paise INTEGER DEFAULT 0,
            expense_salaries_paise INTEGER DEFAULT 0,
            expense_maintenance_paise INTEGER DEFAULT 0,
            expense_other_paise INTEGER DEFAULT 0,
            sessions_count INTEGER DEFAULT 0,
            occupancy_rate_pct REAL DEFAULT 0,
            peak_occupancy_pct REAL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    tracing::info!("Business metrics tables initialized");
    Ok(())
}

/// Upsert (insert-or-replace) daily business metrics for a given date.
pub async fn upsert_daily_metrics(
    pool: &SqlitePool,
    date: &str,
    metrics: &DailyBusinessMetrics,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO daily_business_metrics
            (date, revenue_gaming_paise, revenue_cafe_paise, revenue_other_paise,
             expense_rent_paise, expense_utilities_paise, expense_salaries_paise,
             expense_maintenance_paise, expense_other_paise,
             sessions_count, occupancy_rate_pct, peak_occupancy_pct)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(date) DO UPDATE SET
            revenue_gaming_paise = excluded.revenue_gaming_paise,
            revenue_cafe_paise = excluded.revenue_cafe_paise,
            revenue_other_paise = excluded.revenue_other_paise,
            expense_rent_paise = excluded.expense_rent_paise,
            expense_utilities_paise = excluded.expense_utilities_paise,
            expense_salaries_paise = excluded.expense_salaries_paise,
            expense_maintenance_paise = excluded.expense_maintenance_paise,
            expense_other_paise = excluded.expense_other_paise,
            sessions_count = excluded.sessions_count,
            occupancy_rate_pct = excluded.occupancy_rate_pct,
            peak_occupancy_pct = excluded.peak_occupancy_pct",
    )
    .bind(date)
    .bind(metrics.revenue_gaming_paise)
    .bind(metrics.revenue_cafe_paise)
    .bind(metrics.revenue_other_paise)
    .bind(metrics.expense_rent_paise)
    .bind(metrics.expense_utilities_paise)
    .bind(metrics.expense_salaries_paise)
    .bind(metrics.expense_maintenance_paise)
    .bind(metrics.expense_other_paise)
    .bind(metrics.sessions_count as i64)
    .bind(metrics.occupancy_rate_pct as f64)
    .bind(metrics.peak_occupancy_pct as f64)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query daily business metrics between two dates (inclusive).
pub async fn query_business_metrics(
    pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<DailyBusinessMetrics>> {
    let rows = sqlx::query_as::<_, BusinessMetricsRow>(
        "SELECT date, revenue_gaming_paise, revenue_cafe_paise, revenue_other_paise,
                expense_rent_paise, expense_utilities_paise, expense_salaries_paise,
                expense_maintenance_paise, expense_other_paise,
                sessions_count, occupancy_rate_pct, peak_occupancy_pct
         FROM daily_business_metrics
         WHERE date >= ?1 AND date <= ?2
         ORDER BY date ASC",
    )
    .bind(start_date)
    .bind(end_date)
    .fetch_all(pool)
    .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        results.push(DailyBusinessMetrics {
            date,
            revenue_gaming_paise: row.revenue_gaming_paise,
            revenue_cafe_paise: row.revenue_cafe_paise,
            revenue_other_paise: row.revenue_other_paise,
            expense_rent_paise: row.expense_rent_paise,
            expense_utilities_paise: row.expense_utilities_paise,
            expense_salaries_paise: row.expense_salaries_paise,
            expense_maintenance_paise: row.expense_maintenance_paise,
            expense_other_paise: row.expense_other_paise,
            sessions_count: row.sessions_count as u32,
            occupancy_rate_pct: row.occupancy_rate_pct as f32,
            peak_occupancy_pct: row.peak_occupancy_pct as f32,
        });
    }
    Ok(results)
}

/// Compute EBITDA summary across a date range.
pub async fn get_ebitda_summary(
    pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<EbitdaSummary> {
    let metrics = query_business_metrics(pool, start_date, end_date).await?;
    let days = metrics.len() as u32;

    let mut total_revenue: i64 = 0;
    let mut total_expenses: i64 = 0;
    let mut best_day: Option<(String, i64)> = None;
    let mut worst_day: Option<(String, i64)> = None;

    for m in &metrics {
        let day_rev = m.revenue_gaming_paise + m.revenue_cafe_paise + m.revenue_other_paise;
        let day_exp = m.expense_rent_paise
            + m.expense_utilities_paise
            + m.expense_salaries_paise
            + m.expense_maintenance_paise
            + m.expense_other_paise;
        let day_ebitda = day_rev - day_exp;
        total_revenue += day_rev;
        total_expenses += day_exp;

        let date_str = m.date.format("%Y-%m-%d").to_string();
        match &best_day {
            Some((_, best_val)) if day_ebitda <= *best_val => {}
            _ => best_day = Some((date_str.clone(), day_ebitda)),
        }
        match &worst_day {
            Some((_, worst_val)) if day_ebitda >= *worst_val => {}
            _ => worst_day = Some((date_str, day_ebitda)),
        }
    }

    let ebitda = total_revenue - total_expenses;
    let avg_daily = if days > 0 { ebitda / days as i64 } else { 0 };

    Ok(EbitdaSummary {
        total_revenue_paise: total_revenue,
        total_expenses_paise: total_expenses,
        ebitda_paise: ebitda,
        days,
        avg_daily_ebitda_paise: avg_daily,
        best_day: best_day.map(|(d, _)| d),
        worst_day: worst_day.map(|(d, _)| d),
    })
}

#[derive(sqlx::FromRow)]
struct BusinessMetricsRow {
    date: String,
    revenue_gaming_paise: i64,
    revenue_cafe_paise: i64,
    revenue_other_paise: i64,
    expense_rent_paise: i64,
    expense_utilities_paise: i64,
    expense_salaries_paise: i64,
    expense_maintenance_paise: i64,
    expense_other_paise: i64,
    sessions_count: i64,
    occupancy_rate_pct: f64,
    peak_occupancy_pct: f64,
}

// ===========================================================================
// Phase 13 (v29.0): HR employee database
// ===========================================================================

/// Create the employees and attendance tables.
pub async fn init_hr_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS employees (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            role TEXT NOT NULL,
            skills TEXT DEFAULT '[]',
            hourly_rate_paise INTEGER DEFAULT 0,
            phone TEXT DEFAULT '',
            is_active INTEGER DEFAULT 1,
            face_enrollment_id TEXT,
            hired_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // Phase 14: attendance_records
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS attendance_records (
            id TEXT PRIMARY KEY,
            employee_id TEXT NOT NULL,
            date TEXT NOT NULL,
            clock_in TEXT,
            clock_out TEXT,
            source TEXT DEFAULT 'manual',
            hours_worked REAL DEFAULT 0,
            FOREIGN KEY (employee_id) REFERENCES employees(id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_attendance_date ON attendance_records(date)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_attendance_employee ON attendance_records(employee_id)",
    )
    .execute(pool)
    .await?;

    tracing::info!("HR + attendance tables initialized");
    Ok(())
}

/// Insert a new employee.
pub async fn insert_employee(pool: &SqlitePool, employee: &Employee) -> anyhow::Result<()> {
    // P2: Validate employee ID is a non-empty UUID-format string.
    let id_str = employee.id.to_string();
    if id_str.is_empty() || id_str == "00000000-0000-0000-0000-000000000000" {
        anyhow::bail!("insert_employee: invalid employee id '{}'", id_str);
    }

    let role_str = serde_json::to_string(&employee.role)?.replace('"', "");
    let skills_str = serde_json::to_string(&employee.skills)?;

    sqlx::query(
        "INSERT INTO employees
            (id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
    )
    .bind(employee.id.to_string())
    .bind(&employee.name)
    .bind(&role_str)
    .bind(&skills_str)
    .bind(employee.hourly_rate_paise)
    .bind(&employee.phone)
    .bind(employee.is_active as i64)
    .bind(&employee.face_enrollment_id)
    .bind(employee.hired_at.format("%Y-%m-%d").to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// List employees, optionally filtering to active-only.
pub async fn list_employees(
    pool: &SqlitePool,
    active_only: bool,
) -> anyhow::Result<Vec<Employee>> {
    let rows = if active_only {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
             FROM employees WHERE is_active = 1 ORDER BY name ASC",
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
             FROM employees ORDER BY name ASC",
        )
        .fetch_all(pool)
        .await?
    };

    rows.into_iter().map(row_to_employee).collect()
}

/// Get a single employee by ID.
pub async fn get_employee(
    pool: &SqlitePool,
    id: &str,
) -> anyhow::Result<Option<Employee>> {
    let row = sqlx::query_as::<_, EmployeeRow>(
        "SELECT id, name, role, skills, hourly_rate_paise, phone, is_active, face_enrollment_id, hired_at
         FROM employees WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Some(row_to_employee(r)?)),
        None => Ok(None),
    }
}

/// Update an employee's fields. Only non-None fields are changed.
/// P1-5: Uses explicit column-by-column parameterized queries — no dynamic SQL construction.
pub async fn update_employee(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    role: Option<&StaffRole>,
    skills: Option<&[String]>,
    hourly_rate_paise: Option<i64>,
    phone: Option<&str>,
    is_active: Option<bool>,
    face_enrollment_id: Option<&str>,
) -> anyhow::Result<bool> {
    let mut any_updated = false;

    if let Some(n) = name {
        let r = sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
            .bind(n).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(r) = role {
        let role_str = serde_json::to_string(r)?.replace('"', "");
        let r = sqlx::query("UPDATE employees SET role = ?1 WHERE id = ?2")
            .bind(&role_str).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(s) = skills {
        let skills_str = serde_json::to_string(s)?;
        let r = sqlx::query("UPDATE employees SET skills = ?1 WHERE id = ?2")
            .bind(&skills_str).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(rate) = hourly_rate_paise {
        let r = sqlx::query("UPDATE employees SET hourly_rate_paise = ?1 WHERE id = ?2")
            .bind(rate).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(p) = phone {
        let r = sqlx::query("UPDATE employees SET phone = ?1 WHERE id = ?2")
            .bind(p).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(a) = is_active {
        let r = sqlx::query("UPDATE employees SET is_active = ?1 WHERE id = ?2")
            .bind(if a { 1i64 } else { 0i64 }).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }
    if let Some(f) = face_enrollment_id {
        let r = sqlx::query("UPDATE employees SET face_enrollment_id = ?1 WHERE id = ?2")
            .bind(f).bind(id).execute(pool).await?;
        if r.rows_affected() > 0 { any_updated = true; }
    }

    Ok(any_updated)
}

#[derive(sqlx::FromRow)]
struct EmployeeRow {
    id: String,
    name: String,
    role: String,
    skills: String,
    hourly_rate_paise: i64,
    phone: String,
    is_active: i64,
    face_enrollment_id: Option<String>,
    hired_at: String,
}

fn row_to_employee(row: EmployeeRow) -> anyhow::Result<Employee> {
    let role_json = format!("\"{}\"", row.role);
    let role: StaffRole = serde_json::from_str(&role_json)?;
    let skills: Vec<String> = serde_json::from_str(&row.skills).unwrap_or_default();
    let hired_at = chrono::NaiveDate::parse_from_str(&row.hired_at, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    Ok(Employee {
        id: Uuid::parse_str(&row.id)?,
        name: row.name,
        role,
        skills,
        hourly_rate_paise: row.hourly_rate_paise,
        phone: row.phone,
        is_active: row.is_active != 0,
        face_enrollment_id: row.face_enrollment_id,
        hired_at,
    })
}

// ===========================================================================
// Phase 14 (v29.0): Attendance tracking
// ===========================================================================

/// Record a clock-in/clock-out attendance entry.
pub async fn record_attendance(
    pool: &SqlitePool,
    employee_id: &str,
    date: &str,
    clock_in: Option<&str>,
    clock_out: Option<&str>,
    source: &str,
) -> anyhow::Result<()> {
    // P2: Validate employee_id is a non-empty, UUID-like string.
    if employee_id.trim().is_empty() {
        anyhow::bail!("record_attendance: employee_id must not be empty");
    }
    // Validate date is non-empty (basic guard — full format validation happens at the API layer).
    if date.trim().is_empty() {
        anyhow::bail!("record_attendance: date must not be empty");
    }

    let id = Uuid::new_v4().to_string();

    // P2-1 + P2-4: Compute hours_worked in whole minutes (integer) to avoid f64 drift.
    // Handle overnight shifts where clock_out < clock_in by adding 24h.
    let hours = match (clock_in, clock_out) {
        (Some(ci), Some(co)) => {
            let t_in = chrono::NaiveTime::parse_from_str(ci, "%H:%M");
            let t_out = chrono::NaiveTime::parse_from_str(co, "%H:%M");
            match (t_in, t_out) {
                (Ok(i), Ok(o)) => {
                    let mut total_minutes = (o - i).num_minutes();
                    // P2-1: overnight shift — clock_out before clock_in means next day
                    if total_minutes < 0 {
                        total_minutes += 24 * 60;
                    }
                    // P2-4: integer minutes → hours only for storage (avoids f64 accumulation)
                    total_minutes as f64 / 60.0
                }
                (Err(e), _) => {
                    tracing::warn!(
                        "record_attendance: clock_in parse failed for employee '{}' on '{}': '{}' — {}. Defaulting hours_worked to 0.",
                        employee_id, date, ci, e
                    );
                    0.0
                }
                (_, Err(e)) => {
                    tracing::warn!(
                        "record_attendance: clock_out parse failed for employee '{}' on '{}': '{}' — {}. Defaulting hours_worked to 0.",
                        employee_id, date, co, e
                    );
                    0.0
                }
            }
        }
        _ => 0.0,
    };

    sqlx::query(
        "INSERT INTO attendance_records
            (id, employee_id, date, clock_in, clock_out, source, hours_worked)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
    )
    .bind(&id)
    .bind(employee_id)
    .bind(date)
    .bind(clock_in)
    .bind(clock_out)
    .bind(source)
    .bind(hours)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query attendance records by date and/or employee.
pub async fn query_attendance(
    pool: &SqlitePool,
    date: Option<&str>,
    employee_id: Option<&str>,
) -> anyhow::Result<Vec<AttendanceRecord>> {
    let rows = match (date, employee_id) {
        // P2-2: All query branches have LIMIT to prevent unbounded result sets
        (Some(d), Some(eid)) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE date = ?1 AND employee_id = ?2
                 ORDER BY clock_in ASC
                 LIMIT 1000",
            )
            .bind(d)
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (Some(d), None) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE date = ?1
                 ORDER BY clock_in ASC
                 LIMIT 1000",
            )
            .bind(d)
            .fetch_all(pool)
            .await?
        }
        (None, Some(eid)) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records WHERE employee_id = ?1
                 ORDER BY date DESC, clock_in ASC
                 LIMIT 1000",
            )
            .bind(eid)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, AttendanceRow>(
                "SELECT id, employee_id, date, clock_in, clock_out, source, hours_worked
                 FROM attendance_records
                 ORDER BY date DESC, clock_in ASC
                 LIMIT 500",
            )
            .fetch_all(pool)
            .await?
        }
    };

    rows.into_iter().map(row_to_attendance).collect()
}

#[derive(sqlx::FromRow)]
struct AttendanceRow {
    id: String,
    employee_id: String,
    date: String,
    clock_in: Option<String>,
    clock_out: Option<String>,
    source: String,
    hours_worked: f64,
}

fn row_to_attendance(row: AttendanceRow) -> anyhow::Result<AttendanceRecord> {
    let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
        .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
    Ok(AttendanceRecord {
        id: Uuid::parse_str(&row.id)?,
        employee_id: Uuid::parse_str(&row.employee_id)?,
        date,
        clock_in: row.clock_in,
        clock_out: row.clock_out,
        source: row.source,
        // P2: Clamp negative DB values — guard against corrupt/legacy rows.
        hours_worked: row.hours_worked.max(0.0),
    })
}

// ===========================================================================
// Phase 17 (v29.0): Payroll & labor cost
// ===========================================================================

/// Calculate monthly payroll by joining employees and attendance_records.
pub async fn calculate_monthly_payroll(
    pool: &SqlitePool,
    year: i32,
    month: u32,
) -> anyhow::Result<PayrollSummary> {
    let start_date = format!("{:04}-{:02}-01", year, month);
    // P1: Use exclusive upper bound (first day of next month) to avoid including
    // entries from the next month when the date field is a string-compared YYYY-MM-DD.
    // "YYYY-MM-31" would include next-month entries for months with < 31 days
    // because string comparison: "2024-03-01" > "2024-02-31" (doesn't exist but sorts after Feb).
    // Using < next_month_start is always correct regardless of days in month.
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1u32)
    } else {
        (year, month + 1)
    };
    let next_month_start = format!("{:04}-{:02}-01", next_year, next_month);

    let rows = sqlx::query_as::<_, PayrollRow>(
        "SELECT e.id AS employee_id, e.name, e.hourly_rate_paise,
                COALESCE(SUM(a.hours_worked), 0) AS total_hours
         FROM employees e
         LEFT JOIN attendance_records a
             ON a.employee_id = e.id
             AND a.date >= ?1 AND a.date < ?2
         WHERE e.is_active = 1
         GROUP BY e.id
         ORDER BY e.name ASC",
    )
    .bind(&start_date)
    .bind(&next_month_start)
    .fetch_all(pool)
    .await?;

    let mut total_hours = 0.0f64;
    let mut total_paise = 0i64;
    let mut by_employee = Vec::with_capacity(rows.len());

    for row in rows {
        // P1: Compute wages in integer paise to avoid f64 accumulation.
        // hours_worked is stored as f64 (minutes/60); convert back to whole minutes
        // then do integer-only multiplication: minutes * rate_paise / 60.
        let worked_minutes = (row.total_hours * 60.0).round() as i64;
        let emp_total = worked_minutes.max(0) * row.hourly_rate_paise / 60;
        total_hours += row.total_hours;
        total_paise += emp_total;
        by_employee.push(EmployeePayroll {
            employee_id: row.employee_id,
            name: row.name,
            hours_worked: row.total_hours,
            rate_paise: row.hourly_rate_paise,
            total_paise: emp_total,
        });
    }

    Ok(PayrollSummary {
        year,
        month,
        total_hours,
        total_paise,
        by_employee,
    })
}

// ===========================================================================
// Phase 21 (v29.0): Maintenance KPIs
// ===========================================================================

/// Maintenance KPI metrics
#[derive(Debug, serde::Serialize)]
pub struct MaintenanceKPIs {
    pub period_days: u32,
    pub total_events: u32,
    pub total_tasks: u32,
    pub mttr_minutes: f64,        // Mean Time To Repair
    pub mtbf_hours: f64,          // Mean Time Between Failures
    pub self_heal_rate: f64,      // % of issues auto-resolved
    pub prediction_accuracy: f64, // % of predictive alerts that preceded actual failure
    pub false_positive_rate: f64, // % of alerts that were false positives
    pub downtime_minutes: u32,    // Total downtime in period
    pub tasks_completed: u32,
    pub tasks_open: u32,
}

/// Calculate maintenance KPIs for the given number of days.
pub async fn calculate_kpis(pool: &SqlitePool, days: u32) -> anyhow::Result<MaintenanceKPIs> {
    let since = Utc::now() - chrono::Duration::days(days as i64);
    let since_str = since.to_rfc3339();

    // Total events in period
    let (total_events,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_events WHERE detected_at >= ?1",
    )
    .bind(&since_str)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Total tasks in period
    let (total_tasks,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_tasks WHERE created_at >= ?1",
    )
    .bind(&since_str)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // MTTR: average (resolved_at - detected_at) for resolved events
    let mttr_row: Option<(f64,)> = sqlx::query_as(
        "SELECT AVG((julianday(resolved_at) - julianday(detected_at)) * 1440) \
         FROM maintenance_events \
         WHERE detected_at >= ?1 AND resolved_at IS NOT NULL",
    )
    .bind(&since_str)
    .fetch_optional(pool)
    .await?;
    let mttr_minutes = mttr_row.and_then(|(v,)| if v.is_nan() { None } else { Some(v) }).unwrap_or(0.0);

    // Self-heal count (resolution_method contains "AutoHealed")
    let (self_heal_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_events \
         WHERE detected_at >= ?1 AND resolution_method LIKE '%AutoHealed%'",
    )
    .bind(&since_str)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let self_heal_rate = if total_events > 0 {
        self_heal_count as f64 / total_events as f64
    } else {
        0.0
    };

    // Total downtime
    let downtime_row: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(downtime_minutes), 0) FROM maintenance_events WHERE detected_at >= ?1",
    )
    .bind(&since_str)
    .fetch_optional(pool)
    .await?;
    let downtime_minutes = downtime_row.map(|(v,)| v as u32).unwrap_or(0);

    // MTBF: total hours in period / number of failure events
    let total_hours = days as f64 * 24.0;
    let mtbf_hours = if total_events > 0 {
        total_hours / total_events as f64
    } else {
        total_hours // no failures = entire period is MTBF
    };

    // Tasks completed vs open
    let (tasks_completed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_tasks \
         WHERE created_at >= ?1 AND status IN ('Completed', 'Verified')",
    )
    .bind(&since_str)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // P1: Include PendingValidation in open tasks — tasks awaiting validation are
    // still "open" (not yet completed/cancelled/failed). Omitting it caused those
    // tasks to disappear from both tasks_open and tasks_completed counts.
    let (tasks_open,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM maintenance_tasks \
         WHERE created_at >= ?1 AND status IN ('Open', 'Assigned', 'InProgress', 'PendingValidation')",
    )
    .bind(&since_str)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    Ok(MaintenanceKPIs {
        period_days: days,
        total_events: total_events as u32,
        total_tasks: total_tasks as u32,
        mttr_minutes,
        mtbf_hours,
        self_heal_rate,
        prediction_accuracy: 0.0, // requires ground truth data — placeholder
        false_positive_rate: 0.0, // requires labeled alert data — placeholder
        downtime_minutes,
        tasks_completed: tasks_completed as u32,
        tasks_open: tasks_open as u32,
    })
}

// ===========================================================================
// Phase 29 (v29.0): HR <-> Scheduler Auto-Assignment
// ===========================================================================

/// Auto-assign a maintenance task to the best available technician.
///
/// Finds active employees with role 'Technician' or 'Manager' whose skills
/// match the task's component, then picks the one with the fewest open tasks.
/// Returns the assigned employee ID, or None if no suitable employee found.
pub async fn auto_assign_task(
    pool: &SqlitePool,
    task_id: &str,
) -> anyhow::Result<Option<String>> {
    // Get task component (stored as JSON-serialized string)
    let component: Option<String> = sqlx::query_scalar(
        "SELECT component FROM maintenance_tasks WHERE id = ?1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    let component = match component {
        Some(c) => c,
        None => return Ok(None),
    };

    // Find available active employees (Technician or Manager roles)
    let employees = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, skills FROM employees \
         WHERE is_active = 1 AND (role = 'Technician' OR role = 'Manager') LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    // Find best match: employee with matching skill and lowest current task load
    let mut best_id: Option<String> = None;
    let mut best_load = i64::MAX;

    // Normalize component for matching — strip JSON quotes if present
    let component_lower = component.to_lowercase().replace('"', "");

    for (emp_id, _name, skills_json) in &employees {
        let skills: Vec<String> = serde_json::from_str(skills_json).unwrap_or_default();
        let has_skill = skills
            .iter()
            .any(|s| s.to_lowercase().contains(&component_lower))
            || skills
                .iter()
                .any(|s| s.to_lowercase() == "general");

        if has_skill || skills.is_empty() {
            // Count open tasks for this employee
            let load: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM maintenance_tasks \
                 WHERE assigned_to = ?1 AND status NOT IN ('Completed', 'Failed', 'Cancelled')",
            )
            .bind(emp_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            if load < best_load {
                best_load = load;
                best_id = Some(emp_id.clone());
            }
        }
    }

    if let Some(ref emp_id) = best_id {
        sqlx::query(
            "UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2",
        )
        .bind(emp_id)
        .bind(task_id)
        .execute(pool)
        .await?;
        tracing::info!(target: "maint-store", task_id, employee_id = %emp_id, "Task auto-assigned");
    }

    Ok(best_id)
}

#[derive(sqlx::FromRow)]
struct PayrollRow {
    employee_id: String,
    name: String,
    hourly_rate_paise: i64,
    total_hours: f64,
}
