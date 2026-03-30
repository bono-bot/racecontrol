//! SW-06: Direct HTTP survival reporting to racecontrol server.
//!
//! After a successful restart (with or without rollback), the watchdog reports
//! its survival status directly to the racecontrol server. This is a fire-and-forget
//! HTTP POST — failures are logged but never block the service loop.
//!
//! Uses blocking reqwest (same as reporter.rs) — no tokio needed.

use std::time::Duration;

/// HTTP timeout for survival report delivery.
const REPORT_TIMEOUT: Duration = Duration::from_secs(5);

/// Survival report payload sent to the server.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SurvivalReport {
    pub pod_id: String,
    pub watchdog_version: String,
    pub build_id: String,
    pub restart_count: u32,
    pub rollback_depth: u32,
    pub health_poll_attempts: u32,
    pub health_poll_ok: bool,
    pub binary_validated: bool,
    pub timestamp: String,
    /// "normal" | "rollback" | "maintenance"
    pub recovery_mode: String,
}

/// Build the survival report URL.
pub fn build_survival_url(core_url: &str, pod_id: &str) -> String {
    format!(
        "{}/api/v1/pods/{}/watchdog-survival",
        core_url.trim_end_matches('/'),
        pod_id
    )
}

/// Send a survival report to the racecontrol server. Fire-and-forget.
pub fn send_survival_report(core_url: &str, report: &SurvivalReport) {
    let url = build_survival_url(core_url, &report.pod_id);
    tracing::info!(
        "Sending survival report to {} (mode={}, rollback_depth={})",
        url, report.recovery_mode, report.rollback_depth
    );

    let client = match reqwest::blocking::Client::builder()
        .timeout(REPORT_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to build HTTP client for survival report: {}", e);
            return;
        }
    };

    match client.post(&url).json(report).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::info!("Survival report delivered successfully");
            } else {
                tracing::warn!("Survival report delivery failed: HTTP {}", resp.status());
            }
        }
        Err(e) => {
            tracing::warn!("Survival report delivery failed: {}", e);
        }
    }
}

/// Create a survival report from current state.
pub fn create_report(
    pod_id: &str,
    watchdog_version: &str,
    build_id: &str,
    restart_count: u32,
    rollback_depth: u32,
    health_result: &crate::health_poller::HealthPollResult,
    binary_validated: bool,
    recovery_mode: &str,
) -> SurvivalReport {
    SurvivalReport {
        pod_id: pod_id.to_string(),
        watchdog_version: watchdog_version.to_string(),
        build_id: build_id.to_string(),
        restart_count,
        rollback_depth,
        health_poll_attempts: health_result.attempts,
        health_poll_ok: health_result.healthy,
        binary_validated,
        timestamp: chrono::Utc::now().to_rfc3339(),
        recovery_mode: recovery_mode.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_survival_url_basic() {
        let url = build_survival_url("http://192.168.31.23:8080", "pod_1");
        assert_eq!(url, "http://192.168.31.23:8080/api/v1/pods/pod_1/watchdog-survival");
    }

    #[test]
    fn test_build_survival_url_strips_trailing_slash() {
        let url = build_survival_url("http://192.168.31.23:8080/", "pod_3");
        assert_eq!(url, "http://192.168.31.23:8080/api/v1/pods/pod_3/watchdog-survival");
    }

    #[test]
    fn test_survival_report_serialization() {
        let report = SurvivalReport {
            pod_id: "pod_1".to_string(),
            watchdog_version: "0.1.0".to_string(),
            build_id: "abc123".to_string(),
            restart_count: 3,
            rollback_depth: 1,
            health_poll_attempts: 2,
            health_poll_ok: true,
            binary_validated: true,
            timestamp: "2026-03-31T12:00:00Z".to_string(),
            recovery_mode: "rollback".to_string(),
        };
        let json = serde_json::to_string(&report).expect("serialize OK");
        assert!(json.contains("\"pod_id\":\"pod_1\""));
        assert!(json.contains("\"recovery_mode\":\"rollback\""));
    }
}
