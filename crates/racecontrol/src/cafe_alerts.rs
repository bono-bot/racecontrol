//! Low-stock alert engine for cafe inventory.
//! Fires WhatsApp + email when a countable item breaches its threshold.
//! Cooldown: 4 hours per item, tracked via last_stock_alert_at in cafe_items.
//! Called by: cafe::restock_cafe_item (now), cafe order handler (Phase 154).

use std::time::Duration;
use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::state::AppState;

const ALERT_COOLDOWN_SECS: i64 = 4 * 3600; // 4 hours

/// A cafe item that is currently at or below its low-stock threshold.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LowStockItem {
    pub id: String,
    pub name: String,
    pub stock_quantity: i64,
    pub low_stock_threshold: i64,
}

/// Check whether item_id has breached its low-stock threshold and fire alerts if so.
/// - No-ops for uncountable items, items above threshold, or items within cooldown.
/// - On breach: fires WhatsApp alert + email alert, updates last_stock_alert_at.
/// - Never panics. All errors are logged as warnings.
pub async fn check_low_stock_alerts(db: &SqlitePool, config: &Config, item_id: &str) {
    // 1. Fetch item state + last alert time
    let row: Option<(bool, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT is_countable, stock_quantity, low_stock_threshold, last_stock_alert_at
         FROM cafe_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let (is_countable, stock, threshold, last_alert_at) = match row {
        Some(r) => r,
        None => {
            tracing::warn!(target: "cafe_alerts", "Item {} not found for low-stock check", item_id);
            return;
        }
    };

    // 2. Guard: uncountable, no threshold set, or stock OK
    if !is_countable || threshold <= 0 || stock > threshold {
        return;
    }

    // 3. Cooldown check
    if let Some(last_at_str) = &last_alert_at {
        if let Ok(last_at) =
            chrono::NaiveDateTime::parse_from_str(last_at_str, "%Y-%m-%d %H:%M:%S")
        {
            let elapsed = (chrono::Utc::now().naive_utc() - last_at).num_seconds();
            if elapsed < ALERT_COOLDOWN_SECS {
                tracing::debug!(
                    target: "cafe_alerts",
                    "Low-stock alert for item {} suppressed (cooldown: {}s remaining)",
                    item_id,
                    ALERT_COOLDOWN_SECS - elapsed
                );
                return;
            }
        }
    }

    // 4. Fetch item name for message formatting
    let name: Option<(String,)> = sqlx::query_as("SELECT name FROM cafe_items WHERE id = ?")
        .bind(item_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();
    let item_name = name.map(|(n,)| n).unwrap_or_else(|| item_id.to_string());

    // 5. Record alert timestamp BEFORE sending (prevents double-send on slow network)
    let update_result = sqlx::query(
        "UPDATE cafe_items SET last_stock_alert_at = datetime('now') WHERE id = ?",
    )
    .bind(item_id)
    .execute(db)
    .await;
    if let Err(e) = update_result {
        tracing::warn!(target: "cafe_alerts", "Failed to record alert timestamp for {}: {}", item_id, e);
        return; // Don't send alert if we can't record it — prevents phantom cooldown gaps
    }

    tracing::info!(
        target: "cafe_alerts",
        "Low-stock breach: {} (stock={}, threshold={}). Firing alerts.",
        item_name, stock, threshold
    );

    // 6. Fire WhatsApp alert
    send_low_stock_whatsapp(config, &item_name, stock, threshold).await;

    // 7. Fire email alert
    send_low_stock_email(config, db, item_id, &item_name, stock, threshold).await;
}

/// Reset alert cooldown for an item — call after restock above threshold.
/// This allows the next breach to alert even if 4h hasn't passed.
pub async fn reset_alert_cooldown(db: &SqlitePool, item_id: &str) {
    let result = sqlx::query(
        "UPDATE cafe_items SET last_stock_alert_at = NULL WHERE id = ?",
    )
    .bind(item_id)
    .execute(db)
    .await;
    if let Err(e) = result {
        tracing::warn!(target: "cafe_alerts", "Failed to reset alert cooldown for {}: {}", item_id, e);
    }
}

/// GET /api/v1/cafe/items/low-stock
/// Returns countable items where stock_quantity <= low_stock_threshold (and threshold > 0).
/// Used by admin dashboard banner.
pub async fn list_low_stock_items(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let items: Vec<LowStockItem> = sqlx::query_as(
        "SELECT id, name, stock_quantity, low_stock_threshold
         FROM cafe_items
         WHERE is_countable = 1
           AND low_stock_threshold > 0
           AND stock_quantity <= low_stock_threshold
         ORDER BY name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(target: "cafe_alerts", "Failed to query low-stock items: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "items": items })))
}

/// Send WhatsApp low-stock alert via Evolution API.
/// Mirrors the pattern in whatsapp_alerter::send_whatsapp — best-effort, never panics.
async fn send_low_stock_whatsapp(
    config: &Config,
    item_name: &str,
    stock: i64,
    threshold: i64,
) {
    if !config.alerting.enabled {
        tracing::debug!(target: "cafe_alerts", "WA alerting disabled, skipping low-stock WA for {}", item_name);
        return;
    }

    let (evo_url, evo_key, evo_instance, phone) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
        &config.alerting.uday_phone,
    ) {
        (Some(url), Some(key), Some(inst), Some(phone)) => (url, key, inst, phone),
        _ => {
            tracing::warn!(target: "cafe_alerts", "Evolution API or uday_phone not configured, skipping WA low-stock alert");
            return;
        }
    };

    let ist = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string();

    let message = format!(
        "[CAFE] Low Stock Alert: {} -- Only {} unit(s) remaining (threshold: {}). Please restock. {}",
        item_name, stock, threshold, ist
    );

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({ "number": phone, "text": message });

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "cafe_alerts", "Failed to build HTTP client for WA alert: {}", e);
            return;
        }
    };

    match client
        .post(&url)
        .header("apikey", evo_key.as_str())
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "cafe_alerts", "WA low-stock alert sent for {}", item_name);
        }
        Ok(resp) => {
            tracing::warn!(target: "cafe_alerts", "Evolution API returned {} for low-stock WA alert", resp.status());
        }
        Err(e) => {
            tracing::warn!(target: "cafe_alerts", "WA low-stock alert send failed: {}", e);
        }
    }
}

