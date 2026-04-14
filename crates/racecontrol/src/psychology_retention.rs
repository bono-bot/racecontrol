//! Psychology retention subsystem — driving passport, PB-beaten notifications,
//! variable rewards, streak-at-risk nudges, and membership expiry warnings.

use std::sync::Arc;
use crate::state::AppState;
use rand::Rng;
use super::{NotificationChannel};
use super::nudge::queue_notification;

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
