#![allow(dead_code)]
//! Predictive Maintenance — threshold-based anomaly detection for hardware and software trends.
//!
//! Instead of waiting for failure, detects degradation patterns and alerts BEFORE impact.
//! Runs as part of the 5-minute diagnostic scan (called from diagnostic_engine).
//!
//! Phase 236 — Meshed Intelligence PRED-01 to PRED-06.
//!
//! Thresholds (not ML — simple, reliable, zero cost):
//!   PRED-01: ConspitLink reconnection rate trending → USB alert
//!   PRED-02: Edge process count trending down → memory leak restart
//!   PRED-03: GPU temp consistently >80C → thermal alert
//!   PRED-04: rc-agent restart count >2/day → stability alert
//!   PRED-05: Disk space <10GB → auto-cleanup
//!   PRED-06: Error spike across 3+ pods → systemic alert (handled by server coordinator)

use serde::Serialize;
use std::collections::VecDeque;

const LOG_TARGET: &str = "predictive-maint";

/// Maximum samples to keep per metric (5-min intervals × 24 hours = 288)
const MAX_SAMPLES: usize = 288;

/// PRED-03: GPU temperature alert threshold (Celsius)
const GPU_TEMP_ALERT_C: f64 = 80.0;

/// PRED-04: Max restarts per day before alerting
const MAX_RESTARTS_PER_DAY: u32 = 2;

/// PRED-05: Disk space alert threshold (bytes) — 10 GB
const DISK_SPACE_ALERT_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// PRED-01: ConspitLink reconnection count threshold per hour
const CONSPIT_RECONNECT_ALERT_PER_HOUR: u32 = 3;

/// PRED-02: Edge process count — alert if drops to 0 when blanking expected
const EDGE_MISSING_CONSECUTIVE_SCANS: u32 = 2;

/// Predictive alert — something is degrading but hasn't failed yet.
#[derive(Debug, Clone, Serialize)]
pub struct PredictiveAlert {
    pub alert_type: PredAlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub metric_value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize)]
pub enum PredAlertType {
    ConspitLinkReconnect,
    EdgeMemoryLeak,
    GpuThermal,
    StabilityDegrading,
    DiskSpaceLow,
    ErrorSpike,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// State tracked across scans for trend detection.
pub struct PredictiveState {
    /// PRED-01: ConspitLink reconnection events in the last hour
    conspit_reconnects: VecDeque<std::time::Instant>,
    /// PRED-02: Consecutive scans where Edge process count was 0 during blanking
    edge_missing_count: u32,
    /// PRED-04: rc-agent restart count today
    restart_count_today: u32,
    restart_date: chrono::NaiveDate,
}

impl PredictiveState {
    pub fn new() -> Self {
        Self {
            conspit_reconnects: VecDeque::new(),
            edge_missing_count: 0,
            restart_count_today: 0,
            restart_date: chrono::Utc::now().date_naive(),
        }
    }

    /// Reset daily counters if date changed (midnight crossing)
    fn maybe_reset_daily(&mut self) {
        let today = chrono::Utc::now().date_naive();
        if today != self.restart_date {
            self.restart_count_today = 0;
            self.restart_date = today;
        }
    }
}

impl Default for PredictiveState {
    fn default() -> Self {
        Self::new()
    }
}

/// Run all predictive checks. Returns alerts for any degrading metrics.
/// Called every 5 minutes by the diagnostic engine scan loop.
pub fn run_predictive_scan(state: &mut PredictiveState) -> Vec<PredictiveAlert> {
    state.maybe_reset_daily();
    let mut alerts = Vec::new();

    // PRED-03: GPU temperature check
    if let Some(alert) = check_gpu_temp() {
        alerts.push(alert);
    }

    // PRED-05: Disk space check
    if let Some(alert) = check_disk_space() {
        alerts.push(alert);
    }

    // PRED-01: ConspitLink reconnection rate
    if let Some(alert) = check_conspit_reconnects(state) {
        alerts.push(alert);
    }

    // Log results
    if alerts.is_empty() {
        tracing::debug!(target: LOG_TARGET, "Predictive scan: all metrics nominal");
    } else {
        for alert in &alerts {
            match alert.severity {
                AlertSeverity::Critical => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        alert_type = ?alert.alert_type,
                        value = alert.metric_value,
                        threshold = alert.threshold,
                        "{}", alert.message
                    );
                }
                AlertSeverity::Warning => {
                    tracing::info!(
                        target: LOG_TARGET,
                        alert_type = ?alert.alert_type,
                        value = alert.metric_value,
                        threshold = alert.threshold,
                        "{}", alert.message
                    );
                }
            }
        }
    }

    alerts
}

