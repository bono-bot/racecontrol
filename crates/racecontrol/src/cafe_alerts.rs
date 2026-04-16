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
    if let Some(last_at_str) = &last_alert_at
        && let Ok(last_at) =
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

#[cfg(test)]
#[path = "cafe_alerts_tests.rs"]
mod tests;
