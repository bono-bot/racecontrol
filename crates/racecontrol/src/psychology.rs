//! Psychology Engine — centralized badge evaluation, streak tracking,
//! notification budget enforcement, and multi-channel dispatch.
//!
//! Core types and badge/streak logic live here; notification dispatch,
//! channel routing, driving passport, and retention loops are in the
//! `nudge` submodule.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::state::AppState;

// ─── Submodules ──────────────────────────────────────────────────────────────

#[path = "psychology_nudge.rs"]
mod nudge;

// Re-export everything the rest of the crate uses from the nudge submodule.
pub use nudge::{
    is_whatsapp_budget_exceeded, queue_notification, spawn_dispatcher,
    update_driving_passport, backfill_driving_passport,
    notify_pb_beaten_holders, maybe_grant_variable_reward,
    check_streak_at_risk, check_membership_expiry_warnings,
};

// Test-only re-exports: these are pub(super) in nudge but tests need direct access.
#[cfg(test)]
use nudge::{drain_notification_queue, resolve_template};

#[cfg(test)]
#[path = "psychology_tests.rs"]
mod tests;

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
