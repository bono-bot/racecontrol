use std::sync::Arc;
use axum::{Json, extract::State};
use serde_json::{json, Value};
use crate::state::AppState;

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
