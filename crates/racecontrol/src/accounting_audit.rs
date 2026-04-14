use std::sync::Arc;

use serde_json::Value;
use sqlx::{Column, Row};
use uuid::Uuid;

use crate::state::AppState;

/// Record a change to any config table (pricing_rules, coupons, packages, etc.)
/// old_values/new_values should be JSON strings of the before/after state.
pub async fn log_audit(
    state: &Arc<AppState>,
    table_name: &str,
    row_id: &str,
    action: &str,
    old_values: Option<&str>,
    new_values: Option<&str>,
    staff_id: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, old_values, new_values, staff_id)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(table_name)
    .bind(row_id)
    .bind(action)
    .bind(old_values)
    .bind(new_values)
    .bind(staff_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to write audit log for {}.{}: {}", table_name, row_id, e);
    }
}

/// Record a sensitive admin action in audit_log with action_type classification.
/// Fire-and-forget: never blocks the caller on DB errors.
pub async fn log_admin_action(
    state: &Arc<AppState>,
    action_type: &str,
    details: &str,
    staff_id: Option<&str>,
    ip_address: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, action_type, new_values, staff_id, ip_address)
         VALUES (?, 'admin_actions', ?, 'create', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&id)
    .bind(action_type)
    .bind(details)
    .bind(staff_id)
    .bind(ip_address)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to write admin audit log for {}: {}", action_type, e);
    }
}

/// Fetch the current row as JSON for audit trail (before an update/delete).
/// Returns None if the row doesn't exist.
pub async fn snapshot_row(
    state: &Arc<AppState>,
    table_name: &str,
    id: &str,
) -> Option<String> {
    // Build a simple SELECT * for the row. Since we know our table names,
    // we validate against an allowlist to prevent SQL injection.
    let allowed_tables = [
        "pricing_tiers", "pricing_rules", "coupons", "packages",
        "membership_tiers", "kiosk_experiences", "kiosk_settings",
        "tournaments",
    ];

    if !allowed_tables.contains(&table_name) {
        return None;
    }

    let query = format!("SELECT * FROM {} WHERE id = ?", table_name);

    // Use sqlx::query to get raw row, then convert to JSON manually.
    // Since different tables have different schemas, we use a generic approach.
    let row = sqlx::query(&query)
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    row.map(|r| {
        let mut map = serde_json::Map::new();
        for col in r.columns() {
            let name = col.name();
            // Try to extract as string (most SQLite values can be retrieved this way)
            if let Ok(val) = r.try_get::<Option<String>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.map(Value::String).unwrap_or(Value::Null),
                );
            } else if let Ok(val) = r.try_get::<Option<i64>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.map(|v| Value::Number(v.into())).unwrap_or(Value::Null),
                );
            } else if let Ok(val) = r.try_get::<Option<f64>, _>(name) {
                map.insert(
                    name.to_string(),
                    val.and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
                        .unwrap_or(Value::Null),
                );
            }
        }
        serde_json::to_string(&map).unwrap_or_default()
    })
}
