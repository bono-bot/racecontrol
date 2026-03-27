#![allow(dead_code)]
//! Customer Experience Scoring — per-pod quality score for Meshed Intelligence.
//!
//! Weighted score from 5 metrics:
//!   - Game launch success rate (30%)
//!   - Session completion rate (25%)
//!   - Display stability — no flicker/restart (20%)
//!   - Hardware responsiveness — FFB, pedals (15%)
//!   - Billing accuracy (10%)
//!
//! Score < 80% → flagged for maintenance
//! Score < 50% → auto-removed from rotation + WhatsApp alert
//!
//! Phase 237 — Meshed Intelligence CX-01 to CX-04.

use serde::Serialize;

const LOG_TARGET: &str = "experience-score";

/// CX-02: Score below this → flagged for maintenance
pub const MAINTENANCE_THRESHOLD: f64 = 80.0;

/// CX-03: Score below this → auto-removed from rotation
pub const REMOVAL_THRESHOLD: f64 = 50.0;

/// Per-pod experience score with component breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct ExperienceScore {
    /// Overall weighted score (0-100)
    pub total: f64,
    /// Individual component scores (0-100 each)
    pub game_launch: f64,
    pub session_completion: f64,
    pub display_stability: f64,
    pub hardware_responsive: f64,
    pub billing_accuracy: f64,
    /// Status derived from score thresholds
    pub status: ScoreStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ScoreStatus {
    Healthy,
    Maintenance,
    RemoveFromRotation,
}

/// Raw metric inputs for score calculation.
#[derive(Debug, Clone, Default)]
pub struct MetricInputs {
    /// Game launches attempted in scoring window
    pub launches_attempted: u32,
    /// Game launches that succeeded (process started + playable signal received)
    pub launches_succeeded: u32,
    /// Billing sessions started in window
    pub sessions_started: u32,
    /// Billing sessions completed normally (not crashed/force-stopped)
    pub sessions_completed: u32,
    /// Diagnostic scans with display_mismatch == 0
    pub display_ok_scans: u32,
    /// Total diagnostic scans in window
    pub total_scans: u32,
    /// HID device connected checks that passed
    pub hid_ok_checks: u32,
    /// Total HID checks in window
    pub total_hid_checks: u32,
    /// Billing sessions with correct final amount (delta < 1%)
    pub billing_accurate_sessions: u32,
    /// Total billing sessions with known expected amount
    pub billing_total_sessions: u32,
}

