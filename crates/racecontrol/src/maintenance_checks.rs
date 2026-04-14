//! Phase 16: Pre-Maintenance Automated Checks + Phase 10: Business-Aware Priority Scoring.
//!
//! Extracted from maintenance_engine.rs — pre-maintenance validation and
//! business-context priority calculation.

use serde::Serialize;

// ─── Phase 16: Pre-Maintenance Automated Checks ─────────────────────────────

/// Pre-maintenance check results — validate system state before starting work
#[derive(Debug, Clone, Serialize)]
pub struct PreMaintenanceCheck {
    pub pod_id: u8,
    pub checks_passed: bool,
    pub has_active_session: bool,
    pub recent_backup: bool,
    pub pod_reachable: bool,
    pub messages: Vec<String>,
}

/// Run pre-maintenance validation before starting a maintenance task
pub async fn run_pre_checks(
    pod_id: u8,
    state: &std::sync::Arc<crate::state::AppState>,
) -> PreMaintenanceCheck {
    let mut check = PreMaintenanceCheck {
        pod_id,
        checks_passed: true,
        has_active_session: false,
        recent_backup: true, // assume true until we can verify
        pod_reachable: false,
        messages: Vec::new(),
    };

    // Check if pod has active billing session
    let pods = state.pods.read().await;
    if let Some(pod) = pods.values().find(|p| p.number == pod_id as u32) {
        check.pod_reachable = true;
        if pod.billing_session_id.is_some() {
            check.has_active_session = true;
            check.checks_passed = false;
            check.messages.push(format!(
                "Pod {} has active billing session — defer maintenance",
                pod_id
            ));
        }
    } else {
        check.messages.push(format!(
            "Pod {} not connected — cannot verify state",
            pod_id
        ));
        check.checks_passed = false;
    }

    check
}

// ─── Phase 10: Business-Aware Priority Scoring ──────────────────────────────

/// Calculate priority 1-100 weighted by business context.
/// GPT-4.1 death spiral fix: use EXPECTED revenue, not actual.
pub fn calculate_priority(severity: &str, _pod_id: u8, is_peak: bool, has_active_session: bool) -> u8 {
    let base = match severity {
        "Critical" => 80,
        "High" => 60,
        "Medium" => 40,
        _ => 20,
    };
    let peak_factor = if is_peak { 1.5 } else { 1.0 };
    let session_factor = if has_active_session { 1.4 } else { 1.0 };
    let score = (base as f64 * peak_factor * session_factor).min(100.0);
    score as u8
}

/// Check if the venue is currently operating (ping-based, not clock-based).
/// Replaces hardcoded peak hours with venue_state reachability check.
/// Rule: "If server or James is on, venue is open."
pub fn is_peak_hours() -> bool {
    crate::venue_state::venue_is_open()
}