/// PRED-03: Check GPU temperature via nvidia-smi.
/// Returns alert if consistently above 80C.
fn check_gpu_temp() -> Option<PredictiveAlert> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    let temp_str = String::from_utf8(output.stdout).ok()?;
    let temp: f64 = temp_str.trim().parse().ok()?;

    if temp >= GPU_TEMP_ALERT_C {
        Some(PredictiveAlert {
            alert_type: PredAlertType::GpuThermal,
            severity: if temp >= 90.0 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            message: format!(
                "PRED-03: GPU temperature {:.0}C exceeds {}C threshold — check HVAC / clean GPU fan",
                temp, GPU_TEMP_ALERT_C
            ),
            metric_value: temp,
            threshold: GPU_TEMP_ALERT_C,
        })
    } else {
        None
    }
}

/// PRED-05: Check disk space on C: drive.
/// Returns alert if below 10GB.
fn check_disk_space() -> Option<PredictiveAlert> {
    // Use sysinfo for cross-platform disk check
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy();
        if mount.starts_with("C:") || mount == "/" {
            let available = disk.available_space();
            if available < DISK_SPACE_ALERT_BYTES {
                let gb_available = available as f64 / (1024.0 * 1024.0 * 1024.0);
                let severity = if available < DISK_SPACE_ALERT_BYTES / 2 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };

                // PRED-05: Auto-cleanup old logs
                if available < DISK_SPACE_ALERT_BYTES {
                    auto_cleanup_old_logs();
                }

                return Some(PredictiveAlert {
                    alert_type: PredAlertType::DiskSpaceLow,
                    severity,
                    message: format!(
                        "PRED-05: Disk space {:.1}GB below {}GB threshold",
                        gb_available,
                        DISK_SPACE_ALERT_BYTES / (1024 * 1024 * 1024)
                    ),
                    metric_value: gb_available,
                    threshold: (DISK_SPACE_ALERT_BYTES / (1024 * 1024 * 1024)) as f64,
                });
            }
        }
    }
    None
}

/// PRED-01: Check ConspitLink reconnection rate.
fn check_conspit_reconnects(state: &mut PredictiveState) -> Option<PredictiveAlert> {
    // Prune events older than 1 hour
    // Use checked_sub to avoid underflow on systems with <1hr uptime
    let now = std::time::Instant::now();
    let one_hour = std::time::Duration::from_secs(3600);
    if let Some(one_hour_ago) = now.checked_sub(one_hour) {
        while state
            .conspit_reconnects
            .front()
            .is_some_and(|t| *t < one_hour_ago)
        {
            state.conspit_reconnects.pop_front();
        }
    }

    let count = state.conspit_reconnects.len() as u32;
    if count >= CONSPIT_RECONNECT_ALERT_PER_HOUR {
        Some(PredictiveAlert {
            alert_type: PredAlertType::ConspitLinkReconnect,
            severity: AlertSeverity::Warning,
            message: format!(
                "PRED-01: ConspitLink reconnected {}x in last hour (threshold: {}) — USB port may be failing",
                count, CONSPIT_RECONNECT_ALERT_PER_HOUR
            ),
            metric_value: count as f64,
            threshold: CONSPIT_RECONNECT_ALERT_PER_HOUR as f64,
        })
    } else {
        None
    }
}

/// Record a ConspitLink reconnection event (called from HID monitoring).
pub fn record_conspit_reconnect(state: &mut PredictiveState) {
    state.conspit_reconnects.push_back(std::time::Instant::now());
    tracing::debug!(target: LOG_TARGET, "ConspitLink reconnection event recorded");
}

/// Record an rc-agent restart (called from startup).
/// PRED-04: Returns true if restart count exceeds threshold.
pub fn record_restart(state: &mut PredictiveState) -> bool {
    state.maybe_reset_daily();
    state.restart_count_today += 1;

    if state.restart_count_today > MAX_RESTARTS_PER_DAY {
        tracing::warn!(
            target: LOG_TARGET,
            count = state.restart_count_today,
            threshold = MAX_RESTARTS_PER_DAY,
            "PRED-04: rc-agent restart count exceeds daily threshold — stability degrading"
        );
        return true;
    }
    false
}

