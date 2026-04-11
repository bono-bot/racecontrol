//! Lap consistency checker (Phase 364 CONSIST-01).
//!
//! On each LapCompleted event, computes a rolling 3-sigma band from the pod's
//! recent lap times. If the new lap falls outside the band, appends
//! "lap_outlier_lapN" to billing_sessions.suspect_reasons.
//!
//! Guards that prevent false positives:
//!   - Minimum 3 laps in history before flagging
//!   - stddev must exceed 2000ms (catches flat sessions where all laps are ~equal)
//!   - Only valid laps (lap.valid == true) are checked and added to history
//!   - Feature flag phase364_quality_monitor must be enabled

use std::collections::VecDeque;
use std::sync::Arc;

use rc_common::types::LapData;

use crate::state::AppState;

const LOG_TARGET: &str = "lap-consistency";
const MAX_LAP_HISTORY: usize = 50;
const MIN_LAPS_FOR_CHECK: usize = 3;
const MIN_STDDEV_MS: f64 = 2000.0; // guard: do not flag when all laps are consistent

/// Check if a newly completed lap is a statistical outlier (>3-sigma from session mean).
/// If so, appends "lap_outlier_lapN" to the session's suspect_reasons.
/// Also updates the pod's rolling lap history.
pub async fn check_lap_consistency(state: &Arc<AppState>, lap: &LapData) {
    // Feature flag guard -- same pattern as session_audit.rs
    let flag_enabled = {
        let guard = state.feature_flags.read().await;
        guard
            .get("phase364_quality_monitor")
            .map(|r| r.enabled)
            .unwrap_or(true) // Intentional default: true. Flag missing = treat as enabled.
    }; // guard dropped here -- CLAUDE.md never-hold-lock-across-await

    if !flag_enabled {
        return;
    }

    // Only check valid laps -- invalid laps (pit entry, lap_time_ms=0) are noise
    if !lap.valid || lap.lap_time_ms == 0 {
        return;
    }

    let mut pods = state.pods.write().await;
    let Some(pod_info) = pods.get_mut(&lap.pod_id) else {
        return;
    };

    let new_lap_ms = lap.lap_time_ms;

    // Check consistency BEFORE adding the new lap (history = previous laps)
    let is_outlier = check_outlier(&pod_info.recent_lap_times, new_lap_ms);

    // Add new lap to rolling history (cap at MAX_LAP_HISTORY)
    pod_info.recent_lap_times.push_back(new_lap_ms);
    if pod_info.recent_lap_times.len() > MAX_LAP_HISTORY {
        pod_info.recent_lap_times.pop_front();
    }

    // Drop the write guard before the async DB call
    drop(pods);

    if is_outlier {
        tracing::warn!(
            target: LOG_TARGET,
            "CONSIST-01: lap {} is a 3-sigma outlier ({}ms) for pod={} session={}",
            lap.lap_number, new_lap_ms, lap.pod_id, lap.session_id
        );
        let reason = format!("lap_outlier_lap{}", lap.lap_number);
        crate::bot_coordinator::append_suspect_reason(&state.db, &lap.session_id, &reason).await;
    }
}

/// Returns true if new_lap_ms is a statistical outlier (>3-sigma) vs the history.
/// Returns false if history has fewer than MIN_LAPS_FOR_CHECK entries.
/// Returns false if stddev < MIN_STDDEV_MS (all laps are too consistent to flag).
pub fn check_outlier(history: &VecDeque<u32>, new_lap_ms: u32) -> bool {
    let n = history.len();
    if n < MIN_LAPS_FOR_CHECK {
        return false;
    }

    let mean = history.iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let variance = history.iter()
        .map(|&x| { let d = x as f64 - mean; d * d })
        .sum::<f64>() / n as f64;
    let stddev = variance.sqrt();

    if stddev < MIN_STDDEV_MS {
        return false;
    }

    let z_score = ((new_lap_ms as f64) - mean).abs() / stddev;
    z_score > 3.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn make_history(times: &[u32]) -> VecDeque<u32> {
        times.iter().copied().collect()
    }

    #[test]
    fn no_flag_with_fewer_than_3_laps() {
        let history = make_history(&[90_000, 91_000]);
        assert!(!check_outlier(&history, 300_000), "Fewer than 3 laps must never flag");
    }

    #[test]
    fn no_flag_when_stddev_too_low() {
        // All laps at ~90s -- stddev well below 2000ms
        let history = make_history(&[90_000, 90_100, 90_050, 89_950]);
        assert!(!check_outlier(&history, 200_000), "Low stddev must suppress flagging");
    }

    #[test]
    fn flags_extreme_outlier() {
        // Need high variance for stddev > 2000ms guard to pass
        let history = make_history(&[80_000, 120_000, 75_000, 130_000, 85_000]);
        // mean = 98000, stddev ~ 20200ms -> z = (300000-98000)/20200 ~ 10 sigma
        assert!(check_outlier(&history, 300_000), "Extreme outlier with high stddev must be flagged");
    }

    #[test]
    fn does_not_flag_normal_variation() {
        // Realistic race session: laps in 88-94s range with some natural variation
        let history = make_history(&[88_000, 90_000, 89_500, 91_000, 92_000, 88_500]);
        // stddev ~ 1350ms < 2000ms -> guard kicks in, no flagging
        assert!(!check_outlier(&history, 95_000), "Normal lap variation must not be flagged");
    }

    #[test]
    fn respects_min_stddev_guard() {
        // Artificially wide stddev: 60s laps alternating with 120s laps
        let history = make_history(&[60_000, 120_000, 60_000, 120_000, 60_000]);
        // mean=84000, stddev~30000 -> well above 2000ms guard
        // A 200_000ms lap: z = (200000-84000)/30000 ~ 3.87 sigma -> flag
        assert!(check_outlier(&history, 200_000), "High-variance session should flag large outlier");
        // A 90_000ms lap: z = (90000-84000)/30000 ~ 0.2 sigma -> no flag
        assert!(!check_outlier(&history, 90_000), "Within-band lap must not be flagged");
    }
}
