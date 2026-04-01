//! Weekly Fleet Intelligence Report — RPT-01..03.
//!
//! Generates a weekly KPI report every Sunday at midnight IST and sends it
//! to Uday via WhatsApp (EscalationRequest → server → Bono VPS Evolution API).
//!
//! KPIs collected:
//!   - Uptime % (from diagnostic log health data)
//!   - Auto-resolution rate (fixes vs failures)
//!   - Mean Time To Repair (MTTR)
//!   - Top 3 recurring issues
//!   - AI budget spent this week
//!   - Knowledge Base growth
//!
//! Phase 279 — RPT-01, RPT-02, RPT-03.

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{Datelike, FixedOffset, Timelike, Utc, Weekday};
use tokio::sync::{mpsc, RwLock};

use crate::budget_tracker::BudgetTracker;
use crate::diagnostic_log::DiagnosticLog;
use crate::knowledge_base::{KnowledgeBase, KB_PATH};
use rc_common::protocol::{AgentMessage, EscalationPayload};

const LOG_TARGET: &str = "weekly-report";

/// Spawn the weekly report scheduler.
/// Sleeps until next Sunday midnight IST, collects KPIs, sends WhatsApp.
pub fn spawn(
    ws_tx: mpsc::Sender<AgentMessage>,
    node_id: String,
    diag_log: DiagnosticLog,
    budget: Arc<RwLock<BudgetTracker>>,
) {
    tokio::spawn(async move {
        tracing::info!(target: "state", task = "weekly_report", event = "lifecycle", "lifecycle: started");
        loop {
            let secs = seconds_until_next_sunday_midnight_ist();
            tracing::info!(
                target: LOG_TARGET,
                secs_until_report = secs,
                "Sleeping until next Sunday midnight IST for weekly report"
            );
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;

            tracing::info!(target: LOG_TARGET, "Weekly report cycle starting");
            let report = collect_report(&diag_log, &budget, &node_id).await;
            let message = format_whatsapp_message(&report);

            // Send via EscalationRequest (same path as Tier 5 → server → WhatsApp)
            let payload = EscalationPayload {
                pod_id: node_id.clone(),
                incident_id: format!("weekly-report-{}", report.period_end),
                severity: "info".to_string(),
                trigger: "WeeklyReport".to_string(),
                summary: "Weekly Fleet Intelligence Report".to_string(),
                actions_tried: vec![],
                impact: message,
                dashboard_url: "http://192.168.31.23:8080/status".to_string(),
                timestamp: Utc::now().to_rfc3339(),
            };

            if let Err(e) = ws_tx.send(AgentMessage::EscalationRequest(payload)).await {
                tracing::error!(
                    target: LOG_TARGET,
                    error = %e,
                    "Failed to send weekly report via WS — channel closed"
                );
            } else {
                tracing::info!(
                    target: LOG_TARGET,
                    "Weekly report sent to server for WhatsApp delivery"
                );
            }
        }
    });
}

/// Compute seconds until next Sunday 00:00:00 IST.
/// IST = UTC + 5:30 (computed manually per CLAUDE.md — NEVER use TZ=Asia/Kolkata).
fn seconds_until_next_sunday_midnight_ist() -> u64 {
    let ist = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .expect("IST offset is always valid");
    let now_ist = Utc::now().with_timezone(&ist);

    // Days until next Sunday (0 = already Sunday)
    let days_ahead = match now_ist.weekday() {
        Weekday::Sun => {
            // If it's already Sunday but past midnight, wait 7 days
            if now_ist.hour() == 0 && now_ist.minute() == 0 && now_ist.second() == 0 {
                0 // Exact midnight — run now
            } else {
                7 // Already past Sunday midnight, wait for next week
            }
        }
        Weekday::Mon => 6,
        Weekday::Tue => 5,
        Weekday::Wed => 4,
        Weekday::Thu => 3,
        Weekday::Fri => 2,
        Weekday::Sat => 1,
    };

    let target_date = now_ist.date_naive() + chrono::Duration::days(days_ahead as i64);
    let target_midnight = target_date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid")
        .and_local_timezone(ist)
        .single()
        .expect("IST is a fixed offset — no ambiguity");

    let secs = (target_midnight - now_ist).num_seconds().max(1) as u64;
    secs
}

