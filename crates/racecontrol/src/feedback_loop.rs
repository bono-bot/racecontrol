//! v29.0 Phase 28: Feedback loop — tracks prediction accuracy, maintenance outcomes,
//! and feeds results back to improve future predictions.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

const LOG_TARGET: &str = "feedback-loop";

#[derive(Debug, Clone, Serialize)]
pub struct PredictionOutcome {
    pub prediction_id: String,
    pub prediction_type: String, // "anomaly", "rul", "demand"
    pub predicted_at: DateTime<Utc>,
    pub predicted_value: f64,
    pub actual_value: Option<f64>,
    pub was_accurate: Option<bool>,
    pub lead_time_hours: Option<f64>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackMetrics {
    pub total_predictions: u32,
    pub accurate_predictions: u32,
    pub precision: f64,
    pub recall: f64,
    pub mean_lead_time_hours: f64,
    pub false_positive_rate: f64,
    pub period_days: u32,
}

/// Initialize feedback tables.
pub async fn init_feedback_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS prediction_outcomes (
            id TEXT PRIMARY KEY,
            prediction_type TEXT NOT NULL,
            predicted_at TEXT NOT NULL,
            predicted_value REAL,
            actual_value REAL,
            was_accurate INTEGER,
            lead_time_hours REAL,
            notes TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS admin_overrides (
            id TEXT PRIMARY KEY,
            recommendation_id TEXT,
            recommendation_type TEXT NOT NULL,
            original_action TEXT NOT NULL,
            final_action TEXT NOT NULL,
            overridden_by TEXT NOT NULL,
            reason TEXT DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_outcomes_type ON prediction_outcomes(prediction_type)")
        .execute(pool)
        .await?;

    tracing::info!(target: LOG_TARGET, "Feedback tables initialized");
    Ok(())
}

/// Record a prediction outcome.
pub async fn record_outcome(pool: &SqlitePool, outcome: &PredictionOutcome) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO prediction_outcomes \
         (id, prediction_type, predicted_at, predicted_value, actual_value, was_accurate, lead_time_hours, notes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(&outcome.prediction_id)
    .bind(&outcome.prediction_type)
    .bind(outcome.predicted_at.to_rfc3339())
    .bind(outcome.predicted_value)
    .bind(outcome.actual_value)
    .bind(outcome.was_accurate.map(|b| b as i32))
    .bind(outcome.lead_time_hours)
    .bind(&outcome.notes)
    .execute(pool)
    .await?;
    Ok(())
}

/// Record admin override for feedback learning.
pub async fn record_override(
    pool: &SqlitePool,
    recommendation_id: &str,
    rec_type: &str,
    original: &str,
    final_action: &str,
    overridden_by: &str,
    reason: &str,
) -> anyhow::Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO admin_overrides \
         (id, recommendation_id, recommendation_type, original_action, final_action, overridden_by, reason) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&id)
    .bind(recommendation_id)
    .bind(rec_type)
    .bind(original)
    .bind(final_action)
    .bind(overridden_by)
    .bind(reason)
    .execute(pool)
    .await?;
    tracing::info!(target: LOG_TARGET, rec_type, "Admin override recorded");
    Ok(())
}

/// Calculate feedback metrics for a period.
pub async fn calculate_feedback_metrics(
    pool: &SqlitePool,
    days: u32,
) -> anyhow::Result<FeedbackMetrics> {
    let since = (Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1",
    )
    .bind(&since)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let accurate: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate = 1",
    )
    .bind(&since)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let false_pos: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM prediction_outcomes WHERE created_at > ?1 AND was_accurate = 0",
    )
    .bind(&since)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let avg_lead: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(lead_time_hours), 0) FROM prediction_outcomes \
         WHERE created_at > ?1 AND was_accurate = 1",
    )
    .bind(&since)
    .fetch_one(pool)
    .await
    .unwrap_or(0.0);

    let precision = if total > 0 {
        accurate as f64 / total as f64
    } else {
        0.0
    };
    let fpr = if total > 0 {
        false_pos as f64 / total as f64
    } else {
        0.0
    };

    Ok(FeedbackMetrics {
        total_predictions: total as u32,
        accurate_predictions: accurate as u32,
        precision,
        recall: precision, // simplified — need separate tracking of missed failures
        mean_lead_time_hours: avg_lead,
        false_positive_rate: fpr,
        period_days: days,
    })
}
