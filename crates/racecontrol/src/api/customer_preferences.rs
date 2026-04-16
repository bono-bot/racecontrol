//! Customer opt-in/opt-out preferences API (Phase 387).
//!
//! Anti-spam infrastructure: every customer controls what promotional messages
//! they receive. Must be deployed and verified before any marketing messages.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::state::AppState;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomerPreferences {
    pub driver_id: String,
    pub opt_in_promotions: bool,
    pub channel_preference: String,
    pub frequency_cap_per_week: i32,
    pub consecutive_ignored: i32,
    pub auto_paused: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    pub opt_in_promotions: Option<bool>,
    pub channel_preference: Option<String>,
    pub frequency_cap_per_week: Option<i32>,
}

// ─── GET /customer/preferences ──────────────────────────────────────────────

pub(crate) async fn get_preferences(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (bool, String, i32, i32, bool)>(
        "SELECT opt_in_promotions, channel_preference, frequency_cap_per_week,
                consecutive_ignored, auto_paused
         FROM customer_preferences WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((opt_in, channel, cap, ignored, paused))) => {
            Json(json!({
                "driver_id": driver_id,
                "opt_in_promotions": opt_in,
                "channel_preference": channel,
                "frequency_cap_per_week": cap,
                "consecutive_ignored": ignored,
                "auto_paused": paused,
            }))
        }
        Ok(None) => {
            // No preferences set yet — return defaults (opted in)
            Json(json!({
                "driver_id": driver_id,
                "opt_in_promotions": true,
                "channel_preference": "whatsapp",
                "frequency_cap_per_week": 3,
                "consecutive_ignored": 0,
                "auto_paused": false,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to fetch preferences for {}: {}", driver_id, e);
            Json(json!({"error": "Failed to fetch preferences"}))
        }
    }
}

// ─── PUT /customer/preferences ──────────────────────────────────────────────

pub(crate) async fn update_preferences(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<UpdatePreferencesRequest>,
) -> Json<Value> {
    // Validate channel_preference if provided
    if let Some(ref ch) = req.channel_preference {
        if !["whatsapp", "email", "both", "none"].contains(&ch.as_str()) {
            return Json(json!({"error": "Invalid channel_preference. Must be: whatsapp, email, both, none"}));
        }
    }

    // Validate frequency_cap if provided
    if let Some(cap) = req.frequency_cap_per_week {
        if cap < 0 || cap > 10 {
            return Json(json!({"error": "frequency_cap_per_week must be 0-10"}));
        }
    }

    // Upsert preferences
    let opt_in = req.opt_in_promotions.unwrap_or(true);
    let channel = req.channel_preference.unwrap_or_else(|| "whatsapp".to_string());
    let cap = req.frequency_cap_per_week.unwrap_or(3);

    let result = sqlx::query(
        "INSERT INTO customer_preferences (driver_id, opt_in_promotions, channel_preference, frequency_cap_per_week, updated_at)
         VALUES (?, ?, ?, ?, datetime('now'))
         ON CONFLICT(driver_id) DO UPDATE SET
            opt_in_promotions = excluded.opt_in_promotions,
            channel_preference = excluded.channel_preference,
            frequency_cap_per_week = excluded.frequency_cap_per_week,
            auto_paused = CASE WHEN excluded.opt_in_promotions = 1 THEN 0 ELSE auto_paused END,
            consecutive_ignored = CASE WHEN excluded.opt_in_promotions = 1 THEN 0 ELSE consecutive_ignored END,
            updated_at = datetime('now')"
    )
    .bind(&driver_id)
    .bind(opt_in)
    .bind(&channel)
    .bind(cap)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({
            "ok": true,
            "driver_id": driver_id,
            "opt_in_promotions": opt_in,
            "channel_preference": channel,
            "frequency_cap_per_week": cap,
        })),
        Err(e) => {
            tracing::error!("Failed to update preferences for {}: {}", driver_id, e);
            Json(json!({"error": "Failed to update preferences"}))
        }
    }
}

// ─── POST /customer/opt-out — WhatsApp "stop" handler ───────────────────────

pub(crate) async fn opt_out(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let result = sqlx::query(
        "INSERT INTO customer_preferences (driver_id, opt_in_promotions, opted_out_at, updated_at)
         VALUES (?, 0, datetime('now'), datetime('now'))
         ON CONFLICT(driver_id) DO UPDATE SET
            opt_in_promotions = 0,
            opted_out_at = datetime('now'),
            updated_at = datetime('now')"
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Customer {} opted out of promotions", driver_id);
            Json(json!({ "ok": true, "driver_id": driver_id, "opted_out": true }))
        }
        Err(e) => {
            tracing::error!("Failed to opt out {}: {}", driver_id, e);
            Json(json!({"error": "Failed to opt out"}))
        }
    }
}

