//! Fleet Intelligence Reports — weekly automated reports for Uday.
//!
//! Phase 240 — REPORT-01 to REPORT-04.
//!
//! Generates a weekly summary formatted for WhatsApp delivery.

use serde::Serialize;

/// Weekly fleet intelligence report
#[derive(Debug, Clone, Serialize)]
pub struct FleetReport {
    pub period: String,
    pub issues_detected: u32,
    pub auto_resolved: u32,
    pub escalated: u32,
    pub average_mttr_ms: f64,
    pub total_cost: f64,
    pub kb_total: i64,
    pub kb_new_this_week: u32,
    pub kb_promoted: u32,
    pub kb_hardened: u32,
    pub recommendations: Vec<String>,
}

/// REPORT-04: Format report for WhatsApp delivery via Evolution API.
pub fn format_whatsapp_report(report: &FleetReport) -> String {
    let auto_pct = if report.issues_detected > 0 {
        (report.auto_resolved as f64 / report.issues_detected as f64 * 100.0) as u32
    } else {
        100
    };

    format!(
        "Racing Point Fleet Intelligence Report\n\
         {}\n\
         \n\
         Issues: {} detected, {} auto-resolved ({}%), {} escalated\n\
         Avg MTTR: {:.0}ms\n\
         Cost: ${:.2}\n\
         \n\
         Knowledge Base: {} solutions ({} new, {} promoted, {} hardened)\n\
         \n\
         Recommendations:\n\
         {}",
        report.period,
        report.issues_detected,
        report.auto_resolved,
        auto_pct,
        report.escalated,
        report.average_mttr_ms,
        report.total_cost,
        report.kb_total,
        report.kb_new_this_week,
        report.kb_promoted,
        report.kb_hardened,
        report
            .recommendations
            .iter()
            .map(|r| format!("- {}", r))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_whatsapp_report() {
        let report = FleetReport {
            period: "Week of 2026-03-27".to_string(),
            issues_detected: 47,
            auto_resolved: 42,
            escalated: 5,
            average_mttr_ms: 23000.0,
            total_cost: 67.40,
            kb_total: 89,
            kb_new_this_week: 12,
            kb_promoted: 5,
            kb_hardened: 2,
            recommendations: vec!["Swap Gemini for GPT-4.1".to_string()],
        };
        let msg = format_whatsapp_report(&report);
        assert!(msg.contains("47 detected"));
        assert!(msg.contains("42 auto-resolved"));
        assert!(msg.contains("89%"));
        assert!(msg.contains("$67.40"));
        assert!(msg.contains("89 solutions"));
    }
}
