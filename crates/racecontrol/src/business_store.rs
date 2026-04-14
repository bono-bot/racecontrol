//! Business metrics persistence: daily metrics, EBITDA, maintenance KPIs, task auto-assignment.
//!
//! Extracted from `maintenance_store.rs` — Phases 11/21/29 (v29.0).

use crate::maintenance_models::DailyBusinessMetrics;
use crate::maintenance_models::EbitdaSummary;
use chrono::Utc;
use sqlx::SqlitePool;

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
    .bind(i64::from(metrics.sessions_count))
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
        // MMA-R1: Return error on invalid date instead of silent 2000-01-01 fallback
        let date = chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("invalid business metric date '{}': {}", row.date, e))?;
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
            // MMA-R1: Use try_from instead of `as` for safe narrowing
            sessions_count: u32::try_from(row.sessions_count.max(0)).unwrap_or(0),
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
    // MMA-R1: Use try_from instead of `as` for safe narrowing
    let downtime_minutes = downtime_row
        .map(|(v,)| u32::try_from(v.max(0)).unwrap_or(0))
        .unwrap_or(0);

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

    // MMA-R1: Use try_from instead of `as` for safe narrowing
    Ok(MaintenanceKPIs {
        period_days: days,
        total_events: u32::try_from(total_events.max(0)).unwrap_or(0),
        total_tasks: u32::try_from(total_tasks.max(0)).unwrap_or(0),
        mttr_minutes,
        mtbf_hours,
        self_heal_rate,
        prediction_accuracy: 0.0, // requires ground truth data — placeholder
        false_positive_rate: 0.0, // requires labeled alert data — placeholder
        downtime_minutes,
        tasks_completed: u32::try_from(tasks_completed.max(0)).unwrap_or(0),
        tasks_open: u32::try_from(tasks_open.max(0)).unwrap_or(0),
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
/// MMA-R1: Use transaction + `assigned_to IS NULL` guard to prevent race conditions.
pub async fn auto_assign_task(
    pool: &SqlitePool,
    task_id: &str,
) -> anyhow::Result<Option<String>> {
    let mut tx = pool.begin().await?;

    // Get task component — only if task exists and is unassigned
    let component: Option<String> = sqlx::query_scalar(
        "SELECT component FROM maintenance_tasks WHERE id = ?1 AND assigned_to IS NULL",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;

    let component = match component {
        Some(c) => c,
        None => {
            // Task doesn't exist or already assigned
            tx.rollback().await?;
            return Ok(None);
        }
    };

    // Find available active employees (Technician or Manager roles)
    let employees = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, skills FROM employees \
         WHERE is_active = 1 AND (role = 'Technician' OR role = 'Manager') LIMIT 20",
    )
    .fetch_all(&mut *tx)
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
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);

            if load < best_load {
                best_load = load;
                best_id = Some(emp_id.clone());
            }
        }
    }

    if let Some(ref emp_id) = best_id {
        // MMA-R1: Use `assigned_to IS NULL` guard to prevent overwriting concurrent assignment
        let result = sqlx::query(
            "UPDATE maintenance_tasks SET assigned_to = ?1, status = 'Assigned' WHERE id = ?2 AND assigned_to IS NULL",
        )
        .bind(emp_id)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            // Concurrent assignment won — rollback
            tx.rollback().await?;
            return Ok(None);
        }

        tx.commit().await?;
        tracing::info!(target: "maint-store", task_id, employee_id = %emp_id, "Task auto-assigned");
    } else {
        tx.rollback().await?;
    }

    Ok(best_id)
}
