//! Request and response types for the race weekend module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::WeekendPhase;

/// Request body for POST /api/v1/games/weekend
#[derive(Debug, Deserialize)]
pub struct CreateWeekendRequest {
    pub pod_ids: Vec<String>,
    pub track: String,
    pub car_class: String,
    #[serde(default = "default_practice_minutes")]
    pub practice_minutes: u32,
    #[serde(default = "default_quali_minutes")]
    pub quali_minutes: u32,
    #[serde(default = "default_race_laps")]
    pub race_laps: u32,
}

fn default_practice_minutes() -> u32 { 10 }
fn default_quali_minutes() -> u32 { 10 }
fn default_race_laps() -> u32 { 10 }

/// Summary returned to callers after weekend creation.
#[derive(Debug, Serialize)]
pub struct WeekendSummary {
    pub weekend_id: String,
    pub ac_session_id: String,
    pub phase: WeekendPhase,
    pub track: String,
    pub car_class: String,
    pub pod_ids: Vec<String>,
    pub practice_minutes: u32,
    pub quali_minutes: u32,
    pub race_laps: u32,
}

/// Status snapshot returned by the status endpoint.
#[derive(Debug, Serialize)]
pub struct WeekendStatus {
    pub weekend_id: String,
    pub current_session: WeekendPhase,
    pub track: String,
    pub car_class: String,
    pub connected_pods: Vec<String>,
    pub total_pods: usize,
    pub practice_minutes: u32,
    pub quali_minutes: u32,
    pub race_laps: u32,
    pub phase_changed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
