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

/// §S-293 D-PHASE-γ-2 Option C — Audit-log atomicity variant.
///
/// Same semantics as `log_admin_action` but participates in a caller-owned
/// `sqlx::Transaction<'_, sqlx::Sqlite>` instead of acquiring its own
/// connection from the pool. Caller decides commit/rollback.
///
/// Use this from wallet-touching routes where the audit-log row MUST land
/// in the same atomic scope as the wallet UPDATE/INSERT so that a partial
/// failure cannot leave the system in a "wallet wrote but audit missing"
/// state (I-17 invariant per RCA-2026-05-14-I13-I15-I17 §3).
///
/// Returns `Result<(), sqlx::Error>` instead of fire-and-forget so the
/// caller can propagate via `?` and trigger their tx rollback path.
///
/// The original `log_admin_action` (pool-based, fire-and-forget) remains
/// unchanged for non-wallet sites (§S-280 D-PHASE-γ-2.1 8.C universal
/// audit-atomicity is DEFERRED V2.1+).
pub async fn log_admin_action_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    action_type: &str,
    details: &str,
    staff_id: Option<&str>,
    ip_address: Option<&str>,
) -> Result<(), sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO audit_log (id, table_name, row_id, action, action_type, new_values, staff_id, ip_address)
         VALUES (?, 'admin_actions', ?, 'create', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&id)
    .bind(action_type)
    .bind(details)
    .bind(staff_id)
    .bind(ip_address)
    .execute(&mut **tx)
    .await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// Minimal in-memory SQLite fixture exercising the wallets + audit_log
    /// surface needed by `log_admin_action_in_tx` atomicity invariants.
    /// Mirrors the wallet.rs::tests::setup_test_pool pattern (§S-287
    /// PR-B-α). Real sqlx + real SQLite — no mocks.
    async fn setup_audit_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                table_name TEXT NOT NULL,
                row_id TEXT NOT NULL,
                action TEXT NOT NULL,
                action_type TEXT,
                old_values TEXT,
                new_values TEXT,
                staff_id TEXT,
                ip_address TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create audit_log test table");

        sqlx::query(
            "CREATE TABLE wallets (
                driver_id TEXT PRIMARY KEY,
                balance_paise INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create wallets test table");

        pool
    }

    /// §S-293 D-PHASE-γ-2 Option C — I-17.invariant.atomic-audit (rollback half).
    ///
    /// Per RCA-2026-05-14-I13-I15-I17 §3 the audit-log atomicity invariant:
    /// log_admin_action_in_tx + the caller's wallet table write either BOTH
    /// commit or BOTH roll back. No partial state where the wallet was
    /// mutated but the audit_log row is missing.
    ///
    /// Scenario: BEGIN tx → wallet INSERT → log_admin_action_in_tx → ROLLBACK
    /// (simulated by dropping tx without commit). After drop, both the
    /// wallet row AND the audit_log row MUST be absent.
    #[tokio::test]
    async fn log_admin_action_in_tx_atomic_rollback_with_caller_tx() {
        let pool = setup_audit_pool().await;

        // Open caller tx.
        let mut tx = pool.begin().await.expect("begin tx");

        // Caller writes to wallets table inside tx.
        sqlx::query(
            "INSERT INTO wallets (driver_id, balance_paise) VALUES ('driver-rollback', 1500)",
        )
        .execute(&mut *tx)
        .await
        .expect("wallet insert inside tx");

        // Caller writes audit log inside SAME tx via the _in_tx variant.
        log_admin_action_in_tx(
            &mut tx,
            "wallet_topup",
            "{\"driver_id\":\"driver-rollback\",\"amount\":1500}",
            Some("staff-1"),
            None,
        )
        .await
        .expect("audit insert inside tx");

        // Drop tx WITHOUT commit → both writes roll back.
        drop(tx);

        // Invariant: audit_log MUST be empty (no orphaned audit row).
        let audit_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&pool)
            .await
            .expect("count audit rows");
        assert_eq!(
            audit_count.0, 0,
            "invariant: rollback MUST drop audit_log row (no partial-write orphan)"
        );

        // Invariant: wallets MUST be empty (rollback consistent).
        let wallet_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wallets")
            .fetch_one(&pool)
            .await
            .expect("count wallet rows");
        assert_eq!(
            wallet_count.0, 0,
            "invariant: rollback MUST drop wallet row (atomicity sanity check)"
        );
    }

    /// §S-293 D-PHASE-γ-2 Option C — I-17.invariant.atomic-audit (commit half).
    ///
    /// Companion to the rollback test: when caller commits the tx both the
    /// wallet write AND the audit_log row land together. Verifies the
    /// positive case so the rollback test isn't a vacuous "both empty"
    /// assertion.
    #[tokio::test]
    async fn log_admin_action_in_tx_atomic_commit_with_caller_tx() {
        let pool = setup_audit_pool().await;

        let mut tx = pool.begin().await.expect("begin tx");

        sqlx::query(
            "INSERT INTO wallets (driver_id, balance_paise) VALUES ('driver-commit', 2500)",
        )
        .execute(&mut *tx)
        .await
        .expect("wallet insert inside tx");

        log_admin_action_in_tx(
            &mut tx,
            "wallet_topup",
            "{\"driver_id\":\"driver-commit\",\"amount\":2500}",
            Some("staff-2"),
            Some("10.0.0.1"),
        )
        .await
        .expect("audit insert inside tx");

        tx.commit().await.expect("commit tx");

        // Invariant: exactly one audit_log row with the expected fields.
        let audit_row: (String, String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT action_type, action, staff_id, ip_address FROM audit_log",
        )
        .fetch_one(&pool)
        .await
        .expect("read audit row");
        assert_eq!(audit_row.0, "wallet_topup", "action_type column");
        assert_eq!(audit_row.1, "create", "action column hard-coded to 'create'");
        assert_eq!(audit_row.2.as_deref(), Some("staff-2"), "staff_id column");
        assert_eq!(audit_row.3.as_deref(), Some("10.0.0.1"), "ip_address column");

        // Invariant: exactly one wallet row at committed balance.
        let wallet_row: (String, i64) =
            sqlx::query_as("SELECT driver_id, balance_paise FROM wallets")
                .fetch_one(&pool)
                .await
                .expect("read wallet row");
        assert_eq!(wallet_row.0, "driver-commit");
        assert_eq!(wallet_row.1, 2500);
    }
}
