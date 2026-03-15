use rc_common::types::WatchdogCrashReport;
use tracing;

/// Build the crash report URL from core URL and pod ID.
/// Extracted as a testable helper.
pub fn build_crash_report_url(core_url: &str, pod_id: &str) -> String {
    format!(
        "{}/api/v1/pods/{}/watchdog-crash",
        core_url.trim_end_matches('/'),
        pod_id
    )
}

/// Send a crash report to rc-core. Fire-and-forget: logs errors but never
/// panics or returns an error that would stop the restart loop.
pub fn send_crash_report(core_url: &str, report: &WatchdogCrashReport) {
    let url = build_crash_report_url(core_url, &report.pod_id);
    tracing::info!("Sending crash report to {}", url);

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to build HTTP client for crash report: {}", e);
            return;
        }
    };

    match client.post(&url).json(report).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::info!("Crash report delivered successfully");
            } else {
                tracing::warn!(
                    "Crash report delivery failed: HTTP {}",
                    resp.status()
                );
            }
        }
        Err(e) => {
            tracing::warn!("Crash report delivery failed: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_crash_report_url_basic() {
        let url = build_crash_report_url("http://192.168.31.23:8080", "pod_1");
        assert_eq!(url, "http://192.168.31.23:8080/api/v1/pods/pod_1/watchdog-crash");
    }

    #[test]
    fn test_build_crash_report_url_strips_trailing_slash() {
        let url = build_crash_report_url("http://192.168.31.23:8080/", "pod_3");
        assert_eq!(url, "http://192.168.31.23:8080/api/v1/pods/pod_3/watchdog-crash");
    }

    #[test]
    fn test_build_crash_report_url_different_pod() {
        let url = build_crash_report_url("http://10.0.0.1:9090", "pod_8");
        assert_eq!(url, "http://10.0.0.1:9090/api/v1/pods/pod_8/watchdog-crash");
    }
}