// ─── POST /customer/opt-in — re-engage after opt-out ────────────────────────

pub(crate) async fn opt_in(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let result = sqlx::query(
        "INSERT INTO customer_preferences (driver_id, opt_in_promotions, auto_paused, consecutive_ignored, updated_at)
         VALUES (?, 1, 0, 0, datetime('now'))
         ON CONFLICT(driver_id) DO UPDATE SET
            opt_in_promotions = 1,
            auto_paused = 0,
            consecutive_ignored = 0,
            opted_out_at = NULL,
            auto_paused_at = NULL,
            updated_at = datetime('now')"
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Customer {} opted back in to promotions", driver_id);
            Json(json!({ "ok": true, "driver_id": driver_id, "opted_in": true }))
        }
        Err(e) => {
            tracing::error!("Failed to opt in {}: {}", driver_id, e);
            Json(json!({"error": "Failed to opt in"}))
        }
    }
}

// ─── Send-time enforcement ──────────────────────────────────────────────────

/// Check if a promotional message can be sent to this customer.
/// Returns Ok(()) if allowed, Err(reason) if blocked.
pub async fn can_send_promo(state: &Arc<AppState>, driver_id: &str) -> Result<(), String> {
    let row = sqlx::query_as::<_, (bool, bool, i32, i32, Option<String>)>(
        "SELECT opt_in_promotions, auto_paused, frequency_cap_per_week,
                promos_sent_this_week, week_start
         FROM customer_preferences WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    let Some((opt_in, paused, cap, sent_this_week, week_start)) = row else {
        // No preferences record — default is opted in, no cap hit
        return Ok(());
    };

    if !opt_in {
        return Err("Customer opted out of promotions".to_string());
    }

    if paused {
        return Err("Customer auto-paused (3 consecutive ignored offers)".to_string());
    }

    // Check weekly frequency cap
    let now = chrono::Utc::now();
    let current_week = now.format("%Y-W%V").to_string();

    if week_start.as_deref() == Some(current_week.as_str()) && sent_this_week >= cap {
        return Err(format!("Weekly cap reached ({sent_this_week}/{cap})"));
    }

    Ok(())
}

/// Record that a promo was sent. Updates weekly counter and resets if new week.
pub async fn record_promo_sent(state: &Arc<AppState>, driver_id: &str, campaign_id: Option<&str>, channel: &str, message_preview: &str) -> Result<(), String> {
    let now = chrono::Utc::now();
    let current_week = now.format("%Y-W%V").to_string();

    // Update preferences: increment weekly counter or reset if new week
    sqlx::query(
        "UPDATE customer_preferences SET
            promos_sent_this_week = CASE
                WHEN week_start = ? THEN promos_sent_this_week + 1
                ELSE 1
            END,
            week_start = ?,
            last_promo_sent_at = datetime('now'),
            updated_at = datetime('now')
         WHERE driver_id = ?"
    )
    .bind(&current_week)
    .bind(&current_week)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    // Log to promo_delivery_log
    sqlx::query(
        "INSERT INTO promo_delivery_log (driver_id, campaign_id, channel, message_preview)
         VALUES (?, ?, ?, ?)"
    )
    .bind(driver_id)
    .bind(campaign_id)
    .bind(channel)
    .bind(message_preview)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    Ok(())
}

/// Mark a promo as ignored. After 3 consecutive ignores, auto-pause the customer.
pub async fn record_promo_ignored(state: &Arc<AppState>, driver_id: &str) -> Result<bool, String> {
    // Increment consecutive_ignored
    sqlx::query(
        "UPDATE customer_preferences SET
            consecutive_ignored = consecutive_ignored + 1,
            updated_at = datetime('now')
         WHERE driver_id = ?"
    )
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    // Check if we should auto-pause (3 consecutive ignored)
    let count: Option<(i32,)> = sqlx::query_as(
        "SELECT consecutive_ignored FROM customer_preferences WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    if let Some((ignored,)) = count {
        if ignored >= 3 {
            sqlx::query(
                "UPDATE customer_preferences SET auto_paused = 1, auto_paused_at = datetime('now'), updated_at = datetime('now') WHERE driver_id = ?"
            )
            .bind(driver_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {e}"))?;

            tracing::info!("Auto-paused promotions for {} after {} consecutive ignores", driver_id, ignored);
            return Ok(true); // was paused
        }
    }

    Ok(false) // not paused
}

/// Reset ignore counter when customer engages (redeems an offer, visits venue, etc.)
pub async fn record_engagement(state: &Arc<AppState>, driver_id: &str) -> Result<(), String> {
    sqlx::query(
        "UPDATE customer_preferences SET
            consecutive_ignored = 0,
            auto_paused = 0,
            auto_paused_at = NULL,
            updated_at = datetime('now')
         WHERE driver_id = ?"
    )
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    Ok(())
}