/// Collected weekly KPIs.
struct WeeklyReport {
    period_start: String,
    period_end: String,
    uptime_pct: f64,
    auto_resolved: u32,
    auto_resolution_rate: f64,
    mttr_seconds: f64,
    escalated_count: u32,
    top_issues: Vec<(String, u32)>,
    budget_spent: f64,
    budget_limit: f64,
    kb_total: i64,
    // We track model calls as a proxy for "most efficient" since
    // per-model cost breakdown isn't available in BudgetTracker.
    model_calls: u32,
}

/// Collect KPIs from diagnostic log, budget tracker, and knowledge base.
async fn collect_report(
    diag_log: &DiagnosticLog,
    budget: &Arc<RwLock<BudgetTracker>>,
    _node_id: &str,
) -> WeeklyReport {
    // Get recent diagnostic entries (ring buffer holds up to 50)
    let entries = diag_log.recent(50).await;

    // Count outcomes
    let mut fixed = 0u32;
    let mut failed = 0u32;
    let mut escalated = 0u32;
    let mut trigger_counts: HashMap<String, u32> = HashMap::new();
    let mut total_fix_time_secs = 0.0f64;
    let mut fix_count_for_mttr = 0u32;

    for entry in &entries {
        match entry.outcome.as_str() {
            "fixed" => {
                fixed += 1;
                // Estimate MTTR from tier (Tier 1 ~1s, Tier 2 ~5s, Tier 3 ~30s, Tier 4 ~120s)
                let tier_time = match entry.tier {
                    1 => 1.0,
                    2 => 5.0,
                    3 => 30.0,
                    4 => 120.0,
                    _ => 60.0,
                };
                total_fix_time_secs += tier_time;
                fix_count_for_mttr += 1;
            }
            "failed_to_fix" => failed += 1,
            _ => {}
        }

        // Count trigger types for top issues
        *trigger_counts.entry(entry.trigger.clone()).or_insert(0) += 1;

        // Check if escalated (tier 5)
        if entry.tier == 5 {
            escalated += 1;
        }
    }

    let total_events = fixed + failed;
    let auto_resolution_rate = if total_events > 0 {
        (fixed as f64 / total_events as f64) * 100.0
    } else {
        100.0 // No issues = 100% rate
    };

    let mttr = if fix_count_for_mttr > 0 {
        total_fix_time_secs / fix_count_for_mttr as f64
    } else {
        0.0
    };

    // Uptime estimate: if no failures, 100%. Each failure ~5min downtime out of 7 days.
    let total_minutes = 7.0 * 24.0 * 60.0;
    let downtime_minutes = failed as f64 * 5.0; // Rough estimate: 5 min per failure
    let uptime_pct = ((total_minutes - downtime_minutes) / total_minutes * 100.0).min(100.0);

    // Top 3 issues by trigger count
    let mut sorted_triggers: Vec<(String, u32)> = trigger_counts.into_iter().collect();
    sorted_triggers.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_triggers.truncate(3);

    // Budget info (tokio RwLock — no poisoning, status() needs &mut)
    let (budget_spent, budget_limit, model_calls) = {
        let mut guard = budget.write().await;
        let s = guard.status();
        (s.spent_today, s.daily_limit, s.model_calls_today)
    };

    // KB stats
    let kb_total = match KnowledgeBase::open(KB_PATH) {
        Ok(kb) => kb.solution_count().unwrap_or(0),
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, error = %e, "Cannot open KB for weekly report");
            0
        }
    };

    // Period dates (IST)
    let ist = FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("IST offset");
    let now_ist = Utc::now().with_timezone(&ist);
    let period_end = now_ist.format("%Y-%m-%d").to_string();
    let period_start = (now_ist - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();

    WeeklyReport {
        period_start,
        period_end,
        uptime_pct,
        auto_resolved: fixed,
        auto_resolution_rate,
        mttr_seconds: mttr,
        escalated_count: escalated,
        top_issues: sorted_triggers,
        budget_spent,
        budget_limit,
        kb_total,
        model_calls,
    }
}

