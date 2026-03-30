//! v29.0 Phase 26: Auto-aggregate revenue from billing_fsm, cafe, wallet into daily_business_metrics.
//! Runs hourly to keep the EBITDA dashboard populated with real data.

use chrono::{NaiveDate, Utc};
use sqlx::SqlitePool;

use crate::maintenance_models::DailyBusinessMetrics;
use crate::maintenance_store;

const LOG_TARGET: &str = "biz-aggregator";

/// Aggregate today's revenue from billing sessions, cafe orders, and wallet transactions.
pub async fn aggregate_daily_revenue(pool: &SqlitePool, date: NaiveDate) -> anyhow::Result<()> {
    let date_str = date.format("%Y-%m-%d").to_string();

    // MMA-R1: Propagate DB errors instead of swallowing with unwrap_or(0)
    // Gaming revenue: sum of wallet_debit_paise from completed/ended_early billing sessions today
    let gaming: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(wallet_debit_paise), 0) FROM billing_sessions \
         WHERE DATE(ended_at) = ?1 AND status IN ('completed', 'ended_early')",
    )
    .bind(&date_str)
    .fetch_one(pool)
    .await?;

    // Cafe revenue: sum from confirmed cafe orders
    let cafe: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_paise), 0) FROM cafe_orders \
         WHERE DATE(created_at) = ?1 AND status = 'confirmed'",
    )
    .bind(&date_str)
    .fetch_one(pool)
    .await?;

    // Session count (completed + ended_early)
    let sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions \
         WHERE DATE(ended_at) = ?1 AND status IN ('completed', 'ended_early')",
    )
    .bind(&date_str)
    .fetch_one(pool)
    .await?;

    // Occupancy: approximate — sessions per pod per operating hour window (12h day)
    let total_pods = 8.0f32;
    let operating_hours = 12.0f32;
    let occupancy = if sessions > 0 {
        (sessions as f32 / (total_pods * operating_hours) * 100.0).min(100.0)
    } else {
        0.0
    };

    // Build a DailyBusinessMetrics struct, preserving any existing expense data
    let existing = maintenance_store::query_business_metrics(pool, &date_str, &date_str).await;
    let base = existing
        .ok()
        .and_then(|v| v.into_iter().next())
        .unwrap_or(DailyBusinessMetrics {
            date,
            revenue_gaming_paise: 0,
            revenue_cafe_paise: 0,
            revenue_other_paise: 0,
            expense_rent_paise: 0,
            expense_utilities_paise: 0,
            expense_salaries_paise: 0,
            expense_maintenance_paise: 0,
            expense_other_paise: 0,
            sessions_count: 0,
            occupancy_rate_pct: 0.0,
            peak_occupancy_pct: 0.0,
        });

    let metrics = DailyBusinessMetrics {
        date,
        revenue_gaming_paise: gaming,
        revenue_cafe_paise: cafe,
        revenue_other_paise: base.revenue_other_paise,
        expense_rent_paise: base.expense_rent_paise,
        expense_utilities_paise: base.expense_utilities_paise,
        expense_salaries_paise: base.expense_salaries_paise,
        expense_maintenance_paise: base.expense_maintenance_paise,
        expense_other_paise: base.expense_other_paise,
        // MMA-R1: Use try_from instead of `as` for safe narrowing
        sessions_count: u32::try_from(sessions.max(0)).unwrap_or(0),
        occupancy_rate_pct: occupancy,
        peak_occupancy_pct: occupancy.max(base.peak_occupancy_pct),
    };

    maintenance_store::upsert_daily_metrics(pool, &date_str, &metrics).await?;

    tracing::info!(
        target: LOG_TARGET,
        date = %date_str,
        gaming,
        cafe,
        sessions,
        "Daily revenue aggregated"
    );
    Ok(())
}

/// Spawn hourly aggregation task.
pub fn spawn_business_aggregator(pool: SqlitePool) {
    tokio::spawn(async move {
        tracing::info!(target: LOG_TARGET, "Business aggregator started (1h interval)");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let today = Utc::now().date_naive();
            if let Err(e) = aggregate_daily_revenue(&pool, today).await {
                tracing::warn!(target: LOG_TARGET, error = %e, "Revenue aggregation failed");
            }
        }
    });
}
