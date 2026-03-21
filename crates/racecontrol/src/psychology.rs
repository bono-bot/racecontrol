//! Psychology Engine — centralized badge evaluation, streak tracking,
//! notification budget enforcement, and multi-channel dispatch.
//!
//! Phase 1 Foundation: types, enums, JSON criteria parsing, function stubs.
//! Plan 02 fills in the logic and wiring.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use rand::Rng;

// ─── Enums ────────────────────────────────────────────────────────────────────

/// Notification delivery channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationChannel {
    Whatsapp,
    Discord,
    Pwa,
}

impl NotificationChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Whatsapp => "whatsapp",
            Self::Discord => "discord",
            Self::Pwa => "pwa",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "whatsapp" => Some(Self::Whatsapp),
            "discord" => Some(Self::Discord),
            "pwa" => Some(Self::Pwa),
            _ => None,
        }
    }
}

/// Status of a nudge queue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeStatus {
    Pending,
    Sent,
    Failed,
    Expired,
    Throttled,
}

impl NudgeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Throttled => "throttled",
        }
    }
}

// ─── Badge Criteria ───────────────────────────────────────────────────────────

/// Supported metric types for badge criteria evaluation.
/// Adding a new metric type requires a code change here — this is intentional
/// to keep the JSON schema simple (no DSL/scripting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    TotalLaps,
    UniqueTracks,
    UniqueCars,
    SessionCount,
    PbCount,
    StreakWeeks,
    FirstLap,
}

/// Comparison operators for badge criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "<=")]
    Lte,
    #[serde(rename = "<")]
    Lt,
}

/// Badge criteria as stored in the `achievements.criteria_json` column.
/// Example: `{"type": "total_laps", "operator": ">=", "value": 100}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeCriteria {
    #[serde(rename = "type")]
    pub metric_type: MetricType,
    pub operator: Operator,
    pub value: i64,
}

/// Parse a JSON string from the database into a BadgeCriteria.
/// Returns None if the JSON is malformed or uses unsupported fields.
pub fn parse_criteria_json(json_str: &str) -> Option<BadgeCriteria> {
    serde_json::from_str(json_str).ok()
}

/// Evaluate a badge criteria against a driver's actual metric value.
/// Returns true if the driver meets the criteria.
pub fn evaluate_criteria(criteria: &BadgeCriteria, actual_value: i64) -> bool {
    match criteria.operator {
        Operator::Gte => actual_value >= criteria.value,
        Operator::Gt => actual_value > criteria.value,
        Operator::Eq => actual_value == criteria.value,
        Operator::Lte => actual_value <= criteria.value,
        Operator::Lt => actual_value < criteria.value,
    }
}

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum proactive WhatsApp messages per customer per day (FOUND-01).
pub const WHATSAPP_DAILY_BUDGET: i64 = 2;

/// How often the notification dispatcher drains the queue (seconds).
pub const DISPATCHER_INTERVAL_SECS: u64 = 30;

/// Maximum nudge_queue entries to process per drain cycle.
pub const DISPATCHER_BATCH_SIZE: i64 = 10;

/// Days before old nudge_queue entries are cleaned up.
pub const NUDGE_TTL_DAYS: i64 = 7;

/// Grace period for streaks in days (1 week).
pub const STREAK_GRACE_DAYS: i64 = 7;

// ─── Badge Evaluation ────────────────────────────────────────────────────────

/// Evaluate all badge criteria for a driver after a lap/session event.
/// Loads badge definitions from DB, checks each against driver stats,
/// awards new badges, skips already-earned ones.
pub async fn evaluate_badges(state: &Arc<AppState>, driver_id: &str) {
    // 1. Load all active badge definitions
    let badges: Vec<(String, String)> = match sqlx::query_as(
        "SELECT id, criteria_json FROM achievements WHERE is_active = 1"
    )
    .fetch_all(&state.db)
    .await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("[psychology] failed to load achievements: {}", e);
            return;
        }
    };

    // 2. Load already-earned badge IDs for this driver
    let earned: Vec<(String,)> = sqlx::query_as(
        "SELECT achievement_id FROM driver_achievements WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let earned_ids: std::collections::HashSet<String> = earned.into_iter().map(|r| r.0).collect();

    // 3. For each unearned badge, resolve the metric and evaluate
    for (achievement_id, criteria_json) in &badges {
        if earned_ids.contains(achievement_id) {
            continue; // already earned
        }
        let criteria = match parse_criteria_json(criteria_json) {
            Some(c) => c,
            None => {
                tracing::warn!("[psychology] invalid criteria_json for {}: {}", achievement_id, criteria_json);
                continue;
            }
        };

        let actual_value = resolve_metric(state, driver_id, &criteria.metric_type).await;
        if evaluate_criteria(&criteria, actual_value) {
            // Award the badge
            let id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = sqlx::query(
                "INSERT OR IGNORE INTO driver_achievements (id, driver_id, achievement_id) VALUES (?, ?, ?)"
            )
            .bind(&id)
            .bind(driver_id)
            .bind(achievement_id)
            .execute(&state.db)
            .await {
                tracing::error!("[psychology] failed to award badge {}: {}", achievement_id, e);
            } else {
                tracing::info!("[psychology] badge awarded: driver={} achievement={}", driver_id, achievement_id);
            }
        }
    }
}

