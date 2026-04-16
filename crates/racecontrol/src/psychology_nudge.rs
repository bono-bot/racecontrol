//! Psychology nudge subsystem — notification queue, dispatcher, channel routing,
//! and template resolution.

use std::sync::Arc;
use crate::state::AppState;
use super::{NotificationChannel, WHATSAPP_DAILY_BUDGET, DISPATCHER_INTERVAL_SECS,
            DISPATCHER_BATCH_SIZE, NUDGE_TTL_DAYS};

// ─── Notification Budget ──────────────────────────────────────────────────────

/// Check if sending a WhatsApp message to this driver would exceed the daily budget.
/// Returns true if the driver has already received >= WHATSAPP_DAILY_BUDGET proactive messages today.
pub async fn is_whatsapp_budget_exceeded(state: &Arc<AppState>, driver_id: &str) -> bool {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM nudge_queue
         WHERE driver_id = ? AND channel = 'whatsapp' AND status = 'sent'
         AND date(sent_at) = date('now')"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    count >= WHATSAPP_DAILY_BUDGET
}

// ─── Queue Insertion ──────────────────────────────────────────────────────────

/// Queue a notification through the priority system.
/// Inserts into nudge_queue with status='pending'.
/// The background dispatcher picks it up.
pub async fn queue_notification(
    state: &Arc<AppState>,
    driver_id: &str,
    channel: NotificationChannel,
    priority: i32,
    template: &str,
    payload_json: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    // Nudges expire after 24 hours by default
    if let Err(e) = sqlx::query(
        "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', datetime('now', '+1 day'))"
    )
    .bind(&id)
    .bind(driver_id)
    .bind(channel.as_str())
    .bind(priority)
    .bind(template)
    .bind(payload_json)
    .execute(&state.db)
    .await {
        tracing::error!("[psychology] failed to queue notification: {}", e);
    }
}

// ─── Dispatcher ───────────────────────────────────────────────────────────────

/// Spawn the background notification dispatcher task.
/// Runs every DISPATCHER_INTERVAL_SECS, drains nudge_queue, routes to channels.
pub fn spawn_dispatcher(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(DISPATCHER_INTERVAL_SECS)
        );
        loop {
            interval.tick().await;
            if let Err(e) = drain_notification_queue(&state).await {
                tracing::error!("[psychology] dispatcher error: {}", e);
            }
            // Cleanup old entries every cycle (lightweight query)
            if let Err(e) = cleanup_old_nudges(&state).await {
                tracing::error!("[psychology] cleanup error: {}", e);
            }
        }
    });
    tracing::info!("[psychology] notification dispatcher spawned (interval={}s)", DISPATCHER_INTERVAL_SECS);
}

/// Drain the nudge_queue: expire stale entries, process pending in priority order,
/// route to correct channel, mark sent/failed/throttled.
pub(super) async fn drain_notification_queue(state: &Arc<AppState>) -> anyhow::Result<()> {
    // 1. Mark expired entries
    sqlx::query(
        "UPDATE nudge_queue SET status = 'expired'
         WHERE status = 'pending' AND expires_at IS NOT NULL AND datetime(expires_at) < datetime('now')"
    )
    .execute(&state.db)
    .await?;

    // 2. Fetch batch of pending nudges, ordered by priority (1=highest) then creation time
    let pending: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, driver_id, channel, template, payload_json
         FROM nudge_queue
         WHERE status = 'pending'
         ORDER BY priority ASC, scheduled_at ASC
         LIMIT ?"
    )
    .bind(DISPATCHER_BATCH_SIZE)
    .fetch_all(&state.db)
    .await?;

    for (nudge_id, driver_id, channel_str, template, payload_json) in pending {
        let channel = match NotificationChannel::from_str(&channel_str) {
            Some(c) => c,
            None => {
                // Invalid channel — mark as failed
                let _ = sqlx::query("UPDATE nudge_queue SET status = 'failed', error_text = 'invalid channel' WHERE id = ?")
                    .bind(&nudge_id).execute(&state.db).await;
                continue;
            }
        };

        // 3. Check WhatsApp budget before sending
        if channel == NotificationChannel::Whatsapp
            && is_whatsapp_budget_exceeded(state, &driver_id).await {
                let _ = sqlx::query(
                    "UPDATE nudge_queue SET status = 'throttled', error_text = 'daily budget exceeded' WHERE id = ?"
                )
                .bind(&nudge_id)
                .execute(&state.db)
                .await;
                tracing::info!("[psychology] throttled WhatsApp nudge {} for driver {}", nudge_id, driver_id);
                continue;
            }

        // 4. Resolve message content from template + payload
        let message = resolve_template(&template, &payload_json);

        // 5. Route to channel
        let success = match channel {
            NotificationChannel::Whatsapp => {
                // Look up driver phone number
                let phone: Option<(String,)> = sqlx::query_as(
                    "SELECT phone FROM drivers WHERE id = ? AND phone IS NOT NULL"
                )
                .bind(&driver_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
                match phone {
                    Some((p,)) => send_whatsapp(state, &p, &message).await,
                    None => {
                        tracing::warn!("[psychology] no phone for driver {}, skipping WhatsApp nudge", driver_id);
                        false
                    }
                }
            }
            NotificationChannel::Discord => send_discord(state, &message).await,
            NotificationChannel::Pwa => send_pwa_notification(state, &driver_id, &template, &payload_json).await,
        };

        // 6. Update status
        if success {
            let _ = sqlx::query(
                "UPDATE nudge_queue SET status = 'sent', sent_at = datetime('now') WHERE id = ?"
            )
            .bind(&nudge_id)
            .execute(&state.db)
            .await;
        } else {
            let _ = sqlx::query(
                "UPDATE nudge_queue SET status = 'failed', error_text = 'delivery failed' WHERE id = ?"
            )
            .bind(&nudge_id)
            .execute(&state.db)
            .await;
        }
    }

    Ok(())
}