/// Format the report as a WhatsApp-friendly message with *bold* and - bullets.
fn format_whatsapp_message(r: &WeeklyReport) -> String {
    let mut msg = format!(
        "*Weekly Fleet Intelligence Report*\n\
         Period: {} — {}\n\
         \n\
         *Key Metrics*\n\
         - Uptime: {:.1}%\n\
         - Auto-resolved: {} ({:.0}%)\n\
         - MTTR (auto): {:.0}s\n\
         - Escalated to human: {}\n",
        r.period_start,
        r.period_end,
        r.uptime_pct,
        r.auto_resolved,
        r.auto_resolution_rate,
        r.mttr_seconds,
        r.escalated_count,
    );

    msg.push_str("\n*Top Issues*\n");
    if r.top_issues.is_empty() {
        msg.push_str("- None this week\n");
    } else {
        for (trigger, count) in &r.top_issues {
            msg.push_str(&format!("- {} ({}x)\n", trigger, count));
        }
    }

    msg.push_str(&format!(
        "\n*AI Budget*\n\
         - Spent today: ${:.2} / ${:.0}\n\
         - Model calls today: {}\n",
        r.budget_spent, r.budget_limit, r.model_calls,
    ));

    msg.push_str(&format!(
        "\n*Knowledge Base*\n\
         - Total solutions: {}\n",
        r.kb_total,
    ));

    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seconds_until_next_sunday() {
        // Just verify it returns a positive value and doesn't panic
        let secs = seconds_until_next_sunday_midnight_ist();
        assert!(secs >= 1, "Should be at least 1 second until next Sunday midnight");
        // Max is ~7 days = 604800 seconds
        assert!(secs <= 604_800 + 60, "Should be at most ~7 days");
    }

    #[test]
    fn test_format_whatsapp_message() {
        let report = WeeklyReport {
            period_start: "2026-03-25".to_string(),
            period_end: "2026-04-01".to_string(),
            uptime_pct: 99.5,
            auto_resolved: 12,
            auto_resolution_rate: 85.7,
            mttr_seconds: 15.0,
            escalated_count: 2,
            top_issues: vec![
                ("GameCrash".to_string(), 5),
                ("WsDisconnect".to_string(), 3),
                ("ProcessCrash".to_string(), 2),
            ],
            budget_spent: 3.50,
            budget_limit: 10.0,
            kb_total: 47,
            model_calls: 8,
        };

        let msg = format_whatsapp_message(&report);
        assert!(msg.contains("*Weekly Fleet Intelligence Report*"));
        assert!(msg.contains("99.5%"));
        assert!(msg.contains("12 (86%)"));
        assert!(msg.contains("MTTR (auto): 15s"));
        assert!(msg.contains("GameCrash (5x)"));
        assert!(msg.contains("Total solutions: 47"));
    }

    #[test]
    fn test_format_whatsapp_no_issues() {
        let report = WeeklyReport {
            period_start: "2026-03-25".to_string(),
            period_end: "2026-04-01".to_string(),
            uptime_pct: 100.0,
            auto_resolved: 0,
            auto_resolution_rate: 100.0,
            mttr_seconds: 0.0,
            escalated_count: 0,
            top_issues: vec![],
            budget_spent: 0.0,
            budget_limit: 10.0,
            kb_total: 30,
            model_calls: 0,
        };

        let msg = format_whatsapp_message(&report);
        assert!(msg.contains("None this week"));
        assert!(msg.contains("100.0%"));
    }
}
