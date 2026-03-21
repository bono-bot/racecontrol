//! Psychology Engine — centralized badge evaluation, streak tracking,
//! notification budget enforcement, and multi-channel dispatch.
//!
//! Phase 1 Foundation: types, enums, JSON criteria parsing, function stubs.
//! Plans 02 and 03 fill in the logic and wiring.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::state::AppState;

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

// ─── Public API stubs (implemented in Plan 02) ───────────────────────────────

/// Evaluate all badge criteria for a driver after a lap/session event.
/// Loads badge definitions from DB, checks each against driver stats,
/// awards new badges, skips already-earned ones.
pub async fn evaluate_badges(_state: &Arc<AppState>, _driver_id: &str) {
    // Implemented in Plan 02
}

/// Check and update streak for a driver after a session.
/// Compares last_visit_date (IST) with today, increments or resets.
pub async fn update_streak(_state: &Arc<AppState>, _driver_id: &str) {
    // Implemented in Plan 02
}

/// Queue a notification through the priority system.
/// Inserts into nudge_queue with status='pending'.
/// The background dispatcher picks it up.
pub async fn queue_notification(
    _state: &Arc<AppState>,
    _driver_id: &str,
    _channel: NotificationChannel,
    _priority: i32,
    _template: &str,
    _payload_json: &str,
) {
    // Implemented in Plan 02
}

/// Check if sending a WhatsApp message to this driver would exceed the daily budget.
/// Returns true if the driver has already received >= WHATSAPP_DAILY_BUDGET proactive messages today.
pub async fn is_whatsapp_budget_exceeded(_state: &Arc<AppState>, _driver_id: &str) -> bool {
    // Implemented in Plan 02
    false
}

/// Spawn the background notification dispatcher task.
/// Runs every DISPATCHER_INTERVAL_SECS, drains nudge_queue, routes to channels.
pub fn spawn_dispatcher(_state: Arc<AppState>) {
    // Implemented in Plan 02
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
}
