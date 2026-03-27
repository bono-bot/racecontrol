//! Smart Scheduling — auto-wake pods before bookings,
//! dynamic pricing suggestions based on peak/off-peak patterns.

use std::sync::Arc;
use chrono::{Local, NaiveTime, Timelike, Datelike, Weekday, Utc, FixedOffset};
use crate::state::AppState;
use crate::wol;
use rc_common::types::PodStatus;

/// Spawn the scheduler background loop (runs every 60 seconds).
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = tick(&state).await {
                tracing::error!("[scheduler] tick error: {}", e);
            }
        }
    });
}

async fn tick(state: &Arc<AppState>) -> anyhow::Result<()> {
    let now = Local::now();
    let time_now = now.time();

    // Load operating hours from kiosk_settings
    let open = get_setting(state, "business_hours_start").await.unwrap_or_else(|| "10:00".into());
    let open_time = NaiveTime::parse_from_str(&open, "%H:%M").unwrap_or(NaiveTime::from_hms_opt(10, 0, 0).unwrap());

    // Check if auto-scheduling is enabled
    let enabled = get_setting(state, "scheduler_enabled").await.unwrap_or_else(|| "true".into());
    if enabled != "true" {
        return Ok(());
    }

    // ─── Pre-booking wake ────────────────────────────────────────────────────
    // Wake pods 15 minutes before confirmed bookings
    let wake_minutes: i64 = get_setting(state, "scheduler_pre_wake_minutes").await
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);

    let upcoming_bookings = sqlx::query_as::<_, (String, String)>(
        "SELECT b.pod_id, b.start_time FROM bookings b
         WHERE b.status = 'confirmed'
           AND b.pod_id IS NOT NULL
           AND datetime(b.start_time, ? || ' minutes') <= datetime('now')
           AND datetime(b.start_time) > datetime('now')
         ORDER BY b.start_time ASC",
    )
    .bind(format!("-{}", wake_minutes))
    .fetch_all(&state.db)
    .await?;

    for (pod_id, start_time) in &upcoming_bookings {
        // Check if pod is offline
        let pods = state.pods.read().await;
        let is_offline = pods.get(pod_id)
            .map(|p| matches!(p.status, PodStatus::Offline))
            .unwrap_or(true);
        drop(pods);

        if is_offline {
            // Look up MAC address
            let mac = sqlx::query_as::<_, (Option<String>,)>(
                "SELECT config_json FROM pods WHERE id = ?",
            )
            .bind(pod_id)
            .fetch_optional(&state.db)
            .await?
            .and_then(|r| r.0)
            .and_then(|json| {
                serde_json::from_str::<serde_json::Value>(&json).ok()
                    .and_then(|v| v["mac_address"].as_str().map(|s| s.to_string()))
            });

            if let Some(mac) = mac {
                tracing::info!(
                    "[scheduler] Pre-wake: sending WoL to pod {} (booking at {})",
                    pod_id, start_time
                );
                let _ = wol::send_wol(&mac).await;

                // Log the event
                let _ = sqlx::query(
                    "INSERT INTO scheduler_events (id, event_type, pod_id, details, created_at)
                     VALUES (?, 'pre_wake', ?, ?, datetime('now'))",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(pod_id)
                .bind(format!("WoL sent for booking at {}", start_time))
                .execute(&state.db)
                .await;
            }
        }
    }

    // ─── Opening hours: wake all pods ────────────────────────────────────────
    // 10 minutes before opening, wake all pods
    let pre_open_mins: u32 = get_setting(state, "scheduler_pre_open_minutes").await
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let minutes_until_open = minutes_between(time_now, open_time);
    if minutes_until_open > 0 && minutes_until_open <= pre_open_mins as i64 {
        // Only wake once per day — check if we already did
        let today = now.format("%Y-%m-%d").to_string();
        let already_woke = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM scheduler_events
             WHERE event_type = 'open_wake_all' AND date(created_at) = ?",
        )
        .bind(&today)
        .fetch_one(&state.db)
        .await?
        .0;

        if already_woke == 0 {
            tracing::info!("[scheduler] Opening soon — waking all pods");
            wake_all_pods(state).await;

            let _ = sqlx::query(
                "INSERT INTO scheduler_events (id, event_type, details, created_at)
                 VALUES (?, 'open_wake_all', 'Auto-wake all pods before opening', datetime('now'))",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .execute(&state.db)
            .await;
        }
    }

    // ─── Multiplayer invite timeout cleanup ──────────────────────────────
    crate::multiplayer::cleanup_stale_invites(state).await;

    // ─── Peak hour tracking ──────────────────────────────────────────────────
    // Every hour on the hour, snapshot active session count for analytics
    if time_now.minute() == 0 {
        let active_count = {
            let timers = state.billing.active_timers.read().await;
            timers.len() as i64
        };

        let _ = sqlx::query(
            "INSERT INTO scheduler_events (id, event_type, details, created_at)
             VALUES (?, 'hourly_snapshot', ?, datetime('now'))",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(format!(
            "{{\"hour\":{},\"day_of_week\":{},\"active_sessions\":{},\"is_weekend\":{}}}",
            time_now.hour(),
            now.weekday().num_days_from_monday(),
            active_count,
            matches!(now.weekday(), Weekday::Sat | Weekday::Sun)
        ))
        .execute(&state.db)
        .await;
    }

    // ─── Daily retention checks (10:00 AM IST) ────────────────────────────────
    // Run once per hour guard: only fire at 10 AM IST (minute 0-1 window)
    let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let now_ist = Utc::now().with_timezone(&ist_offset);
    if now_ist.hour() == 10 && now_ist.minute() < 2 {
        // Streak-at-risk nudges (RET-05)
        if let Err(e) = crate::psychology::check_streak_at_risk(state).await {
            tracing::error!("[scheduler] check_streak_at_risk error: {}", e);
        }
        // Membership expiry loss-framed warnings (RET-04)
        if let Err(e) = crate::psychology::check_membership_expiry_warnings(state).await {
            tracing::error!("[scheduler] check_membership_expiry_warnings error: {}", e);
        }
    }

    // ─── Reservation expiry cleanup ──────────────────────────────────────
    if let Err(e) = expire_reservations(state).await {
        tracing::error!("[scheduler] expire_reservations error: {}", e);
    }

    Ok(())
}

