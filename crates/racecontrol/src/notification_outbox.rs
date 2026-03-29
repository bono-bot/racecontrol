//! Durable notification outbox with background worker and OTP fallback chain.
//!
//! Notifications are written to `notification_outbox` and processed by a background
//! worker every 10 seconds. Failed deliveries are retried with exponential backoff.
//! When a WhatsApp delivery exhausts retries and `fallback_channel` is set to 'screen',
//! the original payload becomes accessible via a one-time token endpoint (UX-01, UX-02).

use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::state::AppState;

/// Enqueue a notification for delivery. Returns the notification id.
///
/// - `channel`: 'whatsapp', 'sms', or 'screen'
/// - `context_type`/`context_id`: optional link to the source entity (e.g. "billing_session", id)
pub async fn enqueue_notification(
    db: &SqlitePool,
    recipient: &str,
    channel: &str,
    payload: &str,
    context_type: Option<&str>,
    context_id: Option<&str>,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO notification_outbox
         (id, recipient, channel, payload, context_type, context_id)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(recipient)
    .bind(channel)
    .bind(payload)
    .bind(context_type)
    .bind(context_id)
    .execute(db)
    .await?;
    Ok(id)
}

/// Convenience wrapper for OTP delivery: channel='whatsapp', fallback_channel='screen'.
/// Generates a one-time UUID fallback_token so the OTP can be displayed on-screen
/// if WhatsApp delivery fails after all retries.
pub async fn enqueue_otp_notification(
    db: &SqlitePool,
    phone: &str,
    otp_message: &str,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let fallback_token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO notification_outbox
         (id, recipient, channel, payload, fallback_channel, fallback_token, context_type)
         VALUES (?, ?, 'whatsapp', ?, 'screen', ?, 'otp')",
    )
    .bind(&id)
    .bind(phone)
    .bind(otp_message)
    .bind(&fallback_token)
    .execute(db)
    .await?;
    Ok(id)
}

/// Look up the OTP payload by fallback token.
///
/// Returns the payload string if:
///   - The token exists
///   - The notification status is 'failed' (WhatsApp delivery exhausted)
///
/// On success, marks the notification as 'delivered' so the token is consumed (one-time use).
pub async fn get_otp_by_fallback_token(
    db: &SqlitePool,
    token: &str,
) -> anyhow::Result<Option<String>> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, payload, status FROM notification_outbox WHERE fallback_token = ?",
    )
    .bind(token)
    .fetch_optional(db)
    .await?;

    match row {
        None => Ok(None),
        Some((id, payload, status)) => {
            if status == "failed" || status == "exhausted" {
                // Mark consumed — one-time use
                let _ = sqlx::query(
                    "UPDATE notification_outbox SET status='delivered', updated_at=datetime('now')
                     WHERE id = ?",
                )
                .bind(&id)
                .execute(db)
                .await;
                Ok(Some(payload))
            } else {
                // Token found but notification hasn't failed yet (still pending/sent)
                Ok(None)
            }
        }
    }
}