// ─── Template Resolution ──────────────────────────────────────────────────────

/// Simple template resolution: replaces {key} placeholders with payload values.
/// If template is a plain message string, returns it as-is.
pub(super) fn resolve_template(template: &str, payload_json: &str) -> String {
    let payload: serde_json::Value = serde_json::from_str(payload_json)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    let mut result = template.to_string();
    if let Some(obj) = payload.as_object() {
        for (key, value) in obj {
            let placeholder = format!("{{{}}}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }
    result
}

// ─── Channel Send Helpers ─────────────────────────────────────────────────────

/// Send a WhatsApp message via the Evolution API (marketing route).
/// Psychology nudges are retention/engagement messages — routed through marketing channel.
async fn send_whatsapp(state: &Arc<AppState>, phone: &str, message: &str) -> bool {
    let creds = match state.config.evolution_for(crate::config::WhatsAppCategory::Marketing) {
        Some(c) => c,
        None => {
            tracing::debug!("[psychology] WhatsApp not configured, skipping");
            return false;
        }
    };
    let wa_phone = crate::billing::format_wa_phone(phone);
    let url = format!("{}/message/sendText/{}", creds.url, creds.instance);
    let body = serde_json::json!({ "number": wa_phone, "text": message });
    match state.http_client
        .post(&url)
        .header("apikey", creds.api_key.as_str())
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("[psychology] WhatsApp sent to {}", wa_phone);
            true
        }
        Ok(resp) => {
            tracing::warn!("[psychology] WhatsApp send failed: status={}", resp.status());
            false
        }
        Err(e) => {
            tracing::warn!("[psychology] WhatsApp send error: {}", e);
            false
        }
    }
}

/// Send a message to Discord via webhook.
async fn send_discord(state: &Arc<AppState>, content: &str) -> bool {
    if let Some(webhook_url) = &state.config.integrations.discord.webhook_url {
        let body = serde_json::json!({ "content": content });
        match state.http_client.post(webhook_url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("[psychology] Discord message sent");
                true
            }
            Ok(resp) => {
                tracing::warn!("[psychology] Discord send failed: status={}", resp.status());
                false
            }
            Err(e) => {
                tracing::warn!("[psychology] Discord send error: {}", e);
                false
            }
        }
    } else {
        tracing::debug!("[psychology] Discord webhook not configured, skipping");
        false
    }
}

/// Store a PWA notification in nudge_queue for the PWA to poll.
/// True WebSocket push to individual customers is deferred to Phase 3.
/// PWA queries: SELECT * FROM nudge_queue WHERE driver_id = ? AND channel = 'pwa' AND status = 'sent'
async fn send_pwa_notification(state: &Arc<AppState>, driver_id: &str, template: &str, payload_json: &str) -> bool {
    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status, sent_at)
         VALUES (?, ?, 'pwa', 1, ?, ?, 'sent', datetime('now'))"
    )
    .bind(&id)
    .bind(driver_id)
    .bind(template)
    .bind(payload_json)
    .execute(&state.db)
    .await {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("[psychology] PWA notification insert failed: {}", e);
            false
        }
    }
}

/// Delete old resolved nudge entries after NUDGE_TTL_DAYS.
async fn cleanup_old_nudges(state: &Arc<AppState>) -> anyhow::Result<()> {
    let deleted = sqlx::query(
        "DELETE FROM nudge_queue WHERE status IN ('sent', 'failed', 'expired', 'throttled')
         AND datetime(created_at) < datetime('now', ? || ' days')"
    )
    .bind(-NUDGE_TTL_DAYS) // e.g. '-7 days'
    .execute(&state.db)
    .await?;
    if deleted.rows_affected() > 0 {
        tracing::info!("[psychology] cleaned up {} old nudge entries", deleted.rows_affected());
    }
    Ok(())
}