// ─── Reservation expiry ──────────────────────────────────────────────────────

/// Mark expired reservations and create refund debit_intents for completed debits.
/// Runs every tick (60s). Finds reservations where status IN ('pending_debit', 'confirmed')
/// AND expires_at < datetime('now').
async fn expire_reservations(state: &Arc<AppState>) -> anyhow::Result<()> {
    // Find expired reservations (both pending_debit and confirmed can expire)
    let expired = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, driver_id, debit_intent_id FROM reservations
         WHERE status IN ('pending_debit', 'confirmed')
         AND expires_at < datetime('now')
         LIMIT 100"
    )
    .fetch_all(&state.db)
    .await?;

    for (res_id, driver_id, debit_intent_id) in &expired {
        // Mark reservation as expired
        sqlx::query(
            "UPDATE reservations SET status = 'expired', updated_at = datetime('now') WHERE id = ?"
        )
        .bind(res_id)
        .execute(&state.db)
        .await?;

        // Handle debit intent cleanup
        if let Some(intent_id) = debit_intent_id {
            // Check if debit was completed (needs refund) or pending (just cancel)
            let intent_row = sqlx::query_as::<_, (String, i64)>(
                "SELECT status, amount_paise FROM debit_intents WHERE id = ?"
            )
            .bind(intent_id)
            .fetch_optional(&state.db)
            .await?;

            if let Some((intent_status, amount_paise)) = intent_row {
                match intent_status.as_str() {
                    "completed" => {
                        // Create refund debit_intent (negative amount)
                        let refund_id = uuid::Uuid::new_v4().to_string();
                        sqlx::query(
                            "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id, status, origin, created_at, updated_at)
                             VALUES (?, ?, ?, ?, 'pending', 'local', datetime('now'), datetime('now'))"
                        )
                        .bind(&refund_id)
                        .bind(driver_id)
                        .bind(-amount_paise)  // negative = refund
                        .bind(res_id)
                        .execute(&state.db)
                        .await?;
                        tracing::info!("[scheduler] Created refund intent {} for expired reservation {} ({}p)", refund_id, res_id, amount_paise);
                    }
                    "pending" | "processing" => {
                        // Cancel the pending debit intent
                        sqlx::query(
                            "UPDATE debit_intents SET status = 'cancelled', updated_at = datetime('now') WHERE id = ?"
                        )
                        .bind(intent_id)
                        .execute(&state.db)
                        .await?;
                        tracing::info!("[scheduler] Cancelled debit intent {} for expired reservation {}", intent_id, res_id);
                    }
                    _ => {} // failed/cancelled — nothing to do
                }
            }
        }
    }

    if !expired.is_empty() {
        tracing::info!("[scheduler] Expired {} reservation(s)", expired.len());
    }
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn get_setting(state: &AppState, key: &str) -> Option<String> {
    // Check scheduler-specific settings first, then kiosk_settings
    sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .or_else(|| {
            // Fallback: try kiosk_settings (blocking, but these are fast lookups)
            None
        })
}

