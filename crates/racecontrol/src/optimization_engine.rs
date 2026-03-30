//! v29.0 Phase 31: Optimization engine — balances staffing, pricing, maintenance, and revenue.

use serde::Serialize;
use chrono::{NaiveDate, Datelike};

const _LOG_TARGET: &str = "optimizer";

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationRecommendation {
    pub category: String,      // "staffing", "maintenance_window", "pod_rotation", "pricing"
    pub recommendation: String,
    pub impact_description: String,
    pub confidence: f32,
    pub requires_approval: bool,
}

/// Recommend optimal maintenance window for a pod
pub fn recommend_maintenance_window(
    pod_id: u8,
    estimated_duration_minutes: u32,
    forecast_occupancy: &[(u8, f32)], // (hour, occupancy_pct)
) -> OptimizationRecommendation {
    // Find the hour with lowest occupancy
    let best_hour = forecast_occupancy.iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(h, o)| (*h, *o))
        .unwrap_or((3, 0.0)); // default: 3 AM IST

    OptimizationRecommendation {
        category: "maintenance_window".into(),
        recommendation: format!("Schedule Pod {} maintenance at {:02}:00 IST ({} min)", pod_id, best_hour.0, estimated_duration_minutes),
        impact_description: format!("Lowest forecasted occupancy: {:.0}% at {:02}:00", best_hour.1, best_hour.0),
        confidence: 0.7,
        requires_approval: true,
    }
}

/// Recommend staffing levels based on forecast
pub fn recommend_staffing(
    date: NaiveDate,
    forecast_sessions: u32,
    current_staff_count: u32,
) -> OptimizationRecommendation {
    // Rule: 1 technician per 4 pods active, minimum 1
    let recommended = ((forecast_sessions as f32 / 4.0).ceil() as u32).max(1);
    let weekday = date.weekday();
    let is_weekend = matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun);

    let recommendation = if recommended > current_staff_count {
        format!("Increase staff to {} for {} ({}{})", recommended, date, weekday, if is_weekend { " - WEEKEND" } else { "" })
    } else if recommended < current_staff_count {
        format!("Reduce staff to {} for {} (low demand expected)", recommended, date)
    } else {
        format!("Current staff level ({}) is optimal for {}", current_staff_count, date)
    };

    OptimizationRecommendation {
        category: "staffing".into(),
        recommendation,
        impact_description: format!("Forecast: {} sessions, {} staff needed", forecast_sessions, recommended),
        confidence: 0.6,
        requires_approval: true,
    }
}

/// Recommend pod rotation to balance wear
pub fn recommend_pod_rotation(pod_usage_hours: &[(u8, f64)]) -> Vec<OptimizationRecommendation> {
    let mut recs = Vec::new();
    if pod_usage_hours.len() < 2 { return recs; }

    let avg_hours: f64 = pod_usage_hours.iter().map(|(_, h)| h).sum::<f64>() / pod_usage_hours.len() as f64;

    for (pod_id, hours) in pod_usage_hours {
        if *hours > avg_hours * 1.3 {
            recs.push(OptimizationRecommendation {
                category: "pod_rotation".into(),
                recommendation: format!("Pod {} has {:.0}% above average usage — prioritize for next sessions on other pods", pod_id, (hours/avg_hours - 1.0) * 100.0),
                impact_description: format!("Pod {}: {:.0}h vs fleet avg {:.0}h", pod_id, hours, avg_hours),
                confidence: 0.5,
                requires_approval: false,
            });
        }
    }
    recs
}
