//! v29.0 Phase 18: Demand forecasting using historical occupancy patterns.
//! Simple approach: day-of-week + hour-of-day average from last 30 days.

use chrono::Datelike;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct DemandForecast {
    pub date: String,
    pub day_of_week: String,
    pub predicted_occupancy_pct: f32,
    pub predicted_sessions: u32,
    pub confidence: f32,
    pub basis: String, // "30-day historical average"
}

#[derive(Debug, Clone, Serialize)]
pub struct HourlyDemand {
    pub hour: u8,
    pub avg_occupancy_pct: f32,
    pub avg_sessions: f32,
}

/// Generate a 7-day demand forecast based on historical day-of-week patterns
pub async fn forecast_week(pool: &SqlitePool) -> anyhow::Result<Vec<DemandForecast>> {
    let today = chrono::Utc::now().date_naive();
    let mut forecasts = Vec::new();

    for day_offset in 0..7 {
        let target_date = today + chrono::Duration::days(day_offset);
        let dow = target_date.weekday();
        let dow_str = format!("{:?}", dow);

        // Query average metrics for this day of week from last 30 days
        let row: Option<(f64, f64)> = sqlx::query_as(
            "SELECT COALESCE(AVG(occupancy_rate_pct), 0), COALESCE(AVG(sessions_count), 0) \
             FROM daily_business_metrics \
             WHERE date >= date('now', '-30 days') \
             AND CAST(strftime('%w', date) AS INTEGER) = ?1",
        )
        .bind(dow.num_days_from_sunday() as i32)
        .fetch_optional(pool)
        .await?;

        let (avg_occ, avg_sessions) = row.unwrap_or((0.0, 0.0));

        forecasts.push(DemandForecast {
            date: target_date.to_string(),
            day_of_week: dow_str,
            predicted_occupancy_pct: avg_occ as f32,
            predicted_sessions: avg_sessions as u32,
            confidence: if avg_occ > 0.0 { 0.6 } else { 0.1 }, // low confidence if no data
            basis: "30-day day-of-week average".into(),
        });
    }

    Ok(forecasts)
}