/// Returns signed minutes between two times (positive if `to` is after `from`).
fn minutes_between(from: NaiveTime, to: NaiveTime) -> i64 {
    let from_mins = from.hour() as i64 * 60 + from.minute() as i64;
    let to_mins = to.hour() as i64 * 60 + to.minute() as i64;
    to_mins - from_mins
}

async fn wake_all_pods(state: &AppState) {
    let rows = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, config_json FROM pods",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (pod_id, config_json) in rows {
        let mac = config_json.and_then(|json| {
            serde_json::from_str::<serde_json::Value>(&json).ok()
                .and_then(|v| v["mac_address"].as_str().map(|s| s.to_string()))
        });

        if let Some(mac) = mac {
            let _ = wol::send_wol(&mac).await;
            tracing::info!("[scheduler] Wake-all: WoL sent to pod {} ({})", pod_id, mac);
        }
    }
}

// ─── API handlers ─────────────────────────────────────────────────────────────

use axum::{Json, extract::State};
use serde_json::{json, Value};

/// GET /api/v1/scheduler/status — current schedule state
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let now = Local::now();
    let time_now = now.time();

    let open = get_setting(&state, "business_hours_start").await.unwrap_or_else(|| "10:00".into());
    let close = get_setting(&state, "business_hours_end").await.unwrap_or_else(|| "22:00".into());
    let enabled = get_setting(&state, "scheduler_enabled").await.unwrap_or_else(|| "true".into());
    let pre_wake = get_setting(&state, "scheduler_pre_wake_minutes").await.unwrap_or_else(|| "15".into());
    let pre_open = get_setting(&state, "scheduler_pre_open_minutes").await.unwrap_or_else(|| "10".into());
    let post_close = get_setting(&state, "scheduler_post_close_minutes").await.unwrap_or_else(|| "15".into());

    let open_time = NaiveTime::parse_from_str(&open, "%H:%M").unwrap_or(NaiveTime::from_hms_opt(10, 0, 0).unwrap());
    let close_time = NaiveTime::parse_from_str(&close, "%H:%M").unwrap_or(NaiveTime::from_hms_opt(22, 0, 0).unwrap());

    let is_open = time_now >= open_time && time_now < close_time;

    let active_sessions = {
        let timers = state.billing.active_timers.read().await;
        timers.len()
    };

    // Upcoming bookings in next hour
    let upcoming = sqlx::query_as::<_, (String, String, String)>(
        "SELECT b.id, b.pod_id, b.start_time FROM bookings b
         WHERE b.status = 'confirmed'
           AND datetime(b.start_time) > datetime('now')
           AND datetime(b.start_time) <= datetime('now', '+1 hour')
         ORDER BY b.start_time ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Recent scheduler events
    let recent_events = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT id, event_type, details, created_at FROM scheduler_events
         ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "enabled": enabled == "true",
        "current_time": now.format("%H:%M").to_string(),
        "is_open": is_open,
        "business_hours": { "open": open, "close": close },
        "settings": {
            "pre_wake_minutes": pre_wake,
            "pre_open_minutes": pre_open,
            "post_close_minutes": post_close,
        },
        "active_sessions": active_sessions,
        "upcoming_bookings": upcoming.iter().map(|(id, pod, start)| json!({
            "id": id, "pod_id": pod, "start_time": start
        })).collect::<Vec<_>>(),
        "recent_events": recent_events.iter().map(|(id, etype, details, created)| json!({
            "id": id, "event_type": etype, "details": details, "created_at": created
        })).collect::<Vec<_>>(),
    }))
}

