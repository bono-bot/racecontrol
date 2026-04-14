//! Psychology nudge subsystem — notification queue, dispatcher, channel routing,
//! driving passport, and retention loop functions.

use std::sync::Arc;
use crate::state::AppState;
use rand::Rng;
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
        if channel == NotificationChannel::Whatsapp {
            if is_whatsapp_budget_exceeded(state, &driver_id).await {
                let _ = sqlx::query(
                    "UPDATE nudge_queue SET status = 'throttled', error_text = 'daily budget exceeded' WHERE id = ?"
                )
                .bind(&nudge_id)
                .execute(&state.db)
                .await;
                tracing::info!("[psychology] throttled WhatsApp nudge {} for driver {}", nudge_id, driver_id);
                continue;
            }
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

// ─── Driving Passport ─────────────────────────────────────────────────────────

/// Upsert a driver's driving passport entry for a specific track+car combination.
/// Called from persist_lap() after every valid lap INSERT.
/// Uses ON CONFLICT to increment lap_count and update best_lap_ms if faster.
pub async fn update_driving_passport(
    state: &Arc<AppState>,
    driver_id: &str,
    track: &str,
    car: &str,
    lap_time_ms: i64,
) {
    let id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO driving_passport (id, driver_id, track, car, best_lap_ms, lap_count)
         VALUES (?, ?, ?, ?, ?, 1)
         ON CONFLICT(driver_id, track, car) DO UPDATE SET
           lap_count = driving_passport.lap_count + 1,
           best_lap_ms = CASE WHEN excluded.best_lap_ms < driving_passport.best_lap_ms
                         THEN excluded.best_lap_ms ELSE driving_passport.best_lap_ms END"
    )
    .bind(&id)
    .bind(driver_id)
    .bind(track)
    .bind(car)
    .bind(lap_time_ms)
    .execute(&state.db)
    .await {
        tracing::error!("[psychology] driving_passport upsert failed: {}", e);
    }
}

/// Backfill driving_passport from the laps table for a single driver.
/// Called lazily on first /customer/passport API call when passport is empty.
/// Uses INSERT OR IGNORE so concurrent calls are safe (UNIQUE constraint).
/// Only processes valid laps with lap_time_ms > 0.
pub async fn backfill_driving_passport(state: &Arc<AppState>, driver_id: &str) {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO driving_passport (id, driver_id, track, car, first_driven_at, best_lap_ms, lap_count)
         SELECT
             lower(hex(randomblob(16))),
             driver_id,
             track,
             car,
             MIN(created_at) as first_driven_at,
             MIN(lap_time_ms) as best_lap_ms,
             COUNT(*) as lap_count
         FROM laps
         WHERE driver_id = ? AND valid = 1 AND lap_time_ms > 0
         GROUP BY driver_id, track, car"
    )
    .bind(driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                tracing::info!(
                    "[psychology] backfilled {} driving_passport entries for driver {}",
                    r.rows_affected(), driver_id
                );
            }
        }
        Err(e) => tracing::error!("[psychology] driving_passport backfill failed for {}: {}", driver_id, e),
    }
}

// ─── Retention Loop Functions (Phase 92) ─────────────────────────────────────

/// RET-02: Notify other active drivers whose PB on the same track+car was just beaten.
/// Only notifies drivers within 5% of the new time who have been active in the last 30 days.
/// Capped at 5 notifications per PB event to limit fan-out.
pub async fn notify_pb_beaten_holders(
    state: &Arc<AppState>,
    new_holder_driver_id: &str,
    track: &str,
    car: &str,
    new_lap_time_ms: i64,
) {
    // Find up to 5 OTHER active drivers whose PB is now slower but within 5% of new time
    // Integer math: best_lap_ms <= new_lap_time_ms * 105 / 100 means within 5%
    let beaten_drivers: Vec<(String,)> = match sqlx::query_as(
        "SELECT pb.driver_id
         FROM personal_bests pb
         JOIN billing_sessions bs ON bs.driver_id = pb.driver_id
         WHERE pb.track = ?
           AND pb.car = ?
           AND pb.driver_id != ?
           AND pb.best_lap_ms > ?
           AND pb.best_lap_ms <= ? * 105 / 100
           AND bs.status IN ('completed', 'ended_early')
           AND datetime(bs.ended_at) > datetime('now', '-30 days')
         GROUP BY pb.driver_id
         LIMIT 5",
    )
    .bind(track)
    .bind(car)
    .bind(new_holder_driver_id)
    .bind(new_lap_time_ms)
    .bind(new_lap_time_ms)
    .fetch_all(&state.db)
    .await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("[psychology] notify_pb_beaten_holders query failed: {}", e);
            return;
        }
    };

    for (driver_id,) in beaten_drivers {
        let payload = serde_json::json!({
            "track": track,
            "car": car,
            "new_time_ms": new_lap_time_ms,
        }).to_string();
        queue_notification(
            state,
            &driver_id,
            NotificationChannel::Whatsapp,
            3,
            "pb_beaten",
            &payload,
        ).await;
        tracing::info!(
            "[psychology] pb_beaten nudge queued for driver {} (track={} car={} new_time_ms={})",
            driver_id, track, car, new_lap_time_ms
        );
    }
}

