//! v29.0 Phase 30: Business alert engine — monitors financial KPIs and triggers alerts.

use serde::Serialize;
use chrono::{Utc, Datelike, Timelike};

const LOG_TARGET: &str = "alert-engine";

#[derive(Debug, Clone, Serialize)]
pub enum AlertChannel { WhatsApp, Dashboard, Both }

#[derive(Debug, Clone, Serialize)]
pub struct BusinessAlert {
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub channel: AlertChannel,
    pub timestamp: String,
    pub value: f64,
    pub threshold: f64,
}

/// Check business KPIs and generate alerts
pub async fn check_business_alerts(pool: &sqlx::SqlitePool) -> Vec<BusinessAlert> {
    let mut alerts = Vec::new();
    let today = Utc::now().date_naive().to_string();

    // Check: Revenue today vs 7-day average (>30% drop = alert)
    // MMA-R1: DB columns are INTEGER — query as i64, not f64 (was runtime type mismatch)
    let today_rev: i64 = sqlx::query_scalar(
        "SELECT COALESCE(revenue_gaming_paise + revenue_cafe_paise, 0) FROM daily_business_metrics WHERE date = ?1"
    ).bind(&today).fetch_one(pool).await.unwrap_or(0_i64);

    let avg_rev: i64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(revenue_gaming_paise + revenue_cafe_paise), 0) FROM daily_business_metrics WHERE date >= date('now', '-7 days')"
    ).fetch_one(pool).await.unwrap_or(0_i64);

    if avg_rev > 0 && today_rev * 10 < avg_rev * 7 {
        // MMA-v29: Use f64 for percentage display to avoid integer truncation (33% vs 33.3%)
        let drop_pct = if avg_rev > 0 { ((avg_rev - today_rev) as f64 * 100.0 / avg_rev as f64).round() as i64 } else { 0 };
        alerts.push(BusinessAlert {
            alert_type: "RevenueDropAlert".into(),
            severity: "High".into(),
            message: format!("Revenue today ₹{:.0} is {}% below 7-day average ₹{:.0}", today_rev as f64 / 100.0, drop_pct, avg_rev as f64 / 100.0),
            channel: AlertChannel::Both,
            timestamp: Utc::now().to_rfc3339(),
            value: today_rev as f64, threshold: avg_rev as f64 * 0.7,
        });
    }

    // Check: Occupancy below 40% when venue is open
    // Replaced hardcoded peak hours (16-22 IST) with ping-based venue state.
    // Rule: "If server or James is on, venue is open."
    let is_venue_open = crate::venue_state::venue_is_open();
    if is_venue_open {
        // MMA-R1: occupancy_rate_pct is REAL in DB — f64 is correct here
        let occ: f64 = sqlx::query_scalar(
            "SELECT COALESCE(occupancy_rate_pct, 0.0) FROM daily_business_metrics WHERE date = ?1"
        ).bind(&today).fetch_one(pool).await.unwrap_or(0.0);

        if occ > 0.0 && occ < 40.0 {
            alerts.push(BusinessAlert {
                alert_type: "LowOccupancyAlert".into(),
                severity: "Medium".into(),
                message: format!("Peak hour occupancy {:.0}% (threshold: 40%)", occ),
                channel: AlertChannel::Dashboard,
                timestamp: Utc::now().to_rfc3339(),
                value: occ, threshold: 40.0,
            });
        }
    }

    // Check: Maintenance costs exceeding 20% of revenue this month
    let month_start = format!("{}-{:02}-01", Utc::now().year(), Utc::now().month());
    // MMA-R1: DB columns are INTEGER — query as i64, not f64
    let maint_cost: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(expense_maintenance_paise), 0) FROM daily_business_metrics WHERE date >= ?1"
    ).bind(&month_start).fetch_one(pool).await.unwrap_or(0_i64);

    let month_rev: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(revenue_gaming_paise + revenue_cafe_paise), 0) FROM daily_business_metrics WHERE date >= ?1"
    ).bind(&month_start).fetch_one(pool).await.unwrap_or(0_i64);

    if month_rev > 0 && maint_cost * 5 > month_rev {
        // MMA-v29: Use f64 for percentage display to avoid integer truncation
        let cost_pct = if month_rev > 0 { (maint_cost as f64 * 100.0 / month_rev as f64).round() as i64 } else { 0 };
        alerts.push(BusinessAlert {
            alert_type: "MaintenanceCostAlert".into(),
            severity: "High".into(),
            message: format!("Maintenance costs ₹{:.0} are {}% of revenue ₹{:.0}", maint_cost as f64 / 100.0, cost_pct, month_rev as f64 / 100.0),
            channel: AlertChannel::WhatsApp,
            timestamp: Utc::now().to_rfc3339(),
            value: maint_cost as f64, threshold: month_rev as f64 * 0.2,
        });
    }

    for alert in &alerts {
        tracing::warn!(target: LOG_TARGET, alert_type = %alert.alert_type, severity = %alert.severity, "Business alert: {}", alert.message);
    }

    alerts
}

/// Spawn periodic business alert checker (every 30 min).
/// Accepts AppState to dispatch alerts to WhatsApp and dashboard channels.
pub fn spawn_alert_checker(state: std::sync::Arc<crate::state::AppState>) {
    let pool = state.db.clone();
    let config = state.config.clone();
    tokio::spawn(async move {
        // Wait 5 min for system to stabilize
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let alerts = check_business_alerts(&pool).await;
            for alert in &alerts {
                // Dispatch to WhatsApp for WhatsApp/Both channels
                if matches!(alert.channel, AlertChannel::WhatsApp | AlertChannel::Both) {
                    let msg = format!("[{}] {}: {}", alert.severity, alert.alert_type, alert.message);
                    crate::whatsapp_alerter::send_whatsapp(&config, &msg).await;
                }
                // Dashboard channel alerts are logged for now (dashboard WS picks them up via tracing)
                if matches!(alert.channel, AlertChannel::Dashboard | AlertChannel::Both) {
                    tracing::info!(target: LOG_TARGET,
                        alert_type = %alert.alert_type,
                        severity = %alert.severity,
                        channel = "dashboard",
                        "Dashboard alert: {}", alert.message
                    );
                }
            }
        }
    });
}