/// PUT /api/v1/scheduler/settings — update scheduler settings
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let allowed_keys = [
        "scheduler_enabled",
        "scheduler_pre_wake_minutes",
        "scheduler_pre_open_minutes",
        "scheduler_post_close_minutes",
        "business_hours_start",
        "business_hours_end",
    ];

    let mut updated = Vec::new();

    if let Some(obj) = body.as_object() {
        for (key, value) in obj {
            if allowed_keys.contains(&key.as_str()) {
                let val_str = match value {
                    Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };

                // business_hours go to kiosk_settings, rest go to settings
                if key.starts_with("business_hours") {
                    let _ = sqlx::query(
                        "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    )
                    .bind(key)
                    .bind(&val_str)
                    .execute(&state.db)
                    .await;
                } else {
                    let _ = sqlx::query(
                        "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
                    )
                    .bind(key)
                    .bind(&val_str)
                    .execute(&state.db)
                    .await;
                }

                updated.push(key.clone());
            }
        }
    }

    Json(json!({ "updated": updated }))
}

/// GET /api/v1/scheduler/analytics — peak hour analytics from hourly snapshots
pub async fn get_analytics(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Aggregate hourly snapshots from last 30 days
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT details FROM scheduler_events
         WHERE event_type = 'hourly_snapshot'
           AND created_at >= datetime('now', '-30 days')
         ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Parse and aggregate by hour + day_of_week
    let mut hour_totals: std::collections::HashMap<(u32, u32), (i64, i64)> = std::collections::HashMap::new();

    for (details,) in &rows {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(details) {
            let hour = v["hour"].as_u64().unwrap_or(0) as u32;
            let dow = v["day_of_week"].as_u64().unwrap_or(0) as u32;
            let sessions = v["active_sessions"].as_i64().unwrap_or(0);

            let entry = hour_totals.entry((hour, dow)).or_insert((0, 0));
            entry.0 += sessions;
            entry.1 += 1;
        }
    }

    // Build output: average sessions per hour per day
    let mut heatmap: Vec<Value> = hour_totals.iter().map(|((hour, dow), (total, count))| {
        let avg = if *count > 0 { *total as f64 / *count as f64 } else { 0.0 };
        let day_name = match dow {
            0 => "Mon", 1 => "Tue", 2 => "Wed", 3 => "Thu",
            4 => "Fri", 5 => "Sat", 6 => "Sun", _ => "?",
        };
        json!({
            "hour": hour,
            "day_of_week": dow,
            "day_name": day_name,
            "avg_sessions": (avg * 10.0).round() / 10.0,
            "sample_count": count,
        })
    }).collect();

    heatmap.sort_by(|a, b| {
        let da = a["day_of_week"].as_u64().unwrap_or(0);
        let db = b["day_of_week"].as_u64().unwrap_or(0);
        let ha = a["hour"].as_u64().unwrap_or(0);
        let hb = b["hour"].as_u64().unwrap_or(0);
        (da, ha).cmp(&(db, hb))
    });

    // Identify peak/off-peak from overall hourly averages
    let mut hourly_avg: std::collections::HashMap<u32, (f64, i64)> = std::collections::HashMap::new();
    for ((hour, _), (total, count)) in &hour_totals {
        let entry = hourly_avg.entry(*hour).or_insert((0.0, 0));
        entry.0 += *total as f64;
        entry.1 += count;
    }

    let mut peak_hours = Vec::new();
    let mut off_peak_hours = Vec::new();
    let overall_avg: f64 = if hourly_avg.is_empty() {
        0.0
    } else {
        hourly_avg.values().map(|(t, c)| t / *c as f64).sum::<f64>() / hourly_avg.len() as f64
    };

    for (hour, (total, count)) in &hourly_avg {
        let avg = total / *count as f64;
        if avg > overall_avg * 1.3 {
            peak_hours.push(*hour);
        } else if avg < overall_avg * 0.5 {
            off_peak_hours.push(*hour);
        }
    }
    peak_hours.sort();
    off_peak_hours.sort();

    Json(json!({
        "period": "last_30_days",
        "total_snapshots": rows.len(),
        "heatmap": heatmap,
        "peak_hours": peak_hours,
        "off_peak_hours": off_peak_hours,
        "overall_avg_sessions": (overall_avg * 10.0).round() / 10.0,
        "pricing_suggestion": if !peak_hours.is_empty() {
            format!("Consider premium pricing during peak hours ({}) and discounts during off-peak ({})",
                peak_hours.iter().map(|h| format!("{}:00", h)).collect::<Vec<_>>().join(", "),
                off_peak_hours.iter().map(|h| format!("{}:00", h)).collect::<Vec<_>>().join(", "))
        } else {
            "Not enough data yet. Analytics will populate after a few days of operation.".into()
        },
    }))
}
