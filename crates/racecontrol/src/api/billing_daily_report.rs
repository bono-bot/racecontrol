//! Daily billing report handler — extracted from billing_invoice.rs.

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DailyReportQuery {
    date: Option<String>,
}

pub(crate) async fn daily_billing_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyReportQuery>,
) -> Json<Value> {
    let date = params
        .date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.staff_id, sm.name,
                bs.discount_paise, bs.original_price_paise, bs.discount_reason
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN staff_members sm ON bs.staff_id = sm.id
         WHERE date(bs.started_at) = ?
         ORDER BY bs.started_at ASC",
    )
    .bind(&date)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let total_sessions = sessions.len();
            let total_revenue_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.8)
                .sum();
            let total_driving_seconds: i64 = sessions.iter().map(|s| s.6).sum();
            let total_discount_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.13.unwrap_or(0))
                .sum();

            // Build staff summary
            let mut staff_map: std::collections::HashMap<String, (String, usize, i64)> = std::collections::HashMap::new();
            for s in &sessions {
                if s.7 == "cancelled" { continue; }
                let staff_key = s.11.clone().unwrap_or_default();
                let staff_name = s.12.clone().unwrap_or_else(|| "Walk-in / Self".to_string());
                let entry = staff_map.entry(staff_key).or_insert((staff_name, 0, 0));
                entry.1 += 1;
                entry.2 += s.8;
            }
            let staff_summary: Vec<Value> = staff_map
                .into_iter()
                .map(|(id, (name, count, revenue))| {
                    json!({ "staff_id": id, "staff_name": name, "sessions": count, "revenue_paise": revenue })
                })
                .collect();

            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10,
                        "staff_id": s.11, "staff_name": s.12,
                        "discount_paise": s.13, "original_price_paise": s.14,
                        "discount_reason": s.15,
                    })
                })
                .collect();

            Json(json!({
                "date": date,
                "total_sessions": total_sessions,
                "total_revenue_paise": total_revenue_paise,
                "total_discount_paise": total_discount_paise,
                "total_driving_seconds": total_driving_seconds,
                "staff_summary": staff_summary,
                "sessions": list,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