/// Resolve a MetricType to a concrete i64 value for a driver via SQL.
async fn resolve_metric(state: &Arc<AppState>, driver_id: &str, metric: &MetricType) -> i64 {
    match metric {
        MetricType::TotalLaps => {
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(total_laps, 0) FROM drivers WHERE id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::UniqueTracks => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT track) FROM driving_passport WHERE driver_id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::UniqueCars => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT car) FROM driving_passport WHERE driver_id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::SessionCount => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status IN ('completed', 'ended_early')"
            ).bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::PbCount => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::StreakWeeks => {
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(current_streak, 0) FROM streaks WHERE driver_id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
        MetricType::FirstLap => {
            // Auto-award: any driver with >= 1 lap qualifies
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(total_laps, 0) FROM drivers WHERE id = ?")
                .bind(driver_id).fetch_one(&state.db).await.unwrap_or(0)
        }
    }
}

// ─── Streak Tracking ──────────────────────────────────────────────────────────

/// Check and update streak for a driver after a session.
/// Compares last_visit_date (IST) with today, increments or resets.
pub async fn update_streak(state: &Arc<AppState>, driver_id: &str) {
    // Get today's date in IST (Asia/Kolkata = UTC+5:30)
    let now_utc = chrono::Utc::now();
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let today_ist = now_utc.with_timezone(&ist_offset).date_naive();
    let today_str = today_ist.format("%Y-%m-%d").to_string();

    // Load existing streak
    let existing: Option<(String, i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, current_streak, longest_streak, last_visit_date, grace_expires_date FROM streaks WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match existing {
        Some((id, current, longest, last_visit, grace_expires)) => {
            // If already visited today (IST), do nothing
            if last_visit.as_deref() == Some(&today_str) {
                return;
            }

            // Check if within grace period (weekly visit window)
            let within_grace = grace_expires
                .as_deref()
                .and_then(|g| chrono::NaiveDate::parse_from_str(g, "%Y-%m-%d").ok())
                .map(|g| today_ist <= g)
                .unwrap_or(false);

            let (new_streak, new_longest, new_started) = if within_grace {
                // Continue streak
                let s = current + 1;
                let l = std::cmp::max(longest, s);
                (s, l, None) // keep existing streak_started_at
            } else {
                // Grace expired — reset to 1
                (1_i64, std::cmp::max(longest, 1), Some(today_str.clone()))
            };

            // Grace expires STREAK_GRACE_DAYS + 7 days from today
            // Design: weekly visits, 7-day grace on top = 14-day total window
            let new_grace = (today_ist + chrono::Duration::days(STREAK_GRACE_DAYS + 7))
                .format("%Y-%m-%d").to_string();

            let mut query_str = String::from(
                "UPDATE streaks SET current_streak = ?, longest_streak = ?, last_visit_date = ?, grace_expires_date = ?, updated_at = datetime('now')"
            );
            if new_started.is_some() {
                query_str.push_str(", streak_started_at = ?");
            }
            query_str.push_str(" WHERE id = ?");

            let mut q = sqlx::query(&query_str)
                .bind(new_streak)
                .bind(new_longest)
                .bind(&today_str)
                .bind(&new_grace);
            if let Some(ref started) = new_started {
                q = q.bind(started);
            }
            q = q.bind(&id);

            if let Err(e) = q.execute(&state.db).await {
                tracing::error!("[psychology] failed to update streak for {}: {}", driver_id, e);
            }
        }
        None => {
            // No streak record — create one starting at 1
            let id = uuid::Uuid::new_v4().to_string();
            let grace = (today_ist + chrono::Duration::days(STREAK_GRACE_DAYS + 7))
                .format("%Y-%m-%d").to_string();
            if let Err(e) = sqlx::query(
                "INSERT INTO streaks (id, driver_id, current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at) VALUES (?, ?, 1, 1, ?, ?, ?)"
            )
            .bind(&id)
            .bind(driver_id)
            .bind(&today_str)
            .bind(&grace)
            .bind(&today_str)
            .execute(&state.db)
            .await {
                tracing::error!("[psychology] failed to create streak for {}: {}", driver_id, e);
            }
        }
    }
}

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
async fn drain_notification_queue(state: &Arc<AppState>) -> anyhow::Result<()> {
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
fn resolve_template(template: &str, payload_json: &str) -> String {
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

/// Send a WhatsApp message via the Evolution API.
/// Follows the same pattern as billing.rs send_whatsapp_receipt.
async fn send_whatsapp(state: &Arc<AppState>, phone: &str, message: &str) -> bool {
    if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        let wa_phone = crate::billing::format_wa_phone(phone);
        let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = serde_json::json!({ "number": wa_phone, "text": message });
        match state.http_client
            .post(&url)
            .header("apikey", evo_key.as_str())
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
    } else {
        tracing::debug!("[psychology] WhatsApp not configured, skipping");
        false
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

    let mut rng = rand::thread_rng();
    if !rng.gen_bool(threshold) {
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ─── Plan 01 tests (pure logic, no DB) ───────────────────────────────────

    #[test]
    fn test_parse_criteria_json_total_laps() {
        let json = r#"{"type":"total_laps","operator":">=","value":100}"#;
        let criteria = parse_criteria_json(json).expect("should parse");
        assert_eq!(criteria.metric_type, MetricType::TotalLaps);
        assert_eq!(criteria.value, 100);
    }

    #[test]
    fn test_parse_criteria_json_unique_tracks() {
        let json = r#"{"type":"unique_tracks","operator":">=","value":10}"#;
        let criteria = parse_criteria_json(json).expect("should parse");
        assert_eq!(criteria.metric_type, MetricType::UniqueTracks);
    }

    #[test]
    fn test_parse_criteria_json_first_lap() {
        let json = r#"{"type":"first_lap","operator":">=","value":1}"#;
        let criteria = parse_criteria_json(json).expect("should parse");
        assert_eq!(criteria.metric_type, MetricType::FirstLap);
    }

    #[test]
    fn test_parse_criteria_json_invalid_returns_none() {
        assert!(parse_criteria_json("not json").is_none());
        assert!(parse_criteria_json(r#"{"type":"unknown","operator":">=","value":1}"#).is_none());
        assert!(parse_criteria_json(r#"{"type":"total_laps"}"#).is_none()); // missing fields
    }

    #[test]
    fn test_evaluate_criteria_gte() {
        let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Gte, value: 100 };
        assert!(evaluate_criteria(&c, 100));
        assert!(evaluate_criteria(&c, 150));
        assert!(!evaluate_criteria(&c, 99));
    }

    #[test]
    fn test_evaluate_criteria_gt() {
        let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Gt, value: 100 };
        assert!(!evaluate_criteria(&c, 100));
        assert!(evaluate_criteria(&c, 101));
    }

    #[test]
    fn test_evaluate_criteria_eq() {
        let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Eq, value: 50 };
        assert!(evaluate_criteria(&c, 50));
        assert!(!evaluate_criteria(&c, 51));
    }

    #[test]
    fn test_evaluate_criteria_lte() {
        let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Lte, value: 10 };
        assert!(evaluate_criteria(&c, 10));
        assert!(evaluate_criteria(&c, 5));
        assert!(!evaluate_criteria(&c, 11));
    }

    #[test]
    fn test_evaluate_criteria_lt() {
        let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Lt, value: 10 };
        assert!(evaluate_criteria(&c, 9));
        assert!(!evaluate_criteria(&c, 10));
    }

    #[test]
    fn test_notification_channel_as_str() {
        assert_eq!(NotificationChannel::Whatsapp.as_str(), "whatsapp");
        assert_eq!(NotificationChannel::Discord.as_str(), "discord");
        assert_eq!(NotificationChannel::Pwa.as_str(), "pwa");
    }

    #[test]
    fn test_notification_channel_from_str() {
        assert_eq!(NotificationChannel::from_str("whatsapp"), Some(NotificationChannel::Whatsapp));
        assert_eq!(NotificationChannel::from_str("discord"), Some(NotificationChannel::Discord));
        assert_eq!(NotificationChannel::from_str("pwa"), Some(NotificationChannel::Pwa));
        assert_eq!(NotificationChannel::from_str("email"), None);
    }

    #[test]
    fn test_nudge_status_as_str() {
        assert_eq!(NudgeStatus::Pending.as_str(), "pending");
        assert_eq!(NudgeStatus::Sent.as_str(), "sent");
        assert_eq!(NudgeStatus::Failed.as_str(), "failed");
        assert_eq!(NudgeStatus::Expired.as_str(), "expired");
        assert_eq!(NudgeStatus::Throttled.as_str(), "throttled");
    }

    #[test]
    fn test_whatsapp_daily_budget_is_2() {
        assert_eq!(WHATSAPP_DAILY_BUDGET, 2);
    }

    // ─── Plan 02 tests (DB-backed) ────────────────────────────────────────────

    /// Build an in-memory SQLite DB with psychology tables for testing.
    /// Foreign key checks are disabled so tests can insert without creating drivers first.
    async fn make_test_db() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        // Disable foreign keys so tests can insert without parent rows
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();

        // drivers table (minimal — for total_laps and phone lookups)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS drivers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                phone TEXT,
                total_laps INTEGER DEFAULT 0
            )"
        ).execute(&pool).await.unwrap();

        // achievements
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS achievements (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                criteria_json TEXT NOT NULL,
                is_active INTEGER DEFAULT 1
            )"
        ).execute(&pool).await.unwrap();

        // driver_achievements
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS driver_achievements (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                achievement_id TEXT NOT NULL,
                earned_at TEXT DEFAULT (datetime('now')),
                UNIQUE(driver_id, achievement_id)
            )"
        ).execute(&pool).await.unwrap();

        // streaks
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS streaks (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL UNIQUE,
                current_streak INTEGER NOT NULL DEFAULT 0,
                longest_streak INTEGER NOT NULL DEFAULT 0,
                last_visit_date TEXT,
                grace_expires_date TEXT,
                streak_started_at TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        // driving_passport (for UniqueTracks/UniqueCars metric)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS driving_passport (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                track TEXT NOT NULL,
                car TEXT NOT NULL,
                UNIQUE(driver_id, track, car)
            )"
        ).execute(&pool).await.unwrap();

        // billing_sessions (for SessionCount metric)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                status TEXT NOT NULL
            )"
        ).execute(&pool).await.unwrap();

        // personal_bests (for PbCount metric)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS personal_bests (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL
            )"
        ).execute(&pool).await.unwrap();

        // nudge_queue
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nudge_queue (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 5,
                template TEXT NOT NULL,
                payload_json TEXT DEFAULT '{}',
                status TEXT NOT NULL DEFAULT 'pending',
                scheduled_at TEXT DEFAULT (datetime('now')),
                expires_at TEXT,
                sent_at TEXT,
                error_text TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.unwrap();

        pool
    }

    /// Build a minimal AppState using the provided pool.
    async fn make_state_with_db(db: sqlx::SqlitePool) -> Arc<AppState> {
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    // ─── Badge evaluation tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_evaluate_badges_awards_badge_for_100_laps() {
        let db = make_test_db().await;
        let driver_id = "driver-badge-1";

        // Insert driver with 100 total_laps
        sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 100)")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        // Insert achievement: total_laps >= 100
        sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
            .bind("ach-century")
            .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
            .execute(&db)
            .await
            .unwrap();

        let state = make_state_with_db(db).await;
        evaluate_badges(&state, driver_id).await;

        // Badge should be awarded
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century'"
        )
        .bind(driver_id)
        .fetch_one(&state.db)
        .await
        .unwrap();

        assert_eq!(count, 1, "Badge should be awarded for 100 laps");
    }

    #[tokio::test]
    async fn test_evaluate_badges_skips_already_earned() {
        let db = make_test_db().await;
        let driver_id = "driver-badge-2";

        sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 200)")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
            .bind("ach-century-2")
            .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
            .execute(&db)
            .await
            .unwrap();

        // Pre-insert the earned badge
        sqlx::query("INSERT INTO driver_achievements (id, driver_id, achievement_id) VALUES (?, ?, ?)")
            .bind("da-existing")
            .bind(driver_id)
            .bind("ach-century-2")
            .execute(&db)
            .await
            .unwrap();

        let state = make_state_with_db(db).await;
        evaluate_badges(&state, driver_id).await;

        // Should still be exactly 1 row — no duplicate
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century-2'"
        )
        .bind(driver_id)
        .fetch_one(&state.db)
        .await
        .unwrap();

        assert_eq!(count, 1, "Badge should not be duplicated");
    }

    #[tokio::test]
    async fn test_evaluate_badges_does_not_award_below_threshold() {
        let db = make_test_db().await;
        let driver_id = "driver-badge-3";

        // Driver has only 50 laps
        sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 50)")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
            .bind("ach-century-3")
            .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
            .execute(&db)
            .await
            .unwrap();

        let state = make_state_with_db(db).await;
        evaluate_badges(&state, driver_id).await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century-3'"
        )
        .bind(driver_id)
        .fetch_one(&state.db)
        .await
        .unwrap();

        assert_eq!(count, 0, "Badge should NOT be awarded for 50 laps (need 100)");
    }

    // ─── Streak tracking tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_streak_creates_new_row() {
        let db = make_test_db().await;
        let driver_id = "driver-streak-1";

        // Insert driver (FK off so not strictly needed, but good practice)
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        let state = make_state_with_db(db).await;
        update_streak(&state, driver_id).await;

        let row: Option<(i64, i64)> = sqlx::query_as(
            "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (current, longest) = row.expect("streak row should exist");
        assert_eq!(current, 1, "New streak should start at 1");
        assert_eq!(longest, 1, "New longest should start at 1");
    }

    #[tokio::test]
    async fn test_update_streak_same_date_does_not_change() {
        let db = make_test_db().await;
        let driver_id = "driver-streak-2";

        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        let state = make_state_with_db(db).await;

        // Call once to create streak
        update_streak(&state, driver_id).await;

        // Call again — should be idempotent (same IST day)
        update_streak(&state, driver_id).await;

        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT current_streak FROM streaks WHERE driver_id = ?"
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (current,) = row.expect("streak row should exist");
        assert_eq!(current, 1, "Streak should not increment when visiting same day");
    }

    #[tokio::test]
    async fn test_update_streak_within_grace_increments() {
        let db = make_test_db().await;
        let driver_id = "driver-streak-3";

        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        // Insert an existing streak with last_visit 7 days ago (within 14-day grace)
        let past_date = (chrono::Utc::now() - chrono::Duration::days(7))
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let future_grace = (chrono::Utc::now() + chrono::Duration::days(7))
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();

        sqlx::query(
            "INSERT INTO streaks (id, driver_id, current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at) VALUES (?, ?, 3, 3, ?, ?, ?)"
        )
        .bind("streak-id-3")
        .bind(driver_id)
        .bind(&past_date)
        .bind(&future_grace)
        .bind(&past_date)
        .execute(&db)
        .await
        .unwrap();

        let state = make_state_with_db(db).await;
        update_streak(&state, driver_id).await;

        let row: Option<(i64, i64)> = sqlx::query_as(
            "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (current, longest) = row.expect("streak should exist");
        assert_eq!(current, 4, "Streak should increment from 3 to 4 within grace period");
        assert_eq!(longest, 4, "Longest should update when current exceeds it");
    }

    #[tokio::test]
    async fn test_update_streak_after_grace_resets() {
        let db = make_test_db().await;
        let driver_id = "driver-streak-4";

        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();

        // Insert streak with grace that expired 1 day ago
        let past_date = (chrono::Utc::now() - chrono::Duration::days(30))
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let expired_grace = (chrono::Utc::now() - chrono::Duration::days(1))
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();

        sqlx::query(
            "INSERT INTO streaks (id, driver_id, current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at) VALUES (?, ?, 5, 5, ?, ?, ?)"
        )
        .bind("streak-id-4")
        .bind(driver_id)
        .bind(&past_date)
        .bind(&expired_grace)
        .bind(&past_date)
        .execute(&db)
        .await
        .unwrap();

        let state = make_state_with_db(db).await;
        update_streak(&state, driver_id).await;

        let row: Option<(i64, i64)> = sqlx::query_as(
            "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (current, longest) = row.expect("streak should exist");
        assert_eq!(current, 1, "Streak should reset to 1 after grace expires");
        assert_eq!(longest, 5, "Longest should be preserved at previous high");
    }

    // ─── WhatsApp budget tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_budget_not_exceeded_with_zero_sent() {
        let db = make_test_db().await;
        let driver_id = "driver-budget-1";
        let state = make_state_with_db(db).await;

        let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
        assert!(!exceeded, "Budget should not be exceeded with 0 sent messages");
    }

    #[tokio::test]
    async fn test_budget_not_exceeded_with_one_sent() {
        let db = make_test_db().await;
        let driver_id = "driver-budget-2";

        sqlx::query(
            "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'test', 'sent', datetime('now'))"
        )
        .bind("nq-1")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

        let state = make_state_with_db(db).await;
        let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
        assert!(!exceeded, "Budget should not be exceeded with 1 sent message");
    }

    #[tokio::test]
    async fn test_budget_exceeded_with_two_sent() {
        let db = make_test_db().await;
        let driver_id = "driver-budget-3";

        for i in 0..2 {
            sqlx::query(
                "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'test', 'sent', datetime('now'))"
            )
            .bind(format!("nq-budget-{}", i))
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();
        }

        let state = make_state_with_db(db).await;
        let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
        assert!(exceeded, "Budget should be exceeded with 2 sent messages");
    }

    // ─── queue_notification test ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_queue_notification_inserts_pending_row() {
        let db = make_test_db().await;
        let driver_id = "driver-queue-1";
        let state = make_state_with_db(db).await;

        queue_notification(
            &state,
            driver_id,
            NotificationChannel::Pwa,
            3,
            "You have a new badge!",
            "{}",
        ).await;

        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM nudge_queue WHERE driver_id = ?"
        )
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (status,) = row.expect("nudge_queue row should exist");
        assert_eq!(status, "pending", "Queued notification should have status='pending'");
    }

    // ─── drain_notification_queue: throttle test ──────────────────────────────

    #[tokio::test]
    async fn test_drain_throttles_whatsapp_when_budget_exceeded() {
        let db = make_test_db().await;
        let driver_id = "driver-throttle-1";

        // Insert 2 already-sent WhatsApp messages today (budget used up)
        for i in 0..2 {
            sqlx::query(
                "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'prev', 'sent', datetime('now'))"
            )
            .bind(format!("nq-prev-{}", i))
            .bind(driver_id)
            .execute(&db)
            .await
            .unwrap();
        }

        // Insert a new pending WhatsApp message
        sqlx::query(
            "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status) VALUES (?, ?, 'whatsapp', 5, 'Hello!', '{}', 'pending')"
        )
        .bind("nq-new")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

        let state = make_state_with_db(db).await;
        drain_notification_queue(&state).await.unwrap();

        // The pending message should be throttled
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM nudge_queue WHERE id = 'nq-new'"
        )
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (status,) = row.expect("nudge row should exist");
        assert_eq!(status, "throttled", "WhatsApp message should be throttled when budget exceeded");
    }

    // ─── drain_notification_queue: expired entries ────────────────────────────

    #[tokio::test]
    async fn test_drain_marks_expired_entries() {
        let db = make_test_db().await;
        let driver_id = "driver-expire-1";

        // Insert a pending entry that already expired (1 hour ago)
        sqlx::query(
            "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status, expires_at) VALUES (?, ?, 'pwa', 5, 'old', '{}', 'pending', datetime('now', '-1 hour'))"
        )
        .bind("nq-expired")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

        let state = make_state_with_db(db).await;
        drain_notification_queue(&state).await.unwrap();

        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM nudge_queue WHERE id = 'nq-expired'"
        )
        .fetch_optional(&state.db)
        .await
        .unwrap();

        let (status,) = row.expect("nudge row should exist");
        assert_eq!(status, "expired", "Past-deadline entries should be marked expired");
    }

    // ─── resolve_template test ────────────────────────────────────────────────

    #[test]
    fn test_resolve_template_substitutes_placeholders() {
        let result = resolve_template("Hello {name}, you earned {badge}!", r#"{"name":"Uday","badge":"Century"}"#);
        assert_eq!(result, "Hello Uday, you earned Century!");
    }

    #[test]
    fn test_resolve_template_plain_string_passthrough() {
        let result = resolve_template("No placeholders here.", "{}");
        assert_eq!(result, "No placeholders here.");
    }
}
