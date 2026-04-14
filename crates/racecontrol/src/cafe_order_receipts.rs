//! Cafe order receipt delivery — WhatsApp and thermal printer.
//!
//! Extracted from cafe_orders.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use crate::state::AppState;
use super::cafe_order_types::OrderItemDetail;

// ─── Post-Order Side Effects ──────────────────────────────────────────────────

/// Send a WhatsApp order confirmation receipt to the customer's phone.
/// Fire-and-forget: all errors are logged as warnings, never propagated.
pub async fn send_order_receipt_whatsapp(
    state: &Arc<AppState>,
    driver_id: &str,
    receipt_number: &str,
    items: &[OrderItemDetail],
    total_paise: i64,
    new_balance_paise: i64,
) {
    let config = &state.config;
    let db = &state.db;

    if !config.alerting.enabled {
        tracing::debug!(target: "cafe", "WA alerting disabled, skipping receipt for driver {}", driver_id);
        return;
    }

    // Fetch driver phone
    let phone_opt: Option<Option<String>> = sqlx::query_scalar("SELECT phone FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(db)
        .await
        .ok();

    let phone = match phone_opt.flatten() {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            tracing::warn!(target: "cafe", "No phone for driver {}, skipping WA receipt", driver_id);
            return;
        }
    };

    let (evo_url, evo_key, evo_instance) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
    ) {
        (Some(url), Some(key), Some(inst)) => (url, key, inst),
        _ => {
            tracing::warn!(target: "cafe", "Evolution API not configured, skipping WA receipt for {}", receipt_number);
            return;
        }
    };

    let ist = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string();

    let mut items_text = String::new();
    for item in items {
        items_text.push_str(&format!(
            "  {} x{}  Rs.{}\n",
            item.name,
            item.quantity,
            item.line_total_paise / 100
        ));
    }

    let message = format!(
        "[Racing Point Cafe] Order Confirmed!\nReceipt: {}\n{}\n\n{}
Total: Rs.{}\nBalance: Rs.{}\n\nThank you! Your order is being prepared.",
        receipt_number,
        ist,
        items_text,
        total_paise / 100,
        new_balance_paise / 100
    );

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({ "number": phone, "text": message });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "cafe", "Failed to build HTTP client for WA receipt: {}", e);
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
            tracing::info!(target: "cafe", "WA receipt sent for order {} to driver {}", receipt_number, driver_id);
        }
        Ok(resp) => {
            tracing::warn!(target: "cafe", "Evolution API returned {} for WA receipt {}", resp.status(), receipt_number);
        }
        Err(e) => {
            tracing::warn!(target: "cafe", "WA receipt send failed for {}: {}", receipt_number, e);
        }
    }
}

/// Print a thermal receipt via a Node.js script (fire-and-forget).
/// Skipped silently if print_script_path is not configured.
pub async fn print_thermal_receipt(
    state: &Arc<AppState>,
    receipt_number: &str,
    items: &[OrderItemDetail],
    total_paise: i64,
    customer_name: &str,
) {
    let config = &state.config;
    let script_path = match &config.cafe.print_script_path {
        Some(p) => p.clone(),
        None => {
            tracing::debug!(target: "cafe", "Thermal print skipped: print_script_path not configured");
            return;
        }
    };

    let ist = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string();

    let mut items_text = String::new();
    for item in items {
        items_text.push_str(&format!(
            "{}\n  {} x Rs.{} = Rs.{}\n",
            item.name,
            item.quantity,
            item.unit_price_paise / 100,
            item.line_total_paise / 100
        ));
    }

    let receipt_text = format!(
        "================================\n    RACING POINT CAFE\n================================\nReceipt: {}\n{}\nCustomer: {}\n--------------------------------\n{}--------------------------------\nTOTAL: Rs.{}\n================================\n     Thank you!\n================================",
        receipt_number,
        ist,
        customer_name,
        items_text,
        total_paise / 100
    );

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("node")
            .arg(&script_path)
            .arg(&receipt_text)
            .kill_on_drop(true)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            if output.status.success() {
                tracing::info!(target: "cafe", "Thermal receipt printed for {}", receipt_number);
            } else {
                tracing::warn!(
                    target: "cafe",
                    "Print script exited with non-zero status for {}: {}",
                    receipt_number,
                    output.status
                );
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(target: "cafe", "Print script failed to launch for {}: {}", receipt_number, e);
        }
        Err(_) => {
            tracing::warn!(target: "cafe", "Thermal print timed out for {}", receipt_number);
        }
    }
}
