//! Internal row types and SQLite row-to-model conversions for maintenance events and tasks.
//!
//! Private to `maintenance_store` — included via `#[path]`.

use crate::maintenance_models::{MaintenanceEvent, MaintenanceTask};
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Internal row types (sqlx::FromRow)
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
pub(super) struct EventRow {
    pub id: String,
    pub pod_id: Option<i64>,
    pub event_type: String,
    pub severity: String,
    pub component: String,
    pub description: String,
    #[sqlx(rename = "detected_at")]
    pub detected_at_str: Option<String>,
    #[sqlx(rename = "resolved_at")]
    pub resolved_at_str: Option<String>,
    pub resolution_method: Option<String>,
    pub source: String,
    pub correlation_id: Option<String>,
    pub revenue_impact_paise: Option<i64>,
    pub customers_affected: Option<i64>,
    pub downtime_minutes: Option<i64>,
    pub cost_estimate_paise: Option<i64>,
    pub assigned_staff_id: Option<String>,
    pub metadata: String,
}

#[derive(sqlx::FromRow)]
pub(super) struct TaskRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pod_id: Option<i64>,
    pub component: String,
    pub priority: i64,
    pub status: String,
    #[sqlx(rename = "created_at")]
    pub created_at_str: Option<String>,
    #[sqlx(rename = "due_by")]
    pub due_by_str: Option<String>,
    pub assigned_to: Option<String>,
    pub source_event_id: Option<String>,
    pub before_metrics: Option<String>,
    pub after_metrics: Option<String>,
    pub cost_estimate_paise: Option<i64>,
    pub actual_cost_paise: Option<i64>,
}

// ---------------------------------------------------------------------------
// Row -> model conversions
// ---------------------------------------------------------------------------

pub(super) fn row_to_event(row: EventRow) -> anyhow::Result<MaintenanceEvent> {
    // MMA-R1: Return error on date parse failure instead of silent Utc::now() fallback
    // (corrupts historical data, breaks MTTR/KPI calculations)
    let detected_at = match row.detected_at_str.as_deref() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("detected_at parse failed for event {}: '{}' — {}", row.id, s, e))?
            .with_timezone(&Utc),
        None => anyhow::bail!("detected_at is NULL for event {}", row.id),
    };

    let resolved_at = match row.resolved_at_str.as_deref() {
        Some(s) => Some(
            DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow::anyhow!("resolved_at parse failed for event {}: '{}' — {}", row.id, s, e))?
                .with_timezone(&Utc),
        ),
        None => None,
    };

    // MMA-R1: Reject invalid pod_id instead of clamping (masks data corruption)
    let pod_id = match row.pod_id {
        Some(p) => {
            let val = u8::try_from(p)
                .map_err(|_| anyhow::anyhow!("pod_id {} out of u8 range for event {}", p, row.id))?;
            if !(1..=8).contains(&val) {
                anyhow::bail!("pod_id {} out of valid range 1-8 for event {}", val, row.id);
            }
            Some(val)
        }
        None => None,
    };

    // MMA-R1: Reject negative values instead of clamping to 0 or u32::MAX
    let customers_affected = match row.customers_affected {
        Some(c) if c < 0 => anyhow::bail!("customers_affected is negative ({}) for event {}", c, row.id),
        Some(c) => Some(u32::try_from(c)
            .map_err(|_| anyhow::anyhow!("customers_affected {} exceeds u32 for event {}", c, row.id))?),
        None => None,
    };

    let downtime_minutes = match row.downtime_minutes {
        Some(d) if d < 0 => anyhow::bail!("downtime_minutes is negative ({}) for event {}", d, row.id),
        Some(d) => Some(u32::try_from(d)
            .map_err(|_| anyhow::anyhow!("downtime_minutes {} exceeds u32 for event {}", d, row.id))?),
        None => None,
    };

    Ok(MaintenanceEvent {
        id: Uuid::parse_str(&row.id)?,
        pod_id,
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
        customers_affected,
        downtime_minutes,
        cost_estimate_paise: row.cost_estimate_paise,
        assigned_staff_id: row.assigned_staff_id,
        metadata: serde_json::from_str(&row.metadata)?,
    })
}

pub(super) fn row_to_task(row: TaskRow) -> anyhow::Result<MaintenanceTask> {
    // MMA-R1: Return error on date parse failure instead of silent Utc::now() fallback
    let created_at = match row.created_at_str.as_deref() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("created_at parse failed for task {}: '{}' — {}", row.id, s, e))?
            .with_timezone(&Utc),
        None => anyhow::bail!("created_at is NULL for task {}", row.id),
    };

    let due_by = match row.due_by_str.as_deref() {
        Some(s) => Some(
            DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow::anyhow!("due_by parse failed for task {}: '{}' — {}", row.id, s, e))?
                .with_timezone(&Utc),
        ),
        None => None,
    };

    // Wrap status string in quotes for JSON deserialization of enum variant
    let status_json = format!("\"{}\"", row.status);

    // MMA-R1: Reject invalid pod_id instead of clamping
    let pod_id = match row.pod_id {
        Some(p) => {
            let val = u8::try_from(p)
                .map_err(|_| anyhow::anyhow!("pod_id {} out of u8 range for task {}", p, row.id))?;
            if !(1..=8).contains(&val) {
                anyhow::bail!("pod_id {} out of valid range 1-8 for task {}", val, row.id);
            }
            Some(val)
        }
        None => None,
    };

    // MMA-R1: Reject invalid priority instead of clamping to 50
    let priority = u8::try_from(row.priority)
        .map_err(|_| anyhow::anyhow!("priority {} out of u8 range for task {}", row.priority, row.id))?;

    Ok(MaintenanceTask {
        id: Uuid::parse_str(&row.id)?,
        title: row.title,
        description: row.description,
        pod_id,
        component: serde_json::from_str(&row.component)?,
        priority,
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