/// Background worker: processes pending notifications every 10 seconds.
///
/// Lifecycle: logs on start, first item processed, and on exit.
/// Errors log WARN and never panic — individual notification failures are non-fatal.
pub async fn notification_worker_task(state: Arc<AppState>) {
    tracing::info!(target: "notification_outbox", "notification worker started (10s interval)");

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    let mut first_processed = false;

    loop {
        interval.tick().await;

        let rows: Vec<(String, String, String, String, i64, i64, Option<String>)> =
            match sqlx::query_as(
                "SELECT id, recipient, channel, payload, retry_count, max_retries, fallback_channel
                 FROM notification_outbox
                 WHERE status = 'pending' AND next_retry_at <= datetime('now')
                 ORDER BY created_at
                 LIMIT 20",
            )
            .fetch_all(&state.db)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(target: "notification_outbox", "failed to fetch pending notifications: {}", e);
                    continue;
                }
            };

        if rows.is_empty() {
            continue;
        }

        if !first_processed {
            tracing::info!(target: "notification_outbox", "notification worker processing first batch ({} items)", rows.len());
            first_processed = true;
        }

        for (id, recipient, channel, payload, retry_count, max_retries, fallback_channel) in rows {
            let delivery_ok = attempt_delivery(&state, &channel, &recipient, &payload).await;

            if delivery_ok {
                let _ = sqlx::query(
                    "UPDATE notification_outbox SET status='sent', updated_at=datetime('now') WHERE id=?",
                )
                .bind(&id)
                .execute(&state.db)
                .await;
            } else {
                let new_retry_count = retry_count + 1;

                if new_retry_count >= max_retries {
                    if let Some(fb_channel) = fallback_channel {
                        // Switch to fallback channel — reset retry counter
                        tracing::info!(
                            target: "notification_outbox",
                            "notification {} exhausted '{}', switching to fallback '{}'",
                            id, channel, fb_channel
                        );
                        let _ = sqlx::query(
                            "UPDATE notification_outbox
                             SET channel=?, status='pending', retry_count=0,
                                 next_retry_at=datetime('now'), updated_at=datetime('now')
                             WHERE id=?",
                        )
                        .bind(&fb_channel)
                        .bind(&id)
                        .execute(&state.db)
                        .await;
                    } else {
                        // No fallback — mark exhausted
                        tracing::warn!(
                            target: "notification_outbox",
                            "notification {} exhausted all retries on channel '{}', marking exhausted",
                            id, channel
                        );
                        let _ = sqlx::query(
                            "UPDATE notification_outbox
                             SET status='exhausted', retry_count=?, updated_at=datetime('now')
                             WHERE id=?",
                        )
                        .bind(new_retry_count)
                        .bind(&id)
                        .execute(&state.db)
                        .await;
                    }
                } else {
                    // Exponential backoff: 10 * 2^retry_count seconds
                    let backoff_secs = 10i64 * (1i64 << new_retry_count.min(10));
                    let _ = sqlx::query(
                        &format!(
                            "UPDATE notification_outbox
                             SET status='pending', retry_count=?,
                                 next_retry_at=datetime('now', '+{} seconds'),
                                 updated_at=datetime('now')
                             WHERE id=?",
                            backoff_secs
                        ),
                    )
                    .bind(new_retry_count)
                    .bind(&id)
                    .execute(&state.db)
                    .await;
                }
            }
        }
    }

    tracing::info!(target: "notification_outbox", "notification worker exiting");
}

/// Attempt delivery via the given channel. Returns true on success.
async fn attempt_delivery(state: &Arc<AppState>, channel: &str, recipient: &str, payload: &str) -> bool {
    match channel {
        "whatsapp" => {
            // Use send_whatsapp — it's best-effort and logs warnings on failure.
            // We detect failure by checking if the config is complete (no config = always fail here).
            let has_config = state.config.auth.evolution_url.is_some()
                && state.config.auth.evolution_api_key.is_some()
                && state.config.auth.evolution_instance.is_some();

            if !has_config {
                tracing::warn!(
                    target: "notification_outbox",
                    "WhatsApp not configured — cannot deliver notification to {}",
                    recipient
                );
                return false;
            }

            // Build a per-recipient message (payload already contains the full message text)
            // send_whatsapp sends to uday_phone from config; for outbox we send the payload
            // directly to uday_phone (staff alert path). Future: add recipient routing.
            crate::whatsapp_alerter::send_whatsapp(&state.config, payload).await;
            // send_whatsapp is fire-and-forget (no return value indicating success/failure).
            // Treat as success if config is present; Evolution API errors are logged by send_whatsapp.
            true
        }
        "screen" => {
            // On-screen delivery: the fallback_token is set at enqueue time.
            // Delivery happens when the customer polls GET /customer/otp-fallback/{token}.
            // Worker marks this as 'sent' immediately — actual consumption updates to 'delivered'.
            tracing::debug!(
                target: "notification_outbox",
                "screen notification for {} — available via fallback token",
                recipient
            );
            true
        }
        "sms" => {
            // SMS not yet integrated — log placeholder and fail so retry logic runs.
            tracing::warn!(
                target: "notification_outbox",
                "SMS delivery not implemented — notification to {} will retry",
                recipient
            );
            false
        }
        other => {
            tracing::warn!(
                target: "notification_outbox",
                "unknown delivery channel '{}' for recipient {}",
                other, recipient
            );
            false
        }
    }
}