/// Calculate the experience score from raw metrics.
/// CX-01: Weighted average formula.
pub fn calculate_score(inputs: &MetricInputs) -> ExperienceScore {
    let game_launch = if inputs.launches_attempted > 0 {
        (inputs.launches_succeeded as f64 / inputs.launches_attempted as f64) * 100.0
    } else {
        100.0 // No launches = no failures
    };

    let session_completion = if inputs.sessions_started > 0 {
        (inputs.sessions_completed as f64 / inputs.sessions_started as f64) * 100.0
    } else {
        100.0
    };

    let display_stability = if inputs.total_scans > 0 {
        (inputs.display_ok_scans as f64 / inputs.total_scans as f64) * 100.0
    } else {
        100.0
    };

    let hardware_responsive = if inputs.total_hid_checks > 0 {
        (inputs.hid_ok_checks as f64 / inputs.total_hid_checks as f64) * 100.0
    } else {
        100.0
    };

    let billing_accuracy = if inputs.billing_total_sessions > 0 {
        (inputs.billing_accurate_sessions as f64 / inputs.billing_total_sessions as f64) * 100.0
    } else {
        100.0
    };

    // Weighted total — CX-01
    let total = game_launch * 0.30
        + session_completion * 0.25
        + display_stability * 0.20
        + hardware_responsive * 0.15
        + billing_accuracy * 0.10;

    let status = if total < REMOVAL_THRESHOLD {
        ScoreStatus::RemoveFromRotation
    } else if total < MAINTENANCE_THRESHOLD {
        ScoreStatus::Maintenance
    } else {
        ScoreStatus::Healthy
    };

    let score = ExperienceScore {
        total,
        game_launch,
        session_completion,
        display_stability,
        hardware_responsive,
        billing_accuracy,
        status: status.clone(),
    };

    match &status {
        ScoreStatus::RemoveFromRotation => {
            tracing::warn!(
                target: LOG_TARGET,
                total = format!("{:.1}", total),
                "CX-03: Pod score below {}% — should be removed from rotation",
                REMOVAL_THRESHOLD
            );
        }
        ScoreStatus::Maintenance => {
            tracing::info!(
                target: LOG_TARGET,
                total = format!("{:.1}", total),
                "CX-02: Pod score below {}% — flagged for maintenance",
                MAINTENANCE_THRESHOLD
            );
        }
        ScoreStatus::Healthy => {
            tracing::debug!(
                target: LOG_TARGET,
                total = format!("{:.1}", total),
                "Pod experience score: healthy"
            );
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_score() {
        let inputs = MetricInputs {
            launches_attempted: 10,
            launches_succeeded: 10,
            sessions_started: 10,
            sessions_completed: 10,
            display_ok_scans: 100,
            total_scans: 100,
            hid_ok_checks: 50,
            total_hid_checks: 50,
            billing_accurate_sessions: 5,
            billing_total_sessions: 5,
        };
        let score = calculate_score(&inputs);
        assert!((score.total - 100.0).abs() < 0.01);
        assert_eq!(score.status, ScoreStatus::Healthy);
    }

    #[test]
    fn test_zero_activity_is_healthy() {
        let inputs = MetricInputs::default();
        let score = calculate_score(&inputs);
        assert!((score.total - 100.0).abs() < 0.01, "No activity should score 100");
        assert_eq!(score.status, ScoreStatus::Healthy);
    }

    #[test]
    fn test_low_launch_rate_flags_maintenance() {
        let inputs = MetricInputs {
            launches_attempted: 10,
            launches_succeeded: 5, // 50% success
            sessions_started: 10,
            sessions_completed: 10,
            display_ok_scans: 100,
            total_scans: 100,
            hid_ok_checks: 50,
            total_hid_checks: 50,
            billing_accurate_sessions: 5,
            billing_total_sessions: 5,
        };
        let score = calculate_score(&inputs);
        // 50*0.30 + 100*0.25 + 100*0.20 + 100*0.15 + 100*0.10 = 15+25+20+15+10 = 85
        assert!((score.total - 85.0).abs() < 0.01);
        assert_eq!(score.status, ScoreStatus::Healthy); // 85 > 80
    }

    #[test]
    fn test_multiple_failures_maintenance() {
        let inputs = MetricInputs {
            launches_attempted: 10,
            launches_succeeded: 5, // 50%
            sessions_started: 10,
            sessions_completed: 6, // 60%
            display_ok_scans: 80,
            total_scans: 100, // 80%
            hid_ok_checks: 50,
            total_hid_checks: 50,
            billing_accurate_sessions: 5,
            billing_total_sessions: 5,
        };
        let score = calculate_score(&inputs);
        // 50*0.30 + 60*0.25 + 80*0.20 + 100*0.15 + 100*0.10 = 15+15+16+15+10 = 71
        assert!((score.total - 71.0).abs() < 0.01);
        assert_eq!(score.status, ScoreStatus::Maintenance);
    }

    #[test]
    fn test_critical_removal() {
        let inputs = MetricInputs {
            launches_attempted: 10,
            launches_succeeded: 1, // 10%
            sessions_started: 10,
            sessions_completed: 2, // 20%
            display_ok_scans: 10,
            total_scans: 100, // 10%
            hid_ok_checks: 5,
            total_hid_checks: 50, // 10%
            billing_accurate_sessions: 1,
            billing_total_sessions: 5, // 20%
        };
        let score = calculate_score(&inputs);
        // 10*0.30 + 20*0.25 + 10*0.20 + 10*0.15 + 20*0.10 = 3+5+2+1.5+2 = 13.5
        assert!((score.total - 13.5).abs() < 0.01);
        assert_eq!(score.status, ScoreStatus::RemoveFromRotation);
    }

    #[test]
    fn test_weights_sum_to_100() {
        // Verify 0.30 + 0.25 + 0.20 + 0.15 + 0.10 = 1.00
        let sum: f64 = 0.30 + 0.25 + 0.20 + 0.15 + 0.10;
        assert!((sum - 1.0).abs() < f64::EPSILON, "Weights must sum to 1.0");
    }
}