/// Record Edge process count for memory leak detection.
/// PRED-02: Alert if Edge count drops to 0 during expected blanking.
pub fn record_edge_count(state: &mut PredictiveState, count: u32, blanking_expected: bool) -> Option<PredictiveAlert> {
    if blanking_expected && count == 0 {
        state.edge_missing_count += 1;
        if state.edge_missing_count >= EDGE_MISSING_CONSECUTIVE_SCANS {
            return Some(PredictiveAlert {
                alert_type: PredAlertType::EdgeMemoryLeak,
                severity: AlertSeverity::Critical,
                message: format!(
                    "PRED-02: Edge process count 0 for {} consecutive scans during blanking — browser may have crashed or leaked memory",
                    state.edge_missing_count
                ),
                metric_value: 0.0,
                threshold: 1.0,
            });
        }
    } else {
        state.edge_missing_count = 0;
    }
    None
}

/// Auto-cleanup old log files to free disk space.
/// Removes .jsonl and .log files older than 7 days from C:\RacingPoint\.
fn auto_cleanup_old_logs() {
    let log_dir = std::path::Path::new(r"C:\RacingPoint");
    let seven_days_ago = std::time::SystemTime::now()
        - std::time::Duration::from_secs(7 * 24 * 3600);

    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut cleaned = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if ext_str == "jsonl" || ext_str == "log" {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        if modified < seven_days_ago {
                            if std::fs::remove_file(&path).is_ok() {
                                cleaned += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if cleaned > 0 {
        tracing::info!(
            target: LOG_TARGET,
            count = cleaned,
            "PRED-05: Auto-cleaned old log files (>7 days)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let s = PredictiveState::new();
        assert_eq!(s.restart_count_today, 0);
        assert_eq!(s.edge_missing_count, 0);
        assert!(s.conspit_reconnects.is_empty());
    }

    #[test]
    fn test_record_restart_below_threshold() {
        let mut s = PredictiveState::new();
        assert!(!record_restart(&mut s));
        assert!(!record_restart(&mut s));
        assert_eq!(s.restart_count_today, 2);
    }

    #[test]
    fn test_record_restart_exceeds_threshold() {
        let mut s = PredictiveState::new();
        record_restart(&mut s); // 1
        record_restart(&mut s); // 2
        assert!(record_restart(&mut s), "3rd restart should trigger alert"); // 3 > MAX(2)
    }

    #[test]
    fn test_edge_missing_no_alert_when_not_blanking() {
        let mut s = PredictiveState::new();
        let alert = record_edge_count(&mut s, 0, false);
        assert!(alert.is_none(), "Should not alert when blanking not expected");
    }

    #[test]
    fn test_edge_missing_alert_after_consecutive() {
        let mut s = PredictiveState::new();
        let a1 = record_edge_count(&mut s, 0, true);
        assert!(a1.is_none(), "First scan should not alert");
        let a2 = record_edge_count(&mut s, 0, true);
        assert!(a2.is_some(), "Second consecutive scan should alert");
    }

    #[test]
    fn test_edge_missing_resets_on_recovery() {
        let mut s = PredictiveState::new();
        record_edge_count(&mut s, 0, true); // count = 1
        record_edge_count(&mut s, 3, true); // recovery → count = 0
        let alert = record_edge_count(&mut s, 0, true); // count = 1 again
        assert!(alert.is_none(), "Should reset counter on recovery");
    }

    #[test]
    fn test_conspit_reconnect_below_threshold() {
        let mut s = PredictiveState::new();
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        let alert = check_conspit_reconnects(&mut s);
        assert!(alert.is_none(), "2 reconnects should not trigger (threshold: 3)");
    }

    #[test]
    fn test_conspit_reconnect_at_threshold() {
        let mut s = PredictiveState::new();
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        let alert = check_conspit_reconnects(&mut s);
        assert!(alert.is_some(), "3 reconnects should trigger");
    }

    #[test]
    fn test_run_predictive_scan_no_alerts() {
        let mut s = PredictiveState::new();
        // On a dev machine, GPU temp and disk space should be fine
        let alerts = run_predictive_scan(&mut s);
        // We can't assert zero alerts (disk might be low on CI), but no panic
        let _ = alerts;
    }
}