/// RET-03 + RET-06: Maybe credit a surprise bonus to the driver based on trigger type.
/// Probability: pb=15%, milestone=10%. Capped at 5% of driver's total spend per month.
pub async fn maybe_grant_variable_reward(
    state: &Arc<AppState>,
    driver_id: &str,
    trigger: &str,
) {
    let threshold = match trigger {
        "pb" => 0.15_f64,
        "milestone" => 0.10_f64,
        _ => return,
    };

    // Evaluate random gate before any await — ThreadRng is !Send and cannot
    // be held across an await point (tokio::spawn requires Send futures).
    let should_proceed = {
        let mut rng = rand::thread_rng();
        rng.gen_bool(threshold)
    };
    if !should_proceed {
        return;
    }

    // Compute IST month string (YYYY-MM)
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let now_ist = chrono::Utc::now().with_timezone(&ist_offset);
    let month_str = now_ist.format("%Y-%m").to_string();

    // RET-06: check monthly cap (5% of total spend)
    let total_spend: i64 = sqlx::query_scalar(
        "SELECT COALESCE(total_debited_paise, 0) FROM wallets WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let cap_paise = total_spend / 20; // 5% of total spend

    let already_rewarded: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM variable_reward_log
         WHERE driver_id = ? AND month = ?"
    )
    .bind(driver_id)
    .bind(&month_str)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if already_rewarded >= cap_paise {
        tracing::info!(
            "[psychology] variable reward cap reached for driver {} this month (cap={}p already={}p)",
            driver_id, cap_paise, already_rewarded
        );
        return;
    }

    // Base amount: pb=50 credits (5000p), milestone=100 credits (10000p)
    let base_amount: i64 = match trigger {
        "pb" => 5000,
        "milestone" => 10000,
        _ => 5000,
    };
    let amount = base_amount.min(cap_paise - already_rewarded);
    if amount <= 0 {
        return;
    }

    let reward_id = uuid::Uuid::new_v4().to_string();
    let _ = crate::wallet::credit(
        state,
        driver_id,
        amount,
        "bonus",
        Some(&reward_id),
        Some(&format!("Surprise bonus — {}", trigger)),
        None,
    ).await;

    // Log for monthly cap tracking
    let _ = sqlx::query(
        "INSERT INTO variable_reward_log (id, driver_id, amount_paise, trigger, month, created_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))"
    )
    .bind(&reward_id)
    .bind(driver_id)
    .bind(amount)
    .bind(trigger)
    .bind(&month_str)
    .execute(&state.db)
    .await;

    tracing::info!(
        "[psychology] variable reward granted: driver={} trigger={} amount_paise={}",
        driver_id, trigger, amount
    );
}

/// RET-05: Find streaks expiring in 2 days (IST) with current_streak >= 2
/// and queue a streak_at_risk WhatsApp nudge. Deduplicates against recent nudges.
pub async fn check_streak_at_risk(state: &Arc<AppState>) -> anyhow::Result<()> {
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let today_ist = chrono::Utc::now().with_timezone(&ist_offset).date_naive();
    let two_days_from_now = (today_ist + chrono::Duration::days(2))
        .format("%Y-%m-%d").to_string();

    let at_risk: Vec<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT s.driver_id, s.current_streak, s.grace_expires_date
         FROM streaks s
         WHERE date(s.grace_expires_date) = ?
           AND s.current_streak >= 2
           AND NOT EXISTS (
               SELECT 1 FROM nudge_queue nq
               WHERE nq.driver_id = s.driver_id
                 AND nq.template = 'streak_at_risk'
                 AND nq.status IN ('pending', 'sent')
                 AND datetime(nq.created_at) > datetime('now', '-8 days')
           )"
    )
    .bind(&two_days_from_now)
    .fetch_all(&state.db)
    .await?;

    for (driver_id, current_streak, grace_expires_date) in at_risk {
        let payload = serde_json::json!({
            "current_streak": current_streak,
            "grace_expires_date": grace_expires_date,
        }).to_string();
        queue_notification(
            state,
            &driver_id,
            NotificationChannel::Whatsapp,
            2,
            "streak_at_risk",
            &payload,
        ).await;
        tracing::info!(
            "[psychology] streak_at_risk nudge queued for driver {} (streak={} expires={})",
            driver_id, current_streak, grace_expires_date.as_deref().unwrap_or("unknown")
        );
    }

    Ok(())
}

/// RET-04: Find active memberships expiring within 3 days and queue loss-framed
/// WhatsApp warnings. Deduplicates against recent nudges (within 4 days).
pub async fn check_membership_expiry_warnings(state: &Arc<AppState>) -> anyhow::Result<()> {
    let expiring: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT m.driver_id, mt.name, m.expires_at
         FROM memberships m
         JOIN membership_tiers mt ON mt.id = m.tier_id
         WHERE m.status = 'active'
           AND datetime(m.expires_at) <= datetime('now', '+3 days')
           AND datetime(m.expires_at) > datetime('now')
           AND NOT EXISTS (
               SELECT 1 FROM nudge_queue nq
               WHERE nq.driver_id = m.driver_id
                 AND nq.template = 'membership_expiry'
                 AND nq.status IN ('pending', 'sent')
                 AND datetime(nq.created_at) > datetime('now', '-4 days')
           )"
    )
    .fetch_all(&state.db)
    .await?;

    for (driver_id, tier_name, expires_at) in expiring {
        let payload = serde_json::json!({
            "tier_name": tier_name,
            "expires_at": expires_at,
        }).to_string();
        queue_notification(
            state,
            &driver_id,
            NotificationChannel::Whatsapp,
            1,
            "membership_expiry",
            &payload,
        ).await;
        tracing::info!(
            "[psychology] membership_expiry nudge queued for driver {} (tier={} expires={})",
            driver_id, tier_name, expires_at
        );
    }

    Ok(())
}