/// Send email low-stock alert via node send_email.js.
/// Uses config.watchdog.email_enabled, email_recipient, email_script_path.
async fn send_low_stock_email(
    config: &Config,
    _db: &SqlitePool,
    item_id: &str,
    item_name: &str,
    stock: i64,
    threshold: i64,
) {
    if !config.watchdog.email_enabled {
        tracing::debug!(target: "cafe_alerts", "Email alerting disabled, skipping low-stock email for {}", item_name);
        return;
    }

    let recipient = &config.watchdog.email_recipient;
    let script_path = &config.watchdog.email_script_path;

    let subject = format!("[Racing Point Cafe] Low Stock: {}", item_name);
    let body = format!(
        "Cafe Low Stock Alert\n\
         ====================\n\
         \n\
         Item: {}\n\
         Current Stock: {} unit(s)\n\
         Low-Stock Threshold: {}\n\
         \n\
         Please restock this item at your earliest convenience.\n\
         \n\
         Racing Point Operations",
        item_name, stock, threshold
    );

    let result = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new("node")
            .arg(script_path)
            .arg(recipient)
            .arg(&subject)
            .arg(&body)
            .kill_on_drop(true)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            tracing::info!(target: "cafe_alerts", "Low-stock email sent for item {}", item_id);
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(
                target: "cafe_alerts",
                "Email send script failed for {}: status={} stderr={}",
                item_id, output.status, stderr
            );
        }
        Ok(Err(e)) => {
            tracing::warn!(target: "cafe_alerts", "Failed to spawn email send script for {}: {}", item_id, e);
        }
        Err(_) => {
            tracing::warn!(target: "cafe_alerts", "Email send timed out for item {}", item_id);
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::{check_low_stock_alerts, reset_alert_cooldown};
    use crate::config::Config;

    /// Create an in-memory SQLite database with the cafe_items schema needed for tests.
    async fn test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create test pool");

        sqlx::query(
            "CREATE TABLE cafe_items (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_countable BOOLEAN NOT NULL DEFAULT 0,
                stock_quantity INTEGER NOT NULL DEFAULT 0,
                low_stock_threshold INTEGER NOT NULL DEFAULT 0,
                last_stock_alert_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("failed to create cafe_items");

        pool
    }

    /// Insert a test item into the DB.
    async fn insert_item(
        pool: &SqlitePool,
        id: &str,
        name: &str,
        is_countable: bool,
        stock: i64,
        threshold: i64,
        last_alert_at: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO cafe_items (id, name, is_countable, stock_quantity, low_stock_threshold, last_stock_alert_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(is_countable)
        .bind(stock)
        .bind(threshold)
        .bind(last_alert_at)
        .execute(pool)
        .await
        .expect("failed to insert item");
    }

    /// Read last_stock_alert_at for an item from DB.
    async fn get_last_alert_at(pool: &SqlitePool, id: &str) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT last_stock_alert_at FROM cafe_items WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("query failed")
        .flatten()
    }

    fn test_config() -> Config {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        toml::from_str(toml_str).expect("failed to parse test config")
    }

    #[tokio::test]
    async fn skips_uncountable_items() {
        let pool = test_db().await;
        insert_item(&pool, "item-1", "Coffee", false, 0, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-1").await;

        // last_stock_alert_at must remain NULL — no alert fired
        let last_at = get_last_alert_at(&pool, "item-1").await;
        assert!(
            last_at.is_none(),
            "expected last_stock_alert_at=NULL for uncountable item, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn skips_when_stock_above_threshold() {
        let pool = test_db().await;
        insert_item(&pool, "item-2", "Cola", true, 10, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-2").await;

        let last_at = get_last_alert_at(&pool, "item-2").await;
        assert!(
            last_at.is_none(),
            "expected no alert when stock > threshold, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn skips_when_threshold_zero() {
        let pool = test_db().await;
        insert_item(&pool, "item-3", "Water", true, 0, 0, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-3").await;

        let last_at = get_last_alert_at(&pool, "item-3").await;
        assert!(
            last_at.is_none(),
            "expected no alert when threshold=0, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn sets_alert_timestamp_on_breach() {
        let pool = test_db().await;
        // stock=3 <= threshold=5, is_countable=true, no previous alert
        insert_item(&pool, "item-4", "Energy Drink", true, 3, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-4").await;

        let last_at = get_last_alert_at(&pool, "item-4").await;
        assert!(
            last_at.is_some(),
            "expected last_stock_alert_at to be set on breach, got None"
        );
    }

    #[tokio::test]
    async fn suppresses_within_cooldown() {
        let pool = test_db().await;
        // Set last_stock_alert_at to NOW — cooldown active
        insert_item(&pool, "item-5", "Chips", true, 2, 5, Some("2099-01-01 00:00:00")).await;

        // Override with actual current time to ensure cooldown is active
        sqlx::query(
            "UPDATE cafe_items SET last_stock_alert_at = datetime('now') WHERE id = 'item-5'",
        )
        .execute(&pool)
        .await
        .expect("update failed");

        let before = get_last_alert_at(&pool, "item-5").await;

        check_low_stock_alerts(&pool, &test_config(), "item-5").await;

        let after = get_last_alert_at(&pool, "item-5").await;
        // The timestamp should not have changed since cooldown was active
        assert_eq!(
            before, after,
            "expected alert to be suppressed within cooldown window"
        );
    }

    #[tokio::test]
    async fn fires_again_after_cooldown_expired() {
        let pool = test_db().await;
        // Set last_stock_alert_at to 5 hours ago — cooldown expired
        insert_item(&pool, "item-6", "Juice", true, 1, 5, None).await;
        sqlx::query(
            "UPDATE cafe_items SET last_stock_alert_at = datetime('now', '-5 hours') WHERE id = 'item-6'",
        )
        .execute(&pool)
        .await
        .expect("update failed");

        let before = get_last_alert_at(&pool, "item-6").await;

        check_low_stock_alerts(&pool, &test_config(), "item-6").await;

        let after = get_last_alert_at(&pool, "item-6").await;
        assert_ne!(
            before, after,
            "expected alert to fire after cooldown expired (timestamp should update)"
        );
        assert!(after.is_some(), "expected new timestamp to be set");
    }

    #[tokio::test]
    async fn reset_cooldown_clears_timestamp() {
        let pool = test_db().await;
        insert_item(&pool, "item-7", "Sandwich", true, 0, 5, Some("2026-03-22 10:00:00")).await;

        // Verify it is set
        let before = get_last_alert_at(&pool, "item-7").await;
        assert!(before.is_some(), "precondition: last_stock_alert_at should be set");

        reset_alert_cooldown(&pool, "item-7").await;

        let after = get_last_alert_at(&pool, "item-7").await;
        assert!(
            after.is_none(),
            "expected last_stock_alert_at=NULL after reset, got {:?}",
            after
        );
    }

    #[tokio::test]
    async fn list_low_stock_returns_only_breached() {
        // This test is for the query logic only — we test via direct DB query
        // since list_low_stock_items requires axum State which is unavailable in unit tests.
        let pool = test_db().await;

        // Item A: countable, breached (stock <= threshold)
        insert_item(&pool, "item-a", "Bread", true, 2, 5, None).await;
        // Item B: countable, OK (stock > threshold)
        insert_item(&pool, "item-b", "Butter", true, 10, 5, None).await;
        // Item C: uncountable (should be excluded)
        insert_item(&pool, "item-c", "Service", false, 0, 0, None).await;
        // Item D: countable, threshold=0 (should be excluded)
        insert_item(&pool, "item-d", "Gift Card", true, 0, 0, None).await;

        let items: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT id, name, stock_quantity, low_stock_threshold
             FROM cafe_items
             WHERE is_countable = 1
               AND low_stock_threshold > 0
               AND stock_quantity <= low_stock_threshold
             ORDER BY name ASC",
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        assert_eq!(items.len(), 1, "expected exactly 1 breached item, got {:?}", items);
        assert_eq!(items[0].0, "item-a", "expected item-a to be in low-stock list");
    }
}
